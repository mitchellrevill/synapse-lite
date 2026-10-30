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
    pub sensitivity_range: Option<secbird_core::db::DpiRange>,
    pub navigation: Option<super::assignment_navigation::Catalog>,
}
impl Context {
    pub fn from_device(device: &Value) -> Option<Self> {
        let pid = u16::from_str_radix(device["pid"].as_str()?, 16).ok()?;
        if !matches!(pid, 605 | 196 | 197) {
            return None;
        }
        let original_profile = device["profile"].clone();
        let source=&original_profile["_synapseLiteSynapse4"];
        let mappings=(source["decoded"]["mappings"].is_array() || source["pendingMouseAssignments"].is_array() || original_profile["_synapseLiteLocalAssignments"].is_array())
            .then(||serde_json::from_value::<secbird_core::Profile>(original_profile.clone()).ok()
                .and_then(|profile|secbird_core::profile_compat::assignment_state::effective_mappings(&profile,pid).ok())).flatten();
        Some(Self {
            owner: Owner {
                device: device["id"].as_str()?.into(),
                profile: device["active"].as_str()?.into(),
                pid,
            },
            sensitivity_range: (|| {
                let v = &device["sensitivity_assignment"];
                let range = secbird_core::db::DpiRange {
                    min: u16::try_from(v["min"].as_u64()?).ok()?,
                    max: u16::try_from(v["max"].as_u64()?).ok()?,
                    step: u16::try_from(v["step"].as_u64()?).ok()?,
                };
                (range.min > 0 && range.min <= range.max && range.step > 0).then_some(range)
            })(),
            navigation: super::assignment_navigation::Catalog::from_device(device),
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
    pub navigation: super::assignment_navigation::Navigation,
    pub keyboard_group_menu: bool,
    pub keyboard_error: Option<String>,
    pub context: Arc<Context>,
    pub input: Input,
    pub anchor: Anchor,
    pub hyper: bool,
    pub categories: Vec<Category>,
    pub category: Option<Category>,
    pub original_records: Vec<Value>,
    pub sensitivity: secbird_core::sensitivity_assignment::Intent,
    pub dpi_text: [String; 2],
    pub mouse_option: usize,
    pub mouse_choice_confirmed: bool,
    pub media_option: Option<usize>,
    pub windows_option: Option<usize>,
    pub turbo: bool,
    pub speed: u8,
    pub speed_draft: String,
    pub launch: Launch,
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
        let mut category = if context.mappings.is_none() {
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
        let recovered_choice = mouse
            .and_then(|v| v["mouseAssignment"].as_str())
            .and_then(|id| MOUSE_ACTIONS.iter().position(|(v, _)| *v == id));
        let mouse_option = recovered_choice.unwrap_or(0);
        let mouse_choice_confirmed = category != Some(Category::Mouse) || recovered_choice.is_some();
        let turbo = mouse.is_some_and(|v| v["turboMode"]["isTurbo"] == true);
        let speed = mouse
            .and_then(|v| v["turboMode"]["keysPerSecond"].as_u64())
            .filter(|n| (1..=20).contains(n))
            .unwrap_or(7) as u8;
        let mut sensitivity = secbird_core::sensitivity_assignment::Intent {
            input: input.input_id.clone(),
            hyper,
            assignment: if input.input_id == "DKM_SB_03" {
                "DPI_Up"
            } else {
                "DPI_Clutch"
            }
            .into(),
            x: 800,
            y: 800,
            independent: false,
        };
        if let Some(group) = original_records
            .first()
            .filter(|r| r["outputType"] == "sensitivityGroup")
            .map(|r| &r["sensitivityGroup"])
        {
            if let Ok(intent) = serde_json::from_value::<secbird_core::sensitivity_assignment::Intent>(
                json!({"input":input.input_id,"hyper":hyper,"sensitivityAssignment":group["sensitivityAssignment"],"x":group["x"].as_u64().unwrap_or(800),"y":group["y"].as_u64().unwrap_or(800),"independent":group["independent"].as_bool().unwrap_or(false)}),
            ) {
                sensitivity = intent;
            }
        }
        if let Some(saved) = context.original_profile[secbird_core::sensitivity_assignment::DRAFTS]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["input"] == input.input_id && r["hyper"] == hyper)
            })
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            sensitivity = saved;
            if input.functions.iter().any(|f| f == "SENSITIVITY") {
                category = Some(Category::Sensitivity);
            }
        }
        let dpi_text = [sensitivity.x.to_string(), sensitivity.y.to_string()];
        let windows_option = if category == Some(Category::Windows) {
            original_records.first().and_then(|r|r["win8ShortcutsGroup"]["windowsShortcutAssignment"].as_str())
                .and_then(|id|secbird_core::profile_compat::windows_shortcuts::CHOICES.iter().position(|v|v.0==id))
        } else { Some(0) };
        let keyboard_result = original_records.first().filter(|r|r["outputType"] == "keyboardGroup").map(keyboard::Setting::from_record);
        let keyboard_error = keyboard_result.as_ref().and_then(|r|r.as_ref().err()).map(ToString::to_string);
        let keyboard = keyboard_result.and_then(Result::ok).unwrap_or_default();
        let (turbo, speed) = if category == Some(Category::Keyboard) {(keyboard.turbo, keyboard.speed)} else {(turbo,speed)};
        Self {
            navigation: super::assignment_navigation::Navigation::read(original_records.first().filter(|r|r["outputType"]=="profileNavigationGroup").map(|r|&r["profileNavigationGroup"])),
            keyboard,
            keyboard_group_menu: false,
            keyboard_error,
            windows_option,
            launch: Launch::read(original_records.first().map(|r| &r["launchGroup"])),
            context,
            input,
            anchor,
            hyper,
            categories,
            category,
            media_option: if category == Some(Category::Media) {
                original_records
                    .first()
                    .and_then(|r| r["multimediaGroup"]["multimediaAssignment"].as_str())
                    .and_then(|id| {
                        secbird_core::profile_compat::multimedia::CHOICES
                            .iter()
                            .position(|(v, _)| *v == id)
                    })
            } else {
                Some(0)
            },
            original_records,
            sensitivity,
            dpi_text,
            mouse_option,
            mouse_choice_confirmed,
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
        if !self.input.enabled || !self.categories.contains(&category) {
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
            self.windows_option = initial.windows_option;
            self.mouse_option = initial.mouse_option;
            self.mouse_choice_confirmed = initial.mouse_choice_confirmed;
            self.media_option = initial.media_option;
            self.turbo = initial.turbo;
            self.speed = initial.speed;
        } else {
            self.windows_option = Some(0);
            self.mouse_option = 0;
            self.mouse_choice_confirmed = true;
            self.media_option = Some(0);
            self.turbo = false;
            self.speed = 7;
        }
        self.dirty = category == Category::Windows && initial.category != Some(category);
        self.sensitivity = initial.sensitivity;
        self.dpi_text = initial.dpi_text;
        self.speed_draft = self.speed.to_string();
        self.launch = initial.launch;
        self.navigation = initial.navigation;
        if category == Category::Profile && initial.category != Some(category) {self.dirty = true;}
        if category == Category::Media {self.dirty = true;}
        if matches!(category,Category::Default | Category::Disable) {self.dirty = initial.category != Some(category);}
        self.keyboard = initial.keyboard;
        self.keyboard_error = initial.keyboard_error;
        if category == Category::Keyboard {
            self.turbo = self.keyboard.turbo;
            self.speed = self.keyboard.speed;
            self.speed_draft = self.speed.to_string();
        }
    }
    pub fn option_labels(&self) -> Vec<String> {
        if self.category == Some(Category::Profile) {
            return self.context.navigation.as_ref().map(|c|if self.navigation.effects_menu {c.effects.iter().map(|(_,name)|name.clone()).collect()} else {c.targets().into_iter().map(|(guid,name)|if guid.is_some(){name}else{format!("{name} (identity unavailable)")}).collect()}).unwrap_or_default();
        }
        if self.category == Some(Category::Sensitivity) {return self.sensitivity_options().iter().map(|(_,label)|label.to_string()).collect();}
        if self.category == Some(Category::Keyboard) {
            if self.keyboard_group_menu { keyboard::GROUPS.iter().map(|s|s.to_string()).collect() }
            else {keyboard::keys(self.keyboard.key_group).iter().map(|k|format!("{} ({})",k.name,k.id.trim_start_matches("KEY_"))).collect()}
        } else {self.options().iter().map(|(_,label)|label.to_string()).collect()}
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
    pub fn sensitivity_options(&self) -> Vec<(&'static str, &'static str)> {
        secbird_core::sensitivity_assignment::OPTIONS
            .into_iter()
            .filter(|(id, _)| {
                let mut i = self.sensitivity.clone();
                i.assignment = (*id).into();
                i.allowed(self.context.owner.pid)
            })
            .collect()
    }
    pub fn dpi_change(&mut self, axis: usize, text: &str, blur: bool) {
        if axis > 1 || (axis == 1 && !self.sensitivity.independent) {
            return;
        }
        if !blur {
            if text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit()) {
                self.dpi_text[axis] = text.into();
                self.dirty = true;
            }
            return;
        }
        let Some(range) = &self.context.sensitivity_range else {
            return;
        };
        let previous = if axis == 0 {
            self.sensitivity.x
        } else {
            self.sensitivity.y
        };
        // Stock clutch numeric input has no stepDPI prop: clamp integers, don't round.
        let value = self.dpi_text[axis]
            .parse::<u32>()
            .unwrap_or(previous as u32)
            .clamp(range.min as u32, range.max as u32) as u16;
        if axis == 0 {
            self.sensitivity.x = value;
            if !self.sensitivity.independent {
                self.sensitivity.y = value;
            }
        } else {
            self.sensitivity.y = value;
        }
        self.dpi_text = [
            self.sensitivity.x.to_string(),
            self.sensitivity.y.to_string(),
        ];
    }
    pub fn sensitivity_valid(&self) -> bool {
        self.category == Some(Category::Sensitivity)
            && self
                .context
                .sensitivity_range
                .as_ref()
                .is_some_and(|range| {
                    self.sensitivity
                        .validate(self.context.owner.pid, range)
                        .is_ok()
                })
    }
    pub fn select_mouse(&mut self, index: usize) {
        if self.category == Some(Category::Media) {
            if index < secbird_core::profile_compat::multimedia::CHOICES.len() {
                self.media_option = Some(index);
                self.dirty = true;
                self.options_open = false;
            }
            return;
        }
        if index < MOUSE_ACTIONS.len() {
            self.mouse_option = index;
            self.mouse_choice_confirmed = true;
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
    pub fn undo(&mut self) {
        *self = Self::new(self.context.clone(), self.input.clone(), self.anchor.clone(), self.hyper);
    }
    pub fn local_profile(&self, current: &Value) -> Result<Value, String> {
        if !matches!(self.category,Some(Category::Default | Category::Disable)) || !self.can_save() { return Err("This assignment cannot be saved locally".into()); }
        if current != &self.context.original_profile { return Err("Profile changed; reopen the assignment".into()); }
        let mut result = current.clone();
        let rows = result.as_object_mut().ok_or("Invalid profile")?
            .entry("_synapseLiteLocalAssignments").or_insert_with(|| json!([]))
            .as_array_mut().ok_or("Unknown local assignment format; preserved")?;
        rows.retain(|r| r["inputID"] != self.input.input_id || truthy(&r["isHyperShift"]) != self.hyper);
        rows.push(json!({"inputID":self.input.input_id,"inputType":self.input.input_type,
            "isHyperShift":self.hyper,"outputType":self.category.unwrap().output().unwrap()}));
        Ok(result)
    }
    pub fn options(&self) -> &[(&str, &str)] {
        match self.category {Some(Category::Windows)=>secbird_core::profile_compat::windows_shortcuts::CHOICES,Some(Category::Media)=>&secbird_core::profile_compat::multimedia::CHOICES,_=>&MOUSE_ACTIONS}
    }
    pub fn selected_option(&self) -> Option<usize> {
        if self.category == Some(Category::Profile) {
            let catalog = self.context.navigation.as_ref()?;
            return if self.navigation.effects_menu {
                catalog.effects.iter().position(|(id,_)|Some(id)==self.navigation.effect.as_ref())
            } else {
                catalog.targets().iter().position(|(id,_)|id.is_some() && id==&self.navigation.guid)
            };
        }
        match self.category {Some(Category::Windows)=>self.windows_option,Some(Category::Media)=>self.media_option,_=>Some(self.mouse_option)}
    }
    pub fn can_save_windows(&self) -> bool {
        self.category == Some(Category::Windows) && self.dirty && self.windows_option.is_some()
            && self.context.mappings.is_some() && self.input.enabled && self.categories.contains(&Category::Windows)
            && secbird_core::profile_compat::windows_shortcuts::validate_input(self.context.owner.pid,&self.input.input_id,&self.input.input_type).is_ok()
    }
    pub fn select_option(&mut self, index: usize) {
        if self.category == Some(Category::Profile) {self.navigation_option(index); return;}
        if self.category == Some(Category::Windows) {
            if index < self.options().len() {self.windows_option=Some(index);self.dirty=true;self.options_open=false;}
        } else { self.select_mouse(index); }
    }
    pub fn media_edit(&self) -> Option<secbird_core::profile_compat::multimedia::Edit> {
        if self.category != Some(Category::Media) || !self.input.enabled {
            return None;
        }
        Some(secbird_core::profile_compat::multimedia::Edit {
            input_id: self.input.input_id.clone(),
            input_type: self.input.input_type.clone(),
            hyper: self.hyper,
            command: secbird_core::profile_compat::multimedia::CHOICES
                .get(self.media_option?)?
                .0
                .into(),
            expected: self.context.original_profile.clone(),
        })
    }
    pub fn media_valid(&self) -> bool {
        let Some(edit) = self.media_edit() else {
            return false;
        };
        serde_json::from_value::<secbird_core::profile::Profile>(
            self.context.original_profile.clone(),
        )
        .ok()
        .is_some_and(|profile| {
            secbird_core::profile_compat::multimedia::candidate(
                &profile,
                self.context.owner.pid,
                &edit,
            )
            .is_ok()
        })
    }
    pub fn assignment(&self) -> Result<secbird_core::profile_compat::mouse_assignment::Assignment, String> {
        use secbird_core::profile_compat::mouse_assignment::{Assignment, Choice};
        if self.category != Some(Category::Mouse) || !self.input.enabled || !self.categories.contains(&Category::Mouse) {
            return Err("Select a supported Mouse Function first".into());
        }
        if !self.mouse_choice_confirmed { return Err("Select a supported dropdown choice before replacing the retained mouse assignment".into()); }
        let assignment = Assignment {
            input_id: self.input.input_id.clone(), input_type: self.input.input_type.clone(), hyper: self.hyper,
            choice: Choice::parse(MOUSE_ACTIONS.get(self.mouse_option).ok_or("Unknown mouse choice")?.0).map_err(|e|e.to_string())?,
            turbo: if self.turbo && self.turbo_available() {Some(self.speed)} else {None},
        };
        assignment.validate(self.context.owner.pid).map_err(|e|e.to_string())?;
        let mut profile: secbird_core::Profile = serde_json::from_value(self.context.original_profile.clone()).map_err(|e|e.to_string())?;
        secbird_core::profile_compat::mouse_assignment::set_pending(&mut profile, self.context.owner.pid, &self.context.owner.profile, assignment.clone()).map_err(|e|e.to_string())?;
        Ok(assignment)
    }
    pub fn can_save(&self) -> bool {
        self.input.enabled && self.dirty && self.category.is_some_and(|category|self.categories.contains(&category) && match category {
            Category::Keyboard=>self.keyboard_profile().is_ok(), Category::Mouse=>self.assignment().is_ok(),
            Category::Media=>self.media_valid(), Category::Windows=>self.can_save_windows(),
            Category::Profile=>self.navigation_profile().is_ok(),
            Category::Launch=>self.launch_profile().is_ok(), Category::Default | Category::Disable=>true, _=>false,
        })
    }
    pub fn save_reason(&self) -> &'static str {
        if self.category == Some(Category::Profile) {return "Saves locally. Profile and linked lighting activation require verified input routing and recovery.";}
        if self.category == Some(Category::Media) {return "Saves locally; device and host output are unavailable.";}
        if self.category == Some(Category::Windows) {return "Saves locally; Windows shortcut execution is unavailable.";}
        if self.category == Some(Category::Launch) {return "Saves locally. Device trigger delivery is unverified; Save does not launch anything.";}
        if matches!(self.category,Some(Category::Default | Category::Disable)) {return "Saves locally for this profile and layer; hardware delivery is unavailable.";}
        if self.category == Some(Category::Mouse) && !self.mouse_choice_confirmed {
            "The retained mouse choice is unavailable. Select a dropdown choice to replace it; Cancel keeps the original."
        } else if self.category == Some(Category::Keyboard) {
            return "Saves the local desired profile only. Hardware delivery and readback are unavailable.";
        } else if self.category == Some(Category::Mouse) {
            "Save stores this profile's desired assignment locally. Hardware delivery is unavailable; no input or device command is sent."
        } else {
            "Saving this assignment category is unavailable. The original mapping is preserved."
        }
    }
    #[cfg(test)]
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
    pub dpi_drag: Option<(super::assignment_layout::Rect, usize)>,
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
    fn launch_draft() -> Draft {
        let mut i=input(); i.functions.push("LAUNCH_PROGRAM".into());
        let mut d=Draft::new(context(json!([
            {"inputID":"KEY_A","inputType":"KeyInput","outputType":"launchGroup","isHyperShift":false,"futureRow":[9],"launchGroup":{"path":"C:\\old.exe","future":{"keep":true}}},
            {"inputID":"KEY_A","isHyperShift":true,"outputType":"futureGroup","future":123}
        ])),i,Anchor::Key("KEY_A".into()),false);
        d.select(Category::Launch); d
    }
    #[test]
    fn launch_edit_preserves_opaque_fields_and_other_layer_through_backup() {
        let mut d=launch_draft();
        d.launch.path="C:\\Tools\\app.exe".into(); d.launch.arguments="--name \"two words\"".into();
        let next=d.launch_profile().unwrap();
        let rows=&next["_synapseLiteSynapse4"]["decoded"]["mappings"];
        assert_eq!(rows[0]["futureRow"],json!([9])); assert_eq!(rows[0]["launchGroup"]["future"]["keep"],true);
        assert_eq!(rows[1], d.context.mappings.as_ref().unwrap()[1]);
        let mut store=secbird_core::profile::Store::default();
        let device=store.profiles_for("025D:SIM",605); device.active="A".into();
        device.profiles.insert("A".into(),serde_json::from_value(next.clone()).unwrap());
        let bytes=store.native_bytes().unwrap();
        let imported=secbird_core::profile_compat::import_document(&bytes).unwrap();
        assert_eq!(serde_json::to_value(&imported.store.devices["025D:SIM"].profiles["A"]).unwrap(),next);
        d.launch.website=true; d.launch.url="example.com/path".into();
        let website=d.launch_profile().unwrap();
        let g=&website["_synapseLiteSynapse4"]["decoded"]["mappings"][0]["launchGroup"];
        assert_eq!(g["url"],"https://example.com/path"); assert!(g.get("path").is_none()); assert!(g.get("arguments").is_none());
        assert_eq!(g["future"]["keep"],true);
    }
    #[test]
    fn launch_stock_profile_export_import_roundtrip() {
        let mut profile=secbird_core::profile_compat::new_native_profile(605,"Launch fixture").unwrap();
        profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"]=json!([]);
        let ctx=Arc::new(Context::from_device(&json!({"id":"025D:SIM","pid":"025D","active":"Launch fixture","profile":profile})).unwrap());
        let mut i=input(); i.functions.push("LAUNCH_PROGRAM".into());
        let mut d=Draft::new(ctx,i,Anchor::Key("KEY_A".into()),false); d.select(Category::Launch);
        d.launch.website=true; d.launch.url="https://example.invalid/fixture".into();
        let next=d.launch_profile().unwrap();
        let mut store=secbird_core::profile::Store::default();
        let device=store.profiles_for("025D:SIM",605); device.active="Launch fixture".into(); device.profiles.clear();
        device.profiles.insert("Launch fixture".into(),serde_json::from_value(next.clone()).unwrap());
        let exported=secbird_core::profile_compat::export_synapse4(&store,"025D:SIM").unwrap();
        let imported=secbird_core::profile_compat::import_document(&exported.bytes).unwrap();
        let p=imported.store.devices.values().next().unwrap().profiles.values().next().unwrap();
        assert_eq!(p.extra["_synapseLiteSynapse4"]["decoded"]["mappings"],next["_synapseLiteSynapse4"]["decoded"]["mappings"]);
    }
    #[test]
    fn launch_rejects_empty_and_ambiguous_and_cancel_is_lossless() {
        let mut d=launch_draft(); let original=d.context.original_profile.clone();
        d.launch.path.clear(); assert!(d.launch_profile().is_err());
        d.launch.path="C:\\app.exe".into();
        d.original_records[0]["launchGroup"]["arguments"]=json!(["future"]); assert!(d.launch_profile().is_err());
        d.original_records[0]["launchGroup"].as_object_mut().unwrap().remove("arguments");
        d.original_records.push(json!({})); assert!(d.launch_profile().is_err());
        let mut view=View { draft: Some(d), ..View::default() }; view.cancel(); assert!(view.draft.is_none());
        assert_eq!(original["opaque"],json!({"keep":[1,2]}));
    }
    #[test]
    fn disable_restore_layers_roundtrip_preserves_source_and_opaque_metadata() {
        let ctx = context(json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","opaque":17}]));
        let mut d = Draft::new(ctx.clone(), input(), Anchor::Key("KEY_A".into()), false);
        d.select(Category::Disable);
        let disabled = d.local_profile(&ctx.original_profile).unwrap();
        assert_eq!(disabled["_synapseLiteSynapse4"], ctx.original_profile["_synapseLiteSynapse4"]);
        assert_eq!(disabled["opaque"], ctx.original_profile["opaque"]);
        let device = json!({"id":"025D:SIM","pid":"025D","active":"A","profile":disabled});
        let ctx2 = Arc::new(Context::from_device(&device).unwrap());
        let standard = Draft::new(ctx2.clone(), input(), Anchor::Key("KEY_A".into()), false);
        assert_eq!(standard.category, Some(Category::Disable));
        let mut hyper = Draft::new(ctx2.clone(), input(), Anchor::Key("KEY_A".into()), true);
        assert_eq!(hyper.category, Some(Category::Default));
        hyper.select(Category::Disable);
        let both = hyper.local_profile(&ctx2.original_profile).unwrap();
        assert_eq!(both["_synapseLiteLocalAssignments"].as_array().unwrap().len(), 2);
        let mut restore = standard;
        restore.select(Category::Default);
        let restored = restore.local_profile(&ctx2.original_profile).unwrap();
        assert_eq!(restored["_synapseLiteLocalAssignments"][0]["outputType"], "defaultGroup");
        let path = std::env::temp_dir().join(format!("assignment-local-{}.json",std::process::id()));
        let mut store = secbird_core::profile::Store::load(&path).unwrap();
        let profile = serde_json::from_value(restored.clone()).unwrap();
        store.profiles_for("025D:SIM",605).profiles.insert("A".into(),profile);
        store.save().unwrap();
        let loaded = secbird_core::profile::Store::load(&path).unwrap();
        let value = serde_json::to_value(&loaded.devices["025D:SIM"].profiles["A"]).unwrap();
        assert_eq!(value["_synapseLiteLocalAssignments"],restored["_synapseLiteLocalAssignments"]);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn immutable_stale_cancel_and_undo_refuse_changes() {
        let ctx = context(json!([]));
        let mut i = input(); i.enabled = false;
        let mut locked = Draft::new(ctx.clone(), i, Anchor::Key("KEY_A".into()), false);
        locked.select(Category::Disable);
        assert!(!locked.can_save());
        let mut d = Draft::new(ctx.clone(), input(), Anchor::Key("KEY_A".into()), false);
        d.select(Category::Disable);
        assert!(d.local_profile(&json!({})).is_err());
        d.undo(); assert!(!d.dirty); assert_eq!(d.category,Some(Category::Default));
        d.select(Category::Disable);
        let mut view = View { draft: Some(d), ..View::default() }; view.cancel();
        assert!(view.draft.is_none()); assert!(ctx.original_profile.get("_synapseLiteLocalAssignments").is_none());
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
        assert!(d.dirty);
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
        assert!(d.dirty);
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
    #[test]
    fn missing_source_stays_unknown_until_explicit_mouse_authoring() {
        let raw = json!({"id":"00C4:SIM","pid":"00C4","active":"Default","profile":{"future":{"keep":true}}});
        let ctx = Arc::new(Context::from_device(&raw).unwrap());
        assert!(ctx.mappings.is_none());
        let mut draft = Draft::new(ctx.clone(), mouse_inputs().remove(1), Anchor::Mouse(1), false);
        assert_eq!(draft.category, None);
        assert!(!draft.can_save());
        draft.select(Category::Mouse);
        draft.select_mouse(3);
        assert!(draft.can_save());
        assert_eq!(ctx.original_profile, raw["profile"]);
    }
    #[test]
    fn unknown_mouse_choice_needs_explicit_selection_before_save() {
        let ctx=context(json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"FutureChoice","future":{"keep":true}}}]));
        let mut d=Draft::new(ctx.clone(),input(),Anchor::Key("KEY_A".into()),false);
        assert_eq!(d.category,Some(Category::Mouse));
        assert!(!d.mouse_choice_confirmed);
        d.toggle_turbo();d.set_speed(9);
        assert!(!d.can_save());
        d.select_mouse(3);
        assert!(d.can_save());
        assert_eq!(ctx.original_profile["_synapseLiteSynapse4"]["decoded"]["mappings"][0]["mouseGroup"]["mouseAssignment"],"FutureChoice");
    }

