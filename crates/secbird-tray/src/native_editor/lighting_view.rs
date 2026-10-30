//! Source-derived Ornata Lighting UI. Parent alone dispatches ownership-bound
//! commands; source effect IDs are never interpreted as firmware command IDs.
use super::{
    color_editor_layout as color, color_editor_view, lighting_layout as source,
    performance_layout::Rect, performance_view as draw, *,
};
use secbird_core::lighting::LightingSettings;

pub(super) const HEX: u16 = 1500;
pub(super) const RED: u16 = 1501;
pub(super) const GRN: u16 = 1502;
pub(super) const BLUE: u16 = 1503;
pub(super) const BOOST: u16 = 1504;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Owner {
    pub device: String,
    pub profile: String,
}
#[derive(Clone, Copy)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Copy)]
pub(super) enum Scope {
    Brightness,
    Policy,
}
#[derive(Clone)]
pub(super) enum Command {
    OpenStudio,
    CommitIntent {
        owner: Owner,
        settings: LightingSettings,
        scope: Scope,
    },
    ApplyEffect {
        owner: Owner,
        source_id: u8,
        setting: Value,
    },
    PersistCustomColors {
        owner: Owner,
        slots: Vec<Value>,
    },
    Eyedropper,
}
#[derive(Clone, Copy, Default)]
pub(super) enum Capture {
    #[default]
    None,
    Start,
    Release,
}
#[derive(Default)]
pub(super) struct Update {
    pub handled: bool,
    pub rebuild_fields: bool,
    pub capture: Capture,
    pub commands: Vec<Command>,
}
impl Update {
    fn handled() -> Self {
        Self {
            handled: true,
            ..Self::default()
        }
    }
    fn rebuild() -> Self {
        Self {
            handled: true,
            rebuild_fields: true,
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum InputEvent {
    Focus,
    Change,
    Blur,
    Enter,
    Escape,
}
#[derive(Clone)]
pub(super) enum Control {
    OpenStudio,
    BrightnessToggle,
    DisplayToggle,
    IdleToggle,
    BrightnessSlider(source::Rect),
    IdleSlider(source::Rect),
    Mode(bool),
    EffectMenu,
    EffectScroll(source::Rect),
    SelectEffect(u8),
    ColorMenu(usize),
    Preset(usize),
    Custom(usize),
    EditCustom,
    DeleteCustom,
    Random,
    Direction,
    Screen(&'static str),
    Duration(source::Rect),
    BoostStep(i16),
    EditorCancel,
    EditorSave,
    EditorPlane(color::Rect),
    EditorValue(color::Rect),
    EditorEye,
}
#[derive(Clone)]
enum GestureKind {
    Brightness(source::Rect),
    Idle(source::Rect),
    Duration(source::Rect),
    Plane(color::Rect),
    Value(color::Rect),
    EffectScroll { track: source::Rect, grab: f64 },
}
struct Gesture {
    kind: GestureKind,
    before: LightingSettings,
    before_effect: Value,
}

pub(super) struct View {
    pub owner: Option<Owner>,
    pub intent: LightingSettings,
    pub scroll: f64,
    known: bool,
    smoke_effect_override: bool,
    runtime: Value,
    selected: Option<u8>,
    setting: Value,
    cache: HashMap<u8, Value>,
    slots: Vec<Value>,
    advanced: bool,
    effect_menu: bool,
    effect_offset: f64,
    #[cfg(test)]
    test_commands: Option<Vec<Value>>,
    palette: Option<usize>,
    palette_up: bool,
    context_slot: Option<usize>,
    editor: Option<color_editor_view::Editor>,
    gesture: Option<Gesture>,
    notice: String,
    intent_dirty: bool,
    effect_dirty: bool,
    slots_dirty: bool,
    rebuilding: bool,
    focused: Option<u16>,
    boost_draft: Option<String>,
    repeat: Option<Control>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            owner: None,
            intent: LightingSettings::default(),
            scroll: 0.0,
            known: false,
            smoke_effect_override: false,
            runtime: Value::Null,
            selected: None,
            setting: json!({}),
            cache: HashMap::new(),
            slots: empty_slots(),
            advanced: false,
            effect_menu: false,
            effect_offset: 0.0,
            #[cfg(test)]
            test_commands: None,
            palette: None,
            palette_up: false,
            context_slot: None,
            editor: None,
            gesture: None,
            notice: String::new(),
            intent_dirty: false,
            effect_dirty: false,
            slots_dirty: false,
            rebuilding: false,
            focused: None,
            boost_draft: None,
            repeat: None,
        }
    }
}
fn empty_slots() -> Vec<Value> {
    (0..16)
        .map(|_| json!({"color":"#111","picked":false}))
        .collect()
}
fn preset_colors() -> Vec<&'static str> {
    vec![
        "#ffffff", "#ffc0c0", "#ffe0c0", "#ffffc0", "#c0ffc0", "#c0ffff", "#c0c0ff", "#ffc0ff",
        "#e0e0e0", "#ff8080", "#ffc182", "#ffff80", "#80ff80", "#80ffff", "#8080ff", "#ff80ff",
        "#c0c0c0", "#ff0000", "#ff8000", "#ffff00", "#00ff00", "#00ffff", "#0000ff", "#ff00ff",
        "#808080", "#c00000", "#c06000", "#c0c000", "#00c000", "#00c0c0", "#0000c0", "#c000c0",
        "#404040", "#800000", "#804000", "#808000", "#008000", "#008080", "#000080", "#800080",
        "no-color",
    ]
}
fn defaults(id: u8) -> Value {
    match id {
        1 | 6 => json!({"color1":"#00ff00"}),
        2 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false}),
        5 => json!({"color1":"#00ff00","duration":2}),
        7 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false,"duration":2}),
        4 => json!({"direction":2}),
        13 => json!({"direction":1}),
        19 => json!({"color1":"#00ff00","color2":"#0000FF","isRandom":false,"direction":1}),
        11 => json!({"screen":"full"}),
        12 => json!({"color1":"#00ff00","colorBoost":1}),
        _ => json!({}),
    }
}
fn slot_colors(slots: &[Value]) -> Vec<Value> {
    slots
        .iter()
        .filter(|s| s["color"] != "#111")
        .cloned()
        .collect()
}

pub(super) fn sync(view: &mut View, device: &Value, preserve_local: bool) {
    let owner = match (device["id"].as_str(), device["active"].as_str()) {
        (Some(device), Some(profile)) => Some(Owner {
            device: device.into(),
            profile: profile.into(),
        }),
        _ => None,
    };
    let owner_changed = view.owner != owner;
    if owner_changed {
        *view = View::default();
        view.owner = owner;
    }
    view.runtime = device["lighting_runtime"].clone();
    let preserve = !owner_changed
        && (preserve_local
            || view.gesture.is_some()
            || view.repeat.is_some()
            || view.focused.is_some()
            || view.editor.is_some());
    if let Ok(intent) =
        serde_json::from_value::<LightingSettings>(device["lighting_settings"].clone())
    {
        if intent.validate().is_ok() {
            view.known = true;
            if !preserve {
                view.intent_dirty = false;
                view.intent = intent.clone()
            }
            if view.intent == intent {
                view.intent_dirty = false
            }
        }
    }
    if let Some(quick) = device["source_quick_effects"].as_object() {
        let selected = quick
            .get("selectedEffectId")
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok());
        let setting = quick
            .get("selectedEffectSetting")
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !preserve {
            view.effect_dirty = false;
            view.boost_draft = None;
            view.selected = selected;
            view.setting = setting.clone();
            if let Some(id) = selected {
                view.cache.insert(id, setting.clone());
            }
        }
        if selected == view.selected && setting == view.setting {
            view.effect_dirty = false;
        }
        if let Some(slots) = quick.get("customColors").and_then(Value::as_array) {
            if !preserve {
                view.slots_dirty = false;
                view.slots = empty_slots();
                for (i, value) in slots.iter().take(16).enumerate() {
                    if value.is_object() {
                        view.slots[i] = value.clone();
                        // Keep imported opaque slot objects exactly unchanged.
                        // Only a newly edited slot changes its color/picked fields.
                    }
                }
            }
            if slot_colors(&view.slots) == *slots {
                view.slots_dirty = false;
            }
        }
    } else if !preserve {
        view.selected = None;
        view.setting = json!({});
        view.cache.clear();
        view.effect_dirty = false;
    }
}

