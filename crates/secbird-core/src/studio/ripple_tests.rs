use super::*;
use serde_json::{json, Value};

const PROFILE: &str = r#"{"id":"p","title":"Ripple","devices":[{"id":"k","leds":[{"row":0,"col":1,"inputType":"kbd","inputData":"[30,0]"},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"r","type":"layer","name":"ripple","value":1,"title":"RIPPLE","params":{"speed":10,"width":300,"playbackStart":"onPress","playbackEnd":"after","cycles":1,"colorStops":[{"Stop":0,"Color":1,"opaque":1.2300e-7},{"Stop":100,"Color":2}],"future":9.9900e+100},"regions":[{"params":{"width":200,"opaque":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;

fn keys(cols: &[u16]) -> Vec<LedSelection> {
    cols.iter()
        .map(|&col| LedSelection {
            device: "k".into(),
            row: 0,
            col,
        })
        .collect()
}
fn params(draft: &Draft, col: u16) -> Value {
    let saved: Value = serde_json::from_str(draft.json()).unwrap();
    saved["layers"][0]["regions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["devices"][0]["leds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["col"] == col)
        })
        .unwrap()["params"]
        .clone()
}
fn playback(start: RippleStart, end: RippleEnd, cycles: i32) -> RipplePlayback {
    RipplePlayback { start, end, cycles }
}

#[test]
fn ripple_defaults_and_patterns_match_independent_shipped_literals() {
    let source: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-ripple-contract.json"
    ))
    .unwrap();
    let mut raw: Value = serde_json::from_str(PROFILE).unwrap();
    raw["layers"][0]["params"] = json!({});
    let mut draft = Draft::open(&raw.to_string()).unwrap();
    draft
        .set_ripple_property("r", &keys(&[3]), RippleProperty::Speed, 10)
        .unwrap();
    assert_eq!(params(&draft, 3), source["defaults"]);
    for (index, expected) in source["presets"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            serde_json::to_value(ripple_preset(index).unwrap()).unwrap(),
            *expected
        );
    }
    assert!(ripple_preset(5).is_none());
    for speed in 0..=51 {
        assert_eq!(
            RippleProperty::Speed.validate(speed).is_ok(),
            (1..=50).contains(&speed)
        );
    }
    for width in 0..=401 {
        assert_eq!(
            RippleProperty::Width.validate(width).is_ok(),
            [100, 200, 300, 400].contains(&width)
        );
    }
    assert_eq!(source["speed"], json!({"min":1,"max":50,"default":10}));
    assert_eq!(
        source["width"],
        json!({"min":100,"max":400,"step":100,"default":300})
    );
    assert_eq!(source["repeat"], json!({"min":1,"max":100}));
}

#[test]
fn ripple_partition_and_atomic_history_preserve_raw_inheritance_and_partner_led() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let before = draft.json().to_owned();
    draft
        .set_ripple_motion("r", &keys(&[1, 3]), 50, 400)
        .unwrap();
    assert_eq!(params(&draft, 1)["speed"], 50);
    assert_eq!(params(&draft, 1)["width"], 400);
    // Existing regions inherit layer fields without materializing defaults.
    assert!(params(&draft, 1).get("playbackStart").is_none());
    assert_eq!(params(&draft, 2)["width"], 200);
    assert!(params(&draft, 2).get("speed").is_none());
    assert_eq!(params(&draft, 3)["playbackStart"], "onPress");
    assert!(draft.json().contains("9.9900e+100"));
    assert!(draft.json().contains("1.2300e-7"));
    assert!(draft.json().contains("9007199254740993123456789"));
    let after = draft.json().to_owned();
    assert!(draft.undo().unwrap());
    assert_eq!(draft.json(), before);
    assert!(draft.redo().unwrap());
    assert_eq!(draft.json(), after);
    assert_eq!(
        draft
            .ripple_selection_value("r", &keys(&[1, 2]), RippleProperty::Width)
            .unwrap(),
        SelectedRippleValue::Mixed
    );
}

