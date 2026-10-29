use super::*;
use serde_json::{json,Value};
#[test]
fn custom_breathing_stop_materializes_converter_and_matches_first_preview_pixel() {
    let raw=scene().to_string();let mut draft=Draft::open(&raw).unwrap();
    let selection=[LedSelection{device:"keyboard".into(),row:0,col:1}];
    let before=draft.breathing_selection_gradient("breathing",&selection).unwrap();
    let SelectedGradient::Uniform(Some(stops))=before else{panic!("expected converter stops")};
    assert_eq!(stops.iter().map(|s|s.position).collect::<Vec<_>>(),[0,12,25,38,50,62,75,88,100]);
    assert_eq!(stops[2].color,Some(0x103050));assert_eq!(stops[6].color,None);
    draft.edit_breathing_stop("breathing",&selection,StopEdit::Set{index:0,position:0,color:Some(0x112233)}).unwrap();
    let value:Value=serde_json::from_str(draft.json()).unwrap();
    let region=value["layers"][0]["regions"].as_array().unwrap().iter().find(|r|r["params"].get("colorStops").is_some()).unwrap();assert_eq!(region["params"]["duration"],6000);assert_eq!(region["params"]["color"],0x103050);
    assert_eq!(region["params"]["colorStops"][0]["Color"],0x112233);assert_eq!(region["params"]["colorStopsCustom"],true);
    let preview=Preview::from_snapshot(&draft.read_snapshot(),123).unwrap();assert_eq!(preview.frame()[1],crate::Rgb(17,34,51));assert_eq!(preview.final_random_state(),123);
    let edited=draft.json().to_owned();draft.undo().unwrap();assert!(!draft.is_dirty());draft.redo().unwrap();assert_eq!(draft.json(),edited);
}
#[test]
fn inherited_breathing_stop_metadata_none_and_selected_partition_remain_raw() {
    let mut raw=scene();raw["layers"][0]["params"]=json!({"colorStops":[{"Stop":0,"Color":0x123456},{"Stop":100,"Color":0xabcdef}],"duration":28000});
    raw["layers"][0]["regions"][0]["params"]=json!({"duration":6000});
    let text=raw.to_string().replace("\"Color\":1193046","\"Color\":1193046,\"opaque\":9.9900e+100");
    let mut draft=Draft::open(&text).unwrap();let selection=[LedSelection{device:"keyboard".into(),row:0,col:1}];
    draft.edit_breathing_stop("breathing",&selection,StopEdit::Set{index:1,position:100,color:None}).unwrap();
    assert!(draft.json().matches("9.9900e+100").count()>=2,"inherited stop's untouched opaque token must be copied exactly");
    let value:Value=serde_json::from_str(draft.json()).unwrap();let region=value["layers"][0]["regions"].as_array().unwrap().iter().find(|r|r["params"].get("colorStops").is_some()).unwrap();
    assert!(region["params"]["colorStops"][1].get("Color").is_none());assert_eq!(region["params"]["duration"],6000);
    assert_eq!(value["layers"][0]["params"]["colorStops"][1]["Color"],0xabcdef);
    assert!(Preview::from_snapshot(&draft.read_snapshot(),1).is_ok());
}
#[test]
fn custom_breathing_invalid_duplicate_random_and_mixed_edits_refuse_atomically() {
    let selection=[LedSelection{device:"keyboard".into(),row:0,col:1}];let raw=scene().to_string();let mut draft=Draft::open(&raw).unwrap();
    for edit in [StopEdit::Set{index:0,position:12,color:Some(1)},StopEdit::Insert{position:0,color:Some(1)},StopEdit::Set{index:0,position:101,color:Some(1)},StopEdit::Set{index:0,position:0,color:Some(0x1000000)}] {
        assert!(draft.edit_breathing_stop("breathing",&selection,edit).is_err());assert_eq!(draft.json(),raw);assert_eq!(draft.undo_len(),0);
    }
    let mut random=scene();random["layers"][0]["regions"][0]["params"]["randomColor"]=true.into();let random=random.to_string();let mut draft=Draft::open(&random).unwrap();
    assert!(draft.edit_breathing_stop("breathing",&selection,StopEdit::Delete{index:1}).is_err());assert_eq!(draft.json(),random);
    let mut mixed=scene();mixed["layers"][0]["regions"][0]["devices"][0]["leds"]=json!([{"row":0,"col":1}]);let mut other=mixed["layers"][0]["regions"][0].clone();other["devices"][0]["leds"][0]["col"]=4.into();other["params"]["color"]=1.into();mixed["layers"][0]["regions"].as_array_mut().unwrap().push(other);
    let mixed=mixed.to_string();let mut draft=Draft::open(&mixed).unwrap();let selection=[selection[0].clone(),LedSelection{device:"keyboard".into(),row:0,col:4}];
    assert_eq!(draft.breathing_selection_gradient("breathing",&selection).unwrap(),SelectedGradient::Mixed);
    assert!(draft.edit_breathing_stop("breathing",&selection,StopEdit::Delete{index:1}).is_err());assert_eq!(draft.json(),mixed);
}
#[test]
fn custom_breathing_add_delete_limit_and_none_reveal_underlying_layer() {
    let mut draft=Draft::open(&scene().to_string()).unwrap();let selection=[LedSelection{device:"keyboard".into(),row:0,col:1}];
    draft.edit_breathing_stop("breathing",&selection,StopEdit::Insert{position:5,color:None}).unwrap();
    let SelectedGradient::Uniform(Some(stops))=draft.breathing_selection_gradient("breathing",&selection).unwrap() else{panic!("expected gradient")};assert_eq!(stops.len(),10);
    let before=draft.json().to_owned();let history=draft.undo_len();assert!(draft.edit_breathing_stop("breathing",&selection,StopEdit::Insert{position:6,color:Some(1)}).is_err());assert_eq!(draft.json(),before);assert_eq!(draft.undo_len(),history);
    draft.edit_breathing_stop("breathing",&selection,StopEdit::Delete{index:0}).unwrap();
    let preview=Preview::from_snapshot(&draft.read_snapshot(),1).unwrap();assert_eq!(preview.frame()[1],crate::Rgb(4,16,240),"None at the expanded first anchor reveals the retained base layer");
}
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