/// A cached observation is not a profile/intent snapshot. Never call sync or
/// rebuild native EDITs here: a pending gesture, draft and modal stay untouched.
pub(super) fn update_runtime_only(view: &mut View, runtime: Value) -> bool {
    if view.runtime == runtime {
        return false;
    }
    view.runtime = runtime;
    true
}
pub(super) fn runtime_snapshot(view: &View) -> &Value {
    &view.runtime
}
pub(super) fn runtime_feedback_busy(view: &View) -> bool {
    view.gesture.is_some() || view.repeat.is_some() || view.focused.is_some()
}

pub(super) fn active(state: &State) -> bool {
    state.tab == Tab::Lighting && state.device().is_some_and(|d| d["pid"] == "025D")
}
fn geometry(state: &State) -> Option<source::LightingGeometry> {
    source::geometry(
        (f64::from(state.width) / f64::from(state.scale)).max(1000.0),
        128.0,
        state.lighting.scroll,
        state
            .lighting
            .selected
            .filter(|id| source::effect_panel_height(*id).is_some())
            .unwrap_or(3),
    )
}
fn palette_geometry(state: &State) -> Option<color::Geometry> {
    let g = geometry(state)?;
    let anchor = g.colors.get(state.lighting.palette?)?.dropdown;
    color::geometry_with_direction(
        color::Rect {
            x: anchor.x,
            y: anchor.y,
            width: anchor.width,
            height: anchor.height,
        },
        state.lighting.palette_up,
    )
}
fn intent_command(view: &mut View, scope: Scope) -> Update {
    let mut u = Update::handled();
    if let Some(owner) = view.owner.clone() {
        view.intent_dirty = true;
        u.commands.push(Command::CommitIntent {
            owner,
            settings: view.intent.clone(),
            scope,
        })
    }
    u
}
fn effect_command(view: &mut View) -> Update {
    let mut u = Update::handled();
    if let (Some(owner), Some(id)) = (view.owner.clone(), view.selected) {
        view.cache.insert(id, view.setting.clone());
        view.effect_dirty = true;
        view.notice = "Effect preview; backend application must be confirmed.".into();
        #[cfg(test)]
        if let Some(commands) = &mut view.test_commands {
            commands.push(json!({"device":owner.device,"profile":owner.profile,"source_id":id,"setting":view.setting}));
            return u;
        }
        u.commands.push(Command::ApplyEffect {
            owner,
            source_id: id,
            setting: view.setting.clone(),
        });
    }
    u
}
fn slots_command(view: &mut View) -> Update {
    let mut u = Update::handled();
    if let Some(owner) = view.owner.clone() {
        view.slots_dirty = true;
        u.commands.push(Command::PersistCustomColors {
            owner,
            slots: view.slots.clone(),
        })
    }
    u
}
fn set_setting(view: &mut View, key: &str, value: Value) -> Update {
    if !view.setting.is_object() {
        view.setting = json!({})
    }
    view.setting[key] = value;
    effect_command(view)
}
fn set_color(view: &mut View, value: &str) -> Update {
    let key = if view.palette == Some(1) {
        "color2"
    } else {
        "color1"
    };
    view.editor = None;
    view.palette = None;
    view.context_slot = None;
    let mut u = set_setting(view, key, json!(value));
    u.rebuild_fields = true;
    u
}
fn open_editor(state: &mut State, requested: usize) -> Update {
    let picked: Vec<bool> = state
        .lighting
        .slots
        .iter()
        .map(|s| s["color"] != "#111")
        .collect();
    let Some(slot) = color::picker_target(&picked, requested) else {
        return Update::handled();
    };
    let value = state.lighting.slots[slot]["color"]
        .as_str()
        .filter(|v| *v != "#111");
    if value.is_some_and(|v| {
        v.trim_start_matches('#').len() != 6
            || !v
                .trim_start_matches('#')
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
    }) {
        state.lighting.notice =
            "Imported custom-color format is unsupported; its value is retained.".into();
        return Update::handled();
    }
    let editor = color_editor_view::Editor::new(slot, value, &state.assets);
    if !editor.exact_assets {
        state.lighting.notice =
            "Source canvas assets missing; gradient interpolation is unverified.".into();
    }
    state.lighting.editor = Some(editor);
    state.lighting.context_slot = None;
    Update::rebuild()
}

/// Smoke-only presentation entry. The owner comes from the actual snapshot;
/// no action command, intent update, persistence or hardware submission occurs.
/// A source Static presentation is allowed when the current effect has no color
/// control. That override is explicitly reported and never applied to a profile.
pub(super) unsafe fn open_read_only_smoke_editor(state: &mut State) -> bool {
    if state.smoke_requested.as_deref() != Some("keyboard-lighting-color-editor")
        || !active(state)
        || state.lighting.owner.is_none()
    {
        return false;
    }
    let needs_color = state.lighting.advanced
        || state.lighting.setting["isRandom"] == true
        || geometry(state).is_none_or(|g| g.colors.is_empty());
    if needs_color {
        state.lighting.selected = Some(1);
        state.lighting.setting = defaults(1);
        state.lighting.advanced = false;
        state.lighting.smoke_effect_override = true;
    }
    let slot = state.lighting.slots.iter().position(|s| {
        s["color"] == "#111"
            || s["color"].as_str().is_some_and(|v| {
                let text = v.trim_start_matches('#');
                text.len() == 6 && text.bytes().all(|b| b.is_ascii_hexdigit())
            })
    });
    let Some(slot) = slot else {
        return false;
    };
    state.lighting.scroll = 0.0;
    if let Some(g) = geometry(state) {
        state.lighting.scroll = viewport::card_scroll_max(
            g.effects_panel.y + g.effects_panel.height,
            state.content_bottom_css(),
        );
    }
    let palette = action(state, Control::ColorMenu(0));
    debug_assert!(palette.commands.is_empty());
    if state.lighting.palette.is_none() {
        return false;
    }
    let editor = open_editor(state, slot);
    debug_assert!(editor.commands.is_empty());
    state.lighting.editor.is_some()
}

/// Only state/counts are exported; no color values, profile names or identities.
pub(super) fn smoke_metadata(state: &State) -> Value {
    let v = &state.lighting;
    let anchor = palette_geometry(state).is_some();
    json!({
        "palette_open":v.palette.is_some(),
        "source_anchor_available":anchor,
        "color_editor_open":v.editor.is_some(),
        "color_editor_exact_canvas_assets":v.editor.as_ref().is_some_and(|e|e.exact_assets),
        "color_editor_inputs":state.fields.keys().filter(|id|(HEX..BLUE+1).contains(id)).count(),
        "custom_slot_count":v.slots.len(),
        "presentation_only_effect_override":v.smoke_effect_override,
        "scroll_css":v.scroll,
        "ready":active(state)&&v.owner.is_some()&&anchor&&v.editor.as_ref().is_some_and(|e|e.exact_assets)
            && state.fields.keys().filter(|id|(HEX..BLUE+1).contains(id)).count()==4
    })
}

