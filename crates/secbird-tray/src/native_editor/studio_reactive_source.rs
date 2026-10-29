//! Decoder/replay seam only. This does not register for WM_INPUT, discover
//! hardware, enter driver mode or authorize capture. A future live caller must
//! hold and validate RawKeyboardBinding at start/first event/end, own its passive
//! registration and stop on identity/ownership loss. See STUDIO-REACTIVE.md.
use secbird_core::studio::{
    reactive_input::KeyboardKey,
    reactive_replay::{ReactiveReplay, ReplaySession},
};
use windows_sys::Win32::UI::{
    Input::{RAWINPUT, RAWINPUTHEADER, RAWKEYBOARD, RIM_TYPEKEYBOARD},
    WindowsAndMessaging::{WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP},
};

/// Pure exact-handle decoder. `expected_handle` alone is not identity proof;
/// callers must separately authenticate the device collection/container.
pub(super) fn decode(input: &RAWINPUT, expected_handle: usize) -> Option<(KeyboardKey, bool)> {
    if expected_handle == 0
        || input.header.hDevice as usize != expected_handle
        || input.header.dwType != RIM_TYPEKEYBOARD
        // A keyboard payload is smaller than RAWINPUT's largest union member.
        || input.header.dwSize as usize != std::mem::size_of::<RAWINPUTHEADER>() + std::mem::size_of::<RAWKEYBOARD>()
    {
        return None;
    }
    let keyboard = unsafe { input.data.keyboard };
    // RI_KEY_BREAK=1, E0=2, E1=4. No VKey->LED/scan-code fallback for fake or
    // overrun records; PrintScreen/Pause special sequences remain unverified.
    if keyboard.VKey == 255
        || keyboard.MakeCode == 0
        || keyboard.MakeCode == 255
        || keyboard.Flags & !7 != 0
        || keyboard.Flags & 6 == 6
    {
        return None;
    }
    let down = keyboard.Flags & 1 == 0;
    if !matches!(
        (down, keyboard.Message),
        (true, WM_KEYDOWN | WM_SYSKEYDOWN) | (false, WM_KEYUP | WM_SYSKEYUP)
    ) {
        return None;
    }
    let key = KeyboardKey::new(keyboard.MakeCode, keyboard.Flags & 6).ok()?;
    Some((key, down))
}

pub(super) struct ReplayInput {
    session: ReplaySession,
    placement: String,
    handle: usize,
}
impl ReplayInput {
    /// Explicit diagnostic binding. Does not authenticate the numeric handle or
    /// subscribe to input; production Studio never constructs this yet.
    pub(super) fn diagnostic(session: ReplaySession, placement: String, handle: usize) -> Self {
        Self {
            session,
            placement,
            handle,
        }
    }
    pub(super) fn observe(
        &self,
        replay: &mut ReactiveReplay,
        input: &RAWINPUT,
        now_ms: u64,
    ) -> secbird_core::Result<usize> {
        let Some((key, down)) = decode(input, self.handle) else {
            return Ok(0);
        };
        replay.key(&self.session, &self.placement, key, down, now_ms)
    }
    pub(super) fn source_lost(&mut self, replay: &mut ReactiveReplay) {
        self.handle = 0;
        replay.stop_session(&self.session);
    }
}

#[cfg(test)]
#[path = "studio_reactive_source_tests.rs"]
mod tests;
