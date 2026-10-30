//! Owner-bound Customize drafts and local desired-profile validation; no I/O.
//! Source:605 key popup1270/MapMouse and196 groupList; see source spec hashes.
use serde_json::{json, Value};
use std::sync::Arc;
pub(super) use secbird_core::profile_compat::keyboard_assignments as keyboard;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Owner {
    pub device: String,
    pub profile: String,
    pub pid: u16,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Input {
    pub key: String,
    pub input_id: String,
    pub input_type: String,
    pub label: String,
    pub default_label: String,
    pub functions: Vec<String>,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Category {
    Default,
    Ai,
    Keyboard,
    Mouse,
    Sensitivity,
    Macro,
    InterDevice,
    Profile,
    Chroma,
    Brightness,
    HyperShift,
    Launch,
    Media,
    Windows,
    Text,
    Disable,
    Dial,
}
impl Category {
    pub fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "DEFAULT" => Self::Default,
            "AI_LAUNCHER" => Self::Ai,
            "KEYBOARD_FUNCTION" => Self::Keyboard,
            "MOUSE_FUNCTION" => Self::Mouse,
            "SENSITIVITY" => Self::Sensitivity,
            "MACRO" => Self::Macro,
            "INTERDEVICE" => Self::InterDevice,
            "SWITCH_PROFILE" => Self::Profile,
            "SWITCH_LIGHTING" => Self::Chroma,
            "DEVICE_BRIGHTNESS" => Self::Brightness,
            "RAZER_HYPERSHIFT" => Self::HyperShift,
            "LAUNCH_PROGRAM" => Self::Launch,
            "MULTIMEDIA" => Self::Media,
            "WINDOWS_SHORTCUT" => Self::Windows,
            "TEXT_FUNCTION" => Self::Text,
            "DISABLE" => Self::Disable,
            "DIAL_MULTI_FUNCTION" => Self::Dial,
            _ => return None,
        })
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Ai => "AI_LAUNCHER",
            Self::Keyboard => "KEYBOARD_FUNCTION",
            Self::Mouse => "MOUSE_FUNCTION",
            Self::Sensitivity => "SENSITIVITY",
            Self::Macro => "MACRO",
            Self::InterDevice => "INTERDEVICE",
            Self::Profile => "SWITCH_PROFILE",
            Self::Chroma => "SWITCH_LIGHTING",
            Self::Brightness => "DEVICE_BRIGHTNESS",
            Self::HyperShift => "RAZER_HYPERSHIFT",
            Self::Launch => "LAUNCH_PROGRAM",
            Self::Media => "MULTIMEDIA",
            Self::Windows => "WINDOWS_SHORTCUT",
            Self::Text => "TEXT_FUNCTION",
            Self::Disable => "DISABLE",
            Self::Dial => "DIAL_MULTI_FUNCTION",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Ai => "AI LAUNCHER",
            Self::Keyboard => "KEYBOARD FUNCTION",
            Self::Mouse => "MOUSE FUNCTION",
            Self::Sensitivity => "SENSITIVITY",
            Self::Macro => "MACRO",
            Self::InterDevice => "INTER-DEVICE",
            Self::Profile => "SWITCH PROFILE",
            Self::Chroma => "SWITCH CHROMA EFFECT",
            Self::Brightness => "DEVICE BRIGHTNESS",
            Self::HyperShift => "RAZER HYPERSHIFT",
            Self::Launch => "LAUNCH",
            Self::Media => "MULTIMEDIA",
            Self::Windows => "WINDOWS SHORTCUTS",
            Self::Text => "TEXT FUNCTION",
            Self::Disable => "DISABLE",
            Self::Dial => "DIAL MULTI-FUNCTION",
        }
    }
    fn output(self) -> Option<&'static str> {
        Some(match self {
            Self::Default => "defaultGroup",
            Self::Mouse => "mouseGroup",
            Self::Disable => "disableGroup",
            Self::Keyboard => "keyboardGroup",
            Self::Sensitivity => "sensitivityGroup",
            Self::Macro => "macroGroup",
            Self::InterDevice => "interDeviceGroup",
            Self::Profile => "profileNavigationGroup",
            Self::Chroma => "lightPacGroup",
            Self::Brightness => "backlightGroup",
            Self::HyperShift => "hyperShiftGroup",
            Self::Launch => "launchGroup",
            Self::Media => "multimediaGroup",
            Self::Windows => "win8ShortcutsGroup",
            Self::Text => "textBlockGroup",
            Self::Ai => "aiLauncherGroup",
            Self::Dial => return None,
        })
    }
}
pub(super) const MOUSE_ACTIONS: [(&str, &str); 10] = [
    ("Click", "Left Click"),
    ("Menu", "Right Click"),
    ("ScrollButton", "Scroll Click"),
    ("DoubleClick", "Double Click"),
    ("ScrollUp", "Scroll Up"),
    ("ScrollDown", "Scroll Down"),
    ("Previous", "Mouse Button 4"),
    ("Next", "Mouse Button 5"),
    ("ScrollLeft", "Scroll Left"),
    ("ScrollRight", "Scroll Right"),
];
const COMMON: &[&str] = &[
    "DEFAULT",
    "AI_LAUNCHER",
    "KEYBOARD_FUNCTION",
    "MOUSE_FUNCTION",
    "SENSITIVITY",
    "MACRO",
    "INTERDEVICE",
    "SWITCH_PROFILE",
    "SWITCH_LIGHTING",
    "RAZER_HYPERSHIFT",
    "LAUNCH_PROGRAM",
    "MULTIMEDIA",
    "WINDOWS_SHORTCUT",
    "TEXT_FUNCTION",
    "DISABLE",
];
pub(super) fn mouse_inputs() -> Vec<Input> {
    [
        (
            "LeftClick",
            "LeftClick",
            "MouseInput",
            "1",
            "Left Click",
            false,
        ),
        (
            "RightClick",
            "RightClick",
            "MouseInput",
            "2",
            "Right Click",
            true,
        ),
        (
            "ScrollClick",
            "ScrollButton",
            "MouseInput",
            "3",
            "Scroll Click",
            true,
        ),
        ("ScrollUp", "ScrollUp", "MouseInput", "4", "Scroll Up", true),
        (
            "ScrollDown",
            "ScrollDown",
            "MouseInput",
            "5",
            "Scroll Down",
            true,
        ),
        (
            "MouseButton5",
            "Button5",
            "MouseInput",
            "7",
            "Mouse Button 5",
            true,
        ),
        (
            "MouseButton4",
            "Button4",
            "MouseInput",
            "6",
            "Mouse Button 4",
            true,
        ),
        (
            "CycleUpSensitivityStages",
            "DKM_SB_03",
            "DKMInput",
            "8",
            "Cycle Up Sensitivity Stages",
            true,
        ),
    ]
    .into_iter()
    .map(|(key, id, kind, label, default, enabled)| {
        let functions = if matches!(key, "ScrollUp" | "ScrollDown") {
            vec![
                "DEFAULT",
                "KEYBOARD_FUNCTION",
                "MOUSE_FUNCTION",
                "MACRO",
                "MULTIMEDIA",
                "DISABLE",
            ]
        } else {
            COMMON
                .iter()
                .copied()
                .filter(|c| key != "CycleUpSensitivityStages" || *c != "RAZER_HYPERSHIFT")
                .collect()
        };
        Input {
            key: key.into(),
            input_id: id.into(),
            input_type: kind.into(),
            label: label.into(),
            default_label: default.into(),
            functions: functions.into_iter().map(str::to_owned).collect(),
            enabled,
        }
    })
    .collect()
}
#[derive(Clone, Debug)]
pub(super) struct Context {
    pub owner: Owner,
    pub original_profile: Value,
    pub mappings: Option<Vec<Value>>,
}
impl Context {
    pub fn from_device(device: &Value) -> Option<Self> {
        let pid = u16::from_str_radix(device["pid"].as_str()?, 16).ok()?;
        if !matches!(pid, 605 | 196 | 197) {
            return None;
        }
        let original_profile = device["profile"].clone();
        let mappings = original_profile["_synapseLiteSynapse4"]["decoded"]["mappings"]
            .as_array()
            .cloned().or_else(|| (original_profile.is_object() && original_profile.get("_synapseLiteSynapse4").is_none()).then(Vec::new));
        Some(Self {
            owner: Owner {
                device: device["id"].as_str()?.into(),
                profile: device["active"].as_str()?.into(),
                pid,
            },
            original_profile,
            mappings,
        })
    }
    pub fn still_current(&self, device: &Value) -> bool {
        device["id"].as_str() == Some(&self.owner.device)
            && device["active"].as_str() == Some(&self.owner.profile)
            && device["profile"] == self.original_profile
    }
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Anchor {
    Key(String),
    Mouse(usize),
    Drawer(String),
}
#[derive(Clone, Debug)]
pub(super) struct Draft {
    pub keyboard: keyboard::Setting,
    pub keyboard_group_menu: bool,
    pub keyboard_error: Option<String>,
    pub context: Arc<Context>,
    pub input: Input,
    pub anchor: Anchor,
    pub hyper: bool,
    pub categories: Vec<Category>,
    pub category: Option<Category>,
    pub original_records: Vec<Value>,
    pub mouse_option: usize,
    pub turbo: bool,
    pub speed: u8,
    pub speed_draft: String,
    pub dirty: bool,
    pub rail_expanded: bool,
    pub rail_scroll: f64,
    pub form_scroll: f64,
    pub options_open: bool,
    pub options_scroll: f64,
}
impl Draft {
    pub fn new(context: Arc<Context>, input: Input, anchor: Anchor, hyper: bool) -> Self {
        let categories = input
            .functions
            .iter()
            .filter_map(|key| Category::parse(key))
            .filter(|c| !hyper || *c != Category::HyperShift)
            .collect();
        let original_records = context
            .mappings
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|r| {
                r["inputID"].as_str() == Some(&input.input_id)
                    && truthy(&r["isHyperShift"]) == hyper
            })
            .cloned()
            .collect::<Vec<_>>();
        let category = if context.mappings.is_none() {
            None
        } else if original_records.is_empty() {
            Some(Category::Default)
        } else if original_records.len() == 1 {
            let record = &original_records[0];
            if record["inputType"]
                .as_str()
                .is_some_and(|kind| kind != input.input_type)
            {
                None
            } else {
                input
                    .functions
                    .iter()
                    .filter_map(|s| Category::parse(s))
                    .find(|c| c.output() == record["outputType"].as_str())
            }
        } else {
            None
        };
        let mouse = original_records.first().map(|r| &r["mouseGroup"]);
        let mouse_option = mouse
            .and_then(|v| v["mouseAssignment"].as_str())
            .and_then(|id| MOUSE_ACTIONS.iter().position(|(v, _)| *v == id))
            .unwrap_or(0);
        let turbo = mouse.is_some_and(|v| v["turboMode"]["isTurbo"] == true);
        let speed = mouse
            .and_then(|v| v["turboMode"]["keysPerSecond"].as_u64())
            .filter(|n| (1..=20).contains(n))
            .unwrap_or(7) as u8;
        let keyboard_result = original_records.first().filter(|r|r["outputType"] == "keyboardGroup").map(keyboard::Setting::from_record);
        let keyboard_error = keyboard_result.as_ref().and_then(|r|r.as_ref().err()).map(ToString::to_string);
        let keyboard = keyboard_result.and_then(Result::ok).unwrap_or_default();
        let (turbo, speed) = if category == Some(Category::Keyboard) {(keyboard.turbo, keyboard.speed)} else {(turbo,speed)};
        Self {
            keyboard,
            keyboard_group_menu: false,
            keyboard_error,
            context,
            input,
            anchor,
            hyper,
            categories,
            category,
            original_records,
            mouse_option,
            turbo,
            speed,
            speed_draft: speed.to_string(),
            dirty: false,
            rail_expanded: false,
            rail_scroll: 0.0,
            form_scroll: 0.0,
            options_open: false,
            options_scroll: 0.0,
        }
    }
    pub fn select(&mut self, category: Category) {
        if !self.categories.contains(&category) {
            return;
        }
        if self.category == Some(category) {
            self.rail_expanded = false;
            return;
        }
        self.category = Some(category);
        self.dirty = false;
        self.rail_expanded = false;
        self.options_open = false;
        self.form_scroll = 0.0;
        let initial = Self::new(
            self.context.clone(),
            self.input.clone(),
            self.anchor.clone(),
            self.hyper,
        );
        if initial.category == Some(category) {
            self.mouse_option = initial.mouse_option;
            self.turbo = initial.turbo;
            self.speed = initial.speed;
        } else {
            self.mouse_option = 0;
            self.turbo = false;
            self.speed = 7;
        }
        self.speed_draft = self.speed.to_string();
        self.keyboard = initial.keyboard;
        self.keyboard_error = initial.keyboard_error;
        if category == Category::Keyboard {
            self.turbo = self.keyboard.turbo;
            self.speed = self.keyboard.speed;
            self.speed_draft = self.speed.to_string();
        }
    }
    pub fn option_labels(&self) -> Vec<String> {
        if self.category == Some(Category::Keyboard) {
            if self.keyboard_group_menu { keyboard::GROUPS.iter().map(|s|s.to_string()).collect() }
            else {keyboard::keys(self.keyboard.key_group).iter().map(|k|format!("{} ({})",k.name,k.id.trim_start_matches("KEY_"))).collect()}
        } else {MOUSE_ACTIONS.iter().map(|(_,label)|label.to_string()).collect()}
    }
    pub fn keyboard_setting(&self) -> keyboard::Setting {
        let mut setting = self.keyboard.clone();
        setting.turbo = self.turbo && self.turbo_available();
        setting.speed = self.speed;
        setting
    }
    pub fn keyboard_profile(&self) -> Result<(), String> {
        if self.category != Some(Category::Keyboard) || !self.dirty {return Err("Choose a keyboard assignment first".into());}
        if let Some(error) = &self.keyboard_error {return Err(error.clone());}
        let profile = serde_json::from_value(self.context.original_profile.clone()).map_err(|e|format!("Invalid retained profile: {e}"))?;
        keyboard::validate_update(&profile,self.context.owner.pid,&self.input.input_id,&self.input.input_type,self.hyper,&self.keyboard_setting()).map_err(|e|e.to_string())
    }
    pub fn select_keyboard_option(&mut self, index: usize) {
        if self.keyboard_group_menu {
            if index < keyboard::GROUPS.len() {
                self.keyboard.key_group = index;
                self.keyboard.key.clear();
                self.keyboard.modifiers.clear();
                self.dirty = true;
            }
        } else if let Some(key) = keyboard::keys(self.keyboard.key_group).get(index) {
            self.keyboard.key = key.id.clone();
            self.keyboard_error = None;
            self.dirty = true;
        }
        self.options_open = false;
        self.options_scroll = 0.0;
    }
    pub fn modifier(&mut self,index:usize) {
        if self.keyboard.key_group == 5 {return;}
        if let Some(id) = keyboard::MODIFIERS.get(index) {
            if self.keyboard.modifiers.iter().any(|v|v == id) {self.keyboard.modifiers.retain(|v|v != id);}
            else {
                if self.keyboard.key_group != 0 && index < 6 {self.keyboard.modifiers.retain(|v|v != keyboard::MODIFIERS[index ^ 1]);}
                self.keyboard.modifiers.push(id.to_string());
            }
            self.dirty = true;
        }
    }
    pub fn select_mouse(&mut self, index: usize) {
        if index < MOUSE_ACTIONS.len() {
            self.mouse_option = index;
            self.dirty = true;
            self.options_open = false;
        }
    }
    pub fn turbo_available(&self) -> bool {
        !matches!(
            self.input.key.as_str(),
            "ScrollUp"
                | "ScrollDown"
                | "CycleUpSensitivityStages"
                | "DIAL_RIGHT_ROTATION"
                | "DIAL_CLICK"
                | "DIAL_LEFT_ROTATION"
                | "RepeatScrollUp"
                | "RepeatScrollDown"
                | "MEDIA_PREV"
                | "MEDIA_PAUSE_PLAY"
                | "MEDIA_NEXT"
                | "MEDIA_MUTE"
                | "MEDIA_VOLUME_UP"
                | "MEDIA_VOLUME_DOWN"
        )
    }
    pub fn toggle_turbo(&mut self) {
        if self.turbo_available() {
            self.turbo = !self.turbo;
            self.dirty = true;
        }
    }
    pub fn set_speed(&mut self, value: u8) {
        self.speed = value.clamp(1, 20);
        self.speed_draft = self.speed.to_string();
        self.dirty = true;
    }
    pub fn speed_change(&mut self, value: &str) -> bool {
        if value.encode_utf16().count() <= 2
            && (value.is_empty()
                || value == "-"
                || value
                    .strip_prefix('-')
                    .unwrap_or(value)
                    .bytes()
                    .all(|b| b.is_ascii_digit()))
        {
            self.speed_draft = value.into();
            if let Ok(n) = value.parse::<i32>() {
                self.speed = n.clamp(1, 20) as u8;
            }
            self.dirty = true;
            true
        } else {
            false
        }
    }
    pub fn speed_blur(&mut self) {
        let n = self.speed_draft.parse::<i32>().unwrap_or(0).clamp(1, 20) as u8;
        self.set_speed(n);
    }
    pub fn save_reason(&self) -> &'static str {
        if self.category == Some(Category::Keyboard) {return "Saves the local desired profile only. Hardware delivery and readback are unavailable.";}
        if self.context.owner.pid == 605 {
            "Saving assignments is unavailable: Ornata mapping readback is unsupported. No command will be sent."
        } else {
            "Saving assignments is unavailable: the installed mouse layer mapping has not been verified. No command will be sent."
        }
    }
    pub fn provenance(&self) -> &'static str {
        if self.context.mappings.is_some() {
            "last-read persisted desired profile; not hardware readback"
        } else {
            "stored source assignment context unavailable; hardware assignment unknown"
        }
    }
}
#[derive(Clone, Debug)]
pub(super) enum Transition {
    Close,
    Open(Input, Anchor),
    Drawer(bool),
    Layer(bool),
    External,
}
#[derive(Default)]
pub(super) struct View {
    pub saving: bool,
    pub draft: Option<Draft>,
    pub drawer: bool,
    pub drawer_scroll: f64,
    pub customized_only: bool,
    pub filter_open: bool,
    pub hyper: bool,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub hover: Option<(f64, f64)>,
    pub pending: Option<Transition>,
    pub rail_grab: Option<f64>,
    pub drawer_grab: Option<f64>,
    pub form_grab: Option<f64>,
    pub options_grab: Option<f64>,
    pub speed_drag: bool,
    pub spinner: Option<i8>,
}
impl View {
    pub fn request(&mut self, next: Transition) -> Option<Transition> {
        if self.draft.as_ref().is_some_and(|d| d.dirty) {
            self.pending = Some(next);
            None
        } else {
            Some(next)
        }
    }
    pub fn discard(&mut self) -> Option<Transition> {
        if let Some(d) = &mut self.draft {
            d.dirty = false;
        }
        self.pending.take()
    }
    pub fn dismiss_prompt(&mut self) {
        self.pending = None;
    }
    pub fn cancel(&mut self) {
        self.draft = None;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(mappings: Value) -> Arc<Context> {
        Arc::new(Context::from_device(&json!({"id":"025D:SIM","pid":"025D","active":"A","profile":{"_synapseLiteSynapse4":{"decoded":{"mappings":mappings}},"opaque":{"keep":[1,2]}}})).unwrap())
    }
    fn input() -> Input {
        Input {
            key: "A".into(),
            input_id: "KEY_A".into(),
            input_type: "KeyInput".into(),
            label: "A".into(),
            default_label: "A".into(),
            functions: vec![
                "DEFAULT".into(),
                "MOUSE_FUNCTION".into(),
                "DISABLE".into(),
                "RAZER_HYPERSHIFT".into(),
            ],
            enabled: true,
        }
    }
    #[test]
    fn opaque_originals_survive_preview_edit_and_discard() {
        let raw = json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"opaque":[1,2]}}]);
        let ctx = context(raw.clone());
        let before = ctx.original_profile.clone();
        let mut d = Draft::new(ctx.clone(), input(), Anchor::Key("KEY_A".into()), false);
        assert_eq!(d.category, None);
        d.select(Category::Mouse);
        d.select_mouse(4);
        d.toggle_turbo();
        d.set_speed(19);
        assert_eq!(ctx.mappings.as_ref().unwrap(), raw.as_array().unwrap());
        assert_eq!(ctx.original_profile, before);
        assert_eq!(d.original_records[0]["futureGroup"], raw[0]["futureGroup"]);
    }
    #[test]
    fn default_absence_is_desired_profile_only_and_unknown_is_not_default() {
        let mut d = Draft::new(
            context(json!([])),
            input(),
            Anchor::Key("KEY_A".into()),
            false,
        );
        assert_eq!(d.category, Some(Category::Default));
        assert!(d.provenance().contains("not hardware"));
        d.select(Category::Disable);
        assert!(!d.dirty);
        let mut unknown = (*context(json!([]))).clone();
        unknown.mappings = None;
        assert_eq!(
            Draft::new(
                Arc::new(unknown),
                input(),
                Anchor::Key("KEY_A".into()),
                false
            )
            .category,
            None
        );
    }
    #[test]
    fn category_changes_reset_flags_and_hyper_filter_preserves_order() {
        let mut d = Draft::new(
            context(json!([])),
            input(),
            Anchor::Key("KEY_A".into()),
            true,
        );
        assert_eq!(
            d.categories,
            vec![Category::Default, Category::Mouse, Category::Disable]
        );
        d.select(Category::Mouse);
        d.select_mouse(2);
        assert!(d.dirty);
        d.select(Category::Disable);
        assert!(!d.dirty);
    }
    #[test]
    fn dirty_close_prompt_dismiss_keeps_draft_and_discard_releases_transition() {
        let mut v = View::default();
        let mut d = Draft::new(
            context(json!([])),
            input(),
            Anchor::Key("KEY_A".into()),
            false,
        );
        d.select(Category::Mouse);
        d.select_mouse(3);
        v.draft = Some(d);
        assert!(v.request(Transition::Close).is_none());
        v.dismiss_prompt();
        assert!(v.draft.as_ref().unwrap().dirty);
        v.request(Transition::Drawer(true));
        assert!(matches!(v.discard(), Some(Transition::Drawer(true))));
        assert!(!v.draft.as_ref().unwrap().dirty);
        v.cancel();
        assert!(v.draft.is_none());
    }
    #[test]
    fn controlled_speed_and_source_turbo_exclusions() {
        let mut d = Draft::new(
            context(json!([])),
            input(),
            Anchor::Key("KEY_A".into()),
            false,
        );
        assert_eq!(d.speed, 7);
        assert!(!d.speed_change("1e"));
        assert!(d.speed_change("-"));
        d.speed_blur();
        assert_eq!(d.speed, 1);
        assert!(d.speed_change("99"));
        d.speed_blur();
        assert_eq!(d.speed, 20);
        let wheel = mouse_inputs().remove(3);
        let d = Draft::new(context(json!([])), wheel, Anchor::Mouse(3), false);
        assert!(!d.turbo_available());
    }
    #[test]
    fn stale_owner_or_changed_persisted_payload_invalidates() {
        let ctx = context(json!([]));
        let mut current = json!({"id":"025D:SIM","active":"A","profile":ctx.original_profile});
        assert!(ctx.still_current(&current));
        current["active"] = json!("B");
        assert!(!ctx.still_current(&current));
        current["active"] = json!("A");
        current["profile"]["opaque"] = json!({"changed":true});
        assert!(!ctx.still_current(&current));
    }
    #[test]
    fn explicit_keyboard_choice_repairs_unknown_key_but_not_combined_context() {
        for combined in [false,true] {
            let mut record=json!({"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"FUTURE_KEY","keyGroup":0,"modifiers":[],"opaque":{"keep":true}}});
            if combined {record["keyboardGroup"]["mouseGroup"]=json!({"mouseAssignment":"Click"});}
            let ctx=context(json!([record]));let before=ctx.original_profile.clone();
            let mut input=input();input.functions.push("KEYBOARD_FUNCTION".into());
            let mut draft=Draft::new(ctx.clone(),input,Anchor::Key("KEY_A".into()),false);
            assert!(draft.keyboard_error.is_some());
            draft.select_keyboard_option(2);
            assert_eq!(draft.keyboard.key,"KEY_C");
            assert_eq!(draft.keyboard_profile().is_ok(),!combined);
            assert_eq!(ctx.original_profile,before);
        }
    }

}
