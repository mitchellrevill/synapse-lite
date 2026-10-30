use super::*;

#[test]
fn navigation_groups_match_independent_source_converter_cases() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/profile-navigation-source.json"
    ))
    .unwrap();
    assert_eq!(reference["cases"].as_array().unwrap().len(), 10);
    for case in reference["cases"].as_array().unwrap() {
        let specific = case["action"] == "Specific";
        let edit = Edit {
            input_id: "KEY_A".into(),
            input_type: "KeyInput".into(),
            hyper: false,
            action: case["action"].as_str().unwrap().into(),
            guid: specific.then(|| "target-id".into()),
            lightpac_id: (specific && case["linked"] == true).then(|| "effect-id".into()),
        };
        assert_eq!(edit.group(), case["expected"]);
    }
}

fn fixture() -> (Profile, DeviceProfiles, Edit) {
    let mut profile = super::super::new_native_profile(605, "Current").unwrap();
    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
        {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup",
         "profileNavigationGroup":{"future":[1,2],"Name":"old","ProfileId":"stale","guid":"old","lightpacId":"old"},"opaque":17},
        {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"future","opaque":[3]},
        {"inputID":"KEY_B","inputType":"KeyInput","outputType":"future","opaque":true}
    ]);
    profile.buttons.insert("key-a".into(), "key:c".into());
    profile
        .buttons
        .insert("key-a/hypershift".into(), "key:d".into());
    let mut profiles = DeviceProfiles::default();
    profiles.profiles.clear();
    profiles.active = "Current".into();
    profiles.profiles.insert("Current".into(), profile.clone());
    let mut target = super::super::new_native_profile(605, "Target").unwrap();
    target.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["guid"] = json!("target-id");
    profiles.profiles.insert("Target".into(), target);
    (
        profile,
        profiles,
        Edit {
            input_id: "KEY_A".into(),
            input_type: "KeyInput".into(),
            hyper: false,
            action: "Specific".into(),
            guid: Some("target-id".into()),
            lightpac_id: Some("effect-id".into()),
        },
    )
}
#[test]
fn navigation_authoring_preserves_layers_opaque_records_and_roundtrips_source_ids() {
    let (profile, mut profiles, edit) = fixture();
    let next = candidate(&profile, 605, &profiles, &["effect-id"], &edit).unwrap();
    let old = &profile.extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
    let rows = &next.extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
    assert_eq!(
        rows[0]["profileNavigationGroup"],
        json!({"future":[1,2],"profileNavigationAssignment":"Specific","guid":"target-id","lightpacId":"effect-id"})
    );
    assert_eq!(rows[0]["opaque"], old[0]["opaque"]);
    assert_eq!(rows[1], old[1]);
    assert_eq!(rows[2], old[2]);
    assert!(!next.buttons.contains_key("key-a"));
    assert_eq!(next.buttons["key-a/hypershift"], "key:d");
    assert!(profile.buttons.contains_key("key-a"));
    let expected = rows[0].clone();
    profiles.profiles.insert("Current".into(), next);
    let mut store = crate::Store::load(
        &std::env::temp_dir().join(format!("navigation-export-{}.json", std::process::id())),
    )
    .unwrap();
    store.devices.insert("025D".into(), profiles);
    let export = super::super::export_synapse4(&store, "025D").unwrap();
    let imported = super::super::import_document(&export.bytes).unwrap();
    assert_eq!(
        imported.store.devices["025D"].profiles["Current"].extra["_synapseLiteSynapse4"]["decoded"]
            ["mappings"][0],
        expected
    );
}
#[test]
fn navigation_authoring_refuses_deleted_ambiguous_current_and_missing_lighting_targets() {
    let (profile, profiles, mut edit) = fixture();
    let before = serde_json::to_value(&profile).unwrap();
    for mode in 0..6 {
        let mut catalog = profiles.clone();
        let mut effects = vec!["effect-id"];
        match mode {
            0 => {
                catalog.profiles.remove("Target");
            }
            1 => {
                catalog
                    .profiles
                    .insert("Duplicate".into(), catalog.profiles["Target"].clone());
            }
            2 => {
                catalog.active = "Target".into();
            }
            3 => effects.clear(),
            4 => edit.guid = Some("missing".into()),
            _ => edit.guid = None,
        }
        assert!(candidate(&profile, 605, &catalog, &effects, &edit).is_err());
        assert_eq!(serde_json::to_value(&profile).unwrap(), before);
    }
}
#[test]
fn navigation_modes_remove_stale_specific_fields_and_keep_unknown_fields() {
    let (profile, profiles, mut edit) = fixture();
    for (action, _) in &CHOICES[..4] {
        edit.action = (*action).into();
        edit.guid = None;
        edit.lightpac_id = None;
        let next = candidate(&profile, 605, &profiles, &[], &edit).unwrap();
        assert_eq!(
            next.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][0]["profileNavigationGroup"],
            json!({"future":[1,2],"profileNavigationAssignment":action})
        );
    }
    edit.guid = Some("target-id".into());
    assert!(candidate(&profile, 605, &profiles, &[], &edit).is_err());
}
#[test]
fn navigation_input_layers_and_malformed_data_refuse_without_mutating_source() {
    let (profile, profiles, mut edit) = fixture();
    for (pid, input, kind) in [
        (605, "missing", "KeyInput"),
        (196, "LeftClick", "MouseInput"),
        (196, "ScrollUp", "MouseInput"),
        (999, "KEY_A", "KeyInput"),
    ] {
        edit.input_id = input.into();
        edit.input_type = kind.into();
        assert!(candidate(&profile, pid, &profiles, &["effect-id"], &edit).is_err());
    }
    edit.input_id = "KEY_A".into();
    edit.input_type = "KeyInput".into();
    for malformed in [json!(5), json!("false"), json!(null)] {
        let mut p = profile.clone();
        p.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"][0]
            ["isHyperShift"] = malformed;
        assert!(candidate(&p, 605, &profiles, &["effect-id"], &edit).is_err());
    }
    let mut p = profile.clone();
    let rows = p.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"]
        .as_array_mut()
        .unwrap();
    rows.push(rows[0].clone());
    assert!(candidate(&p, 605, &profiles, &["effect-id"], &edit).is_err());
}
