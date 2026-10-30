use super::*;
const PROFILE: &str = r#"{"id":"p","title":"Tidal","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"t","type":"layer","name":"tidal","value":19,"title":"TIDAL","visible":true,"params":{"centerPointX":125,"centerPointY":123,"centerPointX2":5000,"centerPointY2":5000,"counterclockwise":false,"future":9.9900e+100},"regions":[{"opaque":1.2300e-7,"params":{"color":123,"color2":456,"speed":37,"randomColor":false,"playbackStart":"random","playbackEnd":"never","cycles":-1,"foreign":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
fn keys(cols: &[u16]) -> Vec<LedSelection> {
    cols.iter()
        .map(|&col| LedSelection {
            device: "k".into(),
            row: 0,
            col,
        })
        .collect()
}
fn value(d: &Draft, cols: &[u16], p: TidalProperty) -> SelectedTidalValue {
    d.tidal_selection_value("t", &keys(cols), p).unwrap()
}
#[test]
fn tidal_new_membership_uses_source_defaults_and_keeps_layout_layer_wide() {
    let mut d = Draft::open(PROFILE).unwrap();
    d.set_tidal_property("t", &keys(&[3]), TidalProperty::Speed, Some(0))
        .unwrap();
    let raw: serde_json::Value = serde_json::from_str(d.json()).unwrap();
    let params = &raw["layers"][0]["regions"][1]["params"];
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-tidal-converter.json"
    ))
    .unwrap();
    for (k, v) in fixture["ui_defaults"].as_object().unwrap() {
        if k != "speed" {
            assert_eq!(&params[k], v, "{k}");
        }
    }
    assert_eq!(
        raw["layers"][0]["params"]["centerPointX"],
        fixture["ui_layout"]["centerPointX"]
    );

    for (k,v) in serde_json::json!({"color":65280,"color2":255,"randomColor":false,"speed":0,"angle":160,"split":true,"playbackStart":"random","playbackEnd":"never","cycles":-1}).as_object().unwrap() {assert_eq!(&params[k],v,"{k}");}
    assert!(params.get("centerPointX").is_none());
    assert!(params.get("counterclockwise").is_none());
    assert_eq!(
        value(&d, &[1, 3], TidalProperty::Speed),
        SelectedTidalValue::Mixed
    );
    assert!(d.json().contains("9.9900e+100"));
    assert!(d.json().contains("9007199254740993123456789"));
}
#[test]
fn tidal_selection_partition_random_none_and_history_are_atomic() {
    let mut d = Draft::open(PROFILE).unwrap();
    let snapshot = d.read_snapshot();
    d.set_tidal_property("t", &keys(&[1]), TidalProperty::Color, None)
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Color),
        SelectedTidalValue::Uniform(Some(TIDAL_NONE))
    );
    assert_eq!(
        value(&d, &[2], TidalProperty::Color),
        SelectedTidalValue::Uniform(Some(123))
    );
    assert!(d.json().contains("1.2300e-7"));
    d.set_tidal_property("t", &keys(&[1]), TidalProperty::Random, Some(1))
        .unwrap();
    let before = d.json().to_owned();
    assert!(d
        .set_tidal_property("t", &keys(&[1]), TidalProperty::Color2, Some(2))
        .is_err());
    assert_eq!(d.json(), before);
    d.undo().unwrap();
    d.undo().unwrap();
    assert!(!d.is_dirty());
    assert_eq!(d.json(), PROFILE);
    assert!(!d.matches_snapshot(&snapshot));
    d.redo().unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Color),
        SelectedTidalValue::Uniform(Some(TIDAL_NONE))
    );
}
#[test]
fn tidal_layout_conversion_rounds_at_half_grid_and_save_tracks_params() {
    let mut d = Draft::open(PROFILE).unwrap();
    let before = d.checkpoint();
    d.set_tidal_layout(
        "t",
        TidalLayout {
            x: 20,
            y: 19,
            inward: true,
        },
    )
    .unwrap();
    assert!(d.is_dirty());
    let raw: serde_json::Value = serde_json::from_str(d.json()).unwrap();
    let p = &raw["layers"][0]["params"];
    assert_eq!(p["centerPointX"], 1);
    assert_eq!(p["centerPointY"], -2);
    assert_eq!(p["counterclockwise"], true);
    assert!(d.json().contains("9.9900e+100"));
    assert!(d.json().contains("9007199254740993123456789"));
    d.accept_saved(before).unwrap();
    assert!(d.is_dirty());
    let saved = d.checkpoint();
    d.accept_saved(saved).unwrap();
    assert!(!d.is_dirty());
    d.undo().unwrap();
    assert!(d.is_dirty());
    d.redo().unwrap();
    assert!(!d.is_dirty());
    let stable = d.json().to_owned();
    assert!(d
        .set_tidal_layout(
            "t",
            TidalLayout {
                x: 10001,
                y: 0,
                inward: false
            }
        )
        .is_err());
    assert_eq!(d.json(), stable);
}
#[test]
fn tidal_playback_controls_apply_source_coupled_transitions_without_starting_inputs() {
    let mut d = Draft::open(PROFILE).unwrap();
    let sel = keys(&[1]);
    d.set_tidal_property("t", &sel, TidalProperty::Start, Some(2))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::End),
        SelectedTidalValue::Uniform(Some(1))
    );
    assert_eq!(
        value(&d, &[1], TidalProperty::Cycles),
        SelectedTidalValue::Uniform(Some(1))
    );
    d.set_tidal_property("t", &sel, TidalProperty::Cycles, Some(100))
        .unwrap();
    d.set_tidal_property("t", &sel, TidalProperty::End, Some(2))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Cycles),
        SelectedTidalValue::Uniform(Some(0))
    );
    d.set_tidal_property("t", &sel, TidalProperty::Start, Some(0))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::End),
        SelectedTidalValue::Uniform(Some(0))
    );
    let before = d.json().to_owned();
    assert!(d
        .set_tidal_property("t", &sel, TidalProperty::End, Some(2))
        .is_err());
    assert!(d
        .set_tidal_property("t", &sel, TidalProperty::Cycles, Some(2))
        .is_err());
    assert_eq!(d.json(), before);
}
#[test]
fn tidal_invalid_unknown_membership_conflicting_id_and_overlap_do_not_mutate() {
    let mut d = Draft::open(PROFILE).unwrap();
    for (property, v) in [
        (TidalProperty::Speed, 51),
        (TidalProperty::Color, 0x1000000),
        (TidalProperty::Start, 3),
        (TidalProperty::Cycles, 0),
        (TidalProperty::Random, 2),
    ] {
        assert!(d
            .set_tidal_property("t", &keys(&[1]), property, Some(v))
            .is_err());
        assert_eq!(d.json(), PROFILE);
    }
    assert!(d
        .set_tidal_property("t", &keys(&[99]), TidalProperty::Speed, Some(3))
        .is_err());
    let bad = PROFILE.replace("\"value\":19", "\"value\":2");
    let mut d = Draft::open(&bad).unwrap();
    assert!(d
        .set_tidal_property("t", &keys(&[1]), TidalProperty::Speed, Some(3))
        .is_err());
    assert_eq!(d.json(), bad);
    let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    let region = raw["layers"][0]["regions"][0].clone();
    raw["layers"][0]["regions"]
        .as_array_mut()
        .unwrap()
        .push(region);
    let raw = raw.to_string();
    let mut d = Draft::open(&raw).unwrap();
    assert!(d
        .set_tidal_property("t", &keys(&[1]), TidalProperty::Speed, Some(3))
        .is_err());
    assert_eq!(d.json(), raw);
}
#[test]
fn tidal_imported_gradient_preserved_and_picker_edit_refuses() {
    let raw=PROFILE.replace("\"speed\":37","\"colorStops\":[{\"Stop\":80,\"Color\":1},{\"Stop\":20,\"future\":1.2300e-7}],\"speed\":37");
    let mut d = Draft::open(&raw).unwrap();
    assert!(d
        .set_tidal_property("t", &keys(&[1]), TidalProperty::Color, Some(5))
        .is_err());
    assert_eq!(d.json(), raw);
    d.set_tidal_property("t", &keys(&[1]), TidalProperty::Speed, Some(50))
        .unwrap();
    assert!(d.json().contains("1.2300e-7"));
    let descriptor = serde_json::to_value(&d.tidal_region_descriptors("t").unwrap()[0]).unwrap();
    assert_eq!(descriptor["ColorStops"][0]["Stop"], 0);
    assert_eq!(descriptor["ColorStops"][1]["Stop"], 80);
    assert_eq!(descriptor["ColorStops"][2]["Stop"], 20);
    assert_eq!(descriptor["ColorStops"][2]["Color"], TIDAL_NONE);
}
#[test]
fn tidal_descriptors_match_isolated_shipped_converter_replay() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-tidal-converter.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        raw["layers"][0]["params"] = case["input"].clone();
        raw["layers"][0]["regions"][0]["params"] = serde_json::json!({});
        let d = Draft::open(&raw.to_string()).unwrap();
        let before = d.json().to_owned();
        let descriptor = d.tidal_region_descriptors("t").unwrap();
        assert_eq!(
            serde_json::to_value(&descriptor[0]).unwrap(),
            case["output"]["params"]
        );
        assert_eq!(d.json(), before);
        assert_eq!(d.undo_len(), 0);
    }
}
#[test]
fn tidal_preview_refuses_instead_of_substituting_wave() {
    let fixture = include_str!("../../tests/fixtures/studio-preview-profile.json");
    let mut raw: serde_json::Value = serde_json::from_str(fixture).unwrap();
    raw["layers"][0]["name"] = "tidal".into();
    raw["layers"][0]["value"] = 19.into();
    raw["layers"][0]["visible"] = true.into();
    let d = Draft::open(&raw.to_string()).unwrap();
    let before = d.json().to_owned();
    assert!(Preview::from_snapshot(&d.read_snapshot(), 1).is_err());
    assert_eq!(d.json(), before);
}

