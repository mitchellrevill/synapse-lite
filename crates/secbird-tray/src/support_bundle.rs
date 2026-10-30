use std::os::windows::ffi::OsStringExt;
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub unsafe fn dialog(owner: HWND) {
    let title = wide("Local support bundle");
    let choice=MessageBoxW(owner,wide("Include recent error categories?\nRaw logs, profiles, serials and paths are always excluded.\nYes: include categories. No: omit them. Cancel: stop.").as_ptr(),title.as_ptr(),MB_YESNOCANCEL | MB_DEFBUTTON3);
    if choice == IDCANCEL {
        return;
    }
    let bytes =
        serde_json::to_vec_pretty(&secbird_core::support_bundle::preview(choice == IDYES)).unwrap();
    let preview = format!(
        "{}\n\nSave this exact local report? Nothing is uploaded.",
        String::from_utf8_lossy(&bytes)
    );
    if MessageBoxW(owner, wide(&preview).as_ptr(), title.as_ptr(), MB_OKCANCEL) != IDOK {
        return;
    }
    let mut buffer = vec![0u16; 32768];
    let name = wide("synapse-lite-support.json");
    buffer[..name.len()].copy_from_slice(&name);
    let filter = wide("JSON report\0*.json\0");
    let extension = wide("json");
    let mut options: OPENFILENAMEW = std::mem::zeroed();
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = owner;
    options.lpstrFile = buffer.as_mut_ptr();
    options.nMaxFile = buffer.len() as u32;
    options.lpstrFilter = filter.as_ptr();
    options.lpstrDefExt = extension.as_ptr();
    options.Flags = OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST;
    if GetSaveFileNameW(&mut options) == 0 {
        return;
    }
    let end = buffer.iter().position(|c| *c == 0).unwrap();
    let path = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buffer[..end]));
    let message=match secbird_core::support_bundle::save_new(&path,&bytes){Ok(())=>"Support bundle saved locally.".to_owned(),Err(e)=>format!("Could not save support bundle ({}). Choose a new filename; existing files are never replaced.",e.kind())};
    MessageBoxW(owner, wide(&message).as_ptr(), title.as_ptr(), MB_OK);
}
