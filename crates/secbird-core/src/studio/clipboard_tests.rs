use super::*;
const PROFILE: &str = r#"{"id":"p","title":"Desk","devices":[{"id":"kbd","productId":605,"ledRef":[{"row":0,"col":1,"future":9007199254740993123456789},{"row":0,"col":2},{"row":0,"col":3},{"row":0,"col":4}]},{"id":"mouse","productId":196,"ledRef":[{"row":0,"col":1}]}],"layers":[{"id":"g","title":"Group","type":"group","layers":[{"id":"a","groupId":"g","type":"layer","title":"FIRE","name":"fire","value":8,"params":{"futureBase":1.2300e-7},"regions":[{"params":{"color":1,"color2":2,"future":9.9900e+100},"opaqueRegion":9007199254740993123456789,"devices":[{"id":"kbd","opaqueDevice":1.2300e-7,"leds":[{"row":0,"col":1,"opaqueLed":9007199254740993123456789},{"row":0,"col":2}]}]},{"params":{"color":3,"color2":4,"future":9.9900e+100},"opaqueRegion":9007199254740993123456789,"devices":[{"id":"kbd","leds":[{"row":0,"col":3}]}]}]}]}],"unknown":9.9900e+100}"#;
fn keys(cols: &[u16]) -> Vec<LedSelection> {
    cols.iter()
        .map(|col| LedSelection {
            device: "kbd".into(),
            row: 0,
            col: *col,
        })
        .collect()
}
#[test]
fn property_clipboard_mixed_copy_is_read_only_and_single_device_only() {
    let draft = Draft::open(PROFILE).unwrap();
    assert!(draft.copy_selection_properties("a", &keys(&[1, 2])).is_ok());
    assert!(draft
        .copy_selection_properties("a", &keys(&[1, 3]))
        .is_err());
    assert!(draft
        .copy_selection_properties("a", &keys(&[1, 4]))
        .is_err());
    assert!(draft.copy_selection_properties("a", &[]).is_err());
    assert!(draft.copy_selection_properties("a", &keys(&[99])).is_err());
    let mut multi = keys(&[1]);
    multi.push(LedSelection {
        device: "mouse".into(),
        row: 0,
        col: 1,
    });
    assert!(draft.copy_selection_properties("a", &multi).is_err());
    assert_eq!(draft.json(), PROFILE);
    assert_eq!(draft.undo_len(), 0);
}
#[test]
fn property_clipboard_paste_mixed_and_unassigned_preserves_metadata_and_undo() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let clip = draft.copy_selection_properties("a", &keys(&[1])).unwrap();
    let mut destination = keys(&[3, 4]);
    destination.push(LedSelection {
        device: "mouse".into(),
        row: 0,
        col: 1,
    });
    assert!(draft
        .paste_selection_properties("a", &destination, &clip)
        .unwrap());
    assert_eq!(draft.undo_len(), 1);
    assert!(draft.is_dirty());
    for key in &destination {
        assert_eq!(
            draft
                .selection_color("a", &[key.clone()], ColorProperty::FireHot)
                .unwrap(),
            SelectedColor::Uniform(Some(1))
        );
    }
    let edited = draft.json().to_owned();
    for literal in [
        "9.9900e+100",
        "1.2300e-7",
        "9007199254740993123456789",
        "opaqueRegion",
        "opaqueDevice",
        "opaqueLed",
    ] {
        assert!(edited.contains(literal), "{literal}");
    }
    draft.undo().unwrap();
    assert_eq!(draft.json(), PROFILE);
    assert!(!draft.is_dirty());
    draft.redo().unwrap();
    assert_eq!(draft.json(), edited);
    let after = draft.json().to_owned();
    assert!(draft
        .paste_selection_properties("g", &keys(&[1]), &clip)
        .is_err());
    assert_eq!(draft.json(), after);
    let mut other = Draft::open(PROFILE).unwrap();
    assert!(other
        .paste_selection_properties("a", &keys(&[1]), &clip)
        .is_err());
}
#[test]
fn property_clipboard_reset_merges_defaults_remove_keeps_other_memberships_and_history() {
    let mut draft = Draft::open(PROFILE).unwrap();
    assert!(draft
        .reset_selection_properties("a", &keys(&[1, 3, 4]))
        .unwrap());
    assert_eq!(
        draft
            .selection_color("a", &keys(&[1, 3, 4]), ColorProperty::FireHot)
            .unwrap(),
        SelectedColor::Uniform(Some(0xff0000))
    );
    assert_eq!(
        draft
            .selection_color("a", &keys(&[2]), ColorProperty::FireHot)
            .unwrap(),
        SelectedColor::Uniform(Some(1))
    );
    assert!(draft.json().contains("9.9900e+100"));
    draft.undo().unwrap();
    assert_eq!(draft.json(), PROFILE);
    draft.remove_selection_effect("a", &keys(&[1, 4])).unwrap();
    assert_eq!(draft.undo_len(), 1);
    assert_eq!(draft.redo_len(), 0);
    assert_eq!(
        draft
            .selection_color("a", &keys(&[2]), ColorProperty::FireHot)
            .unwrap(),
        SelectedColor::Uniform(Some(1))
    );
    let after = draft.json().to_owned();
    draft.undo().unwrap();
    assert_eq!(draft.json(), PROFILE);
    draft.redo().unwrap();
    assert_eq!(draft.json(), after);
}
#[test]
fn property_clipboard_unknown_effects_conflicts_overlap_and_atomic_refusal() {
    for raw in [
        PROFILE.replace("\"name\":\"fire\"", "\"name\":\"future\""),
        PROFILE.replace("\"value\":8", "\"value\":99"),
        PROFILE.replace("\"col\":3}]}", "\"col\":1}]}"),
    ] {
        let mut draft = Draft::open(&raw).unwrap();
        assert!(draft.copy_selection_properties("a", &keys(&[1])).is_err());
        let before = draft.json().to_owned();
        assert!(draft.reset_selection_properties("a", &keys(&[99])).is_err());
        assert_eq!(draft.json(), before);
        assert_eq!(draft.undo_len(), 0);
    }
}
#[test]
fn property_clipboard_none_color_clears_known_field_but_retains_target_future() {
    let raw = PROFILE.replace("\"color\":1,\"color2\":2,", "");
    let mut draft = Draft::open(&raw).unwrap();
    let clip = draft.copy_selection_properties("a", &keys(&[1])).unwrap();
    draft
        .paste_selection_properties("a", &keys(&[3]), &clip)
        .unwrap();
    assert_eq!(
        draft
            .selection_color("a", &keys(&[3]), ColorProperty::FireHot)
            .unwrap(),
        SelectedColor::Uniform(None)
    );
    assert!(draft.json().contains("9.9900e+100"));
}
#[test]
fn property_clipboard_all_recovered_resets_have_defaults_and_retained_layer_base() {
    for (name, value) in [
        ("ripple", 1),
        ("wave", 2),
        ("reactive", 3),
        ("spectrum", 4),
        ("breathing", 5),
        ("static", 6),
        ("starlight", 7),
        ("fire", 8),
        ("ambient", 11),
        ("audio", 12),
        ("wheel", 13),
        ("chroma-generate", 18),
        ("tidal", 19),
    ] {
        let raw = PROFILE.replace(
            "\"name\":\"fire\",\"value\":8",
            &format!("\"name\":\"{name}\",\"value\":{value}"),
        );
        let mut draft = Draft::open(&raw).unwrap();
        draft
            .reset_selection_properties("a", &keys(&[1, 4]))
            .unwrap();
        assert!(draft.json().contains("\"futureBase\":1.2300e-7"));
        draft.undo().unwrap();
        assert_eq!(draft.json(), raw);
    }
}

#[test]
fn property_clipboard_layer_wide_params_participate_in_dirty_state_and_history() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let clip = draft.copy_selection_properties("a", &keys(&[1])).unwrap();
    let original = item(&draft.current, "a").unwrap().unwrap();
    let mut base = Object::parse(original.get("params").unwrap()).unwrap();
    base.set("centerPointX", Some(&encode(&42).unwrap()));
    draft
        .commit(vec![Change {
            id: "a".into(),
            field: "params",
            before: original.get("params").map(RawValue::to_owned),
            after: encode(&base).unwrap(),
        }])
        .unwrap();
    assert!(draft.is_dirty());
    let checkpoint = draft.checkpoint();
    draft.accept_saved(checkpoint).unwrap();
    assert!(!draft.is_dirty());
    draft
        .paste_selection_properties("a", &keys(&[1]), &clip)
        .unwrap();
    assert!(draft.is_dirty());
    assert!(!draft.json().contains("centerPointX"));
    draft.undo().unwrap();
    assert!(!draft.is_dirty());
    assert!(draft.json().contains("centerPointX"));
}