#[test]
fn tidal_direction_does_not_rewrite_imported_centers_and_active_start_does_not_reset_cycles() {
    let raw = PROFILE.replace("\"centerPointX\":125", "\"centerPointX\":1.2500e+2");
    let mut d = Draft::open(&raw).unwrap();
    d.set_tidal_direction("t", true).unwrap();
    assert!(d.json().contains("1.2500e+2"));
    d.set_tidal_property("t", &keys(&[1]), TidalProperty::Start, Some(1))
        .unwrap();
    d.set_tidal_property("t", &keys(&[1]), TidalProperty::Cycles, Some(7))
        .unwrap();
    let before = d.json().to_owned();
    assert!(!d
        .set_tidal_property("t", &keys(&[1]), TidalProperty::Start, Some(1))
        .unwrap());
    assert_eq!(d.json(), before);
}

#[test]
fn tidal_playback_remembers_last_cycle_field_edit_as_source_widget_does() {
    let mut d = Draft::open(PROFILE).unwrap();
    let selection = keys(&[1]);
    d.set_tidal_playback_mode("t", &selection, TidalProperty::Start, 1, Some(7))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Cycles),
        SelectedTidalValue::Uniform(Some(7))
    );
    d.set_tidal_playback_mode("t", &selection, TidalProperty::End, 2, Some(7))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Cycles),
        SelectedTidalValue::Uniform(Some(0))
    );
    d.set_tidal_playback_mode("t", &selection, TidalProperty::End, 1, Some(7))
        .unwrap();
    assert_eq!(
        value(&d, &[1], TidalProperty::Cycles),
        SelectedTidalValue::Uniform(Some(7))
    );
    let before = d.json().to_owned();
    assert!(d
        .set_tidal_playback_mode("t", &selection, TidalProperty::Start, 1, Some(101))
        .is_err());
    assert_eq!(d.json(), before);
}

#[test]
fn tidal_descriptor_limits_refuse_without_mutation() {
    let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
    raw["layers"][0]["regions"][0]["params"]["colorStops"] =
        serde_json::json!(vec![serde_json::json!({"Stop":1,"Color":1}); 11]);
    let d = Draft::open(&raw.to_string()).unwrap();
    let before = d.json().to_owned();
    assert!(d.tidal_region_descriptors("t").is_err());
    assert_eq!(d.json(), before);
    assert_eq!(d.undo_len(), 0);
}
