//! Source modal Browse/Save uses common pickers only after posting out of State.
use super::*;
use std::os::windows::ffi::OsStringExt;
use windows_sys::Win32::UI::Controls::Dialogs::*;
pub(super) fn browse(owner: HWND) -> Result<Option<PathBuf>, String> {
    profile_files::choose_import(owner)
}
pub(super) fn save(owner: HWND) -> Result<Option<(PathBuf, profile_files::Format)>, String> {
    let filters=wide("Synapse 4 selected profiles\0*.synapse4\0Full native backup - all saved devices and profiles\0*.json\0");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let initial = wide(&format!("profiles - {stamp}.synapse4"));
    let mut buffer = vec![0u16; 32768];
    buffer[..initial.len()].copy_from_slice(&initial);
    let mut options: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = owner;
    options.lpstrFilter = filters.as_ptr();
    options.nFilterIndex = 1;
    options.lpstrFile = buffer.as_mut_ptr();
    options.nMaxFile = buffer.len() as u32;
    let title = wide("Save as");
    options.lpstrTitle = title.as_ptr();
    options.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
    if unsafe { GetSaveFileNameW(&mut options) } == 0 {
        let code = unsafe { CommDlgExtendedError() };
        return if code == 0 {
            Ok(None)
        } else {
            Err(format!("Windows Save dialog failed: {code:#x}"))
        };
    }
    let end = buffer
        .iter()
        .position(|c| *c == 0)
        .ok_or("Windows returned an unterminated path")?;
    let path = std::ffi::OsString::from_wide(&buffer[..end]).into();
    let format = if options.nFilterIndex == 2 {
        profile_files::Format::Native
    } else {
        profile_files::Format::Synapse4
    };
    Ok(Some((path, format)))
}
