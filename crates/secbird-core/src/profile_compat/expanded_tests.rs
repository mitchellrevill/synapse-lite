use super::*;
use crate::buttons::{Action, Layer};
use crate::features::DpiStages;

#[test]
fn ornata_snap_tap_preserves_source_shape_and_pair_edits_without_claiming_replay() {
    let source=json!({"guid":"snap-fixture","name":"Fixture","mappings":[],
        "snapTap":{"isEnabled":true,"future":{"keep":true},"keyList":[
            {"id":1,"key1":"KEY_A","key2":"KEY_D","futurePair":7}]}});
    let mut imported=import_document(&document(source.clone(),605)).unwrap();
    assert!(imported.store.devices["025D"].profiles["Fixture"].snap_tap.is_some());
    assert!(imported.warnings.iter().any(|w|w.contains("host input routing")));
    assert_eq!(payload(&imported.store,"025D","Fixture"),source);
    let native=import_document(&imported.store.native_bytes().unwrap()).unwrap();
    assert_eq!(payload(&native.store,"025D","Fixture"),source);
    let settings=imported.store.devices.get_mut("025D").unwrap().profiles.get_mut("Fixture").unwrap().snap_tap.as_mut().unwrap();
    assert!(!settings.plan().is_ok(),"unknown extensions must not silently run");
    settings.is_enabled=false;
    settings.key_list.clear();
    let changed=payload(&imported.store,"025D","Fixture");
    assert_eq!(changed["snapTap"]["isEnabled"],false);
    assert_eq!(changed["snapTap"]["keyList"],json!([]));
    assert_eq!(changed["snapTap"]["future"],source["snapTap"]["future"]);
    let mut unsupported=source.clone();
    unsupported["snapTap"]["keyList"][0]["key2"]=json!("KEY_A");
    let imported=import_document(&document(unsupported.clone(),605)).unwrap();
    assert!(imported.store.devices["025D"].profiles["Fixture"].snap_tap.is_none());
    assert!(imported.warnings.iter().any(|w|w.contains("unsupported Snap Tap")));
    assert_eq!(payload(&imported.store,"025D","Fixture"),unsupported);
}

#[test]
fn advanced_import_notices_follow_the_settings_actually_translated() {
    let source = json!({"guid":"advanced-notices","name":"Fixture","mappings":[],
        "rotation":{"isEnabled":true,"value":5},
        "dynamicSensitivity":{"state":0,"mode":0,"customSensorAccelerations":[],"templateId":0},
        "inGamePollingRate":{"state":0,"wired":1000,"wireless":1000}});
    for product in [196, 197] {
        let imported = import_document(&document(source.clone(), product)).unwrap();
        let key = if product == 196 { "00C4" } else { "00C5" };
        let profile = &imported.store.devices[key].profiles["Fixture"];
        assert_eq!(profile.rotation.as_ref().unwrap().value, 5);
        assert!(profile.dynamic_sensitivity.is_some());
        assert!(!imported.warnings.iter().any(|w| w.contains("rotation") || w.contains("dynamicSensitivity")));
        assert!(imported.warnings.iter().any(|w| w.contains("inGamePollingRate")));
        assert_eq!(payload(&imported.store, key, "Fixture"), source);
    }
    let mut sparse = source.clone();
    sparse["rotation"] = json!({"value":5,"futureRotation":{"keep":true}});
    sparse["dynamicSensitivity"] = json!({"state":0,"mode":0,"futureCurve":[1,2]});
    let mut imported = import_document(&document(sparse.clone(), 196)).unwrap();
    assert_eq!(payload(&imported.store, "00C4", "Fixture"), sparse);
    let profile = imported.store.devices.get_mut("00C4").unwrap().profiles.get_mut("Fixture").unwrap();
    profile.rotation.as_mut().unwrap().value = -5;
    profile.dynamic_sensitivity.as_mut().unwrap().mode = 1;
    let edited = payload(&imported.store, "00C4", "Fixture");
    assert_eq!(edited["rotation"]["value"], -5);
    assert_eq!(edited["rotation"]["futureRotation"], sparse["rotation"]["futureRotation"]);
    assert_eq!(edited["dynamicSensitivity"]["mode"], 1);
    assert_eq!(edited["dynamicSensitivity"]["futureCurve"], sparse["dynamicSensitivity"]["futureCurve"]);
    let mut unsupported = source;
    unsupported["rotation"]["value"] = json!(90);
    unsupported["dynamicSensitivity"]["mode"] = json!(255);
    let imported = import_document(&document(unsupported.clone(), 196)).unwrap();
    let profile = &imported.store.devices["00C4"].profiles["Fixture"];
    assert!(profile.rotation.is_none());
    assert!(profile.dynamic_sensitivity.is_none());
    assert!(imported.warnings.iter().any(|w| w.contains("unsupported Rotation")));
    assert!(imported.warnings.iter().any(|w| w.contains("unsupported Dynamic Sensitivity")));
    assert_eq!(payload(&imported.store, "00C4", "Fixture"), unsupported);
}

