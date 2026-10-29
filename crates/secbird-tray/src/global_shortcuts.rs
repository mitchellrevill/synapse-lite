//! Owned Win32 hotkey registrations. No keyboard/mouse hooks or synthetic keys.
use secbird_core::global_shortcuts::{self as model, Chord, Library};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    UI::{
        Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT},
        WindowsAndMessaging::*,
    },
};

pub const RELOAD: u32 = WM_APP + 180;
static OWNER: AtomicUsize = AtomicUsize::new(0);
static STATUS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
pub fn status() -> BTreeMap<String, String> {
    STATUS.lock().unwrap_or_else(|p| p.into_inner()).clone()
}
pub fn changed() {
    let hwnd = OWNER.load(Ordering::Acquire) as HWND;
    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, RELOAD, 0, 0);
        }
    }
}
pub struct Runtime {
    hwnd: HWND,
    active: BTreeMap<i32, (String, Chord, u16)>,
    next: i32,
    suspended: bool,
}
impl Runtime {
    /// Construct on the thread that owns hwnd. Test runs never publish their owner.
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            active: BTreeMap::new(),
            next: 0x3000,
            suspended: false,
        }
    }
    pub fn publish(&self) {
        OWNER.store(self.hwnd as usize, Ordering::Release);
    }
    pub fn clear(&mut self) {
        for id in self.active.keys() {
            unsafe {
                UnregisterHotKey(self.hwnd, *id);
            }
        }
        self.active.clear();
        // IDs are never reused: queued notifications from a removed definition
        // cannot be interpreted as the replacement assignment.
    }
    pub fn suspend(&mut self) {
        self.suspended = true;
        self.clear();
        STATUS.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }
    pub fn resume(&mut self) {
        self.suspended = false;
        self.reload();
    }
    pub fn reload(&mut self) {
        self.clear();
        let path = secbird_core::config_dir().join("global-shortcuts.json");
        let mut status = BTreeMap::new();
        match model::load(&path) {
            Ok(library) => status = self.configure(&library),
            Err(error) => {
                status.insert(String::new(), error.to_string());
            }
        }
        *STATUS.lock().unwrap_or_else(|p| p.into_inner()) = status;
    }
    pub fn configure(&mut self, library: &Library) -> BTreeMap<String, String> {
        self.clear();
        let mut status = BTreeMap::new();
        if self.suspended {
            return status;
        }
        for row in library.records() {
            let id = row["guid"].as_str().unwrap_or_default().to_string();
            let reason = if !model::enabled(row) {
                Some("Disabled".into())
            } else {
                model::blocked(row)
            };
            if let Some(reason) = reason {
                status.insert(id, reason);
                continue;
            }
            let chord = model::host_chord(row).unwrap();
            let command = model::media_command(row).unwrap();
            if library.records().iter().any(|other| {
                other["guid"] != row["guid"]
                    && model::enabled(other)
                    && model::host_chord(other).ok() == Some(chord)
            }) {
                status.insert(
                    id,
                    "Conflicting Windows chord; neither shortcut registered".into(),
                );
                continue;
            }
            if self.next > 0xbfff {
                status.insert(
                    id,
                    "Registration identity budget exhausted; restart tray".into(),
                );
                continue;
            }
            let token = self.next;
            self.next += 1;
            if unsafe {
                RegisterHotKey(self.hwnd, token, chord.modifiers | MOD_NOREPEAT, chord.key)
            } == 0
            {
                status.insert(
                    id,
                    format!(
                        "Windows refused registration ({}); chord may be reserved or in use",
                        unsafe { GetLastError() }
                    ),
                );
            } else {
                self.active.insert(token, (id.clone(), chord, command));
                status.insert(
                    id,
                    "Registered · Windows chord · either side modifiers".into(),
                );
            }
        }
        status
    }
    /// This accepts OS hotkey notifications only, never key-down/up transitions.
    /// Exact token + chord check rejects stale/cancelled and mismatched messages.
    pub fn command(&self, id: usize, packed: isize) -> Option<u16> {
        if self.suspended {
            return None;
        }
        let (_, chord, command) = self.active.get(&(i32::try_from(id).ok()?))?;
        ((packed as u32 & 0xffff) == chord.modifiers
            && ((packed as u32 >> 16) & 0xffff) == chord.key)
            .then_some(*command)
    }
    pub fn dispatch(&self, id: usize, packed: isize) -> Result<(), String> {
        let Some(command) = self.command(id, packed) else {
            return Ok(());
        };
        // Capture the target once. UIPI/timeout failures are surfaced; no fallback
        // injection, delayed output, retry, or target switch can affect typing.
        let target = unsafe { GetForegroundWindow() };
        send_command(target, command)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.clear();
        let _ = OWNER.compare_exchange(self.hwnd as usize, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}
fn send_command(target: HWND, command: u16) -> Result<(), String> {
    if target.is_null() {
        return Err("No foreground window for multimedia command".into());
    }
    let mut result = 0;
    if unsafe {
        SendMessageTimeoutW(
            target,
            WM_APPCOMMAND,
            target as usize,
            (command as isize) << 16,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            200,
            &mut result,
        )
    } == 0
    {
        return Err(format!(
            "Multimedia command was refused or timed out ({})",
            unsafe { GetLastError() }
        ));
    }
    if result == 0 {
        return Err("Foreground application did not acknowledge the multimedia command".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ptr::{null, null_mut},
        sync::atomic::AtomicU32,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    static COMMAND: AtomicU32 = AtomicU32::new(0);
    unsafe extern "system" fn proc(hwnd: HWND, message: u32, w: usize, l: isize) -> isize {
        if message == WM_APPCOMMAND {
            COMMAND.store(((l as u32 >> 16) & 0xfff) as u32, Ordering::SeqCst);
            return 1;
        }
        DefWindowProcW(hwnd, message, w, l)
    }
    #[test]
    #[ignore = "creates owned message-only HWNDs and temporarily registers CtrlAltShiftF23; no personal input or output"]
    fn owned_hotkeys_dispatch_conflict_disable_suspend_and_cleanup() {
        unsafe {
            let class: Vec<u16> = format!("SynapseShortcutTest{}", std::process::id())
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let instance = GetModuleHandleW(null());
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(proc);
            wc.lpszClassName = class.as_ptr();
            wc.hInstance = instance;
            assert_ne!(RegisterClassW(&wc), 0);
            let window = || {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    instance,
                    null(),
                )
            };
            let owner = window();
            let receiver = window();
            assert!(!owner.is_null() && !receiver.is_null());
            let mut runtime = Runtime::new(owner);
            let mut library = Library::default();
            let mut row = model::create("owned");
            row["inputID"] = serde_json::json!("KEY_F23");
            row["inputModifiers"] =
                serde_json::json!(["KEY_LEFT_CTRL", "KEY_LEFT_ALT", "KEY_LEFT_SHIFT"]);
            model::set_enabled(&mut row, true).unwrap();
            library.push(row).unwrap();
            let result = runtime.configure(&library);
            assert!(result["owned"].starts_with("Registered"), "{result:?}");
            let token = *runtime.active.keys().next().unwrap();
            let chord = model::host_chord(&library.records()[0]).unwrap();
            let packed = ((chord.key << 16) | chord.modifiers) as isize;
            assert_eq!(runtime.command(token as usize, packed), Some(14));
            assert_eq!(runtime.command(token as usize, packed ^ 1), None);
            COMMAND.store(0, Ordering::SeqCst);
            send_command(receiver, runtime.command(token as usize, packed).unwrap()).unwrap();
            assert_eq!(COMMAND.load(Ordering::SeqCst), 14);
            // A second owner sees an OS conflict, then succeeds after cleanup.
            assert_eq!(
                RegisterHotKey(receiver, 1, chord.modifiers | MOD_NOREPEAT, chord.key),
                0
            );
            runtime.suspend();
            assert_eq!(runtime.command(token as usize, packed), None);
            assert_ne!(
                RegisterHotKey(receiver, 1, chord.modifiers | MOD_NOREPEAT, chord.key),
                0
            );
            UnregisterHotKey(receiver, 1);
            runtime.suspended = false;
            runtime.configure(&library);
            assert_eq!(runtime.command(token as usize, packed), None);
            assert_ne!(*runtime.active.keys().next().unwrap(), token);
            let mut duplicate = library.records()[0].clone();
            duplicate["guid"] = serde_json::json!("conflict");
            library.push(duplicate).unwrap();
            let result = runtime.configure(&library);
            assert!(runtime.active.is_empty());
            assert!(result.values().all(|s| s.contains("Conflicting")));
            library.remove("conflict");
            model::set_enabled(&mut library.envelope["mappings"][0], false).unwrap();
            runtime.configure(&library);
            assert!(runtime.active.is_empty());
            model::set_enabled(&mut library.envelope["mappings"][0], true).unwrap();
            runtime.configure(&library);
            drop(runtime);
            assert_ne!(
                RegisterHotKey(receiver, 1, chord.modifiers | MOD_NOREPEAT, chord.key),
                0
            );
            UnregisterHotKey(receiver, 1);
            DestroyWindow(owner);
            DestroyWindow(receiver);
            UnregisterClassW(class.as_ptr(), instance);
        }
    }
}
