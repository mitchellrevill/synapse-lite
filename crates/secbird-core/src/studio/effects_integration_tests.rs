//! Consolidation checks: independently implemented effects share one compositor.
use super::*;
use serde_json::{json, Value};

fn scene() -> Value {
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    let prototype = profile["layers"][0].clone();
    let make = |name: &str, value: u16, params: Value| {
        let mut layer = prototype.clone();
        layer["id"] = json!(name);
        layer["name"] = json!(name);
        layer["value"] = json!(value);
        layer["params"] = json!({});
        layer["regions"][0]["params"] = params;
        layer
    };
    let mut wheel = make(
        "wheel",
        13,
        json!({"speed":60,"cycles":-1,"colorStops":[{"Stop":0,"Color":0},{"Stop":100,"Color":0xffffff}]}),
    );
    wheel["params"] = json!({"centerPointX":125,"centerPointY":123,"centerPointX2":5000,"centerPointY2":5000,"counterclockwise":false});
    let leds = wheel["regions"][0]["devices"][0]["leds"]
        .as_array()
        .unwrap()
        .clone();
    wheel["regions"][0]["devices"][0]["leds"] = Value::Array(leds.into_iter().step_by(3).collect());
    let breathing = make(
        "breathing",
        5,
        json!({"randomColor":true,"duration":500,"cycles":2,"playbackStart":"random","playbackEnd":"after"}),
    );
    let spectrum = make(
        "spectrum",
        4,
        json!({"duration":18870,"colorStops":[{"Stop":0,"Color":0xff0000},{"Stop":100,"Color":0x00ff00}]}),
    );
    let fire = make("fire", 8, json!({"color":0xff4400,"color2":0x112233}));
    profile["layers"] = json!([wheel, breathing, spectrum, fire]);
    profile
}

#[test]
fn integrated_effects_keep_explicit_clock_origin_random_ownership_and_snapshot() {
    let raw = scene().to_string();
    let draft = Draft::open(&raw).unwrap();
    let snapshot = draft.read_snapshot();
    let mut first = Preview::from_snapshot_at(&snapshot, 123, 1700000000).unwrap();
    let mut second = Preview::from_snapshot_at(&snapshot, 123, 1700000000).unwrap();
    assert_eq!(first.effect_count(), 4);
    let mut fire_only = scene();
    fire_only["layers"] = json!([fire_only["layers"][3].clone()]);
    let fire = Preview::from_snapshot_at(
        &Draft::open(&fire_only.to_string()).unwrap().read_snapshot(),
        123,
        1700000000,
    )
    .unwrap();
    assert_eq!(
        first.final_random_state(),
        fire.final_random_state(),
        "Breathing must not consume Fire's shared CRT state"
    );
    let initial = first.frame().to_vec();
    let mut changed = false;
    for _ in 0..300 {
        assert_eq!(first.frame(), second.frame());
        changed |= first.frame() != initial;
        first.advance().unwrap();
        second.advance().unwrap();
    }
    assert!(changed);
    assert!(draft.matches_snapshot(&snapshot));
    assert_eq!(draft.json(), raw);
    assert!(!draft.is_dirty());
}

#[test]
fn integrated_effects_preserve_tidal_input_and_ambient_admission_gates() {
    let mut profile = scene();
    let mut gated = profile["layers"][0].clone();
    gated["id"] = json!("tidal");
    gated["name"] = json!("tidal");
    gated["value"] = json!(19);
    gated["visible"] = json!(false);
    profile["layers"].as_array_mut().unwrap().insert(0, gated);
    let preview = |profile: &Value| {
        Preview::from_snapshot_at(
            &Draft::open(&profile.to_string()).unwrap().read_snapshot(),
            123,
            1700000000,
        )
    };
    assert!(preview(&profile).is_ok());
    profile["layers"][0]["visible"] = json!(true);
    assert!(preview(&profile)
        .err()
        .unwrap()
        .to_string()
        .contains("Tidal"));
    profile["layers"][0]["name"] = json!("ripple");
    profile["layers"][0]["value"] = json!(1);
    assert!(preview(&profile)
        .err()
        .unwrap()
        .to_string()
        .contains("ripple"));
    profile["layers"][0]["name"] = json!("ambient");
    profile["layers"][0]["value"] = json!(11);
    profile["layers"][0]["params"] = json!({});
    profile["layers"][0]["regions"][0]["params"] = json!({"screen":"full","blur":1});
    assert!(preview(&profile)
        .err()
        .unwrap()
        .to_string()
        .contains("Ambient"));
    let diagnostic = Draft::open(&profile.to_string()).unwrap();
    assert_eq!(
        Preview::from_snapshot_synthetic_ambient(&diagnostic.read_snapshot(), 123)
            .unwrap()
            .effect_count(),
        5
    );
    profile["layers"][0]["visible"] = json!(false);
    profile["layers"][2]["regions"][0]["params"]["playbackStart"] = json!("onPress");
    assert!(preview(&profile)
        .err()
        .unwrap()
        .to_string()
        .contains("input ownership"));
}