    #[test]
    fn multimedia_unknown_is_retained_until_explicit_selection_and_cancel_is_local() {
        let raw = json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":"FutureCommand","opaque":42}}]);
        let ctx = context(raw.clone());
        let mut input = input();
        input.functions.push("MULTIMEDIA".into());
        let mut d = Draft::new(ctx.clone(), input, Anchor::Key("KEY_A".into()), false);
        assert_eq!(d.category, Some(Category::Media));
        assert_eq!(d.media_option, None);
        assert!(!d.media_valid());
        d.select_mouse(99);
        assert_eq!(d.media_option, None);
        assert!(!d.dirty);
        d.select_mouse(7);
        assert_eq!(d.media_edit().unwrap().command, "Play");
        assert!(d.media_valid());
        assert!(d.dirty);
        let mut view = View { draft: Some(d), ..View::default() };
        view.cancel();
        assert!(view.draft.is_none());
        assert_eq!(ctx.mappings.as_ref().unwrap(), raw.as_array().unwrap());
    }
}

#[cfg(test)]
mod windows_tests {
    use super::*;
    #[test]
    fn windows_reopens_known_and_unknown_choices_without_replacing_source() {
        for (id,expected) in [("Copy",Some(13)),("FutureShortcut",None)] {
            let profile=json!({"_synapseLiteSynapse4":{"decoded":{"mappings":[{"inputID":"KEY_A","inputType":"KeyInput","outputType":"win8ShortcutsGroup","win8ShortcutsGroup":{"windowsShortcutAssignment":id,"opaque":8}}]}}});
            let ctx=Arc::new(Context::from_device(&json!({"id":"SIM","pid":"025D","active":"Fixture","profile":profile})).unwrap());
            let input=Input{key:"A".into(),input_id:"KEY_A".into(),input_type:"KeyInput".into(),label:"A".into(),default_label:"A".into(),functions:vec!["WINDOWS_SHORTCUT".into(),"DEFAULT".into()],enabled:true};
            let mut draft=Draft::new(ctx,input,Anchor::Key("KEY_A".into()),false);
            assert_eq!(draft.category,Some(Category::Windows));assert_eq!(draft.windows_option,expected);assert!(!draft.can_save_windows());
            draft.select_option(25);assert!(draft.can_save_windows());assert_eq!(draft.original_records[0]["win8ShortcutsGroup"]["windowsShortcutAssignment"],id);
            draft.select(Category::Default);draft.select(Category::Windows);assert_eq!(draft.windows_option,expected);assert!(!draft.dirty);
        }
    }
}

