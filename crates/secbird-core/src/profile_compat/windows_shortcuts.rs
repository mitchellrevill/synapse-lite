//! Device assignment authoring only. No host input or device delivery.
use crate::{profile::Profile, Error, Result};
use serde_json::{json, Value};

/// Shipped 605/196 standard Windows list, in source order. Dial-only additions excluded.
pub const CHOICES: &[(&str, &str)] = &[
    ("Calculator", "Launch Calculator"),
    ("MSPaint", "Launch MSPaint"),
    ("Notepad", "Launch Notepad"),
    ("Snipping_Tool", "Launch Snipping Tool"),
    ("LaunchTaskManager", "Launch Task Manager"),
    ("MSCopilot", "Launch Windows Copilot"),
    ("User_Directory", "Open User Directory"),
    ("PowerUserMenu", "Open System Utility Settings Menu"),
    ("ShowDesktop", "Show Desktop"),
    ("CycleApps", "Cycle Apps"),
    ("SwitchApps", "Switch Apps"),
    ("CloseApp", "Close App"),
    ("Cut", "Cut"),
    ("Copy", "Copy"),
    ("Paste", "Paste"),
    ("Mail", "Mail"),
    ("ThisPC", "This PC"),
    ("Refresh", "Refresh"),
    ("DisplayBrightnessUp", "Display Brightness Up"),
    ("DisplayBrightnessDown", "Display Brightness Down"),
    ("File_Explorer", "Open File Explorer"),
    ("LockComputer", "Lock Computer"),
    ("WindowsZoomIn", "Windows Zoom In"),
    ("WindowsZoomOut", "Windows Zoom Out"),
    ("OfficeZoomIn", "Office Zoom In"),
    ("OfficeZoomOut", "Office Zoom Out"),
];

pub fn validate_input(pid: u16, input: &str, kind: &str) -> Result<()> {
    let supported = match pid {
        605 => kind == "KeyInput" && super::mappings::key_usage(input).is_some(),
        196 | 197 => matches!(
            (input, kind),
            (
                "RightClick" | "ScrollButton" | "Button4" | "Button5",
                "MouseInput"
            ) | ("DKM_SB_03", "DKMInput")
        ),
        _ => false,
    };
    if supported {
        Ok(())
    } else {
        Err(Error::Unsupported(
            "Windows shortcuts are unavailable for this input".into(),
        ))
    }
}