pub(super) unsafe fn action(state: &mut State, control: Control) -> Update {
    if !active(state) {
        return Update::default();
    }
    match control {
        Control::OpenStudio => Update {
            handled: true,
            commands: vec![Command::OpenStudio],
            ..Update::default()
        },
        Control::BrightnessToggle => {
            if let Some(b) = &mut state.lighting.intent.brightness {
                b.set_enabled(!b.enabled);
                intent_command(&mut state.lighting, Scope::Brightness)
            } else {
                state.lighting.notice =
                    "Source brightness enable state is unknown; choose a slider value first."
                        .into();
                Update::handled()
            }
        }
        Control::DisplayToggle | Control::IdleToggle => {
            if !state.lighting.intent.policy_controls_enabled() {
                return Update::handled();
            }
            let policy = state
                .lighting
                .intent
                .switch_off
                .get_or_insert_with(Default::default);
            if matches!(control, Control::DisplayToggle) {
                policy.display_off = !policy.display_off
            } else {
                policy.idle_enabled = !policy.idle_enabled
            }
            intent_command(&mut state.lighting, Scope::Policy)
        }
        Control::Mode(advanced) => {
            state.lighting.advanced = advanced;
            state.lighting.effect_menu = false;
            state.lighting.palette = None;
            state.lighting.editor = None;
            state.lighting.notice.clear();
            Update::rebuild()
        }
        Control::EffectMenu => {
            state.lighting.effect_menu = !state.lighting.effect_menu;
            if state.lighting.effect_menu {
                state.lighting.effect_offset =
                    source::effect_selected_scroll(state.lighting.selected);
            }
            state.lighting.palette = None;
            state.lighting.editor = None;
            Update::rebuild()
        }
        Control::SelectEffect(id) => {
            if !source::QUICK_EFFECTS.iter().any(|e| e.0 == id) {
                return Update::handled();
            }
            if let Some(old) = state.lighting.selected {
                state
                    .lighting
                    .cache
                    .insert(old, state.lighting.setting.clone());
            }
            state.lighting.boost_draft = None;
            state.lighting.selected = Some(id);
            state.lighting.setting = state
                .lighting
                .cache
                .get(&id)
                .cloned()
                .unwrap_or_else(|| defaults(id));
            state.lighting.effect_menu = false;
            state.lighting.palette = None;
            state.lighting.editor = None;
            let mut u = effect_command(&mut state.lighting);
            u.rebuild_fields = true;
            u
        }
        Control::ColorMenu(which) => {
            if state.lighting.setting["isRandom"] == true {
                return Update::handled();
            }
            if state.lighting.palette == Some(which) {
                state.lighting.palette = None
            } else if let Some(g) = geometry(state) {
                if let Some(anchor) = g.colors.get(which) {
                    state.lighting.palette = Some(which);
                    state.lighting.palette_up = anchor.dropdown.y + 27.0 + 252.0
                        >= f64::from(state.height) / f64::from(state.scale);
                }
            }
            state.lighting.effect_menu = false;
            state.lighting.editor = None;
            state.lighting.context_slot = None;
            Update::rebuild()
        }
        Control::Preset(index) => {
            let values = preset_colors();
            if let Some(value) = values.get(index) {
                set_color(&mut state.lighting, value)
            } else {
                Update::handled()
            }
        }
        Control::Custom(index) => {
            if index >= 16 {
                return Update::handled();
            }
            let value = state.lighting.slots[index]["color"]
                .as_str()
                .unwrap_or("#111")
                .to_string();
            if value == "#111" {
                open_editor(state, index)
            } else {
                set_color(&mut state.lighting, &value)
            }
        }
        Control::EditCustom => {
            if let Some(slot) = state.lighting.context_slot {
                open_editor(state, slot)
            } else {
                Update::handled()
            }
        }
        Control::DeleteCustom => {
            if let Some(index) = state.lighting.context_slot.take() {
                state.lighting.slots.remove(index);
                state
                    .lighting
                    .slots
                    .push(json!({"color":"#111","picked":false}));
                slots_command(&mut state.lighting)
            } else {
                Update::handled()
            }
        }
        Control::Random => {
            let random = state.lighting.setting["isRandom"] != true;
            let mut u = set_setting(&mut state.lighting, "isRandom", json!(random));
            u.rebuild_fields = true;
            u
        }
        Control::Direction => {
            let next = source::direction_toggle(
                state.lighting.selected.unwrap_or(4),
                state.lighting.setting["direction"].as_i64(),
            );
            set_setting(&mut state.lighting, "direction", json!(next))
        }
        Control::Screen(value) => set_setting(&mut state.lighting, "screen", json!(value)),
        Control::BoostStep(delta) => {
            let value = state
                .lighting
                .boost_draft
                .as_deref()
                .map(|v| v.parse::<f64>().unwrap_or(0.0))
                .unwrap_or_else(|| state.lighting.setting["colorBoost"].as_f64().unwrap_or(1.0));
            let value = (value + f64::from(delta) * 0.25).clamp(0.25, 4.0);
            state.lighting.boost_draft = Some(value.to_string());
            let mut u = set_setting(&mut state.lighting, "colorBoost", json!(value));
            u.rebuild_fields = true;
            u
        }
        Control::EditorCancel => {
            state.lighting.editor = None;
            Update::rebuild()
        }
        Control::EditorSave => {
            let Some(editor) = state.lighting.editor.as_ref() else {
                return Update::handled();
            };
            let Some(value) = editor.save_color() else {
                state.lighting.notice = "Finish all RGB channels before saving.".into();
                return Update::handled();
            };
            let slot = editor.slot;
            state.lighting.slots[slot]["color"] = json!(value);
            state.lighting.slots[slot]["picked"] = json!(true);
            let mut u = slots_command(&mut state.lighting);
            let applied = set_color(&mut state.lighting, &value);
            u.commands.extend(applied.commands);
            u.rebuild_fields = true;
            u
        }
        Control::EditorEye => {
            state.lighting.notice =
                "Native eyedropper is not available; no desktop capture was started.".into();
            let mut u = Update::handled();
            u.commands.push(Command::Eyedropper);
            u
        }
        Control::BrightnessSlider(_)
        | Control::IdleSlider(_)
        | Control::EffectScroll(_)
        | Control::Duration(_)
        | Control::EditorPlane(_)
        | Control::EditorValue(_) => Update::handled(),
    }
}

