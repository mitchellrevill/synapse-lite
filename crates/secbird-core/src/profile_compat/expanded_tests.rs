use super::*;
use crate::buttons::{Action, Layer};
use crate::features::DpiStages;

fn document(profile: Value, pid: u16) -> Vec<u8> {
    let mut envelope = settings::default_envelope(pid).unwrap();
    envelope["profiles"] = json!([{
        "name": "Fixture", "hash": synapse_hash(&profile).unwrap(),
        "payload": STANDARD.encode(serde_json::to_vec(&profile).unwrap())
    }]);
    serde_json::to_vec(&envelope).unwrap()
}

fn payload(store: &Store, key: &str, name: &str) -> Value {
    let export = export_synapse4(store, key).unwrap();
    let envelope: Value = serde_json::from_slice(&export.bytes).unwrap();
    let entry = envelope["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap();
    decode_record(entry).unwrap()
}

#[test]
fn dpi_stages_and_sleep_translate_and_preserve_inactive_axis_and_unknown_fields() {
    let source = json!({
        "guid":"stage-fixture", "name":"Fixture", "dpi":{"x":800,"y":800}, "powerSavingValue":7,
        "dpiStages":{"active":2,"enable":true,"count":2,"futureStages":"retain", "stages":[
            {"x":400,"y":1200,"independent":false,"visible":true,"futureStage":42},
            {"x":800,"y":1600,"independent":true,"visible":true}
        ]}
    });
    let mut report = import_document(&document(source.clone(), 196)).unwrap();
    assert!(report.warnings.is_empty());
    let profile = &report.store.devices["00C4"].profiles["Fixture"];
    assert_eq!(profile.sleep_minutes, Some(7));
    assert_eq!(profile.dpi_stages.as_ref().unwrap().active, 2);
    assert_eq!(
        profile.dpi_stages.as_ref().unwrap().stages,
        [(400, 400), (800, 1600)]
    );
    let unchanged = payload(&report.store, "00C4", "Fixture");
    assert_eq!(unchanged["dpiStages"], source["dpiStages"]);
    let profile = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    profile.sleep_minutes = Some(12);
    profile.dpi_stages.as_mut().unwrap().active = 1;
    profile.dpi_stages.as_mut().unwrap().stages[0] = (1600, 3200);
    let changed = payload(&report.store, "00C4", "Fixture");
    assert_eq!(changed["powerSavingValue"], 12);
    assert_eq!(changed["dpiStages"]["active"], 1);
    assert_eq!(changed["dpiStages"]["futureStages"], "retain");
    assert_eq!(changed["dpiStages"]["stages"][0]["futureStage"], 42);
    assert_eq!(changed["dpiStages"]["stages"][0]["y"], 3200);
}

#[test]
fn hidden_disabled_stages_and_conditional_gaming_remain_explicitly_unsupported() {
    let stages = json!({"guid":"hidden", "name":"Fixture", "dpiStages":{
        "active":1,"enable":false,"count":1,"stages":[
            {"x":400,"y":400,"independent":false,"visible":true},
            {"x":800,"y":800,"independent":false,"visible":false}
        ]
    }});
    let report = import_document(&document(stages.clone(), 196)).unwrap();
    assert!(report.store.devices["00C4"].profiles["Fixture"]
        .dpi_stages
        .is_none());
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("hidden DPI stages")));
    assert_eq!(
        payload(&report.store, "00C4", "Fixture")["dpiStages"],
        stages["dpiStages"]
    );
    let gaming = json!({"guid":"conditional", "name":"Fixture", "gamingMode":{
        "state":2,"isWindowsKeyDisabled":true,"isAltTabDisabled":true,"isAltF4Disabled":false
    }});
    let report = import_document(&document(gaming.clone(), 605)).unwrap();
    assert!(report.store.devices["025D"].profiles["Fixture"]
        .game_mode
        .is_none());
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("conditional gaming")));
    assert_eq!(
        payload(&report.store, "025D", "Fixture")["gamingMode"],
        gaming["gamingMode"]
    );
}

