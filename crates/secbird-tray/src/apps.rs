//! Which program owns a window: for switching profiles with the program in front.

use std::collections::BTreeSet;
use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, TRUE};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsWindowVisible, GW_OWNER,
};

/// Full executable location of the foreground window; failure never guesses a name.
pub fn foreground() -> Option<String> {
    path_of(unsafe { GetForegroundWindow() })
}

/// Exe names of programs with a visible, titled top-level window, sorted. Offered as
/// suggestions in the editor so nobody has to know the exe name of their game.
pub fn running() -> Vec<String> {
    running_paths()
        .iter()
        .map(|path| secbird_core::profile::normalize_app(path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Visible application locations, for explicit local link selection. No file scan.
pub fn running_paths() -> Vec<String> {
    unsafe extern "system" fn each(hwnd: HWND, list: LPARAM) -> BOOL {
        let list = &mut *(list as *mut BTreeSet<String>);
        if list.len() >= 128 {
            return 0;
        }
        let shown = IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null();
        if shown && GetWindowTextLengthW(hwnd) > 0 {
            if let Some(exe) = path_of(hwnd) {
                list.insert(exe);
            }
        }
        TRUE
    }
    let mut list = BTreeSet::<String>::new();
    unsafe { EnumWindows(Some(each), &mut list as *mut _ as LPARAM) };
    list.retain(|path| {
        !matches!(
            secbird_core::profile::normalize_app(path).as_str(),
            "explorer.exe" | "applicationframehost.exe" | "textinputhost.exe" | "secbird-tray.exe"
        )
    });
    list.into_iter().collect()
}

/// Limited query rights work on most processes, elevated games included; protected
/// processes refuse and yield `None`.
fn path_of(hwnd: HWND) -> Option<String> {
    if hwnd.is_null() {
        return None;
    }
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        if pid == GetCurrentProcessId() {
            return None;
        }
        path_of_pid(pid)
    }
}
fn path_of_pid(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
                != 0;
        CloseHandle(process);
        if !ok || len == 0 || len as usize > buf.len() {
            return None;
        }
        let path = String::from_utf16(&buf[..len as usize]).ok()?;
        secbird_core::profile::normalize_app_path(&path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_process_location_is_read_without_launching_or_borrowing_window_identity() {
        let observed = path_of_pid(unsafe { GetCurrentProcessId() }).expect("own process query");
        let expected = secbird_core::profile::normalize_app_path(
            &std::env::current_exe().unwrap().to_string_lossy(),
        )
        .unwrap();
        assert_eq!(observed, expected);
        assert!(path_of_pid(0).is_none());
        assert!(path_of(std::ptr::null_mut()).is_none());
    }
}
