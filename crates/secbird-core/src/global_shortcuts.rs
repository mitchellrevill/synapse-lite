//! Bounded source Global Shortcuts envelope and explicit local host intent.
//! No input recording, vendor mapping publication, or device access.
use crate::{Error, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SHORTCUTS: usize = 256;
pub const HOST: &str = "_synapseLiteHost";
pub const CATEGORIES: &[(&str, &str)] = &[
    ("SWITCH_DEVICE_PROFILE", "interDeviceGroup"),
    ("SWITCH_DEVICE_SENSITIVITY", "interDeviceGroup"),
    ("SWITCH_LIGHTING", "lightPacGroup"),
    ("MACRO", "macroGroup"),
    ("TEXT_FUNCTION", "textBlockGroup"),
    ("LAUNCH_PROGRAM", "launchGroup"),
    ("MULTIMEDIA", "multimediaGroup"),
    ("WINDOWS_SHORTCUT", "win8ShortcutsGroup"),
];
pub const MEDIA: &[(&str, &str, u16)] = &[
    ("VolumeUp", "Volume up", 10),
    ("VolumeDown", "Volume down", 9),
    ("MuteVolume", "Mute", 8),
    ("Play", "Play / pause", 14),
    ("NextTrack", "Next track", 11),
    ("PrevTrack", "Previous track", 12),
];
fn invalid(s: impl std::fmt::Display) -> Error {
    Error::InvalidArg(format!("Global Shortcuts: {s}"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Library {
    pub envelope: Value,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            envelope: json!({"mappings":[]}),
        }
    }
}
impl Library {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("document exceeds 2 MiB; original retained"));
        }
        crate::profile_compat::check_json_complexity(bytes)?;
        let library = Self {
            envelope: serde_json::from_slice(bytes).map_err(invalid)?,
        };
        library.validate()?;
        Ok(library)
    }
    pub fn records(&self) -> &[Value] {
        self.envelope["mappings"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub fn record(&self, id: &str) -> Option<&Value> {
        self.records()
            .iter()
            .find(|r| r["guid"].as_str() == Some(id))
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec(&self.envelope).map_err(invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("document exceeds 2 MiB; original retained"));
        }
        crate::profile_compat::check_json_complexity(&bytes)?;
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        let rows = self
            .envelope
            .get("mappings")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("expected source object with mappings array"))?;
        if rows.len() > MAX_SHORTCUTS {
            return Err(invalid("more than 256 shortcuts"));
        }
        let mut ids = BTreeSet::new();
        for row in rows {
            if !row.is_object() {
                return Err(invalid("mapping must be an object"));
            }
            let id = row["guid"]
                .as_str()
                .ok_or_else(|| invalid("missing GUID"))?;
            if id.is_empty()
                || id.len() > 128
                || id.chars().any(char::is_control)
                || !ids.insert(id)
            {
                return Err(invalid("invalid or duplicate GUID; original retained"));
            }
            for key in ["inputID", "inputType", "outputType"] {
                if row.get(key).is_some_and(|v| {
                    !v.is_null()
                        && v.as_str()
                            .is_none_or(|s| s.len() > 256 || s.chars().any(char::is_control))
                }) {
                    return Err(invalid(format!("invalid {key}")));
                }
            }
            if let Some(v) = row.get("inputModifiers").filter(|v| !v.is_null()) {
                let mods = v
                    .as_array()
                    .ok_or_else(|| invalid("modifiers must be an array"))?;
                if mods.len() > 8
                    || mods
                        .iter()
                        .any(|v| v.as_str().is_none_or(|s| s.len() > 128))
                {
                    return Err(invalid("invalid modifier list"));
                }
            }
            if row
                .get("isHyperShift")
                .is_some_and(|v| !v.is_null() && !v.is_boolean())
            {
                return Err(invalid("invalid Hypershift state"));
            }
            // Unknown versions survive interchange, but can never arm a trigger.
            if let Some(host) = row.get(HOST) {
                if !host.is_object() {
                    return Err(invalid("invalid local host preferences"));
                }
            }
        }
        Ok(())
    }
    /// Compare the complete base, including foreign fields; stale editors cannot overwrite it.
    pub fn replace(&self, expected: &Self, next: &Self) -> Result<Self> {
        if self != expected {
            return Err(invalid(
                "shortcuts changed elsewhere; export your draft, then reload",
            ));
        }
        next.encode()?;
        // Imported legacy conflicts may be retained. An edit cannot introduce an
        // additional enabled ambiguous Windows chord.
        for row in next.records().iter().filter(|r| enabled(r)) {
            if let Ok(chord) = host_chord(row) {
                if next.records().iter().any(|other| {
                    other["guid"] != row["guid"]
                        && enabled(other)
                        && host_chord(other).ok() == Some(chord)
                }) {
                    return Err(invalid(
                        "enabled Windows chords conflict; disable or change one trigger",
                    ));
                }
            }
        }
        Ok(next.clone())
    }
    pub fn push(&mut self, record: Value) -> Result<()> {
        let mut next = self.clone();
        next.envelope["mappings"]
            .as_array_mut()
            .unwrap()
            .push(record);
        next.encode()?;
        *self = next;
        Ok(())
    }
    pub fn duplicate(&mut self, id: &str, guid: &str) -> Result<()> {
        let mut copy = self
            .record(id)
            .cloned()
            .ok_or_else(|| invalid("shortcut no longer exists"))?;
        copy["guid"] = json!(guid);
        for key in [
            "inputID",
            "inputType",
            "keyInput",
            "mouseInput",
            "keyValue",
            "hasWarning",
        ] {
            copy.as_object_mut().unwrap().remove(key);
        }
        copy["inputModifiers"] = json!([]);
        if copy.get(HOST).is_none() {
            copy[HOST] = json!({"version":1,"enabled":false});
        } else {
            copy[HOST]["enabled"] = json!(false);
        }
        self.push(copy)
    }
    pub fn remove(&mut self, id: &str) {
        self.envelope["mappings"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["guid"] != id);
    }
    /// Import never arms input. Every raw source field and appEngine/hash survives.
    pub fn import(bytes: &[u8]) -> Result<Self> {
        let mut library = Self::parse(bytes)?;
        for row in library.envelope["mappings"].as_array_mut().unwrap() {
            if row.get(HOST).is_none() {
                row[HOST] = json!({"version":1,"enabled":false});
            } else {
                row[HOST]["enabled"] = json!(false);
            }
        }
        library.encode()?;
        Ok(library)
    }
}
pub fn create(guid: &str) -> Value {
    json!({"guid":guid,"inputType":"KeyInput","inputID":"KEY_A","inputModifiers":["KEY_LEFT_CTRL","KEY_LEFT_SHIFT"],"isHyperShift":false,"outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":"Play"},HOST:{"version":1,"scope":"windows-chord","enabled":false}})
}
pub fn enabled(row: &Value) -> bool {
    row[HOST]["version"] == 1 && row[HOST]["enabled"] == true
}
pub fn set_enabled(row: &mut Value, value: bool) -> Result<()> {
    if row
        .get(HOST)
        .is_some_and(|h| h.get("version").is_some_and(|v| *v != 1))
    {
        return Err(invalid("unknown local host version retained"));
    }
    if row.get(HOST).is_none() {
        row[HOST] = json!({"version":1});
    }
    row[HOST]["version"] = json!(1);
    row[HOST]["enabled"] = json!(value);
    Ok(())
}
/// Source comparison is order independent and preserves left/right and context.
pub fn source_conflict(a: &Value, b: &Value) -> bool {
    let key = a["inputID"].as_str().filter(|s| !s.is_empty());
    if key.is_none()
        || key != b["inputID"].as_str()
        || a["isHyperShift"].as_bool().unwrap_or(false)
            != b["isHyperShift"].as_bool().unwrap_or(false)
    {
        return false;
    }
    let mods = |r: &Value| {
        r["inputModifiers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    };
    mods(a) == mods(b)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chord {
    pub modifiers: u32,
    pub key: u32,
}
pub fn key_vk(id: &str) -> Option<u32> {
    let key = id.strip_prefix("KEY_")?;
    if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() {
        return Some(key.as_bytes()[0].to_ascii_uppercase() as u32);
    }
    if let Some(f) = key
        .strip_prefix('F')
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| (1..=24).contains(v) && *v != 12)
    {
        return Some(0x70 + f - 1);
    }
    Some(match key {
        "SPACEBAR" | "SPACE" => 32,
        "ENTER" => 13,
        "TAB" => 9,
        "ESC" | "ESCAPE" => 27,
        "UP_ARROW" => 38,
        "DOWN_ARROW" => 40,
        "LEFT_ARROW" => 37,
        "RIGHT_ARROW" => 39,
        "HOME" => 36,
        "END" => 35,
        "PAGE_UP" => 33,
        "PAGE_DOWN" => 34,
        "INSERT" => 45,
        "DELETE" => 46,
        _ => return None,
    })
}
pub fn host_chord(row: &Value) -> std::result::Result<Chord, String> {
    if row[HOST]["version"] != 1 || row[HOST]["scope"] != "windows-chord" {
        return Err("Choose Windows chord scope explicitly; source triggers are retained without registration".into());
    }
    if row["isHyperShift"] == true {
        return Err("Hypershift requires proved device routing and held-key recovery".into());
    }
    if row["inputType"] != "KeyInput" {
        return Err("Mouse/device triggers require proved routing; no hook installed".into());
    }
    let key = row["inputID"]
        .as_str()
        .and_then(key_vk)
        .ok_or("Choose a supported key (F12 is reserved by Windows)")?;
    let mut modifiers = 0;
    for m in row["inputModifiers"].as_array().into_iter().flatten() {
        modifiers |= match m.as_str() {
            Some("KEY_LEFT_CTRL" | "KEY_RIGHT_CTRL") => 2,
            Some("KEY_LEFT_SHIFT" | "KEY_RIGHT_SHIFT") => 4,
            Some("KEY_LEFT_ALT" | "KEY_RIGHT_ALT") => 1,
            Some("KEY_LEFT_GUI" | "KEY_RIGHT_GUI" | "KEY_WINDOWS") => 8,
            _ => return Err("Unsupported modifier; source identity retained".into()),
        };
    }
    if modifiers & 3 == 0 {
        return Err("Windows chords require Ctrl or Alt to protect ordinary typing".into());
    }
    Ok(Chord { modifiers, key })
}
pub fn media_command(row: &Value) -> std::result::Result<u16, String> {
    if row["outputType"] != "multimediaGroup" {
        return Err(
            "Assignment retained; host execution is currently proved only for multimedia commands"
                .into(),
        );
    }
    if row["multimediaGroup"]["useTurbo"] == true {
        return Err(
            "Turbo multimedia assignment retained; repeated host output is not implemented".into(),
        );
    }
    let assignment = row["multimediaGroup"]["multimediaAssignment"]
        .as_str()
        .ok_or("Choose a multimedia assignment")?;
    MEDIA
        .iter()
        .find(|m| m.0 == assignment)
        .map(|m| m.2)
        .ok_or_else(|| "Unsupported multimedia assignment retained".into())
}
pub fn blocked(row: &Value) -> Option<String> {
    host_chord(row).err().or_else(|| media_command(row).err())
}
pub fn trigger_label(row: &Value) -> String {
    let mut parts: Vec<String> = row["inputModifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|s| s.trim_start_matches("KEY_").replace('_', " "))
        .collect();
    if row["isHyperShift"] == true {
        parts.push("Hypershift".into());
    }
    parts.push(
        row["inputID"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Choose trigger")
            .trim_start_matches("KEY_")
            .replace('_', " "),
    );
    parts.join(" + ")
}
pub fn assignment_label(row: &Value) -> String {
    let group = row["outputType"].as_str().unwrap_or("Unassigned");
    let detail = match group {
        "multimediaGroup" => row[group]["multimediaAssignment"]
            .as_str()
            .map(|id| MEDIA.iter().find(|m| m.0 == id).map_or(id, |m| m.1)),
        "textBlockGroup" => row[group]["text"].as_str(),
        "launchGroup" => row[group]["path"]
            .as_str()
            .or_else(|| row[group]["url"].as_str()),
        "macroGroup" => row[group]["name"]
            .as_str()
            .or_else(|| row[group]["guid"].as_str()),
        "lightPacGroup" => row[group]["Name"]
            .as_str()
            .or_else(|| row[group]["ID"].as_str()),
        _ => None,
    };
    let category = match group {
        "multimediaGroup" => "Multimedia",
        "textBlockGroup" => "Text function",
        "launchGroup" => "Launch program",
        "macroGroup" => "Macro",
        "lightPacGroup" => "Switch lighting",
        "win8ShortcutsGroup" => "Windows shortcut",
        "interDeviceGroup" => "Inter-device assignment",
        _ => "Retained assignment",
    };
    format!(
        "{category}{}",
        detail
            .map(|v| format!(" · {}", v.chars().take(80).collect::<String>()))
            .unwrap_or_default()
    )
}
pub fn load(path: &Path) -> Result<Library> {
    match std::fs::File::open(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Library::default()),
        Err(e) => Err(invalid(e)),
        Ok(file) => {
            let mut bytes = vec![];
            file.take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(invalid)?;
            Library::parse(&bytes)
        }
    }
}
/// Independent app-level file: never rewrites device profiles or recovery records.
pub fn save(path: &Path, expected: &Library, next: &Library) -> Result<()> {
    let bytes = next.encode()?;
    // Cooperating writers must serialize the compare-and-replace operation.
    // A crashed writer leaves a lock for deliberate recovery, never truncation.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(invalid)?;
    }
    let lock = path.with_extension("json.lock");
    let handle = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|e| {
            invalid(format!(
                "{e}; another writer or retained recovery lock blocks saving"
            ))
        })?;
    struct Guard<'a>(&'a Path, std::fs::File);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            let _ = &self.1;
            let _ = std::fs::remove_file(self.0);
        }
    }
    let _guard = Guard(&lock, handle);
    let current = load(path)?;
    current.replace(expected, next)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(invalid)?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| {
            invalid(format!(
                "{e}; inspect retained shortcut temp file before retrying"
            ))
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(invalid)?;
    drop(file);
    std::fs::rename(&temporary, path).map_err(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_interchange_preserves_foreign_envelope_and_disabled_unknown_assignments() {
        let source = json!({"hash":"foreign-hash","appEngine":{"hash":"retained","mappings":[{"foreign":1}]},"future":{"a":[1,2]},"mappings":[{"guid":"unknown","inputType":"MouseInput","inputID":"Button5","inputModifiers":["KEY_RIGHT_CTRL"],"isHyperShift":true,"outputType":"futureGroup","futureGroup":{"raw":[1,2,3]},"isEnabled":true}]});
        let imported = Library::import(&serde_json::to_vec(&source).unwrap()).unwrap();
        let mut roundtrip = imported.envelope.clone();
        roundtrip["mappings"][0]
            .as_object_mut()
            .unwrap()
            .remove(HOST);
        assert_eq!(roundtrip, source);
        assert!(!enabled(&imported.records()[0]));
        assert!(blocked(&imported.records()[0]).is_some());
        assert_eq!(
            Library::parse(&imported.encode().unwrap()).unwrap(),
            imported
        );
        assert!(media_command(&json!({"outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":"MicVolumeUp"}})).is_err());
        assert_eq!(media_command(&create("new")).unwrap(), 14);
    }
    #[test]
    fn duplicate_clears_trigger_keeps_assignment_and_does_not_reuse_guid() {
        let mut library = Library::default();
        let mut row = create("one");
        row["foreign"] = json!({"opaque":true});
        set_enabled(&mut row, true).unwrap();
        library.push(row.clone()).unwrap();
        library.duplicate("one", "two").unwrap();
        let duplicate = &library.records()[1];
        assert_eq!(duplicate["multimediaGroup"], row["multimediaGroup"]);
        assert_eq!(duplicate["foreign"], row["foreign"]);
        assert_eq!(duplicate["guid"], "two");
        assert!(duplicate.get("inputID").is_none());
        assert!(!enabled(duplicate));
        assert!(library.duplicate("one", "two").is_err());
        assert_eq!(library.records().len(), 2);
    }
    #[test]
    fn conflict_compares_context_sets_and_windows_aliases() {
        let mut a = create("a");
        let mut b = create("b");
        b["inputModifiers"] = json!(["KEY_LEFT_SHIFT", "KEY_LEFT_CTRL"]);
        assert!(source_conflict(&a, &b));
        b["isHyperShift"] = json!(true);
        assert!(!source_conflict(&a, &b));
        assert!(host_chord(&b).is_err());
        b["isHyperShift"] = json!(false);
        b["inputModifiers"] = json!(["KEY_RIGHT_SHIFT", "KEY_RIGHT_CTRL"]);
        assert!(!source_conflict(&a, &b));
        assert_eq!(host_chord(&a), host_chord(&b));
        set_enabled(&mut a, true).unwrap();
        set_enabled(&mut b, true).unwrap();
        let mut next = Library::default();
        next.push(a).unwrap();
        next.push(b).unwrap();
        assert!(Library::default()
            .replace(&Library::default(), &next)
            .is_err());
        set_enabled(&mut next.envelope["mappings"][1], false).unwrap();
        assert!(Library::default()
            .replace(&Library::default(), &next)
            .is_ok());
        for key in ["KEY_F12", "KEY_FN", "KEY_LEFT_CTRL"] {
            next.envelope["mappings"][0]["inputID"] = json!(key);
            assert!(host_chord(&next.records()[0]).is_err());
        }
        next.envelope["mappings"][0]["inputID"] = json!("KEY_A");
        next.envelope["mappings"][0]["inputModifiers"] = json!([]);
        assert!(host_chord(&next.records()[0]).is_err());
    }
    #[test]
    fn malformed_versions_counts_complexity_and_sizes_are_refused_without_mutation() {
        for data in [
            b"null".as_slice(),
            b"[]",
            b"{\"mappings\":{}}",
            b"{\"mappings\":[{\"guid\":\"x\",\"inputModifiers\":1}]}",
            b"{\"mappings\":[{\"guid\":\"x\"},{\"guid\":\"x\"}]}",
        ] {
            assert!(Library::parse(data).is_err());
        }
        assert!(Library::parse(&vec![b' '; MAX_BYTES + 1]).is_err());
        let mut library = Library::default();
        for i in 0..MAX_SHORTCUTS {
            library.push(create(&i.to_string())).unwrap();
        }
        let before = library.clone();
        assert!(library.push(create("overflow")).is_err());
        assert_eq!(library, before);
        let mut row = create("future");
        row[HOST]["version"] = json!(2);
        row[HOST]["enabled"] = json!(true);
        assert!(!enabled(&row));
        assert!(host_chord(&row).is_err());
        assert!(set_enabled(&mut row, true).is_err());
    }
    #[test]
    fn atomic_save_refuses_stale_edits_corrupt_files_and_retained_recovery_temp() {
        let folder = std::env::temp_dir().join(format!(
            "shortcuts-core-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("shortcuts.json");
        let base = Library::default();
        let mut next = base.clone();
        next.push(create("one")).unwrap();
        save(&path, &base, &next).unwrap();
        let disk = std::fs::read(&path).unwrap();
        assert_eq!(load(&path).unwrap(), next);
        assert!(save(&path, &base, &base).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), disk);
        std::fs::write(path.with_extension("json.tmp"), b"retained draft").unwrap();
        assert!(save(&path, &next, &base).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), disk);
        assert_eq!(
            std::fs::read(path.with_extension("json.tmp")).unwrap(),
            b"retained draft"
        );
        std::fs::remove_file(path.with_extension("json.tmp")).unwrap();
        std::fs::write(path.with_extension("json.lock"), b"crashed writer").unwrap();
        assert!(save(&path, &next, &base).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), disk);
        std::fs::remove_file(path.with_extension("json.lock")).unwrap();
        std::fs::write(&path, b"{ corrupt").unwrap();
        assert!(save(&path, &next, &base).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{ corrupt");
        std::fs::remove_dir_all(folder).unwrap();
    }
}