/// Stock MapLaunch uses mutually exclusive path/url. Arguments are a local extension.
#[derive(Clone, Debug, Default)]
pub(super) struct Launch { pub website: bool, pub path: String, pub url: String, pub arguments: String }
impl Launch {
    fn read(group: Option<&Value>) -> Self {
        let g = group.unwrap_or(&Value::Null);
        Self { website: g.get("path").is_none() && g.get("url").is_some(), path: g["path"].as_str().unwrap_or("").into(), url: g["url"].as_str().unwrap_or("").into(), arguments: g["arguments"].as_str().unwrap_or("").into() }
    }
    pub fn valid(&self) -> bool {
        let target = if self.website { &self.url } else { &self.path };
        !target.trim().is_empty() && !target.contains(['\0','\r','\n']) && !self.arguments.contains('\0')
    }
}
impl Draft {
    pub fn launch_profile(&self) -> Result<Value, String> {
        if self.category != Some(Category::Launch) || !self.launch.valid() { return Err("Choose a program or enter a website".into()); }
        if let Some(group) = self.original_records.first().and_then(|r| r.get("launchGroup")) {
            if !group.is_object() || ["path", "url", "arguments"].iter().any(|key| group.get(*key).is_some_and(|v| !v.is_string())) {
                return Err("Unrecognized launch field types are preserved; Save is unavailable".into());
            }
        }
        let mut profile = self.context.original_profile.clone();
        let mappings = profile["_synapseLiteSynapse4"]["decoded"]["mappings"].as_array_mut().ok_or("Source mapping context is unavailable")?;
        if self.original_records.len() > 1 { return Err("Ambiguous assignment records are preserved; Save is unavailable".into()); }
        let mut row = self.original_records.first().cloned().unwrap_or_else(|| json!({}));
        row["inputID"] = json!(self.input.input_id);
        row["inputType"] = json!(self.input.input_type);
        row["isHyperShift"] = json!(self.hyper);
        row["outputType"] = json!("launchGroup");
        if !row["launchGroup"].is_object() { row["launchGroup"] = json!({}); }
        let group = row["launchGroup"].as_object_mut().unwrap();
        if self.launch.website {
            group.remove("path"); group.remove("arguments");
            let url = self.launch.url.trim();
            group.insert("url".into(), json!(if url.contains("://") { url.to_string() } else { format!("https://{url}") }));
        } else {
            group.remove("url"); group.insert("path".into(), json!(self.launch.path));
            if !self.launch.arguments.is_empty() { group.insert("arguments".into(), json!(self.launch.arguments)); }
            else { group.remove("arguments"); }
        }
        if let Some(i) = mappings.iter().position(|r| r["inputID"].as_str() == Some(&self.input.input_id) && truthy(&r["isHyperShift"]) == self.hyper) { mappings[i] = row; } else { mappings.push(row); }
        Ok(profile)
    }

}

