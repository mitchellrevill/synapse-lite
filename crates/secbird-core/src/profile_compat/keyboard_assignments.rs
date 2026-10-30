//! Local desired assignments. No input capture, native output or device writes.
use super::{invalid, macro_library::input_events, mappings, Profile, Result, SOURCE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const GROUPS: [&str; 7] = [
    "All keys",
    "Alphanumeric",
    "Function",
    "Numpad",
    "Navigation",
    "Modifiers",
    "Symbols",
];
pub const MODIFIERS: [&str; 7] = [
    "KEY_LEFT_SHIFT",
    "KEY_RIGHT_SHIFT",
    "KEY_LEFT_CTRL",
    "KEY_RIGHT_CTRL",
    "KEY_LEFT_ALT",
    "KEY_RIGHT_ALT",
    "KEY_LEFT_GUI",
];

pub fn keys(group: usize) -> Vec<&'static input_events::Key> {
    input_events::keys()
        .iter()
        .filter(|key| group == 0 || GROUPS.get(group).is_some_and(|name| *name == key.category))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub key_group: usize,
    pub modifiers: Vec<String>,
    pub turbo: bool,
    pub speed: u8,
}
impl Default for Setting {
    fn default() -> Self {
        Self {
            key: String::new(),
            key_group: 0,
            modifiers: vec![],
            turbo: false,
            speed: 7,
        }
    }
}
impl Setting {
    pub fn validate(&self) -> Result<()> {
        if !keys(self.key_group).iter().any(|key| key.id == self.key)
            || self.key_group >= GROUPS.len()
        {
            return Err(invalid("Choose a supported key in the selected group"));
        }
        if !(1..=20).contains(&self.speed) {
            return Err(invalid("Keyboard Turbo speed must be 1..20"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in &self.modifiers {
            if !MODIFIERS.contains(&id.as_str()) || !seen.insert(id) || *id == self.key {
                return Err(invalid("Unsupported, duplicate or repeated key modifier"));
            }
        }
        // Source dropdown chooses one side per modifier row. All-keys/manual
        // combinations retain both sides, as the source key-recording form does.
        if self.key_group != 0
            && MODIFIERS[..6]
                .chunks(2)
                .any(|pair| pair.iter().all(|id| self.modifiers.iter().any(|v| v == id)))
        {
            return Err(invalid("Choose one side of each modifier"));
        }
        if self.key_group == 5 && !self.modifiers.is_empty() {
            return Err(invalid(
                "Modifier keys do not accept added modifiers in this source form",
            ));
        }
        Ok(())
    }
    pub fn from_record(record: &Value) -> Result<Self> {
        if record["outputType"] != "keyboardGroup" {
            return Err(invalid("Not a keyboard assignment"));
        }
        let group = &record["keyboardGroup"];
        if !group.is_object()
            || group.get("mouseGroup").is_some()
            || record.get("mapping").is_some()
            || record["isInterDevice"] == true
        {
            return Err(invalid(
                "Combined, inter-device and secondary assignments are retained read-only",
            ));
        }
        if group.get("keyGroup").is_some_and(|v| v.as_u64().is_none())
            || group.get("turboMode").is_some_and(|v| {
                !v.is_object()
                    || v.get("isTurbo").is_some_and(|v| !v.is_boolean())
                    || v.get("keysPerSecond").is_some_and(|v| v.as_u64().is_none())
            })
        {
            return Err(invalid("Unsupported keyboard field representation"));
        }
        let setting = Self {
            key: group["key"]
                .as_str()
                .ok_or_else(|| invalid("Unsupported key representation"))?
                .into(),
            key_group: group["keyGroup"].as_u64().unwrap_or(0) as usize,
            modifiers: serde_json::from_value(group["modifiers"].clone())
                .map_err(|_| invalid("Unsupported modifier representation"))?,
            turbo: group["turboMode"]["isTurbo"].as_bool().unwrap_or(false),
            speed: group["turboMode"]["keysPerSecond"]
                .as_u64()
                .unwrap_or(7)
                .try_into()
                .map_err(|_| invalid("Invalid Turbo speed"))?,
        };
        setting.validate()?;
        Ok(setting)
    }
    /// Diagnostic representation only; a binding follows the input down/up.
    pub fn transitions(&self) -> Option<(u16, u8, u8)> {
        input_events::keys()
            .iter()
            .find(|key| key.id == self.key)
            .map(|key| (key.scan_code, key.prefix, key.prefix + 1))
    }
}

/// Prevent legacy native-button export projection from deleting or overwriting
/// this locally authored source record. Native buttons for this input are removed
/// so no existing profile activation route can accidentally deliver it.
pub(super) fn local_only(profile: &Profile, record: &Value) -> bool {
    profile
        .extra
        .get(SOURCE)
        .and_then(|v| v["localKeyboardAssignments"].as_array())
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row["inputID"] == record["inputID"]
                    && row["isHyperShift"].as_bool().unwrap_or(false)
                        == record["isHyperShift"].as_bool().unwrap_or(false)
            })
        })
}