#[test]
fn gaming_mode_translates_and_off_preserves_saved_checkbox_choices() {
    let source = json!({"guid":"gaming", "name":"Fixture", "gamingMode":{
        "state":1,"isWindowsKeyDisabled":true,"isAltTabDisabled":true,"isAltF4Disabled":false,"future":"retain"
    }});
    let mut report = import_document(&document(source, 605)).unwrap();
    assert_eq!(
        report.store.devices["025D"].profiles["Fixture"]
            .game_mode
            .as_deref(),
        Some("win,alt-tab")
    );
    report
        .store
        .devices
        .get_mut("025D")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap()
        .game_mode = Some("off".into());
    let exported = payload(&report.store, "025D", "Fixture");
    assert_eq!(exported["gamingMode"]["state"], 0);
    assert_eq!(exported["gamingMode"]["isWindowsKeyDisabled"], true);
    assert_eq!(exported["gamingMode"]["future"], "retain");
}

#[test]
fn mouse_and_keyboard_mappings_translate_and_deletions_do_not_resurrect() {
    let source = json!({"guid":"buttons", "name":"Fixture", "mappings":[
        {"inputID":"Button5","inputType":"MouseInput","isHyperShift":true,"outputType":"keyboardGroup","future":"retain",
         "keyboardGroup":{"key":"KEY_F13","keyGroup":0,"modifiers":["KEY_LEFT_CTRL","KEY_RIGHT_SHIFT"],"turboMode":{"isTurbo":false,"keysPerSecond":7,"futureTurbo":19}}},
        {"inputID":"Button4","inputType":"MouseInput","outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"Previous"}},
        {"inputID":"ScrollButton","inputType":"MouseInput","outputType":"macroGroup","macroGroup":{"guid":"opaque-macro"}}
    ]});
    let mut report = import_document(&document(source, 196)).unwrap();
    let profile = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    assert_eq!(
        profile.buttons["side-forward/hypershift"]
            .parse::<Action>()
            .unwrap(),
        Action::Key {
            modifiers: 0x21,
            usage: 104
        }
    );
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.contains("macroGroup")));
    profile.buttons.remove("side-back");
    profile.buttons.insert(
        "side-forward/hypershift".into(),
        Action::TurboKey {
            modifiers: 1,
            usage: 4,
            rate: 12,
        }
        .to_string(),
    );
    profile
        .buttons
        .insert("dpi".into(), Action::DpiCycleUp.to_string());
    let exported = payload(&report.store, "00C4", "Fixture");
    let mappings = exported["mappings"].as_array().unwrap();
    assert!(!mappings.iter().any(|record| record["inputID"] == "Button4"));
    let forward = mappings
        .iter()
        .find(|record| record["inputID"] == "Button5")
        .unwrap();
    assert_eq!(forward["future"], "retain");
    assert_eq!(forward["keyboardGroup"]["key"], "KEY_A");
    assert_eq!(forward["keyboardGroup"]["turboMode"]["futureTurbo"], 19);
    assert_eq!(forward["keyboardGroup"]["turboMode"]["keysPerSecond"], 12);
    assert!(mappings
        .iter()
        .any(|record| record["macroGroup"]["guid"] == "opaque-macro"));
    assert!(mappings
        .iter()
        .any(|record| record["inputID"] == "DKM_SB_03"));
}