/// Changes a clone; errors leave the caller's profile untouched. Stale-profile
/// comparison and atomic store persistence belong to the caller.
pub fn edit(
    profile: &Profile,
    pid: u16,
    input: &str,
    kind: &str,
    hyper: bool,
    choice: &str,
) -> Result<Profile> {
    validate_input(pid, input, kind)?;
    if !CHOICES.iter().any(|entry| entry.0 == choice) {
        return Err(Error::InvalidArg("Unknown Windows shortcut".into()));
    }
    let mut next = profile.clone();
    let mappings = next
        .extra
        .get_mut("_synapseLiteSynapse4")
        .and_then(|source| source.get_mut("decoded"))
        .and_then(|decoded| decoded.get_mut("mappings"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            Error::InvalidArg(
                "Source mappings unavailable; import or create a supported profile first".into(),
            )
        })?;
    // A malformed layer marker cannot safely be treated as Standard.
    if mappings
        .iter()
        .any(|r| r["inputID"] == input && r.get("isHyperShift").is_some_and(|v| !v.is_boolean()))
    {
        return Err(Error::InvalidArg(
            "Ambiguous source assignment layer".into(),
        ));
    }
    let indices: Vec<_> = mappings
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r["inputID"] == input && r["isHyperShift"].as_bool().unwrap_or(false) == hyper
        })
        .map(|(i, _)| i)
        .collect();
    if indices.len() > 1 {
        return Err(Error::InvalidArg(
            "Duplicate source assignments; original records retained".into(),
        ));
    }
    let mut record = indices
        .first()
        .map(|i| mappings[*i].clone())
        .unwrap_or_else(|| json!({"inputID":input,"inputType":kind,"isHyperShift":hyper}));
    if record["inputType"] != kind || record.get("mapping").is_some() {
        return Err(Error::Unsupported(
            "Incompatible or analog source assignment".into(),
        ));
    }
    // Remove the old translated native action, so export/runtime cannot replay
    // the previous keyboard/mouse binding over the newly authored foreign group.
    let native_key = super::mappings::setting_key(&record, pid);
    record["isDefault"] = json!(false);
    record["outputType"] = json!("win8ShortcutsGroup");
    if record.get("win8ShortcutsGroup").is_none() {
        record["win8ShortcutsGroup"] = json!({});
    }
    let group = record["win8ShortcutsGroup"]
        .as_object_mut()
        .ok_or_else(|| Error::InvalidArg("Malformed Windows shortcut group".into()))?;
    group.insert("windowsShortcutAssignment".into(), json!(choice));
    if let Some(i) = indices.first() {
        mappings[*i] = record;
    } else {
        mappings.push(record);
    }
    if let Some(key) = native_key {
        next.buttons.remove(&key);
    }
    Ok(next)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Profile {
        let mut p = super::super::new_native_profile(605, "Fixture").unwrap();
        p.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"opaque":[4]},"win8ShortcutsGroup":{"future":7},"foreign":true},
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"future","foreign":[1,2]},
            {"inputID":"KEY_B","inputType":"KeyInput","outputType":"future","foreign":8}
        ]);
        p.buttons.insert("key-a".into(), "key:c".into());
        p.extra.insert("opaque".into(), json!({"retained":true}));
        p
    }
    #[test]
    fn windows_choices_are_unique_exact_standard_source_ids() {
        let source: Value=serde_json::from_str(include_str!("windows_shortcuts_source.json")).unwrap();
        let expected: Vec<(&str,&str)>=source["choices"].as_array().unwrap().iter().map(|v|(v["id"].as_str().unwrap(),v["label"].as_str().unwrap())).collect();
        assert_eq!(CHOICES,expected.as_slice());
        assert_eq!(CHOICES.len(), 26);
        assert_eq!(CHOICES.first().unwrap().0, "Calculator");
        assert_eq!(CHOICES.last().unwrap().0, "OfficeZoomOut");
        let ids: std::collections::BTreeSet<_> = CHOICES.iter().map(|v| v.0).collect();
        assert_eq!(ids.len(), CHOICES.len());
        for id in ["SwitchAppLeft", "Screenshot", "XboxGameBar"] {
            assert!(!ids.contains(id));
        }
    }
    #[test]
    fn windows_edit_preserves_other_layer_inputs_and_opaque_metadata_and_removes_old_projection() {
        let before = fixture();
        let next = edit(&before, 605, "KEY_A", "KeyInput", false, "LockComputer").unwrap();
        let mappings = &next.extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
        let old = &before.extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
        assert_eq!(mappings[1], old[1]);
        assert_eq!(mappings[2], old[2]);
        assert_eq!(
            mappings[0]["win8ShortcutsGroup"],
            json!({"future":7,"windowsShortcutAssignment":"LockComputer"})
        );
        assert_eq!(mappings[0]["keyboardGroup"], old[0]["keyboardGroup"]);
        assert_eq!(next.extra["opaque"], before.extra["opaque"]);
        assert!(!next.buttons.contains_key("key-a"));
        assert!(before.buttons.contains_key("key-a"));
        let reopened: Profile =
            serde_json::from_slice(&serde_json::to_vec(&next).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(reopened).unwrap(),
            serde_json::to_value(next).unwrap()
        );
    }
    #[test]
    fn windows_restrictions_and_ambiguous_records_refuse_without_changes() {
        let before = fixture();
        let bytes = serde_json::to_vec(&before).unwrap();
        for (pid, id, kind, choice) in [
            (196, "LeftClick", "MouseInput", "Copy"),
            (196, "ScrollUp", "MouseInput", "Copy"),
            (605, "DIAL_CLICK", "DKMInput", "Copy"),
            (605, "KEY_A", "KeyInput", "SwitchAppLeft"),
            (999, "KEY_A", "KeyInput", "Copy"),
        ] {
            assert!(edit(&before, pid, id, kind, false, choice).is_err());
        }
        assert_eq!(serde_json::to_vec(&before).unwrap(), bytes);
        let mut duplicate = fixture();
        let records = duplicate.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]
            ["mappings"]
            .as_array_mut()
            .unwrap();
        records.push(records[0].clone());
        assert!(edit(&duplicate, 605, "KEY_A", "KeyInput", false, "Copy").is_err());
        let mut malformed = fixture();
        malformed.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"][0]
            ["isHyperShift"] = json!("false");
        assert!(edit(&malformed, 605, "KEY_A", "KeyInput", false, "Copy").is_err());
    }
    #[test]
    fn windows_every_choice_survives_real_store_save_reopen_and_synapse_export() {
        let dir = std::env::temp_dir().join(format!("synapse-windows-core-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut store = crate::Store::load(&dir.join("profiles.json")).unwrap();
        let profiles = store.profiles_for("025D:FIXTURE", 605);
        profiles.profiles.clear();
        let mut compatible=fixture();
        // Arbitrary native-only extensions intentionally refuse Synapse export.
        // Their local preservation is covered above; this vector exercises the
        // stock-compatible source metadata path without weakening that refusal.
        compatible.extra.remove("opaque");
        profiles.profiles.insert("Fixture".into(), compatible);
        profiles.active = "Fixture".into();
        for (id, _) in CHOICES {
            let profiles = store.devices.get_mut("025D:FIXTURE").unwrap();
            profiles.profiles.insert(
                "Fixture".into(),
                edit(profiles.active(), 605, "KEY_A", "KeyInput", false, id).unwrap(),
            );
            store.save().unwrap();
            store = crate::Store::load(&dir.join("profiles.json")).unwrap();
            let export = super::super::export_synapse4(&store, "025D:FIXTURE").unwrap();
            let imported = super::super::import_document(&export.bytes).unwrap();
            let profile = imported
                .store
                .devices
                .values()
                .next()
                .unwrap()
                .profiles
                .values()
                .next()
                .unwrap();
            assert_eq!(
                profile.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][0]
                    ["win8ShortcutsGroup"]["windowsShortcutAssignment"],
                *id
            );
        }
        std::fs::remove_file(dir.join("profiles.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
