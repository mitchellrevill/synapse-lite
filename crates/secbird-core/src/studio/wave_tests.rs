use super::*;
use serde_json::{json, Value};
fn scene() -> Value {
    let mut raw: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    let mut wave = raw["layers"][0].clone();
    wave["id"] = "wave".into();
    wave["name"] = "wave".into();
    wave["value"] = 2.into();
    wave["params"] = json!({"speed":15,"width":125,"pause":120,"cycles":-1,"angle":90,"playbackStart":"random","playbackEnd":"never"});
    wave["regions"][0]["params"] = json!({"angle":270,"colorStops":[{"Stop":0,"Color":0xf01020},{"Stop":33},{"Stop":66,"Color":0x2080ff},{"Stop":100,"Color":0xf01020}]});
    let mut base = wave.clone();
    base["id"] = "base".into();
    base["name"] = "static".into();
    base["value"] = 6.into();
    base["params"] = json!({});
    base["regions"][0]["params"] = json!({"color":0x0410f0});
    raw["layers"] = json!([wave, base]);
    raw
}
#[test]
fn wave_composition_preserves_snapshot_and_matches200_independent_frames() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-wave-reference.json"
    ))
    .unwrap();
    let draft = Draft::open(&scene().to_string()).unwrap();
    let before = draft.json().to_owned();
    let mut preview = Preview::from_snapshot(&draft.read_snapshot(), 123).unwrap();
    let mut hash = md5::Context::new();
    for _ in 0..200 {
        for c in preview.frame() {
            hash.consume([c.0, c.1, c.2]);
        }
        preview.advance().unwrap();
    }
    assert_eq!(
        format!("{:x}", hash.compute()),
        reference["composition"]["logical_rgb_md5"]
            .as_str()
            .unwrap()
    );
    assert_eq!(preview.final_random_state(), 123);
    assert_eq!(draft.json(), before);
    assert!(!draft.is_dirty());
    assert!(preview.storage_bytes() < 100_000);
}
#[test]
fn unsupported_wave_modes_refuse_without_mutating_the_file() {
    for (key, value) in [
        ("cycles", json!(0)),
        ("cycles", json!(2)),
        ("split", json!(true)),
        ("counterclockwise", json!(false)),
        ("angle", json!(45)),
        ("speed", json!(51)),
        ("width", json!(99)),
        ("playbackStart", json!("onPress")),
        ("playbackEnd", json!("toggle")),
    ] {
        let mut raw = scene();
        raw["layers"][0]["regions"][0]["params"][key] = value;
        let draft = Draft::open(&raw.to_string()).unwrap();
        let before = draft.json().to_owned();
        assert!(
            Preview::from_snapshot(&draft.read_snapshot(), 1).is_err(),
            "{key}"
        );
        assert_eq!(draft.json(), before);
        assert!(!draft.is_dirty());
    }
    let mut raw = scene();
    raw["layers"][0]["params"]
        .as_object_mut()
        .unwrap()
        .remove("speed");
    let draft = Draft::open(&raw.to_string()).unwrap();
    assert!(Preview::from_snapshot(&draft.read_snapshot(), 1).is_err());
}
