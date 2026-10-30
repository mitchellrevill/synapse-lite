//! Cached local information dialog; USB access stays on the editor worker.
use super::*;
pub(super) unsafe fn show(state: &mut State, id: &str) {
    let Some(text) = state.information_cache.get(id).cloned() else { return; };
    let body = format!("{text}\n\nCached snapshot. YES: refresh read-only details. NO: copy details. CANCEL: close.");
    let choice = MessageBoxW(state.hwnd, wide(&body).as_ptr(), wide("Device information — local snapshot").as_ptr(), MB_YESNOCANCEL | MB_DEFBUTTON3);
    if choice == IDYES { state.queue(Job::DeviceInformation(id.into())); }
    if choice == IDNO
        && !crate::win::set_clipboard(&text) { state.error("Clipboard is busy; reopen Device info and try copying again."); }
}