/// Run the same validation without allocating a new profile GUID during paint.
pub fn validate_update(
    profile: &Profile,
    pid: u16,
    input: &str,
    input_type: &str,
    hyper: bool,
    setting: &Setting,
) -> Result<()> {
    let mut preview = profile.clone();
    if !preview.extra.contains_key(SOURCE) {
        preview
            .extra
            .insert(SOURCE.into(), json!({"decoded":{"mappings":[]}}));
    }
    update(&preview, pid, input, input_type, hyper, setting).map(|_| ())
}

pub fn update(
    profile: &Profile,
    pid: u16,
    input: &str,
    input_type: &str,
    hyper: bool,
    setting: &Setting,
) -> Result<Profile> {
    setting.validate()?;
    if !matches!(pid, 605 | 196 | 197) {
        return Err(invalid("Unsupported assignment device"));
    }
    let identity = json!({"inputID":input,"inputType":input_type,"isHyperShift":hyper});
    let native_key = mappings::setting_key(&identity, pid)
        .ok_or_else(|| invalid("Unsupported input identity"))?;
    let expected_type = if pid == 605 {
        "KeyInput"
    } else if input.starts_with("DKM_") {
        "DKMInput"
    } else {
        "MouseInput"
    };
    if input_type != expected_type || (pid != 605 && input == "LeftClick") {
        return Err(invalid("Input is not assignable"));
    }
    if setting.turbo && matches!(input, "ScrollUp" | "ScrollDown" | "DKM_SB_03") {
        return Err(invalid("Turbo is unavailable for this source input"));
    }
    let mut next = profile.clone();
    if !next.extra.contains_key(SOURCE) {
        let template = super::new_native_profile(pid, "Local assignment")?;
        next.extra
            .insert(SOURCE.into(), template.extra[SOURCE].clone());
    }
    let source = next
        .extra
        .get_mut(SOURCE)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Source profile context is unavailable"))?;
    let records = source
        .get_mut("decoded")
        .and_then(|v| v.get_mut("mappings"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("Source mappings are unavailable"))?;
    if records
        .iter()
        .any(|r| r["inputID"] == input && r.get("isHyperShift").is_some_and(|v| !v.is_boolean()))
    {
        return Err(invalid("Ambiguous layer representation; original retained"));
    }
    let matches: Vec<usize> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r["inputID"] == input && r["isHyperShift"].as_bool().unwrap_or(false) == hyper
        })
        .map(|(i, _)| i)
        .collect();
    if matches.len() > 1 {
        return Err(invalid(
            "Ambiguous duplicate input mapping; original retained",
        ));
    }
    let mut record = matches
        .first()
        .map(|i| records[*i].clone())
        .unwrap_or(identity);
    if !record.is_object()
        || record.get("mapping").is_some()
        || record["isInterDevice"] == true
        || record.get("inputType").is_some_and(|v| v != input_type)
        || record.get("isHyperShift").is_some_and(|v| !v.is_boolean())
    {
        return Err(invalid("Unsupported mapping context; original retained"));
    }
    let object = record.as_object_mut().unwrap();
    let group = object
        .entry("keyboardGroup")
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| invalid("Unsupported keyboard group"))?;
    if group.contains_key("mouseGroup") {
        return Err(invalid(
            "Combined keyboard/mouse mapping is retained read-only",
        ));
    }
    group.insert("key".into(), json!(setting.key));
    group.insert("keyGroup".into(), json!(setting.key_group));
    group.insert("modifiers".into(), json!(setting.modifiers));
    let turbo = group
        .entry("turboMode")
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| invalid("Unsupported Turbo metadata"))?;
    turbo.insert("isTurbo".into(), json!(setting.turbo));
    turbo.insert("keysPerSecond".into(), json!(setting.speed));
    object.insert("outputType".into(), json!("keyboardGroup"));
    match matches.first() {
        Some(i) => records[*i] = record,
        None => records.push(record),
    }
    let local = source
        .entry("localKeyboardAssignments")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or_else(|| invalid("Invalid local assignment metadata"))?;
    if !local
        .iter()
        .any(|v| v["inputID"] == input && v["isHyperShift"] == hyper)
    {
        local.push(json!({"inputID":input,"isHyperShift":hyper}));
    }
    next.buttons.remove(&native_key);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_keyboard_assignment_supports_native_profiles_and_mouse_layers() {
        let mut original=Profile::default();
        original.extra.insert("opaque".into(),json!({"keep":[1,2,3]}));
        original.buttons.insert("side-back/hypershift".into(),"key:x".into());
        original.buttons.insert("side-back".into(),"key:y".into());
        let mut setting=setting();setting.turbo=false;
        validate_update(&original,196,"Button4","MouseInput",true,&setting).unwrap();
        assert!(!original.extra.contains_key(SOURCE));
        let next=update(&original,196,"Button4","MouseInput",true,&setting).unwrap();
        assert_eq!(next.extra["opaque"],original.extra["opaque"]);
        assert_eq!(next.buttons["side-back"],"key:y");
        assert!(!next.buttons.contains_key("side-back/hypershift"));
        let rows=next.extra[SOURCE]["decoded"]["mappings"].as_array().unwrap();
        let record=rows.iter().find(|r|r["inputID"]=="Button4" && r["isHyperShift"]==true).unwrap();
        assert_eq!(Setting::from_record(record).unwrap(),setting);
        let again=update(&next,196,"Button4","MouseInput",true,&setting).unwrap();
        assert_eq!(again,next);
        assert!(update(&original,196,"ScrollUp","MouseInput",false,&self::setting()).is_err());
    }
    fn setting() -> Setting {
        Setting {
            key: "KEY_C".into(),
            key_group: 1,
            modifiers: vec!["KEY_RIGHT_CTRL".into()],
            turbo: true,
            speed: 11,
        }
    }
    fn profile() -> Profile {
        let mut profile = super::super::new_native_profile(605, "Fixture").unwrap();
        profile.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = json!([
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","keyGroup":1,"modifiers":[],"future":{"keep":17},"turboMode":{"isTurbo":false,"keysPerSecond":7,"opaque":true}},"futureRoot":[3,4]},
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"retain":true}},
            {"inputID":"KEY_B","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"retain":18}}
        ]);
        profile.buttons.insert("key-a".into(), "key:b".into());
        profile
    }
    #[test]
    fn local_keyboard_assignment_preserves_both_layers_and_opaque_metadata() {
        let original = profile();
        let next = update(&original, 605, "KEY_A", "KeyInput", false, &setting()).unwrap();
        let before = &original.extra[SOURCE]["decoded"]["mappings"];
        let after = &next.extra[SOURCE]["decoded"]["mappings"];
        assert_eq!(after[1], before[1]);
        assert_eq!(after[2], before[2]);
        assert_eq!(after[0]["futureRoot"], before[0]["futureRoot"]);
        assert_eq!(after[0]["keyboardGroup"]["future"], json!({"keep":17}));
        assert_eq!(after[0]["keyboardGroup"]["turboMode"]["opaque"], true);
        assert_eq!(Setting::from_record(&after[0]).unwrap(), setting());
        assert!(!next.buttons.contains_key("key-a"));
        assert_eq!(original.buttons["key-a"], "key:b");
        let hyper = update(&next, 605, "KEY_A", "KeyInput", true, &setting()).unwrap();
        assert_eq!(hyper.extra[SOURCE]["decoded"]["mappings"][0], after[0]);
        assert_eq!(
            hyper.extra[SOURCE]["decoded"]["mappings"][1]["futureGroup"],
            before[1]["futureGroup"]
        );
    }
    #[test]
    fn local_keyboard_assignment_native_and_source_round_trip() {
        let next = update(&profile(), 605, "KEY_A", "KeyInput", false, &setting()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "keyboard-assignments-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = crate::Store::load(&path).unwrap();
        store
            .device(605)
            .profiles
            .insert("Fixture".into(), next.clone());
        store.device(605).active = "Fixture".into();
        store.save().unwrap();
        let reopened = crate::Store::load(&path).unwrap();
        assert_eq!(reopened.devices["025D"].profiles["Fixture"], next);
        let export = super::super::export_synapse4(&reopened, "025D").unwrap();
        let imported = super::super::import_document(&export.bytes).unwrap();
        let record = &imported.store.devices["025D"].profiles["Fixture"].extra[SOURCE]["decoded"]
            ["mappings"][0];
        assert_eq!(Setting::from_record(record).unwrap(), setting());
        assert_eq!(record["keyboardGroup"]["future"], json!({"keep":17}));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn local_keyboard_assignment_rejects_ambiguous_and_combined_records_atomically() {
        for records in [
            json!([{"inputID":"KEY_A","isHyperShift":false},{"inputID":"KEY_A","isHyperShift":false}]),
            json!([{"inputID":"KEY_A","isHyperShift":"false"}]),
            json!([{"inputID":"KEY_A","keyboardGroup":{"mouseGroup":{}}}]),
            json!([{"inputID":"KEY_A","mapping":{}}]),
            json!([{"inputID":"KEY_A","keyboardGroup":{"turboMode":"future"}}]),
        ] {
            let mut profile = profile();
            profile.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = records;
            let before = profile.clone();
            assert!(update(&profile, 605, "KEY_A", "KeyInput", false, &setting()).is_err());
            assert_eq!(profile, before);
        }
    }
    #[test]
    fn local_keyboard_assignment_validates_keys_modifiers_and_turbo() {
        let mut bad = setting();
        bad.key = "UNKNOWN".into();
        assert!(bad.validate().is_err());
        let mut bad = setting();
        bad.key_group = 2;
        assert!(bad.validate().is_err());
        let mut bad = setting();
        bad.modifiers.push("KEY_RIGHT_CTRL".into());
        assert!(bad.validate().is_err());
        let mut bad = setting();
        bad.modifiers.push("KEY_LEFT_CTRL".into());
        assert!(bad.validate().is_err());
        let mut bad = setting();
        bad.modifiers = vec!["KEY_A".into()];
        assert!(bad.validate().is_err());
        for speed in [0, 21, 255] {
            let mut bad = setting();
            bad.speed = speed;
            assert!(bad.validate().is_err());
        }
        assert!(update(&profile(), 605, "KEY_A", "MouseInput", false, &setting()).is_err());
        assert!(update(
            &profile(),
            196,
            "LeftClick",
            "MouseInput",
            false,
            &setting()
        )
        .is_err());
    }
    #[test]
    fn local_keyboard_assignment_all_source_descriptors_have_balanced_representation() {
        for key in keys(0) {
            let setting = Setting {
                key: key.id.clone(),
                ..Setting::default()
            };
            setting.validate().unwrap();
            assert_eq!(
                setting.transitions(),
                Some((key.scan_code, key.prefix, key.prefix + 1))
            );
        }
        assert_eq!(keys(0).len(), 125);
        let mut s = setting();
        s.key = "KEY_RIGHT_CTRL".into();
        s.modifiers.clear();
        s.key_group = 5;
        assert_eq!(s.transitions(), Some((29, 2, 3)));
        assert!(s.validate().is_ok());
    }
}