#[test]
fn source_effect_edits_preserve_opaque_members_of_the_same_preset() {
    let mut profile = new_native_profile(605,"Effect").unwrap();
    set_source_quick_effect(&mut profile,605,"Effect",1,
        &json!({"color1":"#112233","future":{"value":7}})).unwrap();
    set_source_quick_effect(&mut profile,605,"Effect",1,
        &json!({"color1":"#445566"})).unwrap();
    let effects = source_quick_effects(&profile).unwrap();
    assert_eq!(effects["selectedEffectSetting"],json!({"color1":"#445566","future":{"value":7}}));
    set_source_quick_effect(&mut profile,605,"Effect",3,&json!({})).unwrap();
    assert_eq!(source_quick_effects(&profile).unwrap()["selectedEffectSetting"],json!({}));
}

#[test]
fn custom_palette_preserves_opaque_imports_but_rejects_new_malformed_drafts() {
    let mut profile = new_native_profile(605, "Palette").unwrap();
    let opaque = json!({"color":"future-color-space(1 2 3)","extension":{"v":2}});
    source_quick_effects_mut(&mut profile, 605, "Palette").unwrap()
        .insert("customColors".into(), json!([opaque.clone()]));
    let mut slots = vec![json!({"color":"#111","picked":false});16];
    slots[0] = opaque.clone();
    slots[1] = json!({"color":"#44d62c","picked":false,"future":7});
    set_source_custom_colors(&mut profile, 605, "Palette", &slots).unwrap();
    assert_eq!(source_quick_effects(&profile).unwrap()["customColors"],
        json!([opaque,slots[1].clone()]));
    let before = serde_json::to_value(&profile).unwrap();
    slots[2] = json!({"color":"#12345z","picked":false});
    assert!(set_source_custom_colors(&mut profile,605,"Palette",&slots).is_err());
    assert_eq!(serde_json::to_value(&profile).unwrap(), before);
    slots[2] = json!({"color":"#111","picked":false});
    slots[0] = json!({"color":"#111","picked":false});
    set_source_custom_colors(&mut profile,605,"Palette",&slots).unwrap();
    assert_eq!(source_quick_effects(&profile).unwrap()["customColors"],json!([slots[1].clone()]));
}

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
fn dpi_stages_translate_and_source_sleep_stays_opaque() {
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
    assert_eq!(profile.sleep_minutes, None);
    assert_eq!(profile.dpi_stages.as_ref().unwrap().active, 2);
    assert_eq!(
        profile.dpi_stages.as_ref().unwrap().stages,
        [(400, 1200), (800, 1600)]
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
fn hidden_disabled_stages_translate_and_conditional_gaming_stays_unsupported() {
    let stages = json!({"guid":"hidden", "name":"Fixture", "dpiStages":{
        "active":1,"enable":false,"count":1,"stages":[
            {"x":400,"y":400,"independent":false,"visible":true},
            {"x":800,"y":800,"independent":false,"visible":false}
        ]
    }});
    let report = import_document(&document(stages.clone(), 196)).unwrap();
    let profile = &report.store.devices["00C4"].profiles["Fixture"];
    assert!(report.warnings.is_empty());
    assert_eq!(profile.dpi_stages.as_ref().unwrap().stages, [(400, 400)]);
    assert!(!profile.performance.as_ref().unwrap().enabled);
    assert_eq!(profile.performance.as_ref().unwrap().stages.len(), 2);
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
    assert_eq!(profile.sleep_minutes, None);
    assert_eq!(profile.dpi_stages.as_ref().unwrap().active, 2);
    let decoded = payload(&imported.store, "00C4", "Default");
    assert_eq!(decoded["powerSavingValue"], 9);
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
    assert_eq!(mouse.sleep_minutes, None);
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
fn invalid_stage_counts_and_explicit_native_sleep_bounds_are_rejected() {
    let mut profile = settings::default_profile(196, "Fixture").unwrap();
    profile["dpiStages"]["count"] = json!(4);
    assert!(import_document(&document(profile, 196)).is_err());
    let source = json!({"guid":"opaque-sleep","name":"Fixture","powerSavingValue":16});
    let imported = import_document(&document(source, 196)).unwrap();
    assert_eq!(
        payload(&imported.store, "00C4", "Fixture")["powerSavingValue"],
        16
    );
    let mut native = Store::default();
    native.device(196).active_mut().sleep_minutes = Some(16);
    assert!(export_synapse4(&native, "00C4").is_err());
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

#[test]
fn hidden_rows_and_independent_edit_round_trip_without_losing_foreign_fields() {
    let source = json!({"guid":"hidden-rows","name":"Fixture","dpiStages":{
    "enable":true,"active":3,"count":2,"futureStages":{"keep":true},"stages":[
        {"x":400,"y":900,"independent":false,"visible":true},
        {"x":800,"y":1700,"independent":false,"visible":false,"futureRow":"keep"},
        {"x":1600,"y":3200,"independent":true,"visible":true}
    ]}});
    let mut report = import_document(&document(source.clone(), 196)).unwrap();
    assert!(report.warnings.is_empty());
    let p = &report.store.devices["00C4"].profiles["Fixture"];
    assert_eq!(p.dpi_stages.as_ref().unwrap().active, 2);
    assert_eq!(
        p.dpi_stages.as_ref().unwrap().stages,
        vec![(400, 900), (1600, 3200)]
    );
    assert_eq!(
        payload(&report.store, "00C4", "Fixture")["dpiStages"],
        source["dpiStages"]
    );
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    let mut intent = crate::performance::Performance::from_profile(p)
        .unwrap()
        .unwrap();
    intent.enabled = false;
    intent.store_in_profile(p).unwrap();
    let out = payload(&report.store, "00C4", "Fixture");
    assert_eq!(out["dpiStages"]["stages"], source["dpiStages"]["stages"]);
    assert_eq!(out["dpiStages"]["active"], 3);
    assert_eq!(out["dpiStages"]["enable"], false);
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    intent.enabled = true;
    intent.set_visible_count(3).unwrap();
    intent.select_visible(2).unwrap();
    intent.set_independent(1, true).unwrap();
    // Revealing the row and enabling XY must recover its original stored Y.
    assert_eq!(intent.stages[1].y, 1700);
    intent.store_in_profile(p).unwrap();
    let out = payload(&report.store, "00C4", "Fixture");
    assert_eq!(out["dpiStages"]["stages"][1]["y"], 1700);
    assert_eq!(out["dpiStages"]["stages"][1]["futureRow"], "keep");
    assert_eq!(
        out["dpiStages"]["futureStages"],
        source["dpiStages"]["futureStages"]
    );
    let imported = import_document(&export_synapse4(&report.store, "00C4").unwrap().bytes).unwrap();
    assert_eq!(
        imported.store.devices["00C4"].profiles["Fixture"]
            .performance
            .as_ref(),
        Some(&intent)
    );
}

#[test]
fn device_wide_power_is_retained_in_native_backup_and_explicitly_omitted_from_foreign_profiles() {
    let mut store = Store::default();
    store.device(196).profiles.insert(
        "Fixture".into(),
        new_native_profile(196, "Fixture").unwrap(),
    );
    store.device(196).power = Some(crate::DevicePower {
        critical_threshold_percent: Some(45),
        ..Default::default()
    });
    let exported = export_synapse4(&store, "00C4").unwrap();
    assert!(exported
        .warnings
        .iter()
        .any(|w| w.contains("Device-wide Power")));
    let source = payload(&store, "00C4", "Fixture");
    assert_eq!(
        source["lowPowerMode"], 30,
        "global45 must not overwrite an unrelated original profile field"
    );
    let native = import_document(&serde_json::to_vec(&store).unwrap()).unwrap();
    assert_eq!(
        native.store.devices["00C4"]
            .power
            .as_ref()
            .unwrap()
            .critical_threshold_percent,
        Some(45)
    );
}

#[test]
fn source_sleep_provenance_prevents_activation_writes_and_retains_original_payload() {
    use crate::sim::Sim;
    for pid in [196, 197] {
        let source = json!({"guid":"source-sleep","name":"Fixture","powerSavingValue":5,"future":"keep","mappings":[]});
        let key = format!("{pid:04X}");
        let imported = import_document(&document(source.clone(), pid)).unwrap();
        let p = &imported.store.devices[&key].profiles["Fixture"];
        assert!(p.sleep_minutes.is_none());
        assert_eq!(p.extra[SOURCE]["deviceSleepScope"], true);
        assert_eq!(payload(&imported.store, &key, "Fixture"), source);
        let native = import_document(&serde_json::to_vec(&imported.store).unwrap()).unwrap();
        assert_eq!(payload(&native.store, &key, "Fixture"), source);
        let sim = Sim::wireless_mouse();
        sim.state().idle_seconds = Some(240);
        let dev = sim
            .open(pid, Sim::spec_for(crate::sim::WIRELESS_MOUSE_PID))
            .unwrap();
        assert!(p.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert_eq!(sim.state().idle_seconds, Some(240));
        assert!(!sim.state().log.contains(&crate::commands::IDLE_TIME));
        let default = new_native_profile(pid, "Computer-Default").unwrap();
        // Stage settings are supported for the source default too.
        sim.state().dpi_stages = Some((1, vec![(800, 800)]));
        assert!(default.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert_eq!(sim.state().idle_seconds, Some(240));
        assert!(!sim.state().log.contains(&crate::commands::IDLE_TIME));
    }
}

#[test]
fn explicit_legacy_sleep_override_and_old_source_deletion_are_unchanged() {
    let source = json!({"guid":"sleep-overlay","name":"Fixture","powerSavingValue":5});
    let mut report = import_document(&document(source, 196)).unwrap();
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    p.sleep_minutes = Some(7);
    assert_eq!(
        payload(&report.store, "00C4", "Fixture")["powerSavingValue"],
        7
    );
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    p.sleep_minutes = None;
    assert_eq!(
        payload(&report.store, "00C4", "Fixture")["powerSavingValue"],
        5,
        "new scope marker retains opaque original when override removed"
    );
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    p.extra
        .get_mut(SOURCE)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("deviceSleepScope");
    p.sleep_minutes = Some(7);
    let native = import_document(&serde_json::to_vec(&report.store).unwrap()).unwrap();
    let p = &native.store.devices["00C4"].profiles["Fixture"];
    assert_eq!(
        p.sleep_minutes,
        Some(7),
        "native JSON does not reinterpret old source metadata"
    );
    let sim = crate::sim::Sim::wireless_mouse();
    let dev = sim
        .open(
            196,
            crate::sim::Sim::spec_for(crate::sim::WIRELESS_MOUSE_PID),
        )
        .unwrap();
    assert!(p.apply(&dev).iter().all(|r| r.result.is_ok()));
    assert_eq!(sim.state().idle_seconds, Some(420));
    let p = report
        .store
        .devices
        .get_mut("00C4")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    p.sleep_minutes = None;
    assert!(
        payload(&report.store, "00C4", "Fixture")
            .get("powerSavingValue")
            .is_none(),
        "older no-flag known-field deletion remains deletion"
    );
}

#[test]
fn source_lighting_retains_disabled_value_policy_flags_and_unknown_fields() {
    let source = json!({"guid":"lighting-intent","name":"Fixture","mappings":[],
        "brightness":{"isEnabled":false,"value":73,"futureBrightness":{"keep":true}},
        "switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":7,"futurePolicy":[1,2]},
        "quickEffects":{"selectedEffectId":3}});
    let mut report = import_document(&document(source.clone(), 605)).unwrap();
    assert!(report.warnings.is_empty());
    let profile = &report.store.devices["025D"].profiles["Fixture"];
    let intent = profile.lighting_settings.as_ref().unwrap();
    assert_eq!(intent.brightness.as_ref().unwrap().value, 73);
    assert!(!intent.brightness.as_ref().unwrap().enabled);
    assert_eq!(profile.lighting["backlight"].brightness, Some(0));
    assert!(
        !profile.lighting["backlight"].off,
        "brightness switch must not disable an effect"
    );
    assert_eq!(payload(&report.store, "025D", "Fixture"), source);
    let mut intent = intent.clone();
    intent.brightness.as_mut().unwrap().set_enabled(true);
    intent.switch_off.as_mut().unwrap().idle_enabled = true;
    let p = report
        .store
        .devices
        .get_mut("025D")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    intent.store_in_profile(p).unwrap();
    let out = payload(&report.store, "025D", "Fixture");
    assert_eq!(out["brightness"]["value"], 73);
    assert_eq!(out["brightness"]["isEnabled"], true);
    assert_eq!(
        out["brightness"]["futureBrightness"],
        source["brightness"]["futureBrightness"]
    );
    assert_eq!(out["switchOffLighting"]["idleMinutes"], 7);
    assert_eq!(
        out["switchOffLighting"]["futurePolicy"],
        source["switchOffLighting"]["futurePolicy"]
    );
    let native = import_document(&serde_json::to_vec(&report.store).unwrap()).unwrap();
    assert_eq!(
        native.store.devices["025D"].profiles["Fixture"].lighting_settings,
        Some(intent)
    );
    let again = import_document(&export_synapse4(&native.store, "025D").unwrap().bytes).unwrap();
    assert!(again.warnings.iter().any(|w| w.contains("host runtime")));
    assert_eq!(payload(&again.store, "025D", "Fixture"), out);
}

#[test]
fn effect_off_is_independent_from_brightness_and_typed_deletion_is_not_resurrected() {
    let source = json!({"guid":"lighting-off","name":"Fixture","mappings":[],
        "brightness":{"isEnabled":true,"value":73},
        "switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":7},
        "quickEffects":{"selectedEffectId":0}});
    let mut report = import_document(&document(source.clone(), 605)).unwrap();
    assert!(report.store.devices["025D"].profiles["Fixture"].lighting["backlight"].off);
    assert_eq!(
        payload(&report.store, "025D", "Fixture")["brightness"],
        source["brightness"]
    );
    assert_eq!(
        payload(&report.store, "025D", "Fixture")["quickEffects"]["selectedEffectId"],
        0
    );
    let p = report
        .store
        .devices
        .get_mut("025D")
        .unwrap()
        .profiles
        .get_mut("Fixture")
        .unwrap();
    let mut intent = p.lighting_settings.clone().unwrap();
    intent.brightness = None;
    intent.switch_off = None;
    intent.store_in_profile(p).unwrap();
    let out = payload(&report.store, "025D", "Fixture");
    assert!(out.get("brightness").is_none());
    assert!(out.get("switchOffLighting").is_none());
    assert_eq!(out["quickEffects"]["selectedEffectId"], 0);
}
