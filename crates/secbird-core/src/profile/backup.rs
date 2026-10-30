//! Explicit whole-store recovery. Never merges or rewrites GUID references.
use super::Store;
use crate::{
    profile_compat::{check_json_complexity, MAX_DOCUMENT_BYTES},
    Error, Result,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn err(e: impl std::fmt::Display) -> Error {
    Error::Db(format!("Profile backup: {e}"))
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    if !std::fs::symlink_metadata(path)
        .map_err(err)?
        .file_type()
        .is_file()
    {
        return Err(err("requires a regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(err)?
        .take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    validate(&bytes)?;
    Ok(bytes)
}
pub fn validate(bytes: &[u8]) -> Result<Store> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(err("exceeds 2 MiB"));
    }
    let content = bytes.strip_prefix(&[239, 187, 191]).unwrap_or(bytes);
    check_json_complexity(content)?;
    let value: serde_json::Value = serde_json::from_slice(content).map_err(err)?;
    if !value
        .get("devices")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(err("requires a native store with devices"));
    }
    let store: Store = serde_json::from_value(value).map_err(err)?;
    store.validate_save()?;
    Ok(store)
}
/// A fresh destination is mandatory: existing backups are never overwritten.
pub fn create(path: &Path, bytes: &[u8]) -> Result<()> {
    validate(bytes)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(err)?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(err(e));
    }
    Ok(())
}
/// Caller supplies the exact preview snapshot. A recovery file must be created
/// successfully before the atomic replacement. Retain it on any later failure.
pub fn restore(path: &Path, expected: &[u8], incoming: &[u8], recovery: &Path) -> Result<()> {
    validate(incoming)?;
    if read(path)? != expected {
        return Err(err("store changed since preview; preview again"));
    }
    create(recovery, expected)?;
    let temporary = recovery.with_extension("pending");
    create(&temporary, incoming)?;
    if read(path)? != expected {
        let _ = std::fs::remove_file(&temporary);
        return Err(err("store changed; recovery retained"));
    }
    if let Err(e) = replace(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err(format!("replacement failed; recovery retained: {e}")));
    }
    Ok(())
}
#[cfg(windows)]
fn replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn replace(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(source, target)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_preserves_opaque_bytes_and_refuses_stale_or_conflicting_backup() {
        let dir = std::env::temp_dir().join(format!(
            "store-backup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        let recovery = dir.join("recovery.json");
        let old = br#"{"devices":{},"future":{"guid":"opaque-reference"}}"#;
        let new = br#"{"devices":{},"future":{"guid":"same-reference","unknown":[1,2]}}"#;
        create(&path, old).unwrap();
        assert!(restore(&path, new, new, &recovery).is_err());
        assert!(!recovery.exists());
        restore(&path, old, new, &recovery).unwrap();
        assert_eq!(read(&path).unwrap(), new);
        assert_eq!(read(&recovery).unwrap(), old);
        assert!(restore(&path, new, old, &recovery).is_err());
        assert_eq!(read(&path).unwrap(), new);
        for p in [path, recovery] {
            std::fs::remove_file(p).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn locked_destination_keeps_original_and_recovery() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = std::env::temp_dir().join(format!("store-locked-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        let recovery = dir.join("recovery.json");
        let old = br#"{"devices":{}}"#;
        let new = br#"{"devices":{},"future":true}"#;
        create(&path, old).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(restore(&path, old, new, &recovery).is_err());
        assert_eq!(read(&path).unwrap(), old);
        assert_eq!(read(&recovery).unwrap(), old);
        assert!(!recovery.with_extension("pending").exists());
        drop(lock);
        for p in [path, recovery] {
            std::fs::remove_file(p).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn rejects_invalid_missing_active_and_oversize() {
        for bytes in [
            b"{}".as_slice(),
            br#"{"devices":{"x":{"active":"missing","profiles":{}}}}"#,
            &vec![b' '; MAX_DOCUMENT_BYTES + 1],
        ] {
            assert!(validate(bytes).is_err());
        }
    }
}