#[test]
fn supported_mapping_actions_round_trip_in_normal_and_hypershift_layers() {
    let actions = [
        Action::Disabled,
        Action::HyperShift,
        Action::DoubleClick,
        Action::Mouse(1),
        Action::Mouse(5),
        Action::Mouse(9),
        Action::TurboMouse {
            button: 2,
            rate: 10,
        },
        Action::Key {
            modifiers: 0x88,
            usage: 104,
        },
        Action::TurboKey {
            modifiers: 0x04,
            usage: 4,
            rate: 17,
        },
        Action::DpiUp,
        Action::DpiDown,
        Action::DpiCycleUp,
        Action::DpiCycleDown,
        Action::DpiClutch { x: 400, y: 800 },
    ];
    for action in actions {
        for layer in [Layer::Normal, Layer::HyperShift] {
            let mut store = Store::default();
            let profile = store.device(196).active_mut();
            profile.buttons.insert(
                crate::buttons::setting_key("side-forward", layer),
                action.to_string(),
            );
            let bytes = export_synapse4(&store, "00C4").unwrap().bytes;
            let imported = import_document(&bytes).unwrap();
            let key = crate::buttons::setting_key("side-forward", layer);
            assert_eq!(
                imported.store.devices["00C4"].profiles["Default"].buttons[&key]
                    .parse::<Action>()
                    .unwrap(),
                action
            );
        }
    }
}

#[test]
fn native_profiles_export_using_observed_templates_and_unique_v4_guids() {
    let mut store = Store::default();
    let profile = store.device(196).active_mut();
    profile.dpi = Some((1600, 3200));
    profile.polling = Some(500);
    profile.sleep_minutes = Some(9);
    profile.dpi_stages = Some(DpiStages {
        active: 2,
        stages: vec![(400, 400), (1600, 3200)],
        ..Default::default()
    });
    let bytes = export_synapse4(&store, "00C4").unwrap().bytes;
    let imported = import_document(&bytes).unwrap();
    let profile = &imported.store.devices["00C4"].profiles["Default"];
    assert_eq!(profile.dpi, Some((1600, 3200)));
    assert_eq!(profile.polling, Some(500));
    assert_eq!(profile.sleep_minutes, Some(9));
    assert_eq!(profile.dpi_stages.as_ref().unwrap().active, 2);
    let decoded = payload(&imported.store, "00C4", "Default");
    let guid = decoded["guid"].as_str().unwrap();
    assert_eq!(guid.len(), 36);
    assert_eq!(&guid[14..15], "4");
    assert!(matches!(&guid[19..20], "8" | "9" | "a" | "b"));
    let profile = store.device(605).active_mut();
    profile.game_mode = Some("win,alt-f4".into());
    profile.buttons.insert(
        "key-a".into(),
        Action::Key {
            modifiers: 1,
            usage: 104,
        }
        .to_string(),
    );
    let bytes = export_synapse4(&store, "025D").unwrap().bytes;
    let imported = import_document(&bytes).unwrap();
    assert_eq!(
        imported.store.devices["025D"].profiles["Default"]
            .game_mode
            .as_deref(),
        Some("win,alt-f4")
    );
    assert_eq!(
        imported.store.devices["025D"].profiles["Default"].buttons["key-a"]
            .parse::<Action>()
            .unwrap(),
        Action::Key {
            modifiers: 1,
            usage: 104
        }
    );
}

#[test]
fn add_profile_constructor_matches_observed_defaults_and_retains_opaque_fields() {
    let mouse = new_native_profile(196, "Computer-Default").unwrap();
    assert_eq!(mouse.dpi, Some((800, 800)));
    assert_eq!(mouse.polling, Some(1000));
    assert_eq!(mouse.sleep_minutes, Some(5));
    assert_eq!(mouse.dpi_stages.as_ref().unwrap().active, 3);
    assert_eq!(
        mouse.dpi_stages.as_ref().unwrap().stages,
        [
            (400, 400),
            (800, 800),
            (1600, 1600),
            (3200, 3200),
            (6400, 6400)
        ]
    );
    assert_eq!(mouse.extra[SOURCE]["decoded"]["lowPowerMode"], 30);
    let keyboard = new_native_profile(605, "Computer-Default").unwrap();
    assert_eq!(keyboard.game_mode.as_deref(), Some("off"));
    assert_eq!(keyboard.lighting["backlight"].brightness, Some(100));
    assert_eq!(
        keyboard.lighting["backlight"].effect,
        Some(Effect::Spectrum)
    );
    assert_eq!(
        keyboard.extra[SOURCE]["decoded"]["switchOffLighting"]["isIdleEnabled"],
        false
    );
    assert_ne!(
        mouse.extra[SOURCE]["decoded"]["guid"],
        keyboard.extra[SOURCE]["decoded"]["guid"]
    );
    assert!(new_native_profile(605, "").is_err());
    assert!(new_native_profile(0xffff, "Unknown").is_err());
}