pub(super) unsafe fn begin(state: &mut State, control: Control, point: Point) -> Update {
    if !active(state) {
        return Update::default();
    }
    if matches!(control, Control::BoostStep(_)) {
        state.lighting.repeat = Some(control.clone());
        let mut update = action(state, control);
        update.capture = Capture::Start;
        return update;
    }
    let kind = match control {
        Control::EffectScroll(track) => {
            let g = geometry(state).unwrap();
            let d = source::effect_dropdown(
                g.effect_dropdown,
                f64::from(state.height) / f64::from(state.scale),
                state.lighting.effect_offset,
            );
            // This 12-row list has less than one page of scroll range, so a
            // Chromium track page-step always reaches the corresponding end.
            if point.y < d.thumb.y || point.y >= d.thumb.y + d.thumb.height {
                state.lighting.effect_offset = if point.y < d.thumb.y {
                    0.0
                } else {
                    d.max_scroll
                };
                return Update::handled();
            }
            Some(GestureKind::EffectScroll {
                track,
                grab: point.y - d.thumb.y,
            })
        }
        Control::BrightnessSlider(r) => Some(GestureKind::Brightness(r)),
        Control::IdleSlider(r) if state.lighting.intent.idle_slider_enabled() => {
            Some(GestureKind::Idle(r))
        }
        Control::Duration(r) => Some(GestureKind::Duration(r)),
        Control::EditorPlane(r) => Some(GestureKind::Plane(r)),
        Control::EditorValue(r) => Some(GestureKind::Value(r)),
        _ => None,
    };
    if let Some(kind) = kind {
        state.lighting.gesture = Some(Gesture {
            kind,
            before: state.lighting.intent.clone(),
            before_effect: state.lighting.setting.clone(),
        });
        let mut u = pointer(state, point);
        u.capture = Capture::Start;
        u
    } else {
        // Click-only controls execute once through action on mouse release.
        Update::default()
    }
}
fn slider_value(point: Point, r: source::Rect, min: f64, max: f64) -> f64 {
    ((point.x - r.x - 8.0) / (r.width - 16.0))
        .clamp(0.0, 1.0)
        .mul_add(max - min, min)
        .round()
}
pub(super) unsafe fn pointer(state: &mut State, point: Point) -> Update {
    if !active(state) {
        return Update::default();
    }
    let Some(kind) = state.lighting.gesture.as_ref().map(|g| g.kind.clone()) else {
        return Update::handled();
    };
    match kind {
        GestureKind::EffectScroll { track, grab } => {
            let thumb = track.height * 178.0 / 300.0;
            state.lighting.effect_offset =
                ((point.y - track.y - grab) / (track.height - thumb) * 122.0).clamp(0.0, 122.0);
        }
        GestureKind::Brightness(r) => {
            let value = slider_value(point, r, 0.0, 100.0) as u8;
            let b = state
                .lighting
                .intent
                .brightness
                .get_or_insert_with(Default::default);
            let _ = b.set_value(value);
        }
        GestureKind::Idle(r) => {
            if let Some(policy) = &mut state.lighting.intent.switch_off {
                policy.idle_minutes = slider_value(point, r, 1.0, 15.0) as u16
            }
        }
        GestureKind::Duration(r) => {
            state.lighting.setting["duration"] = json!(slider_value(point, r, 1.0, 3.0))
        }
        GestureKind::Plane(r) => {
            if let Some(editor) = &mut state.lighting.editor {
                editor.plane_pointer(point.x - r.x, point.y - r.y);
            }
        }
        GestureKind::Value(r) => {
            if let Some(editor) = &mut state.lighting.editor {
                editor.value_pointer(point.x - r.x);
            }
        }
    }
    refresh_editor_fields(state, None);
    Update::handled()
}
pub(super) unsafe fn finish(state: &mut State) -> Update {
    if state.lighting.repeat.take().is_some() {
        return Update {
            capture: Capture::Release,
            ..Update::handled()
        };
    }
    let Some(gesture) = state.lighting.gesture.take() else {
        return Update::default();
    };
    let mut u = match gesture.kind {
        GestureKind::Brightness(_) => intent_command(&mut state.lighting, Scope::Brightness),
        GestureKind::Idle(_) => intent_command(&mut state.lighting, Scope::Policy),
        GestureKind::Duration(_) => effect_command(&mut state.lighting),
        _ => Update::handled(),
    };
    u.capture = Capture::Release;
    u
}
pub(super) fn cancel(state: &mut State) -> Update {
    if state.lighting.repeat.take().is_some() {
        return Update {
            capture: Capture::Release,
            ..Update::handled()
        };
    }
    if let Some(gesture) = state.lighting.gesture.take() {
        state.lighting.intent = gesture.before;
        state.lighting.setting = gesture.before_effect;
        Update {
            capture: Capture::Release,
            ..Update::handled()
        }
    } else {
        Update::default()
    }
}
/// Called by the dedicated capture timer at the shipped 300ms step interval.
pub(super) unsafe fn tick(state: &mut State) -> Update {
    match state.lighting.repeat.clone() {
        Some(control) if active(state) => action(state, control),
        _ => Update::default(),
    }
}
pub(super) fn scroll(state: &mut State, delta: i16) -> Update {
    if !active(state) {
        return Update::default();
    }
    if state.lighting.effect_menu {
        state.lighting.effect_offset = if delta < 0 {
            (state.lighting.effect_offset + 25.0).min(122.0)
        } else {
            (state.lighting.effect_offset - 25.0).max(0.0)
        };
        return Update::handled();
    }
    let Some(g) = source::geometry(
        (f64::from(state.width) / f64::from(state.scale)).max(1000.0),
        128.0,
        0.0,
        state.lighting.selected.unwrap_or(3),
    ) else {
        return Update::handled();
    };
    let bottom =
        (g.sleep_panel.y + g.sleep_panel.height).max(g.effects_panel.y + g.effects_panel.height);
    let max =
        super::viewport::card_scroll_max(bottom, f64::from(state.height) / f64::from(state.scale));
    state.lighting.scroll =
        (state.lighting.scroll + if delta < 0 { 40.0 } else { -40.0 }).clamp(0.0, max);
    Update::rebuild()
}
fn contains(r: color::Rect, p: Point) -> bool {
    p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height
}
pub(super) fn outside(state: &mut State, point: Point) -> Update {
    if let Some(g) = palette_geometry(state) {
        if contains(g.palette, point) || contains(g.editor, point) {
            return Update::default();
        }
    }
    if let Some(g) = geometry(state) {
        if state.lighting.effect_menu {
            let d = source::effect_dropdown(
                g.effect_dropdown,
                f64::from(state.height) / f64::from(state.scale),
                state.lighting.effect_offset,
            );
            let within = |r: source::Rect| {
                point.x >= r.x
                    && point.x < r.x + r.width
                    && point.y >= r.y
                    && point.y < r.y + r.height
            };
            if within(d.panel) || within(g.effect_dropdown) {
                return Update::default();
            }
        }
        if let Some(which) = state.lighting.palette {
            let r = g.colors[which].dropdown;
            if point.x >= r.x && point.x < r.x + r.width && point.y >= r.y && point.y < r.y + 27.0 {
                return Update::default();
            }
        }
    }
    let changed = state.lighting.palette.is_some()
        || state.lighting.effect_menu
        || state.lighting.editor.is_some();
    state.lighting.palette = None;
    state.lighting.editor = None;
    state.lighting.effect_menu = false;
    state.lighting.context_slot = None;
    if changed {
        Update::rebuild()
    } else {
        Update::default()
    }
}
pub(super) fn context(state: &mut State, point: Point) -> Update {
    if state.lighting.editor.is_some() {
        return Update::default();
    }
    if let Some(g) = palette_geometry(state) {
        for (i, r) in g.custom_slots.iter().enumerate() {
            if contains(*r, point) && state.lighting.slots[i]["color"] != "#111" {
                state.lighting.context_slot = Some(i);
                return Update::handled();
            }
        }
    }
    Update::default()
}

