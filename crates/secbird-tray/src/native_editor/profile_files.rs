//! Bounded profile-file I/O and standard Windows file pickers.
//! Parsing, translation and hardware decisions stay in the core/dispatcher.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::Dialogs::*;

pub const MAX_PROFILE_BYTES: usize = 2 * 1024 * 1024;
static SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub enum Format {
    Native,
    Synapse4,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub fn choose_import(owner: HWND) -> Result<Option<PathBuf>, String> {
    Ok(choose(owner, false)?.map(|(path, _)| path))
}

pub fn choose_export(owner: HWND) -> Result<Option<(PathBuf, Format)>, String> {
    choose(owner, true)
}

fn choose(owner: HWND, save: bool) -> Result<Option<(PathBuf, Format)>, String> {
    let (filter, extension, name) = match save {
        false => (
            "Synapse / Secretarybird profiles\0*.synapse4;*.synapse3;*.json\0All files\0*.*\0",
            "synapse4",
            "",
        ),
        true => (
            "Synapse 4 profiles\0*.synapse4\0Secretarybird-compatible backup\0*.json\0",
            "synapse4",
            "Profiles",
        ),
    };
    let filter = wide(filter);
    let extension = wide(extension);
    let title = wide(if save {
        "Export profiles"
    } else {
        "Import profiles"
    });
    let mut path = vec![0u16; 32768];
    let initial = wide(name);
    path[..initial.len()].copy_from_slice(&initial);
    let mut options: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    options.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    options.hwndOwner = owner;
    options.lpstrFilter = filter.as_ptr();
    options.nFilterIndex = 1;
    options.lpstrFile = path.as_mut_ptr();
    options.nMaxFile = path.len() as u32;
    options.lpstrTitle = title.as_ptr();
    // Preserve the path covered by Windows' overwrite confirmation, including
    // when the user changes the format filter. Never rewrite its suffix later.
    options.lpstrDefExt = if save {
        std::ptr::null()
    } else {
        extension.as_ptr()
    };
    options.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut options)
        } else {
            GetOpenFileNameW(&mut options)
        }
    };
    if accepted == 0 {
        let code = unsafe { CommDlgExtendedError() };
        return if code == 0 {
            Ok(None)
        } else {
            Err(format!("Windows file dialog failed: {code:#x}"))
        };
    }
    let end = path
        .iter()
        .position(|c| *c == 0)
        .ok_or("Windows returned an unterminated file path")?;
    let format = if save && options.nFilterIndex == 2 {
        Format::Native
    } else {
        Format::Synapse4
    };
    let path: PathBuf = std::ffi::OsString::from_wide(&path[..end]).into();
    Ok(Some((path, format)))
}

pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open profile file: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read profile file: {e}"))?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("Profile document exceeds the 2 MiB interchange budget".into());
    }
    Ok(bytes)
}

/// The selected file is replaced only after the complete new document is flushed.
/// The file picker owns the normal overwrite prompt; failed writes preserve the old file.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("Profile export exceeds the 2 MiB interchange budget".into());
    }
    let parent = path.parent().ok_or("Export path has no parent directory")?;
    let mut pending = None;
    for _ in 0..8 {
        let sequence = SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".synapse-lite-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                pending = Some((temporary, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Cannot create export: {error}")),
        }
    }
    let (temporary, mut file) = pending.ok_or("Cannot reserve a temporary export file")?;
    let result = (|| {
        file.write_all(bytes)
            .map_err(|e| format!("Cannot write export: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Cannot flush export: {e}"))?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| format!("Cannot replace export: {e}"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_export_replaces_old_file_and_failed_export_preserves_it() {
        let folder = std::env::temp_dir().join(format!(
            "synapse-export-{}-{}",
            std::process::id(),
            SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("profiles.json");
        std::fs::write(&path, b"old").unwrap();
        write(&path, b"complete new document").unwrap();
        assert_eq!(read(&path).unwrap(), b"complete new document");
        assert!(write(&path, &vec![0; MAX_PROFILE_BYTES + 1]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"complete new document");
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