#[test]
fn copied_profiles_renew_identity_and_legacy_duplicate_guids_are_also_unique() {
    let source = json!({"guid":"original-guid", "name":"Fixture","dpi":{"x":800,"y":800}});
    let mut report = import_document(&document(source, 196)).unwrap();
    let mut copy = report.store.devices["00C4"].profiles["Fixture"].clone();
    renew_profile_identity(&mut copy).unwrap();
    assert_ne!(copy.extra[SOURCE]["decoded"]["guid"], "original-guid");
    report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .insert("Copy".into(), copy);
    let original = payload(&report.store, "00C4", "Fixture");
    let copy = payload(&report.store, "00C4", "Copy");
    assert_eq!(original["guid"], "original-guid");
    assert_ne!(copy["guid"], original["guid"]);
    let legacy = report.store.devices["00C4"].profiles["Fixture"].clone();
    report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .insert("A legacy copy".into(), legacy);
    let bytes = export_synapse4(&report.store, "00C4").unwrap().bytes;
    let exported: Value = serde_json::from_slice(&bytes).unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for entry in exported["profiles"].as_array().unwrap() {
        assert!(ids.insert(
            decode_record(entry).unwrap()["guid"]
                .as_str()
                .unwrap()
                .to_string()
        ));
    }
    assert_eq!(
        payload(&report.store, "00C4", "Fixture")["guid"],
        "original-guid"
    );
}

#[test]
fn aggregate_store_budget_rejects_many_individually_valid_imports() {
    let mut first = Store::default();
    first
        .extra
        .insert("source-one".into(), json!("x".repeat(1_100_000)));
    let mut second = Store::default();
    second
        .extra
        .insert("source-two".into(), json!("y".repeat(1_100_000)));
    validate_store_budget(&first).unwrap();
    validate_store_budget(&second).unwrap();
    first.import(second);
    assert!(validate_store_budget(&first).is_err());
}

#[test]
fn invalid_stage_counts_and_sleep_bounds_are_rejected() {
    let mut profile = settings::default_profile(196, "Fixture").unwrap();
    profile["dpiStages"]["count"] = json!(4);
    assert!(import_document(&document(profile, 196)).is_err());
    let profile = json!({"guid":"bad-sleep","name":"Fixture","powerSavingValue":16});
    assert!(import_document(&document(profile, 196)).is_err());
}

#[test]
fn foreign_export_refuses_unknown_native_extensions_without_losing_native_data() {
    let mut store = Store::default();
    store
        .device(196)
        .active_mut()
        .extra
        .insert("futureAction".into(), json!({"binding":"opaque"}));
    assert!(matches!(
        export_synapse4(&store, "00C4"),
        Err(Error::Unsupported(_))
    ));
    let native = serde_json::to_vec(&store).unwrap();
    assert_eq!(
        import_document(&native).unwrap().store.devices["00C4"].profiles["Default"].extra
            ["futureAction"]["binding"],
        "opaque"
    );
    store.device(196).active_mut().extra.clear();
    store
        .device(196)
        .extra
        .insert("futureCalibration".into(), json!([1, 2, 3]));
    assert!(matches!(
        export_synapse4(&store, "00C4"),
        Err(Error::Unsupported(_))
    ));
}
