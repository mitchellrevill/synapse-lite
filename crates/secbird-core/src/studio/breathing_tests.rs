use super::*;
use serde_json::{json,Value};
fn scene() -> Value {
    let mut raw:Value=serde_json::from_str(include_str!("../../tests/fixtures/studio-preview-profile.json")).unwrap();
    let mut breathing=raw["layers"][0].clone();
    breathing["id"]="breathing".into(); breathing["name"]="breathing".into(); breathing["value"]=5.into();
    breathing["params"]=json!({}); breathing["regions"][0]["params"]=json!({"color":0x103050,"duration":6000});
    let mut base=breathing.clone(); base["id"]="base".into();base["name"]="static".into();base["value"]=6.into();base["regions"][0]["params"]=json!({"color":0x0410f0});
    raw["layers"]=json!([breathing,base]);raw
}
#[test]
fn regional_breathing_over_static_matches_independent150_logical_frames() {
    let reference:Value=serde_json::from_str(include_str!("../../tests/fixtures/studio-breathing-reference.json")).unwrap();
    let draft=Draft::open(&scene().to_string()).unwrap();
    let original=draft.json().to_owned();
    let mut preview=Preview::from_snapshot(&draft.read_snapshot(),123).unwrap();
    let mut bytes=vec![];
    for _ in 0..150 {
        for color in preview.frame() {bytes.extend([color.0,color.1,color.2]);}
        preview.advance().unwrap();
    }
    assert_eq!(format!("{:x}",md5::compute(bytes)),reference["composition"]["logical_rgb_md5"].as_str().unwrap());
    assert_eq!(preview.final_random_state(),123,"fixed Breathing must not consume Fire's random state");
    assert_eq!(draft.json(),original);
    assert!(preview.storage_bytes()<100_000);
}
#[test]
fn unsupported_breathing_modes_and_random_fallback_are_not_silently_replaced() {
    for params in [json!({"randomColor":true}),json!({"playbackStart":"onPress"}),json!({"playbackEnd":"toggle"}),json!({"cycles":2}),json!({"colorStops":[]}),json!({"colorStops":[{"Stop":0,"Color":1},{"Stop":0,"Color":2},{"Stop":100}]}),json!({"colorStops":[{"Stop":0,"Color":null},{"Stop":100,"Color":1}]})] {
        let mut raw=scene();raw["layers"][0]["regions"][0]["params"]=params;
        let draft=Draft::open(&raw.to_string()).unwrap();
        assert!(Preview::from_snapshot(&draft.read_snapshot(),1).is_err());
    }
}
#[test]
fn two_color_and_duration_edits_preserve_partner_membership_opaque_fields_and_history() {
    let raw=scene().to_string();
    let raw=format!("{{\"unknown\":9.9900e+100,{}",&raw[1..]);
    let mut draft=Draft::open(&raw).unwrap();
    let selected=[LedSelection{device:"keyboard".into(),row:0,col:1}];
    draft.set_selection_color("breathing",&selected,ColorProperty::BreathingFirst,Some(0xabcdef)).unwrap();
    draft.set_selection_color("breathing",&selected,ColorProperty::BreathingSecond,Some(0x123456)).unwrap();
    draft.set_breathing_duration("breathing",&selected,28000).unwrap();
    assert_eq!(draft.selection_color("breathing",&selected,ColorProperty::BreathingFirst).unwrap(),SelectedColor::Uniform(Some(0xabcdef)));
    assert_eq!(draft.selection_color("breathing",&selected,ColorProperty::BreathingSecond).unwrap(),SelectedColor::Uniform(Some(0x123456)));
    let edited:Value=serde_json::from_str(draft.json()).unwrap();
    assert!(edited["layers"][0]["regions"].as_array().unwrap().iter().any(|region|region["params"]["duration"]==28000));
    assert!(draft.json().contains("9.9900e+100"));
    assert!(draft.set_breathing_duration("base",&selected,6000).is_err());
    assert!(draft.set_breathing_duration("breathing",&selected,7000).is_err(),"Quick7s is not the Studio fast6s authoring preset");
    for _ in 0..3 {assert!(draft.undo().unwrap());}
    assert!(!draft.is_dirty());
    for _ in 0..3 {assert!(draft.redo().unwrap());}
    assert!(draft.json().contains("9.9900e+100"));
    assert!(Preview::from_snapshot(&draft.read_snapshot(),1).is_ok());
}
#[test]
fn colour_buttons_never_silently_replace_an_imported_gradient_or_random_mode() {
    for params in [json!({"colorStops":[{"Stop":0,"Color":1},{"Stop":100,"Color":2}]}),json!({"randomColor":true})] {
        let mut raw=scene();raw["layers"][0]["regions"][0]["params"]=params;
        let mut draft=Draft::open(&raw.to_string()).unwrap();let before=draft.json().to_owned();
        let selection=[LedSelection{device:"keyboard".into(),row:0,col:1}];
        assert!(draft.set_selection_color("breathing",&selection,ColorProperty::BreathingFirst,Some(4)).is_err());
        assert_eq!(draft.json(),before);assert!(!draft.is_dirty());
    }
}
