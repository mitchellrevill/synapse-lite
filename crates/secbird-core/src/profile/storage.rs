//! Bounded native profile storage. Rejected data never reaches a file mutation.
use super::Store;
use crate::{
    profile_compat::{check_json_complexity, MAX_DOCUMENT_BYTES},
    Error, Result,
};
use std::io::{Read, Write};
use std::path::Path;

fn error(path: &Path, message: impl std::fmt::Display) -> Error {
    Error::Db(format!("{}: {message}", path.display()))
}

fn read_document(reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Db(e.to_string()))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::Db(
            "Native profile file exceeds the 2 MiB memory budget; original file retained".into(),
        ));
    }
    Ok(bytes)
}

pub(super) fn load(path: &Path) -> Result<Store> {
    let mut store = match std::fs::File::open(path) {
        Ok(file) => {
            let bytes = read_document(file).map_err(|e| error(path, e))?;
            // Preserve the old loader's acceptance of repeated UTF-8 BOMs.
            let mut content = bytes.as_slice();
            while let Some(rest) = content.strip_prefix(&[0xef, 0xbb, 0xbf]) {
                content = rest;
            }
            check_json_complexity(content).map_err(|e| error(path, e))?;
            serde_json::from_slice(content).map_err(|e| error(path, e))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
        Err(e) => return Err(error(path, e)),
    };
    store.path = path.to_path_buf();
    store.rename_old_buttons();
    Ok(store)
}

#[derive(Default)]
struct BoundedDocument {
    bytes: Vec<u8>,
    overflowed: bool,
}
impl Write for BoundedDocument {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_DOCUMENT_BYTES - self.bytes.len() {
            self.overflowed = true;
            return Err(std::io::Error::other(
                "Native profile save exceeds the 2 MiB memory budget; existing files retained",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode(store: &Store) -> Result<Vec<u8>> {
    let path = &store.path;
    let mut document = BoundedDocument::default();
    if let Err(failure) = serde_json::to_writer_pretty(&mut document, store) {
        if !document.overflowed {
            return Err(error(path, failure));
        }
        // A compact file at the exact limit must remain saveable. Discard the
        // incomplete pretty buffer and retry serialization, never a file write.
        document = BoundedDocument::default();
        serde_json::to_writer(&mut document, store).map_err(|e| error(path, e))?;
    }
    check_json_complexity(&document.bytes).map_err(|e| error(path, e))?;
    Ok(document.bytes)
}

pub(super) fn save(store: &Store) -> Result<()> {
    let path = &store.path;
    let document = encode(store)?;
    // Only a complete, accepted document may create directories or touch tmp.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| error(path, e))?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|e| error(path, e))?;
    file.write_all(&document).map_err(|e| error(path, e))?;
    file.sync_all().map_err(|e| error(path, e))?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(|e| error(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bounded-store-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> std::path::PathBuf {
            self.0.join("profiles.json")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            // Fixed files only; no recursive deletion or caller-controlled path.
            let _ = std::fs::remove_file(self.file());
            let _ = std::fs::remove_file(self.file().with_extension("json.tmp"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    #[test]
    fn capped_read_stops_before_the_rest_of_a_growing_stream() {
        struct Endless(usize);
        impl Read for Endless {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.0 += out.len();
                out.fill(b' ');
                Ok(out.len())
            }
        }
        let mut stream = Endless(0);
        assert!(read_document(&mut stream).is_err());
        assert_eq!(stream.0, MAX_DOCUMENT_BYTES + 1);
    }
    #[test]
    fn accepted_native_data_round_trips_opaque_fields_bom_and_legacy_names() {
        let dir = Directory::new();
        let value = json!({"opaque":{"keep":[1,2,"future"]},"devices":{"025D:synthetic":{"active":"One","profiles":{"One":{"buttons":{"top-extra":"key:a"},"future":{"keep":true}}}}}});
        let mut bytes = vec![0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf];
        bytes.extend(serde_json::to_vec(&value).unwrap());
        std::fs::write(dir.file(), bytes).unwrap();
        let store = Store::load(&dir.file()).unwrap();
        assert_eq!(
            store.devices["025D:synthetic"].profiles["One"].buttons["side-extra"],
            "key:a"
        );
        store.save().unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.file()).unwrap()).unwrap();
        assert_eq!(saved["opaque"], value["opaque"]);
        assert_eq!(
            saved["devices"]["025D:synthetic"]["profiles"]["One"]["future"],
            json!({"keep":true})
        );
        assert_eq!(
            serde_json::to_value(Store::load(&dir.file()).unwrap()).unwrap(),
            saved
        );
    }
    #[test]
    fn rejected_save_keeps_original_and_existing_temporary_file() {
        let dir = Directory::new();
        let mut store = Store::load(&dir.file()).unwrap();
        store.extra.insert("opaque".into(), json!("original"));
        store.save().unwrap();
        let before = std::fs::read(dir.file()).unwrap();
        let temporary = dir.file().with_extension("json.tmp");
        std::fs::write(&temporary, b"old temporary evidence").unwrap();
        store
            .extra
            .insert("large".into(), json!("x".repeat(MAX_DOCUMENT_BYTES)));
        assert!(store.save().is_err());
        assert_eq!(std::fs::read(dir.file()).unwrap(), before);
        assert_eq!(std::fs::read(temporary).unwrap(), b"old temporary evidence");
    }
    #[test]
    fn exact_limit_compact_file_remains_saveable_without_dropping_data() {
        let dir = Directory::new();
        let overhead = serde_json::to_vec(&json!({"devices":{},"opaque":""}))
            .unwrap()
            .len();
        let value = json!({"devices":{},"opaque":"x".repeat(MAX_DOCUMENT_BYTES-overhead)});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(bytes.len(), MAX_DOCUMENT_BYTES);
        std::fs::write(dir.file(), bytes).unwrap();
        let store = Store::load(&dir.file()).unwrap();
        store.save().unwrap();
        let saved = std::fs::read(dir.file()).unwrap();
        assert_eq!(saved.len(), MAX_DOCUMENT_BYTES);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&saved).unwrap(),
            value
        );
    }
    #[test]
    fn oversize_and_structural_budgets_refuse_without_touching_input() {
        let dir = Directory::new();
        for value in [
            json!({"devices":{},"opaque":"x".repeat(MAX_DOCUMENT_BYTES)}),
            json!({"devices":{},"opaque":vec![0;32769]}),
        ] {
            let before = serde_json::to_vec(&value).unwrap();
            std::fs::write(dir.file(), &before).unwrap();
            assert!(Store::load(&dir.file()).is_err());
            assert_eq!(std::fs::read(dir.file()).unwrap(), before);
        }
    }
}