unsafe fn refresh_editor_fields(state: &mut State, skip: Option<u16>) {
    let values = state.lighting.editor.as_ref().map(|e| e.drafts.clone());
    let Some(values) = values else { return };
    state.lighting.rebuilding = true;
    for (i, value) in values.iter().enumerate() {
        let id = [HEX, RED, GRN, BLUE][i];
        if skip == Some(id) {
            continue;
        }
        if let Some(hwnd) = state.fields.get(&id) {
            SetWindowTextW(*hwnd, wide(value).as_ptr());
        }
    }
    state.lighting.rebuilding = false;
}
pub(super) unsafe fn input(state: &mut State, id: u16, event: InputEvent) -> Update {
    if !active(state) || state.lighting.rebuilding || !(HEX..=BOOST).contains(&id) {
        return Update::default();
    }
    if matches!(event, InputEvent::Focus) {
        state.lighting.focused = Some(id);
        if let Some(hwnd) = state.fields.get(&id) {
            SendMessageW(*hwnd, EM_SETSEL, 0, -1);
        }
        return Update::handled();
    }
    if matches!(
        event,
        InputEvent::Blur | InputEvent::Enter | InputEvent::Escape
    ) {
        state.lighting.focused = None;
    }
    let value = state.raw_text(id);
    if id == BOOST {
        if matches!(event, InputEvent::Change) {
            if boost_draft_valid(&value) {
                state.lighting.boost_draft = Some(value.clone());
                if let Some(hwnd) = state.fields.get(&id) {
                    SendMessageW(
                        *hwnd,
                        EM_SETLIMITTEXT,
                        4 + usize::from(value.starts_with('-')),
                        0,
                    );
                }
            } else {
                let previous = state.lighting.boost_draft.clone().unwrap_or_else(|| {
                    state.lighting.setting["colorBoost"]
                        .as_f64()
                        .unwrap_or(1.0)
                        .to_string()
                });
                if let Some(hwnd) = state.fields.get(&id) {
                    state.lighting.rebuilding = true;
                    SetWindowTextW(*hwnd, wide(&previous).as_ptr());
                    SendMessageW(*hwnd, EM_SETSEL, previous.len(), previous.len() as isize);
                    state.lighting.rebuilding = false;
                }
            }
            return Update::handled();
        }
        let value = normalize_boost(&value);
        state.lighting.boost_draft = Some(value.to_string());
        let mut u = set_setting(&mut state.lighting, "colorBoost", json!(value));
        u.rebuild_fields = true;
        return u;
    }
    let Some(editor) = &mut state.lighting.editor else {
        return Update::default();
    };
    let index = usize::from(id - HEX);
    if matches!(event, InputEvent::Change) {
        if !editor.change(index, &value) {
            refresh_editor_fields(state, None)
        } else {
            refresh_editor_fields(state, Some(id))
        }
        Update::handled()
    } else {
        editor.blur(index);
        refresh_editor_fields(state, None);
        Update::handled()
    }
}
/// Controlled shipped stepper input: /^-?\d+(\.\d{0,3})?$/ plus empty/lone minus.
fn boost_draft_valid(value: &str) -> bool {
    if value.is_empty() || value == "-" {
        return true;
    }
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or("");
    if integer.is_empty() || !integer.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(fraction) => {
            fraction.len() <= 3
                && fraction.bytes().all(|b| b.is_ascii_digit())
                && parts.next().is_none()
        }
    }
}
fn normalize_boost(value: &str) -> f64 {
    let value = value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
        .clamp(0.25, 4.0);
    let nearest = (value / 0.25).round() * 0.25;
    if (value - nearest).abs() > 1e-10 {
        (value / 0.25).ceil() * 0.25
    } else {
        value
    }
}
unsafe fn field(state: &mut State, id: u16, value: &str, r: color::Rect, max: usize) {
    state.field(
        id,
        value,
        r.x.round() as i32 + 1,
        r.y.round() as i32 + 1,
        r.width.round() as i32 - 2,
    );
    if let Some(hwnd) = state.fields.get(&id) {
        SendMessageW(*hwnd, WM_SETFONT, state.fonts.input as usize, 1);
        SendMessageW(*hwnd, EM_SETLIMITTEXT, max, 0);
        let style = GetWindowLongPtrW(*hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrW(*hwnd, GWL_STYLE, (style | ES_CENTER as u32) as isize);
        MoveWindow(
            *hwnd,
            ((r.x + 1.0) * f64::from(state.scale)).round() as i32,
            ((r.y + 1.0) * f64::from(state.scale)).round() as i32,
            ((r.width - 2.0) * f64::from(state.scale)).round() as i32,
            ((r.height - 2.0) * f64::from(state.scale)).round() as i32,
            1,
        );
        if r.y < 128.0 || r.y + r.height > f64::from(state.height) / f64::from(state.scale) {
            ShowWindow(*hwnd, SW_HIDE);
        }
    }
}
pub(super) unsafe fn rebuild_fields(state: &mut State) -> bool {
    if !active(state) {
        return false;
    }
    state.lighting.rebuilding = true;
    if let (Some(g), Some(values)) = (
        palette_geometry(state),
        state.lighting.editor.as_ref().map(|e| e.drafts.clone()),
    ) {
        for (i, r) in std::iter::once(g.hex_input).chain(g.rgb_inputs).enumerate() {
            field(
                state,
                HEX + i as u16,
                &values[i],
                r,
                if i == 0 { 6 } else { 3 },
            )
        }
    } else if state.lighting.selected == Some(12) && !state.lighting.advanced {
        if let Some(r) = geometry(state).and_then(|g| g.color_boost) {
            let value = state.lighting.boost_draft.clone().unwrap_or_else(|| {
                state.lighting.setting["colorBoost"]
                    .as_f64()
                    .unwrap_or(1.0)
                    .to_string()
            });
            field(
                state,
                BOOST,
                &value,
                color::Rect {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                },
                4 + usize::from(value.starts_with('-')),
            );
        }
    }
    state.lighting.rebuilding = false;
    true
}

fn pr(r: source::Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
fn cr(r: color::Rect) -> Rect {
    Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
fn at(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn h(p: &mut Painter<'_>, r: Rect, c: Control) {
    draw::hit(p, r, Action::Lighting(c));
}
fn parse_color(value: &str) -> Option<u32> {
    let t = value.strip_prefix('#')?;
    if t.len() != 6 {
        return None;
    }
    u32::from_str_radix(t, 16).ok()
}
unsafe fn hover(state: &State, p: &Painter<'_>, r: Rect) -> bool {
    let mut q = POINT { x: 0, y: 0 };
    GetCursorPos(&mut q);
    ScreenToClient(state.hwnd, &mut q);
    let s = f64::from(p.scale);
    f64::from(q.x) >= r.x * s
        && f64::from(q.x) < (r.x + r.width) * s
        && f64::from(q.y) >= r.y * s
        && f64::from(q.y) < (r.y + r.height) * s
}
unsafe fn pill(p: &Painter<'_>, r: Rect, on: bool, enabled: bool) {
    let alpha = if enabled { 1.0 } else { 0.3 };
    draw::rounded(
        p,
        r,
        draw::faded(if on { GREEN } else { 0x707070 }, PANEL, alpha),
        PANEL,
        9.0,
    );
    draw::rounded(
        p,
        at(r.x + if on { 15.0 } else { 1.0 }, r.y + 1.0, 14.0, 14.0),
        PANEL,
        PANEL,
        7.0,
    );
}
unsafe fn check(
    p: &mut Painter<'_>,
    r: Rect,
    label: &str,
    on: bool,
    enabled: bool,
    control: Control,
) {
    let alpha = if enabled { 1.0 } else { 0.3 };
    draw::rounded(
        p,
        r,
        if on {
            draw::faded(GREEN, PANEL, alpha)
        } else {
            PANEL
        },
        draw::faded(if on { GREEN } else { 0x737373 }, PANEL, alpha),
        2.4,
    );
    if on {
        // Source checkbox pseudo-elements use transform-origin:left top.
        for (left, top, height, degrees) in [(8.6, 16.4, 15.4, -145.0f64), (0.8, 10.2, 9.6, -50.0)]
        {
            let angle = degrees.to_radians();
            let center = (r.x + 1.0 + left, r.y + 1.0 + top);
            let points: Vec<POINT> = [(0.0, 0.0), (3.0, 0.0), (3.0, height), (0.0, height)]
                .into_iter()
                .map(|(x, y)| POINT {
                    x: ((center.0 + x * angle.cos() - y * angle.sin()) * f64::from(p.scale)).round()
                        as i32,
                    y: ((center.1 + x * angle.sin() + y * angle.cos()) * f64::from(p.scale)).round()
                        as i32,
                })
                .collect();
            let brush = CreateSolidBrush(rgb(PANEL));
            let old_brush = SelectObject(p.dc, brush);
            let old_pen = SelectObject(p.dc, GetStockObject(NULL_PEN));
            Polygon(p.dc, points.as_ptr(), points.len() as i32);
            SelectObject(p.dc, old_pen);
            SelectObject(p.dc, old_brush);
            DeleteObject(brush);
        }
    }
    draw::text(
        p,
        label,
        at(r.x + 31.0, r.y + 3.0, 400.0, 17.0),
        p.fonts.normal,
        draw::faded(TEXT, PANEL, alpha),
        DT_LEFT,
    );
    if enabled {
        h(p, r, control)
    }
}
// Preserve the source slider value/range, presentation and control arguments.
#[allow(clippy::too_many_arguments)]
unsafe fn slider(
    p: &mut Painter<'_>,
    r: source::Rect,
    value: f64,
    min: f64,
    max: f64,
    enabled: bool,
    tip: bool,
    tags: [&str; 3],
    control: Control,
) {
    let track = source::track(r);
    let a = if enabled { 1.0 } else { 0.3 };
    draw::rounded(
        p,
        pr(track),
        draw::faded(0x204c19, PANEL, a),
        draw::faded(0x204c19, PANEL, a),
        3.0,
    );
    let fill = source::fill_width(value, min, max, r.width).unwrap_or(8.0);
    draw::rounded(
        p,
        at(track.x, track.y, fill, 6.0),
        draw::faded(GREEN, PANEL, a),
        draw::faded(GREEN, PANEL, a),
        3.0,
    );
    draw::rounded(
        p,
        at(r.x + fill - 8.0, track.y - 5.0, 16.0, 16.0),
        draw::faded(GREEN, PANEL, a),
        draw::faded(GREEN, PANEL, a),
        8.0,
    );
    if tip {
        let value = format!("{value:.0}");
        let mut size = SIZE { cx: 0, cy: 0 };
        let old = SelectObject(p.dc, p.fonts.normal);
        GetTextExtentPoint32W(p.dc, wide(&value).as_ptr(), value.len() as i32, &mut size);
        SelectObject(p.dc, old);
        let width = f64::from(size.cx) / f64::from(p.scale) + 16.0;
        let left = r.x
            + source::tip_left(value.parse().unwrap_or(min), min, max, r.width, width)
                .unwrap_or(0.0);
        draw::rounded(
            p,
            at(left, r.y, width, 22.0),
            draw::faded(GREEN, PANEL, a),
            draw::faded(GREEN, PANEL, a),
            3.0,
        );
        draw::text(
            p,
            &value,
            at(left, r.y, width, 22.0),
            p.fonts.normal,
            draw::faded(PANEL, PANEL, a),
            DT_CENTER,
        );
    }
    for (i, label) in tags.into_iter().enumerate() {
        let x = if i == 0 {
            r.x
        } else if i == 2 {
            r.x + r.width - 70.0
        } else {
            r.x + r.width / 2.0 - 35.0
        };
        draw::text(
            p,
            label,
            at(x, r.y + r.height - 15.0, 70.0, 17.0),
            p.fonts.normal,
            draw::faded(TEXT, PANEL, a),
            if i == 0 {
                DT_LEFT
            } else if i == 2 {
                DT_RIGHT
            } else {
                DT_CENTER
            },
        );
    }
    if enabled {
        h(p, pr(track), control.clone());
        h(p, at(r.x + fill - 8.0, track.y - 5.0, 16.0, 16.0), control)
    }
}
unsafe fn dropdown(state: &State, p: &mut Painter<'_>, r: Rect, label: &str, control: Control) {
    draw::rounded(
        p,
        r,
        PANEL,
        if hover(state, p, r) { GREEN } else { 0x515151 },
        0.0,
    );
    draw::text(
        p,
        label,
        at(r.x + 5.0, r.y + 4.0, r.width - 29.0, 17.0),
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    p.image(
        state.icons.get("expand"),
        r.x.round() as i32 + r.width.round() as i32 - 20,
        r.y.round() as i32 + 8,
        10,
        10,
    );
    h(p, r, control)
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, _device: &Value) -> bool {
    if !active(state) {
        return false;
    }
    let Some(g) = geometry(state) else {
        return false;
    };
    let v = &state.lighting;
    for (r, label) in [
        (g.brightness_panel, "BRIGHTNESS"),
        (g.sleep_panel, "SWITCH OFF LIGHTING"),
        (g.effects_panel, "EFFECTS"),
    ] {
        draw::rounded(p, pr(r), PANEL, PANEL, 5.0);
        draw::text(
            p,
            label,
            at(r.x + 40.0, r.y + 30.0, 510.0, 22.0),
            p.fonts.widget_heading,
            GREEN,
            DT_LEFT,
        );
    }
    if let Some(b) = &v.intent.brightness {
        pill(p, pr(g.brightness_switch), b.enabled, true);
        h(p, pr(g.brightness_switch), Control::BrightnessToggle);
        slider(
            p,
            g.brightness_slider,
            f64::from(b.value),
            0.0,
            100.0,
            b.enabled,
            true,
            ["0", "", "100"],
            Control::BrightnessSlider(g.brightness_slider),
        );
        if !b.enabled {
            let t = source::track(g.brightness_slider);
            h(p, pr(t), Control::BrightnessSlider(g.brightness_slider));
        }
    } else {
        draw::text(
            p,
            "Brightness intent unavailable",
            at(
                g.brightness_panel.x + 40.0,
                g.brightness_panel.y + 72.0,
                480.0,
                22.0,
            ),
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
        let t = source::track(g.brightness_slider);
        draw::rounded(p, pr(t), 0x204c19, 0x204c19, 3.0);
        h(p, pr(t), Control::BrightnessSlider(g.brightness_slider));
    }
    let policy = v.intent.switch_off.as_ref();
    let enabled = v.intent.policy_controls_enabled();
    check(
        p,
        pr(g.display_off_check),
        "When display is turned Off",
        policy.is_some_and(|s| s.display_off),
        enabled,
        Control::DisplayToggle,
    );
    check(
        p,
        pr(g.idle_check),
        "When idle for (minutes)",
        policy.is_some_and(|s| s.idle_enabled),
        enabled,
        Control::IdleToggle,
    );
    slider(
        p,
        g.idle_slider,
        policy.map_or(1.0, |s| f64::from(s.idle_minutes)),
        1.0,
        15.0,
        v.intent.idle_slider_enabled(),
        true,
        ["1", "", "15"],
        Control::IdleSlider(g.idle_slider),
    );
    draw::rounded(p, pr(g.mode_pill), PANEL, BORDER, 18.0);
    for (r, label, advanced) in [
        (g.quick_tab, "Quick Effects", false),
        (g.advanced_tab, "Advanced Effects", true),
    ] {
        draw::rounded(
            p,
            pr(r),
            if v.advanced == advanced { GREEN } else { PANEL },
            if v.advanced == advanced { GREEN } else { PANEL },
            13.0,
        );
        draw::text(
            p,
            label,
            pr(r),
            p.fonts.normal,
            if v.advanced == advanced { PANEL } else { TEXT },
            DT_CENTER,
        );
        h(p, pr(r), Control::Mode(advanced));
    }
    if v.advanced {
        draw::text(
            p,
            "Create and edit advanced lighting effects in Chroma Studio.",
            at(
                g.effects_panel.x + 40.0,
                g.effects_panel.y + 128.0,
                520.0,
                20.0,
            ),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        p.button(
            "CHROMA STUDIO",
            (g.effects_panel.x + 40.0) as i32,
            (g.effects_panel.y + 168.0) as i32,
            180,
            Action::Lighting(Control::OpenStudio),
            false,
        );
    } else {
        wrapped_text(p, "Quick effects are presets that can be saved to a device's profile and synced with other supported Razer Chroma-enabled devices.", g.quick_message.x, g.quick_message.y, 520.0, 17.0);
        let label = v
            .selected
            .and_then(|id| {
                source::QUICK_EFFECTS
                    .iter()
                    .find(|e| e.0 == id)
                    .map(|e| e.1)
            })
            .unwrap_or("Source effect unknown");
        dropdown(state, p, pr(g.effect_dropdown), label, Control::EffectMenu);
        draw::text(
            p,
            "Chroma sync runtime unavailable",
            pr(g.sync),
            p.fonts.normal,
            draw::faded(TEXT, PANEL, 0.3),
            DT_LEFT,
        );
        let random = v.setting["isRandom"] == true;
        for (i, c) in g.colors.iter().enumerate() {
            draw::text(
                p,
                if g.colors.len() == 1 {
                    "Color"
                } else if i == 0 {
                    "Color 1"
                } else {
                    "Color 2"
                },
                pr(c.label),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            let value = v.setting[if i == 0 { "color1" } else { "color2" }]
                .as_str()
                .unwrap_or("no-color");
            draw::rounded(p, pr(c.dropdown), PANEL, 0x515151, 0.0);
            let color = parse_color(value).unwrap_or(0xffffff);
            draw::rounded(
                p,
                at(c.dropdown.x + 2.0, c.dropdown.y + 3.0, 20.0, 20.0),
                draw::faded(color, PANEL, if random { 0.3 } else { 1.0 }),
                PANEL,
                3.0,
            );
            p.image(
                state.icons.get("expand"),
                c.dropdown.x.round() as i32 + 33,
                c.dropdown.y.round() as i32 + 8,
                10,
                10,
            );
            if !random {
                h(p, pr(c.dropdown), Control::ColorMenu(i));
            }
        }
        if let Some(r) = g.random_check {
            check(p, pr(r), "Random color", random, true, Control::Random);
        }
        if let Some(r) = g.duration_slider {
            draw::text(
                p,
                "Duration",
                at(r.x, r.y - 17.0, r.width, 17.0),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            slider(
                p,
                r,
                v.setting["duration"].as_f64().unwrap_or(2.0),
                1.0,
                3.0,
                true,
                false,
                ["short", "medium", "long"],
                Control::Duration(r),
            );
        }
        if !g.direction_tabs.is_empty() {
            let base = 1;
            let labels = if v.selected == Some(19) {
                ["lighting-out", "lighting-in"]
            } else if v.selected == Some(13) {
                ["lighting-cw", "lighting-ccw"]
            } else {
                ["lighting-left", "lighting-right"]
            };
            let values = if v.selected == Some(19) {
                [1, 0]
            } else {
                [base, 2]
            };
            let selected = v.setting["direction"].as_i64();
            draw::text(
                p,
                "Direction",
                at(
                    g.direction_tabs[0].x - 7.0,
                    g.direction_tabs[0].y - 22.0,
                    180.0,
                    17.0,
                ),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            let first = g.direction_tabs[0];
            draw::rounded(
                p,
                at(first.x - 6.0, first.y - 6.0, 97.0, 42.0),
                PANEL,
                BORDER,
                20.0,
            );
            h(
                p,
                at(first.x - 6.0, first.y - 6.0, 97.0, 42.0),
                Control::Direction,
            );
            for (i, r) in g.direction_tabs.iter().enumerate() {
                let active = if i == 0 {
                    selected == Some(values[0])
                } else {
                    selected != Some(values[0])
                };
                draw::rounded(
                    p,
                    pr(*r),
                    if active { GREEN } else { PANEL },
                    if active { GREEN } else { PANEL },
                    15.0,
                );
                let name = format!("{}{}", labels[i], if active { "-active" } else { "" });
                p.image(
                    state.icons.get(name.as_str()),
                    r.x.round() as i32 + 10,
                    r.y.round() as i32 + 5,
                    20,
                    20,
                );
                h(p, pr(*r), Control::Direction);
            }
        }
        if !g.ambient_screens.is_empty() {
            draw::text(
                p,
                "Screen Region",
                at(
                    g.ambient_screens[0].x,
                    g.ambient_screens[0].y - 20.0,
                    300.0,
                    17.0,
                ),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            for (r, name) in g
                .ambient_screens
                .iter()
                .zip(["full", "left", "top", "right", "bottom"])
            {
                let selected = v.setting["screen"] == name;
                draw::rounded(
                    p,
                    pr(*r),
                    PANEL,
                    if selected { GREEN } else { 0x707070 },
                    0.0,
                );
                let inner = match name {
                    "full" => at(r.x + 1.0, r.y + 1.0, 22.0, 14.0),
                    "left" => at(r.x + 1.0, r.y + 1.0, 4.0, 14.0),
                    "right" => at(r.x + 19.0, r.y + 1.0, 4.0, 14.0),
                    "top" => at(r.x + 1.0, r.y + 1.0, 22.0, 4.0),
                    _ => at(r.x + 1.0, r.y + 11.0, 22.0, 4.0),
                };
                draw::rounded(p, inner, 0xffffff, 0xffffff, 0.0);
                h(p, pr(*r), Control::Screen(name));
            }
        }
        if let Some(r) = g.color_boost {
            draw::text(
                p,
                "Color Boost",
                at(r.x, r.y - 17.0, 200.0, 17.0),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            draw::rounded(p, pr(r), PANEL, BORDER, 0.0);
            for (up, delta, name) in [(true, 1, "dpi-stepper-up"), (false, -1, "dpi-stepper-down")]
            {
                let y = r.y + if up { 0.0 } else { 15.0 };
                p.image(
                    state.icons.get(name),
                    r.x.round() as i32 + 45,
                    y.round() as i32 + 3,
                    8,
                    8,
                );
                h(p, at(r.x + 44.0, y, 16.0, 12.0), Control::BoostStep(delta));
            }
        }
    }
    let bottom = (g.sleep_panel.y + g.sleep_panel.height)
        .max(g.effects_panel.y + g.effects_panel.height)
        + 8.0;
    let runtime_issue = v.runtime["error"]
        .as_str()
        .or_else(|| v.runtime["uniform_host"]["error"].as_str())
        .or_else(|| v.runtime["uniform_host"]["configuration_error"].as_str())
        .or_else(|| v.runtime["blocked_reason"].as_str());
    let status = runtime_issue.filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if !v.notice.is_empty() {
            &v.notice
        } else if v.runtime.is_null() {
            "Lighting runtime status unavailable; hardware application is unverified."
        } else {
            ""
        }
    });
    if !status.is_empty() {
        draw::text(
            p,
            status,
            at(30.0, bottom, 1200.0, 20.0),
            p.fonts.small,
            MUTED,
            DT_LEFT,
        );
    }
    if v.effect_menu {
        let d = source::effect_dropdown(
            g.effect_dropdown,
            f64::from(state.height) / f64::from(state.scale),
            v.effect_offset,
        );
        draw::rounded(p, pr(d.panel), 0, 0x515151, 0.0);
        let saved = SaveDC(p.dc);
        let clip = p.rect(
            d.viewport.x.round() as i32,
            d.viewport.y.round() as i32,
            d.viewport.width.round() as i32,
            d.viewport.height.round() as i32,
        );
        IntersectClipRect(p.dc, clip.left, clip.top, clip.right, clip.bottom);
        for (i, (id, label)) in source::QUICK_EFFECTS.iter().enumerate() {
            let r = at(
                d.viewport.x,
                d.viewport.y + i as f64 * 25.0 - d.scroll,
                d.viewport.width - 8.0,
                25.0,
            );
            if r.y + r.height <= d.viewport.y || r.y >= d.viewport.y + d.viewport.height {
                continue;
            }
            if hover(state, p, r) {
                draw::rounded(p, r, 0x1a1a1a, 0x1a1a1a, 0.0);
            }
            draw::text(
                p,
                label,
                at(r.x + 4.0, r.y + 4.0, r.width - 8.0, 17.0),
                p.fonts.normal,
                if v.selected == Some(*id) { GREEN } else { TEXT },
                DT_LEFT,
            );
            let top = r.y.max(d.viewport.y);
            let bottom = (r.y + r.height).min(d.viewport.y + d.viewport.height);
            h(
                p,
                at(r.x, top, r.width, bottom - top),
                Control::SelectEffect(*id),
            );
        }
        RestoreDC(p.dc, saved);
        draw::rounded(p, pr(d.track), 0x222222, 0x222222, 0.0);
        let dragging = matches!(
            v.gesture.as_ref().map(|g| &g.kind),
            Some(GestureKind::EffectScroll { .. })
        );
        let thumb_color = if dragging {
            0x2f941e
        } else if hover(state, p, pr(d.thumb)) {
            GREEN
        } else {
            0x656565
        };
        draw::rounded(
            p,
            at(d.thumb.x + 1.0, d.thumb.y + 1.0, 6.0, d.thumb.height - 2.0),
            thumb_color,
            thumb_color,
            4.0,
        );
        h(p, pr(d.track), Control::EffectScroll(d.track));
    }
    if let Some(pg) = palette_geometry(state) {
        if let Some(editor) = &v.editor {
            color_editor_view::paint(state, p, editor, &pg);
            h(p, cr(pg.cancel), Control::EditorCancel);
            h(p, cr(pg.save), Control::EditorSave);
            h(
                p,
                cr(pg.plane_canvas),
                Control::EditorPlane(pg.plane_canvas),
            );
            h(p, cr(pg.value_bar), Control::EditorValue(pg.value_bar));
            h(
                p,
                at(
                    pg.editor.x + 10.0 + editor.cursor.value_left,
                    pg.editor.y + 140.0,
                    8.0,
                    20.0,
                ),
                Control::EditorValue(pg.value_bar),
            );
            h(p, cr(pg.eyedropper), Control::EditorEye);
        } else {
            draw::rounded(p, cr(pg.palette), PANEL, 0x515151, 0.0);
            let colors = preset_colors();
            for (i, r) in pg.presets.iter().enumerate() {
                swatch(state, p, cr(*r), colors[i]);
                h(p, cr(*r), Control::Preset(i));
            }
            if matches!(v.selected, Some(2 | 7 | 19)) {
                swatch(state, p, cr(pg.no_color), "no-color");
                h(p, cr(pg.no_color), Control::Preset(40));
            }
            draw::text(
                p,
                "Color",
                cr(pg.custom_label),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            for (i, r) in pg.custom_slots.iter().enumerate() {
                let value = v.slots[i]["color"].as_str().unwrap_or("#111");
                swatch(state, p, cr(*r), value);
                if value == "#111" {
                    p.image(
                        state.icons.get("lighting-plus"),
                        r.x.round() as i32 + 6,
                        r.y.round() as i32 + 6,
                        8,
                        8,
                    );
                }
                h(p, cr(*r), Control::Custom(i));
            }
            if let Some(slot) = v.context_slot {
                let menu = color::context_menu(pg.custom_slots[slot]);
                draw::rounded(p, cr(menu), PANEL, BORDER, 0.0);
                for (i, label, control) in [
                    (0, "Edit", Control::EditCustom),
                    (1, "Delete", Control::DeleteCustom),
                ] {
                    let r = at(menu.x + 1.0, menu.y + 1.0 + i as f64 * 29.0, 88.0, 29.0);
                    draw::text(p, label, r, p.fonts.normal, TEXT, DT_LEFT);
                    h(p, r, control);
                }
            }
        }
    }
    true
}
unsafe fn swatch(state: &State, p: &Painter<'_>, r: Rect, value: &str) {
    draw::rounded(
        p,
        r,
        parse_color(value).unwrap_or(if value == "no-color" { 0xc83200 } else { PANEL }),
        0x333333,
        3.0,
    );
    if value == "no-color" {
        p.image(
            state.icons.get("lighting-no-color"),
            r.x.round() as i32 + 1,
            r.y.round() as i32 + 1,
            18,
            18,
        );
    }
}

/// Browser-like word wrapping using the actual loaded source font metrics.
unsafe fn wrapped_text(
    p: &Painter<'_>,
    value: &str,
    x: f64,
    mut y: f64,
    width: f64,
    line_height: f64,
) {
    let old = SelectObject(p.dc, p.fonts.normal);
    let mut line = String::new();
    for word in value.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        let encoded: Vec<u16> = next.encode_utf16().collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(p.dc, encoded.as_ptr(), encoded.len() as i32, &mut size);
        if !line.is_empty() && f64::from(size.cx) > width * f64::from(p.scale) {
            draw::text(
                p,
                &line,
                at(x, y, width, line_height),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
            y += line_height;
            line = word.to_owned();
        } else {
            line = next;
        }
    }
    draw::text(
        p,
        &line,
        at(x, y, width, line_height),
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    SelectObject(p.dc, old);
}

/// Test-only sink at the effect-command boundary: real HWND/hit/capture paths,
/// no backend invocation. It cannot be enabled in a production executable.
#[cfg(test)]
pub(super) fn test_dropdown(state: &mut State, enable: bool) -> Value {
    if enable {
        state.lighting.test_commands = Some(Vec::new());
    }
    let g = geometry(state).unwrap();
    let d = source::effect_dropdown(
        g.effect_dropdown,
        f64::from(state.height) / f64::from(state.scale),
        state.lighting.effect_offset,
    );
    let rect = |r: source::Rect| json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height});
    json!({"open":state.lighting.effect_menu,"selected":state.lighting.selected,"setting":state.lighting.setting,
        "trigger":rect(g.effect_dropdown),"panel":rect(d.panel),"track":rect(d.track),"thumb":rect(d.thumb),
        "scroll":d.scroll,"directions":g.direction_tabs.iter().copied().map(rect).collect::<Vec<_>>(),
        "commands":state.lighting.test_commands})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device(profile: &str, percent: u8, effect: u8) -> Value {
        json!({"id":"ornata", "active":profile,
          "lighting_settings":{"brightness":{"isEnabled":true,"value":percent}},
          "lighting_runtime":{"error":null,"configured_percent":percent},
          "source_quick_effects":{"selectedEffectId":effect,"selectedEffectSetting":{"color1":"#00ff00"},"customColors":[]}})
    }
    #[test]
    fn cached_status_preserves_dirty_intent_editor_and_opaque_slots() {
        let mut view = View::default();
        sync(&mut view, &device("a", 50, 1), false);
        view.intent.brightness.as_mut().unwrap().value = 71;
        view.intent_dirty = true;
        view.slots = vec![json!({"color":"rgb(7, 8, 9)","opaque":[1,2]})];
        view.slots_dirty = true;
        view.editor = Some(color_editor_view::Editor::new(
            0,
            Some("112233"),
            Path::new("missing-test-assets"),
        ));
        view.editor.as_mut().unwrap().drafts[0] = "abcdef".into();
        view.focused = Some(HEX);
        assert!(runtime_feedback_busy(&view));
        assert!(update_runtime_only(
            &mut view,
            json!({"error":"worker fault"})
        ));
        assert_eq!(view.intent.brightness.as_ref().unwrap().value, 71);
        assert!(view.intent_dirty && view.slots_dirty);
        assert_eq!(view.slots[0]["opaque"], json!([1, 2]));
        assert_eq!(view.editor.as_ref().unwrap().drafts[0], "abcdef");
        assert_eq!(view.focused, Some(HEX));
        assert!(!update_runtime_only(
            &mut view,
            json!({"error":"worker fault"})
        ));
    }
    #[test]
    fn boost_controlled_drafts_and_quarter_rounding_match_source() {
        for value in ["", "-", "1", "1.", "-1.2", "0.123", "0001"] {
            assert!(boost_draft_valid(value), "{value}");
        }
        for value in [".", ".5", "+1", "1e0", "1.2345", "1.2.3", "a", "1 2"] {
            assert!(!boost_draft_valid(value), "{value}");
        }
        for (value, expected) in [
            ("", 0.25),
            ("-", 0.25),
            ("-1", 0.25),
            ("0", 0.25),
            ("1.001", 1.25),
            ("1.25", 1.25),
            ("3.999", 4.0),
            ("8", 4.0),
        ] {
            assert_eq!(normalize_boost(value), expected);
        }
    }
    #[test]
    fn pending_edits_survive_old_reply_but_failed_final_reply_reconciles() {
        let mut view = View::default();
        sync(&mut view, &device("a", 50, 1), false);
        view.intent.brightness.as_mut().unwrap().value = 70;
        view.intent_dirty = true;
        sync(&mut view, &device("a", 50, 1), true);
        assert_eq!(view.intent.brightness.as_ref().unwrap().value, 70);
        assert_eq!(view.runtime["configured_percent"], 50);
        sync(&mut view, &device("a", 50, 1), false);
        assert_eq!(view.intent.brightness.as_ref().unwrap().value, 50);
        assert!(!view.intent_dirty);
    }
    #[test]
    fn changed_owner_always_resets_even_with_pending_flag() {
        let mut view = View::default();
        sync(&mut view, &device("a", 50, 1), false);
        view.focused = Some(BOOST);
        sync(&mut view, &device("b", 20, 2), true);
        assert_eq!(view.owner.as_ref().unwrap().profile, "b");
        assert_eq!(view.selected, Some(2));
        assert_eq!(view.intent.brightness.as_ref().unwrap().value, 20);
        assert!(view.focused.is_none());
    }
    #[test]
    fn focused_field_protects_draft_and_malformed_custom_slot_is_bounded() {
        let mut view = View::default();
        sync(&mut view, &device("a", 50, 1), false);
        view.focused = Some(BOOST);
        view.setting["colorBoost"] = json!(2.5);
        sync(&mut view, &device("a", 50, 2), false);
        assert_eq!(view.selected, Some(1));
        view.focused = None;
        let mut d = device("a", 50, 2);
        d["source_quick_effects"]["customColors"] =
            json!([null, 1, {"color":"rgb(7, 8, 9)","extra":7}]);
        sync(&mut view, &d, false);
        assert_eq!(view.slots[0]["color"], "#111");
        assert_eq!(view.slots[2], d["source_quick_effects"]["customColors"][2]);
        assert!(view.slots[2].get("picked").is_none());
        assert_eq!(view.slots.len(), 16);
    }
}
