//! Source multimedia desired-profile authoring only. No input or device output.
use super::*;
use serde::{Deserialize, Serialize};

// Recovered 605/196 module29267 Le and MapMedia getMappingData.
pub const CHOICES: [(&str, &str); 10] = [
    ("VolumeDown", "Volume Down"),
    ("VolumeUp", "Volume Up"),
    ("MuteVolume", "Mute Volume"),
    ("MicVolumeUp", "Microphone Volume Up"),
    ("MicVolumeDown", "Microphone Volume Down"),
    ("MuteMic", "Mute Microphone"),
    ("MuteAll", "Mute All"),
    ("Play", "Play / Pause"),
    ("PrevTrack", "Previous Track"),
    ("NextTrack", "Next Track"),
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub input_id: String,
    pub input_type: String,
    pub hyper: bool,
    pub command: String,
    pub expected: Value,
}
/// Validate a complete candidate before any mutation. Source cache fields are
/// retained, never guessed/recompiled; their presence blocks editing.
pub fn candidate(profile: &Profile, pid: u16, edit: &Edit) -> Result<Profile> {
    if !CHOICES.iter().any(|(id, _)| *id == edit.command) {
        return Err(invalid("Choose a supported multimedia command"));
    }
    if serde_json::to_value(profile).map_err(|e| invalid(e.to_string()))? != edit.expected {
        return Err(invalid("Saved profile changed; reopen the assignment"));
    }
    let mut row =
        json!({"inputID":edit.input_id,"inputType":edit.input_type,"isHyperShift":edit.hyper});
    let key = mappings::setting_key(&row, pid)
        .ok_or_else(|| invalid("Input identity is not verified for local assignment editing"))?;
    let expected_type = if pid == 605 {
        "KeyInput"
    } else if edit.input_id == "DKM_SB_03" {
        "DKMInput"
    } else {
        "MouseInput"
    };
    if !matches!(pid, 605 | 196 | 197)
        || edit.input_type != expected_type
        || (pid != 605 && edit.input_id == "LeftClick")
    {
        return Err(invalid(
            "This input is protected or its identity is not verified",
        ));
    }
    let source = profile
        .extra
        .get(SOURCE)
        .ok_or_else(|| invalid("Source profile context unavailable"))?;
    let decoded = source
        .get("decoded")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Source profile context is malformed"))?;
    if decoded.contains_key("mapping") || decoded.contains_key("nativeMapping") {
        return Err(invalid("Native mapping cache cannot be rewritten safely"));
    }
    let records = decoded
        .get("mappings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Source mappings are unavailable"))?;
    let mut found = None;
    for (i, record) in records.iter().enumerate() {
        if record["inputID"] != edit.input_id {
            continue;
        }
        let hyper = match record.get("isHyperShift") {
            None => false,
            Some(Value::Bool(v)) => *v,
            _ => return Err(invalid("Ambiguous assignment layer")),
        };
        if hyper != edit.hyper {
            continue;
        }
        if found.replace(i).is_some() {
            return Err(invalid("Duplicate assignment records; original retained"));
        }
        if record["inputType"] != edit.input_type
            || record.get("mapping").is_some()
            || record["isInterDevice"] == true
        {
            return Err(invalid(
                "Existing assignment has an unsupported identity or native cache",
            ));
        }
        row = record.clone();
    }
    if !row["multimediaGroup"].is_null() && !row["multimediaGroup"].is_object() {
        return Err(invalid("Existing multimedia data is malformed"));
    }
    if row["multimediaGroup"].is_null() {
        row["multimediaGroup"] = json!({});
    }
    row["outputType"] = json!("multimediaGroup");
    row["isDefault"] = json!(false);
    row["multimediaGroup"]["multimediaAssignment"] = json!(edit.command);
    // The source omits useTurbo for other choices. Clear a previous true
    // without dropping unknown group fields.
    row["multimediaGroup"]
        .as_object_mut()
        .unwrap()
        .remove("useTurbo");
    if matches!(edit.command.as_str(), "VolumeDown" | "VolumeUp") {
        row["multimediaGroup"]["useTurbo"] = json!(true);
    }
    let mut next = profile.clone();
    let records = next.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"]
        .as_array_mut()
        .unwrap();
    if let Some(i) = found {
        records[i] = row;
    } else {
        records.push(row);
    }
    // Remove the old native projection of exactly this input/layer. Multimedia
    // is deliberately never translated into a firmware or host execution action.
    next.buttons.remove(&key);
    Ok(next)
}
/// Store-level ownership and budget gate. Saving is the caller's final step.
pub fn update(store: &mut Store, device: &str, profile: &str, pid: u16, edit: &Edit) -> Result<()> {
    let key = gui::stored_device_key(store, device)
        .ok_or_else(|| invalid("Saved device context unavailable"))?;
    let profiles = store.devices.get(&key).unwrap();
    if profiles.active != profile {
        return Err(invalid("Active profile changed; reopen the assignment"));
    }
    let original = profiles
        .profiles
        .get(profile)
        .ok_or_else(|| invalid("Saved profile unavailable"))?;
    let next = candidate(original, pid, edit)?;
    let mut updated = store.clone();
    updated
        .devices
        .get_mut(&key)
        .unwrap()
        .profiles
        .insert(profile.into(), next);
    validate_store_budget(&updated)?;
    *store = updated;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn edit(profile: &Profile, command: &str, hyper: bool) -> Edit {
        Edit {
            input_id: "KEY_A".into(),
            input_type: "KeyInput".into(),
            hyper,
            command: command.into(),
            expected: serde_json::to_value(profile).unwrap(),
        }
    }
    #[test]
    fn all_ten_choices_encode_source_repeat_and_round_trip_export() {
        for (command, _) in CHOICES {
            let profile = new_native_profile(605, "Fixture").unwrap();
            let next = candidate(&profile, 605, &edit(&profile, command, false)).unwrap();
            let row = &next.extra[SOURCE]["decoded"]["mappings"][0];
            assert_eq!(row["multimediaGroup"]["multimediaAssignment"], command);
            assert_eq!(
                row["multimediaGroup"].get("useTurbo"),
                if command == "VolumeDown" || command == "VolumeUp" {
                    Some(&Value::Bool(true))
                } else {
                    None
                }
            );
            let mut store = Store::default();
            let mut device = crate::profile::DeviceProfiles::default();
            device.active = "Fixture".into();
            device.profiles.clear();
            device.profiles.insert("Fixture".into(), next.clone());
            store.devices.insert("025D:fixture".into(), device);
            let exported = export_synapse4(&store, "025D:fixture").unwrap();
            let reopened = import_document(&exported.bytes).unwrap().store;
            let reopened = &reopened.devices.values().next().unwrap().profiles["Fixture"];
            assert_eq!(reopened.extra[SOURCE]["decoded"]["mappings"][0], *row);
            assert!(next.buttons.is_empty());
        }
    }
    #[test]
    fn edits_preserve_unknown_fields_other_inputs_layers_and_identity() {
        let mut profile = new_native_profile(605, "Fixture").unwrap();
        let records = json!([
          {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"future":[1,2],"multimediaGroup":{"future":"keep","useTurbo":true}},
          {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","opaque":9},
          {"inputID":"KEY_Z","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","opaque":8}]);
        profile.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = records.clone();
        profile.buttons.insert("key-a".into(), "key:b".into());
        profile.buttons.insert("key-z".into(), "key:c".into());
        let before = profile.clone();
        let next = candidate(&profile, 605, &edit(&profile, "MuteMic", false)).unwrap();
        let rows = &next.extra[SOURCE]["decoded"]["mappings"];
        assert_eq!(rows[0]["future"], records[0]["future"]);
        assert_eq!(rows[0]["multimediaGroup"]["future"], "keep");
        assert_eq!(rows[1], records[1]);
        assert_eq!(rows[2], records[2]);
        assert_eq!(
            next.extra[SOURCE]["decoded"]["guid"],
            before.extra[SOURCE]["decoded"]["guid"]
        );
        assert!(!next.buttons.contains_key("key-a"));
        assert_eq!(next.buttons["key-z"], "key:c");
        assert_eq!(profile, before);
    }
    #[test]
    fn invalid_stale_duplicate_cache_and_layer_are_refused_without_mutation() {
        let profile = new_native_profile(605, "Fixture").unwrap();
        assert!(candidate(&profile, 605, &edit(&profile, "Future", false)).is_err());
        let mut stale = edit(&profile, "Play", false);
        stale.expected["future"] = json!(1);
        assert!(candidate(&profile, 605, &stale).is_err());
        for records in [
            json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"mapping":[]}]),
            json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":"false"}]),
            json!([{"inputID":"KEY_A","inputType":"KeyInput"},{"inputID":"KEY_A","inputType":"KeyInput"}]),
        ] {
            let mut p = profile.clone();
            p.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = records;
            let before = p.clone();
            assert!(candidate(&p, 605, &edit(&p, "Play", false)).is_err());
            assert_eq!(p, before);
        }
    }
    #[test]
    fn protected_mouse_and_wrong_input_type_are_refused() {
        let profile = new_native_profile(196, "Fixture").unwrap();
        let mut e = edit(&profile, "Play", false);
        e.input_id = "LeftClick".into();
        e.input_type = "MouseInput".into();
        assert!(candidate(&profile, 196, &e).is_err());
        e.input_id = "RightClick".into();
        assert!(candidate(&profile, 196, &e).is_ok());
        e.input_type = "KeyInput".into();
        assert!(candidate(&profile, 196, &e).is_err());
    }
    #[test]
    fn ownership_refusal_and_native_store_reopen_preserve_opaque_data() {
        let mut store = Store::default();
        let mut d = crate::profile::DeviceProfiles::default();
        d.active = "Fixture".into();
        d.profiles.clear();
        let p = new_native_profile(605, "Fixture").unwrap();
        let e = edit(&p, "Play", true);
        d.profiles.insert("Fixture".into(), p);
        store.devices.insert("025D:fixture".into(), d);
        store.extra.insert("opaque".into(), json!([1, 2]));
        let before = serde_json::to_value(&store).unwrap();
        assert!(update(&mut store, "025D:fixture", "Other", 605, &e).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
        update(&mut store, "025D:fixture", "Fixture", 605, &e).unwrap();
        let dir = std::env::temp_dir().join(format!("multimedia-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profiles.json");
        let bytes = serde_json::to_vec(&store).unwrap();
        std::fs::write(&path, bytes).unwrap();
        let reopened = Store::load(&path).unwrap();
        assert_eq!(reopened.extra["opaque"], json!([1, 2]));
        assert_eq!(
            reopened.devices["025D:fixture"].profiles["Fixture"].extra[SOURCE]["decoded"]
                ["mappings"][0]["isHyperShift"],
            true
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
