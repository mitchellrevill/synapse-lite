use super::*;
use serde_json::{json, Value};
fn profile() -> Value {
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    let mut wheel = profile["layers"][0].clone();
    wheel["id"] = json!("wheel");
    wheel["name"] = json!("wheel");
    wheel["value"] = json!(13);
    wheel["params"] = json!({"centerPointX":125,"centerPointX2":5000,"centerPointY":123,"centerPointY2":5000,"counterclockwise":false});
    wheel["regions"][0]["params"] = json!({"speed":60,"cycles":-1,"colorStops":[{"Stop":0,"Color":0},{"Stop":100,"Color":0xffffff}]});
    let mut base = wheel.clone();
    base["id"] = json!("base");
    base["name"] = json!("static");
    base["value"] = json!(6);
    base["params"] = json!({});
    base["regions"][0]["params"] = json!({"color":0x0410f0});
    let mut points = wheel["regions"][0]["devices"][0]["leds"]
        .as_array()
        .unwrap()
        .clone();
    points.sort_by_key(|p| (p["row"].as_u64(), p["col"].as_u64()));
    wheel["regions"][0]["devices"][0]["leds"] =
        Value::Array(points.into_iter().step_by(3).collect());
    profile["layers"] = json!([wheel, base]);
    profile
}
#[test]
fn wheel_masked_composition_matches200_independent_frames_and_preserves_snapshot() {
    let raw = profile().to_string();
    let draft = Draft::open(&raw).unwrap();
    let snapshot = draft.read_snapshot();
    let mut preview = Preview::from_snapshot(&snapshot, 1234).unwrap();
    let mut digest = md5::Context::new();
    for _ in 0..200 {
        for p in preview.frame() {
            digest.consume([p.0, p.1, p.2]);
        }
        preview.advance().unwrap();
    }
    let reference: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-wheel-reference.json"
    ))
    .unwrap();
    assert_eq!(
        format!("{:x}", digest.compute()),
        reference["composition_rgb_md5"]
    );
    assert!(draft.matches_snapshot(&snapshot));
    assert_eq!(draft.json(), raw);
    assert!(!draft.is_dirty());
    assert_eq!(preview.final_random_state(), 1234);
    assert!(preview.storage_bytes() < 8000);
}
#[test]
fn wheel_center_only_dirty_save_checkpoint_and_unsupported_modes() {
    let raw = profile().to_string();
    let mut draft = Draft::open(&raw).unwrap();
    let before = draft.read_snapshot();
    let mut geometry = draft.wheel_geometry("wheel").unwrap();
    geometry.x = 4960;
    draft.set_wheel_geometry("wheel", geometry).unwrap();
    assert!(draft.is_dirty());
    let escaped = raw.replace("\"name\":\"wheel\"", r#""name":"w\u0068eel""#);
    let mut imported = Draft::open(&escaped).unwrap();
    imported.set_wheel_geometry("wheel", geometry).unwrap();
    assert!(
        imported.is_dirty(),
        "escaped source effect name lost geometry dirty tracking"
    );
    assert!(!draft.matches_snapshot(&before));
    draft.undo().unwrap();
    assert!(!draft.is_dirty());
    assert_eq!(draft.json(), raw);
    draft.redo().unwrap();
    assert!(draft.is_dirty());
    for (key, value) in [
        ("cycles", json!(2)),
        ("playbackStart", json!("onPress")),
        ("playbackEnd", json!("toggle")),
        ("speed", json!(361)),
    ] {
        let mut p = profile();
        p["layers"][0]["regions"][0]["params"][key] = value;
        let d = Draft::open(&p.to_string()).unwrap();
        let before = d.json().to_owned();
        assert!(Preview::from_snapshot(&d.read_snapshot(), 0).is_err());
        assert_eq!(d.json(), before);
        assert!(!d.is_dirty());
    }
}