#[test]
fn ripple_playback_is_one_transaction_and_validates_every_mode_combination() {
    for start in [
        RippleStart::Random,
        RippleStart::OnPress,
        RippleStart::OnSelectedKeys,
    ] {
        for end in [RippleEnd::Never, RippleEnd::After, RippleEnd::Toggle] {
            for cycles in [-2, -1, 0, 1, 2, 100, 101] {
                let mut draft = Draft::open(PROFILE).unwrap();
                let before = draft.json().to_owned();
                let value = playback(start, end, cycles);
                let valid = !(start == RippleStart::Random && end == RippleEnd::Toggle)
                    && if end == RippleEnd::After {
                        (1..=100).contains(&cycles)
                    } else {
                        cycles == -1
                    };
                let result = draft.set_ripple_playback("r", &keys(&[1]), value);
                assert_eq!(result.is_ok(), valid, "{value:?}");
                if !valid {
                    assert_eq!(draft.json(), before);
                    continue;
                }
                assert_eq!(
                    draft.ripple_selection_playback("r", &keys(&[1])).unwrap(),
                    SelectedRipplePlayback::Uniform(Some(value))
                );
                assert_eq!(params(&draft, 1)["cycles"], cycles);
                assert!(params(&draft, 2).get("cycles").is_none());
                draft.undo().unwrap();
                assert_eq!(draft.json(), before);
            }
        }
    }
}

#[test]
fn ripple_gradient_edits_preserve_opaque_stops_and_support_none_and_history() {
    let mut draft = Draft::open(PROFILE).unwrap();
    draft
        .edit_ripple_stop(
            "r",
            &keys(&[1]),
            StopEdit::Set {
                index: 1,
                position: 100,
                color: None,
            },
        )
        .unwrap();
    let gradient = params(&draft, 1)["colorStops"].clone();
    assert!(gradient[1].get("Color").is_none());
    assert!(draft.json().contains("1.2300e-7"));
    assert_eq!(params(&draft, 1)["colorStopsCustom"], true);
    assert!(params(&draft, 2).get("colorStops").is_none());
    draft
        .edit_ripple_stop(
            "r",
            &keys(&[1]),
            StopEdit::Insert {
                position: 50,
                color: Some(0xff00ff),
            },
        )
        .unwrap();
    assert_eq!(params(&draft, 1)["colorStops"].as_array().unwrap().len(), 3);
    draft
        .edit_ripple_stop("r", &keys(&[1]), StopEdit::Delete { index: 1 })
        .unwrap();
    assert_eq!(params(&draft, 1)["colorStops"], gradient);
    draft.set_ripple_preset("r", &keys(&[1]), 4).unwrap();
    assert_eq!(params(&draft, 1)["colorStopsCustom"], false);
    assert_eq!(
        params(&draft, 1)["colorStops"],
        json!([{"Stop":0,"Color":65280},{"Stop":100,"Color":65280}])
    );
}

#[test]
fn ripple_rejects_invalid_edits_atomically_and_does_not_guess_imported_modes() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let before = draft.json().to_owned();
    assert!(draft.set_ripple_motion("r", &keys(&[1]), 50, 99).is_err());
    assert!(draft.set_ripple_motion("r", &keys(&[1]), 0, 300).is_err());
    assert!(draft.set_ripple_preset("r", &keys(&[1]), 5).is_err());
    assert!(draft.set_ripple_motion("r", &keys(&[99]), 1, 100).is_err());
    assert!(draft
        .edit_ripple_stop(
            "r",
            &keys(&[1]),
            StopEdit::Set {
                index: 0,
                position: 0,
                color: Some(0x1000000)
            }
        )
        .is_err());
    assert_eq!(draft.json(), before);
    assert!(!draft.is_dirty());
    assert!(!draft.set_ripple_motion("r", &[], 1, 100).unwrap());
    assert_eq!(
        draft
            .ripple_selection_value("r", &[], RippleProperty::Speed)
            .unwrap(),
        SelectedRippleValue::Empty
    );
    let unknown = PROFILE.replace("\"onPress\"", "\"future-trigger\"");
    let mut draft = Draft::open(&unknown).unwrap();
    draft.set_ripple_motion("r", &keys(&[1]), 1, 100).unwrap();
    assert!(draft.json().contains("future-trigger"));
    assert!(draft.ripple_selection_playback("r", &keys(&[1])).is_err());
}

#[test]
fn ripple_conflicting_identity_and_overlapping_memberships_refuse() {
    for broken in [
        PROFILE.replace("\"value\":1", "\"value\":3"),
        PROFILE.replace("\"name\":\"ripple\"", "\"name\":\"wave\""),
    ] {
        let mut draft = Draft::open(&broken).unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_ripple_motion("r", &keys(&[1]), 1, 100).is_err());
        assert_eq!(draft.json(), before);
    }
    let mut raw: Value = serde_json::from_str(PROFILE).unwrap();
    let duplicate = raw["layers"][0]["regions"][0].clone();
    raw["layers"][0]["regions"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    let mut draft = Draft::open(&raw.to_string()).unwrap();
    let before = draft.json().to_owned();
    assert!(draft.set_ripple_motion("r", &keys(&[1]), 1, 100).is_err());
    assert_eq!(draft.json(), before);
}

