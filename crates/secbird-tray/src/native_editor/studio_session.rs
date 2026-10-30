//! Local editor bookmark only. Never output policy, input, or device I/O.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    os::windows::ffi::OsStrExt,
    path::{Component, Prefix},
};
const MAX_BYTES: usize = 256 * 1024;
#[derive(Serialize, Deserialize)]
pub(crate) struct Selection {
    version: u32,
    path: PathBuf,
    profile_id: Option<String>,
    #[serde(default, flatten)]
    extra: BTreeMap<String, Value>,
}
pub(crate) struct Restored {
    pub path: PathBuf,
    pub archive: Arc<Archive>,
    pub index: usize,
}
impl Selection {
    fn validate(&self) -> Result<(), String> {
        let ordinary = matches!(self.path.components().next(),Some(Component::Prefix(prefix)) if matches!(prefix.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)|Prefix::UNC(..)|Prefix::VerbatimUNC(..)));
        if !ordinary
            || self.path.as_os_str().encode_wide().any(|c| c == 0)
            || self.version != 1
            || !self.path.is_absolute()
            || self.path.as_os_str().encode_wide().count() > 32767
            || self.profile_id.as_ref().is_some_and(|id| id.is_empty() || id.len() > 512 || id.chars().any(char::is_control))
        {
            return Err("Studio session is invalid or unsupported; original retained".into());
        }
        Ok(())
    }
    pub fn from_archive(path: PathBuf, archive: &Archive, index: usize) -> Result<Self, String> {
        let profile_id = if archive.profile_count()==0 {None} else {
            Some(archive.profile_header(index).map_err(|e| e.to_string())?.id
                .ok_or("Composition has no stable profile identity")?)
        };
        let value = Self {
            version: 1,
            path,
            profile_id,
            extra: BTreeMap::new(),
        };
        value.validate()?;
        value.index(archive)?;
        Ok(value)
    }
    fn index(&self, archive: &Archive) -> Result<usize, String> {
        let Some(profile_id)=&self.profile_id else {
            return if archive.profile_count()==0 {Ok(0)} else {Err("Remembered empty library now contains compositions; open it to select one".into())};
        };
        let mut matches = vec![];
        for index in 0..archive.profile_count() {
            if archive
                .profile_header(index)
                .map_err(|e| e.to_string())?
                .id
                .as_deref()
                == Some(profile_id.as_str())
            {
                matches.push(index);
            }
        }
        match matches.as_slice() {
            [index] => Ok(*index),
            _ => Err(
                "Remembered composition is missing or ambiguous; open a file to select it".into(),
            ),
        }
    }
}
fn load(path: &Path) -> Result<Option<Selection>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = vec![];
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Studio session exceeds256KiB; original retained".into());
    }
    let value: Selection =
        serde_json::from_slice(&bytes).map_err(|e| format!("Cannot read Studio session: {e}"))?;
    value.validate()?;
    Ok(Some(value))
}
pub(super) fn save(path: &Path, mut selection: Selection, read_only: bool) -> Result<(), String> {
    if read_only {
        return Err("Read-only mode refuses Studio session changes".into());
    }
    selection.validate()?;
    if let Some(previous) = load(path)? {
        selection.extra = previous.extra;
    }
    let bytes = serde_json::to_vec_pretty(&selection).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Studio session exceeds256KiB; original retained".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    profile_files::atomic_write(path, &bytes)
}
pub(super) fn restore(path: &Path) -> Result<Option<Restored>, String> {
    let Some(selection) = load(path)? else {
        return Ok(None);
    };
    let file =
        File::open(&selection.path).map_err(|e| format!("Cannot reopen Studio file: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Remembered Studio path is not a regular file".into());
    }
    let archive = Arc::new(Archive::read(file).map_err(|e| e.to_string())?);
    let index = selection.index(&archive)?;
    Ok(Some(Restored {
        path: selection.path,
        archive,
        index,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "studio-bookmark-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    fn bytes(ids: &[&str]) -> Vec<u8> {
        serde_json::to_vec(&json!({"version":1,"profiles":ids.iter().map(|id|json!({"id":id,"title":id,"devices":[],"layers":[]})).collect::<Vec<_>>()})).unwrap()
    }
    #[test]
    fn empty_library_reopens_without_a_stale_composition_identity() {
        let folder=folder();let path=folder.join("session.json");let source=folder.join("empty.ChromaEffects");
        let original=bytes(&[]);std::fs::write(&source,&original).unwrap();
        let archive=Archive::parse(&original).unwrap();
        save(&path,Selection::from_archive(source.clone(),&archive,0).unwrap(),false).unwrap();
        let restored=restore(&path).unwrap().unwrap();assert_eq!(restored.archive.profile_count(),0);
        assert_eq!(std::fs::read(&source).unwrap(),original);
        std::fs::write(&source,bytes(&["new"])).unwrap();assert!(restore(&path).is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn restart_follows_identity_across_reorder_without_changing_composition() {
        let folder = folder();
        let path = folder.join("session.json");
        let source = folder.join("test.ChromaEffects");
        let original = bytes(&["first", "second"]);
        std::fs::write(&source, &original).unwrap();
        let archive = Archive::parse(&original).unwrap();
        save(
            &path,
            Selection::from_archive(source.clone(), &archive, 1).unwrap(),
            false,
        )
        .unwrap();
        let restored = restore(&path).unwrap().unwrap();
        assert_eq!(restored.index, 1);
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let reordered = bytes(&["second", "first"]);
        std::fs::write(&source, &reordered).unwrap();
        assert_eq!(restore(&path).unwrap().unwrap().index, 0);
        assert_eq!(std::fs::read(&source).unwrap(), reordered);
        std::fs::write(&source, bytes(&["first"])).unwrap();
        assert!(restore(&path).is_err());
        std::fs::write(&source, bytes(&["second", "second"])).unwrap();
        assert!(restore(&path).is_err());
    }
    #[test]
    fn read_only_unsupported_corrupt_and_excessive_records_are_retained() {
        let folder = folder();
        let path = folder.join("session.json");
        let source = folder.join("test.ChromaEffects");
        let archive = Archive::parse(&bytes(&["one"])).unwrap();
        assert!(save(
            &path,
            Selection::from_archive(source.clone(), &archive, 0).unwrap(),
            true
        )
        .is_err());
        assert!(!path.exists());
        assert!(restore(&path).unwrap().is_none());
        for bytes in [
            b"{broken".to_vec(),
            serde_json::to_vec(&json!({"version":2,"path":source,"profile_id":"one"})).unwrap(),
            vec![b' '; MAX_BYTES + 1],
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(restore(&path).is_err());
            assert!(save(
                &path,
                Selection::from_archive(source.clone(), &archive, 0).unwrap(),
                false
            )
            .is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
    #[test]
    fn unknown_fields_survive_and_failed_replace_preserves_previous_reference() {
        let folder = folder();
        let path = folder.join("session.json");
        let source = folder.join("test.ChromaEffects");
        let archive = Archive::parse(&bytes(&["one"])).unwrap();
        let mut selection = Selection::from_archive(source.clone(), &archive, 0).unwrap();
        selection
            .extra
            .insert("future".into(), json!({"keep":true}));
        save(&path, selection, false).unwrap();
        // A new selection keeps opaque native bookmark metadata.
        let original = std::fs::read(&path).unwrap();
        save(
            &path,
            Selection::from_archive(source.clone(), &archive, 0).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let collision = folder.join("directory");
        std::fs::create_dir(&collision).unwrap();
        assert!(save(
            &collision,
            Selection::from_archive(source, &archive, 0).unwrap(),
            false
        )
        .is_err());
        assert!(collision.is_dir());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(
            Selection::from_archive(PathBuf::from("relative.ChromaEffects"), &archive, 0).is_err()
        );
        for path in [r"\\.\pipe\source", r"\\?\GLOBALROOT\Device\source"] {
            assert!(Selection::from_archive(PathBuf::from(path), &archive, 0).is_err());
        }
    }
}
