//! Page-scoped, passive mouse graph input. No suppression, hooks, logging or SET.
use super::*;
use secbird_core::mouse_input_identity::RawMouseBinding;
use std::{io, mem::size_of};
use windows_sys::Win32::UI::Input::*;
pub(super) const NOTIFY: u32 = WM_APP + 109;
const FLAGS: u32 = RIDEV_INPUTSINK | RIDEV_DEVNOTIFY;
// This Windows host reports0x100 after a successful0x2100 registration.
// DEVNOTIFY is a requested notification option, not reliably returned metadata.
// Accept only these two forms, the exact owning HWND, and no suppression bits.
fn matches_registration(observed: Option<(usize, u32)>, target: usize) -> bool {
    observed.is_some_and(|(window, flags)| {
        window == target && (flags == FLAGS || flags == RIDEV_INPUTSINK)
    })
}
pub(super) type BindingResult = Result<RawMouseBinding, String>;

pub(super) trait Registry {
    fn mouse(&mut self) -> io::Result<Option<(usize, u32)>>;
    fn set(&mut self, target: usize, flags: u32) -> io::Result<()>;
}
pub(super) struct WindowsRegistry;
impl Registry for WindowsRegistry {
    fn mouse(&mut self) -> io::Result<Option<(usize, u32)>> {
        unsafe {
            let mut count = 0;
            if GetRegisteredRawInputDevices(
                null_mut(),
                &mut count,
                size_of::<RAWINPUTDEVICE>() as u32,
            ) == u32::MAX
            {
                return Err(io::Error::last_os_error());
            }
            if count > 64 {
                return Err(io::Error::other("Too many process input registrations"));
            }
            let mut entries: Vec<RAWINPUTDEVICE> = vec![std::mem::zeroed(); count as usize];
            if count == 0 {
                return Ok(None);
            }
            let got = GetRegisteredRawInputDevices(
                entries.as_mut_ptr(),
                &mut count,
                size_of::<RAWINPUTDEVICE>() as u32,
            );
            if got == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            if got as usize > entries.len() {
                return Err(io::Error::other("Input registration list changed"));
            }
            let mut matching = entries[..got as usize]
                .iter()
                .filter(|e| e.usUsagePage == 1 && e.usUsage == 2);
            let result = matching.next().map(|e| (e.hwndTarget as usize, e.dwFlags));
            if matching.next().is_some() {
                return Err(io::Error::other("Ambiguous process mouse registration"));
            }
            Ok(result)
        }
    }
    fn set(&mut self, target: usize, flags: u32) -> io::Result<()> {
        let device = RAWINPUTDEVICE {
            usUsagePage: 1,
            usUsage: 2,
            dwFlags: flags,
            hwndTarget: target as HWND,
        };
        if unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Registration {
    target: usize,
    owned: bool,
}
impl Registration {
    pub(super) fn is_owned(&self) -> bool {
        self.owned
    }
    pub(super) fn owns_live(&self, registry: &mut impl Registry) -> io::Result<bool> {
        Ok(self.owned && matches_registration(registry.mouse()?, self.target))
    }
    pub(super) fn start(&mut self, registry: &mut impl Registry, target: usize) -> io::Result<()> {
        if target == 0 {
            return Err(io::Error::other("Missing editor window"));
        }
        let current = registry.mouse()?;
        if self.owned && matches_registration(current, self.target) && self.target == target {
            return Ok(());
        }
        if current.is_some() {
            return Err(io::Error::other(
                "Another process-local mouse observer owns registration",
            ));
        }
        registry.set(target, FLAGS)?;
        self.target = target;
        self.owned = true;
        let observed = registry.mouse()?;
        if !matches_registration(observed, target) {
            return Err(io::Error::other(format!(
                "Mouse registration did not retain its owner: {:?}",
                observed.map(|(actual, flags)| (actual == target, format!("0x{flags:x}")))
            )));
        }
        Ok(())
    }
    pub(super) fn stop(&mut self, registry: &mut impl Registry) -> io::Result<()> {
        if !self.owned {
            return Ok(());
        }
        if !matches_registration(registry.mouse()?, self.target) {
            self.owned = false;
            return Ok(());
        }
        registry.set(0, RIDEV_REMOVE)?;
        if matches_registration(registry.mouse()?, self.target) {
            return Err(io::Error::other("Mouse observer removal was not verified"));
        }
        self.owned = false;
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct Observer {
    owner: Option<(String, String)>,
    binding: Option<RawMouseBinding>,
    session: Option<mouse_input_worker::Session>,
    sample: mouse_input_worker::Sample,
    error: Option<String>,
}
impl Observer {
    pub(super) fn value(&self) -> f64 {
        if self.sample.active {
            self.sample.speed
        } else {
            0.
        }
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn stop(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.stop();
            let sample = session.snapshot();
            if sample.error.is_some() {
                self.error = sample.error;
            }
        }
        self.sample = Default::default();
    }
}
pub(super) unsafe fn receive(state: &mut State, owner: (String, String), binding: BindingResult) {
    if state.calibration.owner.as_ref() != Some(&owner) {
        return;
    }
    state.mouse_feedback.stop();
    state.mouse_feedback.owner = Some(owner);
    match binding {
        Ok(binding) => {
            state.mouse_feedback.binding = Some(binding);
            state.mouse_feedback.error = None;
        }
        Err(error) => {
            state.mouse_feedback.binding = None;
            state.mouse_feedback.error = Some(error);
        }
    }
    reconcile(state);
}
pub(super) unsafe fn reconcile(state: &mut State) {
    let desired = advanced_view::feedback_owner(state);
    let visible = IsWindowVisible(state.hwnd) != 0
        && IsIconic(state.hwnd) == 0
        && !state.closing
        && state.profile_modal.is_none();
    let observer = &mut state.mouse_feedback;
    if !visible || desired.is_none() || desired != observer.owner {
        observer.stop();
        return;
    }
    if observer.session.is_some() {
        return;
    }
    let Some(binding) = observer.binding.as_ref() else {
        return;
    };
    match mouse_input_worker::Session::start(binding.clone(), state.hwnd, NOTIFY) {
        Ok(session) => {
            observer.session = Some(session);
            observer.error = None;
        }
        Err(error) => {
            observer.binding = None;
            observer.error = Some(format!("Live graph unavailable: {error}"));
        }
    }
}
pub(super) unsafe fn stop(state: &mut State) {
    state.mouse_feedback.stop();
}
pub(super) unsafe fn notification(state: &mut State) {
    if let Some(session) = state.mouse_feedback.session.as_ref() {
        state.mouse_feedback.sample = session.snapshot();
    }
    let changed = state.mouse_feedback.sample.device_changed;
    if let Some(error) = state.mouse_feedback.sample.error.clone() {
        state.mouse_feedback.stop();
        state.mouse_feedback.error = Some(error);
        state.mouse_feedback.binding = None;
    }
    if changed {
        state.mouse_feedback.stop();
        state.mouse_feedback.binding = None;
        state.mouse_feedback.owner = None;
        if state.tab == Tab::Advanced {
            calibration_view::request(state, true);
        }
    }
    reconcile(state);
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) fn movement(input: &RAWINPUT, expected: usize) -> Option<(i32, i32)> {
    if expected == 0
        || input.header.hDevice as usize != expected
        || input.header.dwType != RIM_TYPEMOUSE
        || input.header.dwSize as usize != size_of::<RAWINPUT>()
    {
        return None;
    }
    let mouse = unsafe { input.data.mouse };
    if mouse.usFlags & MOUSE_MOVE_ABSOLUTE != 0 || (mouse.lLastX == 0 && mouse.lLastY == 0) {
        return None;
    }
    Some((mouse.lLastX, mouse.lLastY))
}
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"active":state.mouse_feedback.sample.active,"registered":state.mouse_feedback.sample.registered,"speed":state.mouse_feedback.value(),"accepted_records":state.mouse_feedback.sample.accepted_records,"binding_present":state.mouse_feedback.binding.is_some(),"error":state.mouse_feedback.error})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mock {
        current: Option<(usize, u32)>,
        writes: Vec<(usize, u32)>,
        strip_notification: bool,
    }
    impl Registry for Mock {
        fn mouse(&mut self) -> io::Result<Option<(usize, u32)>> {
            Ok(self.current)
        }
        fn set(&mut self, target: usize, flags: u32) -> io::Result<()> {
            self.writes.push((target, flags));
            self.current = if flags == RIDEV_REMOVE {
                None
            } else {
                Some((
                    target,
                    if self.strip_notification {
                        flags & !RIDEV_DEVNOTIFY
                    } else {
                        flags
                    },
                ))
            };
            Ok(())
        }
    }
    #[test]
    fn notification_option_can_be_omitted_from_readback_without_weakening_owner_or_suppression_checks(
    ) {
        let mut registry = Mock {
            strip_notification: true,
            ..Default::default()
        };
        let mut registration = Registration::default();
        registration.start(&mut registry, 7).unwrap();
        assert_eq!(registry.current, Some((7, 0x100)));
        assert!(registration.owns_live(&mut registry).unwrap());
        assert!(!matches_registration(Some((8, 0x100)), 7));
        assert!(!matches_registration(Some((7, 0x130)), 7));
        assert!(!matches_registration(Some((7, 0)), 7));
        registration.stop(&mut registry).unwrap();
        assert_eq!(registry.current, None);
    }
    #[test]
    fn passive_registration_is_verified_removed_and_never_suppresses_legacy_input() {
        let mut registry = Mock::default();
        let mut registration = Registration::default();
        registration.start(&mut registry, 7).unwrap();
        registration.start(&mut registry, 7).unwrap();
        assert_eq!(registry.writes, [(7, 0x2100)]);
        assert_eq!(FLAGS & RIDEV_NOLEGACY, 0);
        registration.stop(&mut registry).unwrap();
        registration.stop(&mut registry).unwrap();
        assert_eq!(registry.writes, [(7, 0x2100), (0, RIDEV_REMOVE)]);
    }
    #[test]
    fn existing_or_replaced_process_observer_is_not_overwritten_or_removed() {
        let mut registry = Mock {
            current: Some((4, 0)),
            ..Default::default()
        };
        let mut registration = Registration::default();
        assert!(registration.start(&mut registry, 7).is_err());
        assert!(registry.writes.is_empty());
        registry.current = None;
        registration.start(&mut registry, 7).unwrap();
        registry.current = Some((8, FLAGS));
        registration.stop(&mut registry).unwrap();
        assert_eq!(registry.current, Some((8, FLAGS)));
        assert_eq!(registry.writes, [(7, FLAGS)]);
    }
    #[test]
    fn raw_decoder_rejects_unrelated_absolute_nonmouse_and_button_only_records() {
        unsafe {
            let mut packet: RAWINPUT = std::mem::zeroed();
            packet.header.dwSize = size_of::<RAWINPUT>() as u32;
            packet.header.hDevice = 7 as _;
            packet.data.mouse.lLastX = -3;
            packet.data.mouse.lLastY = 4;
            assert_eq!(movement(&packet, 7), Some((-3, 4)));
            assert_eq!(movement(&packet, 8), None);
            packet.data.mouse.usFlags = MOUSE_MOVE_ABSOLUTE;
            assert_eq!(movement(&packet, 7), None);
            packet.data.mouse.usFlags = 0;
            packet.header.dwType = RIM_TYPEKEYBOARD;
            assert_eq!(movement(&packet, 7), None);
            packet.header.dwType = RIM_TYPEMOUSE;
            packet.data.mouse.lLastX = 0;
            packet.data.mouse.lLastY = 0;
            assert_eq!(movement(&packet, 7), None);
        }
    }
}
