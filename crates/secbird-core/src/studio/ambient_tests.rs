use super::*;
use serde_json::{json, Value};
const PROFILE: &str = r#"{"id":"p","title":"Ambient","opaque":9007199254740993123456789,"devices":[{"id":"k","leds":[{"row":0,"col":1,"future":9.9900e+100},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"a","type":"layer","name":"ambient","value":11,"title":"AMBIENT","visible":true,"params":{"future":1.2300e-7},"regions":[{"params":{"screen":"custom","screenRect":{"Left":100.5,"Top":200.25,"Right":300.75,"Bottom":400.125,"extra":9007199254740993123456789},"blur":5,"colorStops":[{"Stop":0,"Color":123}],"speed":17,"opaque":9.9900e+100},"regionFuture":1.2300e-7,"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
fn keys(cols: &[u16]) -> Vec<LedSelection> {
    cols.iter()
        .map(|&col| LedSelection {
            device: "k".into(),
            row: 0,
            col,
        })
        .collect()
}
#[test]
fn ambient_defaults_partition_and_source_presets_preserve_raw_fields() {
    let mut draft = Draft::open(PROFILE).unwrap();
    draft
        .edit_ambient("a", &keys(&[3]), AmbientEdit::Blur(9))
        .unwrap();
    let value: Value = serde_json::from_str(draft.json()).unwrap();
    let params = &value["layers"][0]["regions"][1]["params"];
    assert_eq!(params["screen"], "full");
    assert_eq!(params["blur"], 9);
    assert!(params.get("screenRect").is_none());
    assert_eq!(AMBIENT_PRESETS, ["full", "left", "top", "right", "bottom"]);
    assert_eq!(AmbientRect::preset(1).unwrap().right, 6553.5);
    assert_eq!(AmbientRect::preset(2).unwrap().bottom, 6553.5);
    assert_eq!(AmbientRect::preset(3).unwrap().left, 58981.5);
    assert_eq!(AmbientRect::preset(4).unwrap().top, 58981.5);
    let before = draft.json().to_owned();
    draft
        .edit_ambient("a", &keys(&[1]), AmbientEdit::Preset(1))
        .unwrap();
    assert_eq!(
        draft
            .ambient_selection_value("a", &keys(&[1, 2]), AmbientProperty::Screen)
            .unwrap(),
        SelectedAmbientValue::Mixed
    );
    assert_eq!(
        draft
            .ambient_selection_value("a", &keys(&[2]), AmbientProperty::Screen)
            .unwrap(),
        SelectedAmbientValue::Uniform(Some(AmbientValue::Screen("custom".into())))
    );
    for token in [
        "9007199254740993123456789",
        "9.9900e+100",
        "1.2300e-7",
        "\"colorStops\"",
        "\"speed\":17",
    ] {
        assert!(draft.json().contains(token));
    }
    draft.undo().unwrap();
    assert_eq!(draft.json(), before);
    draft.undo().unwrap();
    assert_eq!(draft.json(), PROFILE);
    assert!(!draft.is_dirty());
    draft.redo().unwrap();
    draft.redo().unwrap();
    assert!(draft.is_dirty());
}
#[test]
fn ambient_custom_save_discard_and_invalid_edits_are_atomic() {
    let mut draft = Draft::open(PROFILE).unwrap();
    let rect = AmbientRect {
        left: 0.25,
        top: 1.5,
        right: 65534.75,
        bottom: 65535.,
    };
    draft
        .edit_ambient("a", &keys(&[1]), AmbientEdit::Rect(rect))
        .unwrap();
    let value: Value = serde_json::from_str(draft.json()).unwrap();
    let params = &value["layers"][0]["regions"][1]["params"];
    assert_eq!(params["screen"], "custom");
    assert_eq!(params["screenRect"]["Left"], 0.25);
    assert!(draft.json().contains("\"extra\":9007199254740993123456789"));
    let archive = crate::studio::Archive::parse(
        format!(r#"{{"version":1,"profiles":[{PROFILE}],"future":9.9900e+100}}"#).as_bytes(),
    )
    .unwrap();
    let bytes = archive.replace_profile_bytes(0, draft.json()).unwrap();
    let reopened = crate::studio::Archive::parse(&bytes).unwrap();
    assert_eq!(reopened.profile_json(0).unwrap(), draft.json());
    assert!(std::str::from_utf8(&bytes).unwrap().contains("9.9900e+100"));
    draft.accept_saved(draft.checkpoint()).unwrap();
    let saved = draft.json().to_owned();
    for edit in [
        AmbientEdit::Blur(0),
        AmbientEdit::Blur(10),
        AmbientEdit::Preset(5),
        AmbientEdit::Rect(AmbientRect {
            left: f64::NAN,
            ..rect
        }),
        AmbientEdit::Rect(AmbientRect {
            left: 65535.,
            ..rect
        }),
        AmbientEdit::Rect(AmbientRect {
            bottom: 65536.,
            ..rect
        }),
    ] {
        let history = draft.undo_len();
        assert!(draft.edit_ambient("a", &keys(&[1]), edit).is_err());
        assert_eq!(draft.json(), saved);
        assert_eq!(draft.undo_len(), history);
    }
    draft
        .edit_ambient("a", &keys(&[1]), AmbientEdit::Blur(1))
        .unwrap();
    draft.discard();
    assert_eq!(draft.json(), saved);
    assert!(!draft.is_dirty());
    assert!(draft
        .edit_ambient("a", &keys(&[99]), AmbientEdit::Blur(5))
        .is_err());
    assert_eq!(
        draft
            .ambient_selection_value("a", &[], AmbientProperty::Blur)
            .unwrap(),
        SelectedAmbientValue::Empty
    );
}
fn scene() -> Value {
    let mut scene: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    let mut ambient = scene["layers"][0].clone();
    ambient["id"] = "ambient".into();
    ambient["name"] = "ambient".into();
    ambient["value"] = 11.into();
    ambient["params"] = json!({});
    ambient["regions"][0]["params"] = json!({"screen":"full","blur":1});
    ambient["regions"][0]["devices"][0]["leds"] = json!([{"row":0,"col":1},{"row":0,"col":4}]);
    let mut base = ambient.clone();
    base["id"] = "base".into();
    base["name"] = "static".into();
    base["value"] = 6.into();
    base["regions"][0]["params"] = json!({"color":0x0011ff});
    scene["layers"] = json!([ambient, base]);
    scene
}
#[test]
fn ambient_synthetic_projection_composes_without_admitting_physical_runtime() {
    let draft = Draft::open(&scene().to_string()).unwrap();
    let snapshot = draft.read_snapshot();
    assert!(Preview::from_snapshot(&snapshot, 1)
        .unwrap_err_string()
        .contains("unverified"));
    let mut preview = Preview::from_snapshot_synthetic_ambient(&snapshot, 1).unwrap();
    let before = draft.json().to_owned();
    let initial = preview.frame().to_vec();
    preview.advance().unwrap();
    assert_ne!(preview.frame(), initial); // exact40ms diagnostic tick
    assert!(preview.storage_bytes() < 10_000);
    assert_eq!(preview.final_random_state(), 1);
    let transparent = crate::ambient_sampling::DesktopFrame::new(1, 1, &[255, 0, 0, 0]).unwrap();
    preview.advance_with_synthetic_frame(&transparent).unwrap();
    assert_eq!(preview.frame()[1], crate::Rgb(0, 17, 255));
    let halves = [255, 0, 0, 255, 0, 255, 0, 255];
    let frame = crate::ambient_sampling::DesktopFrame::new(2, 1, &halves).unwrap();
    preview.advance_with_synthetic_frame(&frame).unwrap();
    assert_eq!(preview.frame()[1], crate::Rgb(255, 0, 0));
    assert_eq!(preview.frame()[4], crate::Rgb(0, 255, 0));
    assert_eq!(preview.frame()[2], crate::Rgb(0, 0, 0));
    assert_eq!(draft.json(), before);
    assert!(!draft.is_dirty());
    assert_eq!(preview.rendered_frames(), 4);
    let mut hidden = scene();
    hidden["layers"][0]["visible"] = false.into();
    let hidden = Draft::open(&hidden.to_string()).unwrap();
    assert!(Preview::from_snapshot(&hidden.read_snapshot(), 1).is_ok());
}
// Keep error checks readable without requiring the opaque Preview to implement Debug.
trait ErrorString {
    fn unwrap_err_string(self) -> String;
}
impl ErrorString for crate::Result<Preview> {
    fn unwrap_err_string(self) -> String {
        match self {
            Ok(_) => panic!("unexpected admitted preview"),
            Err(e) => e.to_string(),
        }
    }
}

#[test]
fn synthetic_ambient_sampling_work_budget_refuses_dense_layer_stacks() {
    let mut raw=scene();
    let mut layer=raw["layers"][0].clone();layer["regions"][0]["params"]["blur"]=9.into();
    raw["layers"]=(0..512).map(|index|{let mut copy=layer.clone();copy["id"]=format!("ambient-{index}").into();copy}).collect::<Vec<_>>().into();
    let draft=Draft::open(&raw.to_string()).unwrap();
    let error=Preview::from_snapshot_synthetic_ambient(&draft.read_snapshot(),1).unwrap_err_string();
    assert!(error.contains("sampling taps"),"{error}");
    assert!(!draft.is_dirty());
}