#[cfg(test)]
mod sensitivity_tests {
    use super::*;
    #[test]
    fn clutch_numeric_bounds_filters_and_reopen_pending_intent() {
        let device = json!({"id":"00C4:SIM","pid":"00C4","active":"A","sensitivity_assignment":{"min":100,"max":26000,"step":50},"profile":{"_synapseLiteSynapse4":{"decoded":{"mappings":[]}}}});
        let ctx = Arc::new(Context::from_device(&device).unwrap());
        let mut d = Draft::new(ctx, mouse_inputs().remove(5), Anchor::Mouse(5), false);
        d.select(Category::Sensitivity);
        assert_eq!(d.sensitivity_options().len(), 6);
        d.dpi_change(0, "401", false);
        d.dpi_change(0, "", true);
        assert_eq!((d.sensitivity.x, d.sensitivity.y), (401, 401));
        d.dpi_change(0, "99999", false);
        d.dpi_change(0, "", true);
        assert_eq!(d.sensitivity.x, 26000);
        d.dpi_change(0, "1e3", false);
        assert_eq!(d.dpi_text[0], "26000");
        d.dpi_change(0, "", false);
        d.dpi_change(0, "", true);
        assert_eq!(d.sensitivity.x, 26000);
        let mut saved = device.clone();
        saved["profile"][secbird_core::sensitivity_assignment::DRAFTS] = json!([d.sensitivity]);
        let reopened = Draft::new(
            Arc::new(Context::from_device(&saved).unwrap()),
            d.input.clone(),
            Anchor::Mouse(5),
            false,
        );
        assert_eq!(reopened.category, Some(Category::Sensitivity));
        assert_eq!(reopened.sensitivity.x, 26000);
        assert!(!reopened.dirty);
        let cycle = Draft::new(
            Arc::new(Context::from_device(&device).unwrap()),
            mouse_inputs().remove(7),
            Anchor::Mouse(7),
            false,
        );
        assert_eq!(
            cycle
                .sensitivity_options()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec!["DPI_Up", "DPI_Down", "DPI_CycleUp", "DPI_CycleDown"]
        );
        let hyper = Draft::new(
            Arc::new(Context::from_device(&device).unwrap()),
            d.input,
            Anchor::Mouse(5),
            true,
        );
        assert_eq!(hyper.sensitivity_options().len(), 5);
    }
}