#[test]
fn ripple_add_assign_and_snapshot_keep_ordinary_runtime_closed() {
    let mut raw: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    raw["layers"] = json!([]);
    let mut draft = Draft::open(&raw.to_string()).unwrap();
    draft.add_effect("abcdef123456", NewEffect::Ripple).unwrap();
    let root: Value = serde_json::from_str(draft.json()).unwrap();
    assert!(root["layers"][0]["regions"].as_array().unwrap().is_empty());
    let device = &raw["devices"][0];
    let led = &device["leds"][0];
    let selection = [LedSelection {
        device: device["id"].as_str().unwrap().into(),
        row: led["row"].as_u64().unwrap() as u16,
        col: led["col"].as_u64().unwrap() as u16,
    }];
    draft
        .set_ripple_motion("abcdef123456", &selection, 10, 300)
        .unwrap();
    let before = draft.json().to_owned();
    assert!(Preview::from_snapshot(&draft.read_snapshot(), 123).is_err());
    assert_eq!(draft.json(), before);
}

#[test]
fn ripple_archive_round_trip_keeps_other_profile_and_raw_envelope_bytes() {
    use crate::studio::Archive;
    let archive = format!(
        r#"{{"version":1,"future":9.9900e+100,"profiles":[{PROFILE},{{"id":"other","title":"Other","opaque":9007199254740993123456789}}]}}"#
    );
    let source = Archive::parse(archive.as_bytes()).unwrap();
    let other = source.profile_json(1).unwrap().to_owned();
    let mut draft = Draft::open(source.profile_json(0).unwrap()).unwrap();
    draft.set_ripple_motion("r", &keys(&[1]), 25, 100).unwrap();
    let value = playback(RippleStart::OnSelectedKeys, RippleEnd::After, 100);
    draft.set_ripple_playback("r", &keys(&[1]), value).unwrap();
    let saved = source.replace_profile_bytes(0, draft.json()).unwrap();
    let saved = Archive::parse(&saved).unwrap();
    assert_eq!(source.profile_json(0).unwrap(), PROFILE);
    assert_eq!(saved.profile_json(1).unwrap(), other);
    let reopened = Draft::open(saved.profile_json(0).unwrap()).unwrap();
    assert!(!reopened.is_dirty());
    assert_eq!(
        reopened
            .ripple_selection_playback("r", &keys(&[1]))
            .unwrap(),
        SelectedRipplePlayback::Uniform(Some(value))
    );
    assert!(String::from_utf8(saved.to_bytes().unwrap())
        .unwrap()
        .contains("9.9900e+100"));
}

#[test]
fn ripple_mixed_missing_and_gradient_limits_remain_explicit() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let mut value = playback(RippleStart::Random, RippleEnd::Never, -1);
    draft.set_ripple_playback("r", &keys(&[1]), value).unwrap();
    assert_eq!(
        draft
            .ripple_selection_playback("r", &keys(&[1, 2]))
            .unwrap(),
        SelectedRipplePlayback::Mixed
    );
    assert_eq!(
        draft.ripple_selection_playback("r", &[]).unwrap(),
        SelectedRipplePlayback::Empty
    );
    value.cycles = 0;
    let before = draft.json().to_owned();
    assert!(draft
        .set_ripple_playback("r", &keys(&[1, 2, 3]), value)
        .is_err());
    assert_eq!(draft.json(), before);
    draft.set_ripple_preset("r", &keys(&[1]), 0).unwrap();
    let before = draft.json().to_owned();
    assert!(draft
        .edit_ripple_stop(
            "r",
            &keys(&[1]),
            StopEdit::Insert {
                position: 10,
                color: None
            }
        )
        .is_err());
    assert_eq!(draft.json(), before);
    assert_eq!(
        draft
            .ripple_selection_gradient("r", &keys(&[1, 2]))
            .unwrap(),
        SelectedGradient::Mixed
    );
    assert!(draft
        .edit_ripple_stop("r", &keys(&[1, 2]), StopEdit::Delete { index: 0 })
        .is_err());
    assert_eq!(draft.json(), before);
    let mut raw: Value = serde_json::from_str(PROFILE).unwrap();
    raw["layers"][0]["params"]
        .as_object_mut()
        .unwrap()
        .remove("cycles");
    let draft = Draft::open(&raw.to_string()).unwrap();
    assert_eq!(
        draft.ripple_selection_playback("r", &keys(&[1])).unwrap(),
        SelectedRipplePlayback::Uniform(None)
    );
}
