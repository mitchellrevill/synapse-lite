//! Pure, bounded projection of Synapse AppEngine macro JSON.
//!
//! This is separate from `macros` (on-device HID-usage bytecode). It preserves
//! original JSON, accepts only evidenced native event fields, and produces a
//! symbolic plan with explicit unresolved runtime requirements. It never sends
//! input or constructs driver packets. See `docs/SOURCE-MACROS.md`.

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;
pub(crate) mod host;
pub use host::HostAction;

const MAX_JSON_BYTES: usize = 256 * 1024;
const MAX_JSON_NODES: usize = 4096;
const MAX_JSON_DEPTH: usize = 32;
// Conservative retained-data accounting: original/decoded/error strings,
// structured nodes and the unexpanded plan. This is not an allocator/RSS
// measurement; input, node, depth and event limits are enforced independently.
const _: () = assert!(
    MAX_JSON_BYTES * 3 + MAX_JSON_NODES * 256 + MAX_JSON_NODES * std::mem::size_of::<Event>()
        <= 2 * 1024 * 1024
);

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub json_bytes: usize,
    pub json_nodes: usize,
    pub json_depth: usize,
    pub events: usize,
    pub expanded_events: u64,
    /// Explicit source delays only; not a wall-clock scheduling estimate.
    pub explicit_delay_ms: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            json_bytes: MAX_JSON_BYTES,
            json_nodes: MAX_JSON_NODES,
            json_depth: MAX_JSON_DEPTH,
            events: 512,
            expanded_events: 100_000,
            explicit_delay_ms: 3_600_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    InvalidLimits,
    Budget(&'static str),
    InvalidJson(String),
    DuplicateJsonKey,
    InvalidField { event: Option<usize>, field: String },
    UnsupportedField { event: Option<usize>, field: String },
    UnsupportedEvent { event: usize, kind: String },
    UnsupportedKeyboardFlag { event: usize, flag: u16 },
    UnsupportedMouse { event: usize },
    UnresolvedPlayback(PlaybackMode),
    ArithmeticOverflow,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source macro planning: {self:?}")
    }
}
impl std::error::Error for PlanError {}

/// Exact input text is retained, including unknown fields and numeric spelling.
/// Parsed duplicate keys are unsuitable for planning; original text survives.
#[derive(Debug)]
pub struct SourceJson {
    original: String,
    parsed: Value,
    duplicates: bool,
    limits: Limits,
}

struct Budget {
    remaining: usize,
    depth: usize,
    duplicates: bool,
    failure: Option<&'static str>,
}
struct Seed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.depth > self.budget.depth {
            self.budget.failure = Some("JSON depth");
            return Err(de::Error::custom("JSON depth budget"));
        }
        if self.budget.remaining == 0 {
            self.budget.failure = Some("JSON nodes");
            return Err(de::Error::custom("JSON node budget"));
        }
        self.budget.remaining -= 1;
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = access.next_element_seed(Seed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            // Keys consume node credit too; unknown metadata is bounded.
            if self.budget.remaining == 0 {
                self.budget.failure = Some("JSON nodes");
                return Err(de::Error::custom("JSON node budget"));
            }
            self.budget.remaining -= 1;
            let value = access.next_value_seed(Seed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            if values.insert(key, value).is_some() {
                self.budget.duplicates = true;
            }
        }
        Ok(Value::Object(values))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLocation {
    RootArray,
    AppEngineEvents,
}

impl SourceJson {
    pub fn parse(text: &str, limits: Limits) -> Result<Self, PlanError> {
        if limits.json_bytes == 0
            || limits.json_bytes > MAX_JSON_BYTES
            || limits.json_nodes == 0
            || limits.json_nodes > MAX_JSON_NODES
            || limits.json_depth == 0
            || limits.json_depth > MAX_JSON_DEPTH
            || limits.events == 0
            || limits.events > MAX_JSON_NODES
        {
            return Err(PlanError::InvalidLimits);
        }
        if text.len() > limits.json_bytes {
            return Err(PlanError::Budget("JSON bytes"));
        }
        let mut budget = Budget {
            remaining: limits.json_nodes,
            depth: limits.json_depth,
            duplicates: false,
            failure: None,
        };
        let mut parser = serde_json::Deserializer::from_str(text);
        let parsed = Seed {
            budget: &mut budget,
            depth: 1,
        }
        .deserialize(&mut parser)
        .and_then(|value| {
            parser.end()?;
            Ok(value)
        });
        let parsed = parsed.map_err(|error| match budget.failure {
            Some(kind) => PlanError::Budget(kind),
            None => PlanError::InvalidJson(error.to_string()),
        })?;
        Ok(Self {
            original: text.into(),
            parsed,
            duplicates: budget.duplicates,
            limits,
        })
    }

    pub fn original_json(&self) -> &str {
        &self.original
    }
    /// A semantic view; use original_json for exact storage/re-export.
    pub fn value(&self) -> &Value {
        &self.parsed
    }

    pub fn plan(
        &self,
        location: EventLocation,
        playback: &PlaybackMetadata<'_>,
    ) -> Result<SymbolicPlan, PlanError> {
        if self.duplicates {
            return Err(PlanError::DuplicateJsonKey);
        }
        let repeats = match playback.mode {
            PlaybackMode::Once => 1,
            PlaybackMode::NTimes(count) => count,
            other => return Err(PlanError::UnresolvedPlayback(other)),
        };
        // The outer document remains opaque storage. This is an explicit
        // projection of the complete selected array, not envelope validation.
        let value = match location {
            EventLocation::RootArray => &self.parsed,
            EventLocation::AppEngineEvents => self
                .parsed
                .pointer("/appEngine/events")
                .ok_or_else(|| invalid(None, "appEngine.events"))?,
        };
        let values = value.as_array().ok_or_else(|| invalid(None, "events"))?;
        if values.is_empty() {
            return Err(invalid(None, "events"));
        }
        if values.len() > self.limits.events {
            return Err(PlanError::Budget("events"));
        }
        let expanded_events = (values.len() as u64)
            .checked_mul(u64::from(repeats))
            .ok_or(PlanError::ArithmeticOverflow)?;
        if expanded_events > self.limits.expanded_events {
            return Err(PlanError::Budget("expanded events"));
        }
        let mut events = Vec::with_capacity(values.len());
        let mut delay = 0u64;
        for (index, value) in values.iter().enumerate() {
            let event = decode_event(index, value)?;
            if let Event::DelayMs(ms) = event {
                delay = delay
                    .checked_add(u64::from(ms))
                    .ok_or(PlanError::ArithmeticOverflow)?;
            }
            events.push(event);
        }
        let explicit_delay_ms = delay
            .checked_mul(u64::from(repeats))
            .ok_or(PlanError::ArithmeticOverflow)?;
        if explicit_delay_ms > self.limits.explicit_delay_ms {
            return Err(PlanError::Budget("explicit delay"));
        }
        Ok(SymbolicPlan {
            events,
            repeats,
            expanded_events,
            explicit_delay_ms,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackMode {
    Once,
    NTimes(u32),
    ContinuousHeld,
    ContinuousToggle,
    Sequence,
    Queue,
    Phased,
    PhasedMacro,
}

/// Borrowed metadata remains in its caller's source storage without rewriting.
#[derive(Debug)]
pub struct PlaybackMetadata<'a> {
    raw: &'a Value,
    mode: PlaybackMode,
}

impl<'a> PlaybackMetadata<'a> {
    /// Source macroGroup fields, not native onboard MacroPlay or output flags.
    pub fn parse(raw: &'a Value) -> Result<Self, PlanError> {
        let object = raw.as_object().ok_or_else(|| invalid(None, "macroGroup"))?;
        if object.len() > 3 || object.keys().any(|key| key.len() > 64) {
            return Err(PlanError::Budget("playback metadata"));
        }
        fields(
            None,
            object,
            &["guid", "macroPlaybackOption", "repeatCount"],
        )?;
        let guid = object
            .get("guid")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(None, "guid"))?;
        if guid.is_empty() || guid.len() > 128 {
            return Err(invalid(None, "guid"));
        }
        let option = object
            .get("macroPlaybackOption")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(None, "macroPlaybackOption"))?;
        let mode = match option {
            "Once" => PlaybackMode::Once,
            "NTimes" => {
                let count = integer(None, object, "repeatCount", u32::MAX as u64)? as u32;
                if count == 0 {
                    return Err(invalid(None, "repeatCount"));
                }
                PlaybackMode::NTimes(count)
            }
            "ContinuousHeld" => PlaybackMode::ContinuousHeld,
            "ContinuousToggle" => PlaybackMode::ContinuousToggle,
            "Sequence" => PlaybackMode::Sequence,
            "Queue" => PlaybackMode::Queue,
            "Phased" => PlaybackMode::Phased,
            "PhasedMacro" => PlaybackMode::PhasedMacro,
            _ => return Err(invalid(None, "macroPlaybackOption")),
        };
        Ok(Self { raw, mode })
    }
    pub fn raw(&self) -> &'a Value {
        self.raw
    }
    pub fn mode(&self) -> PlaybackMode {
        self.mode
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Down,
    Up,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelAxis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Host(Box<HostAction>),
    DelayMs(u32),
    /// Native source scan-code namespace, never a USB HID usage conversion.
    Keyboard {
        scancode: u16,
        flag: u16,
        transition: Transition,
        /// Explicit native metadata; omission is distinct from an explicit zero.
        modifiers: Option<u32>,
        /// Source UnitId, never inferred from an endpoint or device label.
        unit_id: Option<u16>,
    },
    MouseButton {
        id: u16,
        button: Button,
        transition: Transition,
    },
    Wheel {
        id: u16,
        axis: WheelAxis,
        raw_data: u16,
        delta: i16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGap {
    HostLaunchExecution,
    ClipboardAndPasteRecovery,
    NativeIdentityAndRouting,
    CompletionDrivenTiming,
    HeldOutputOwnershipAndCancellation,
    RepeatBoundaryTiming,
}

/// Compact repetition: never allocates an expanded frame/event history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicPlan {
    events: Vec<Event>,
    repeats: u32,
    expanded_events: u64,
    /// Sum of explicit source delays, excluding completion time/implicit gaps.
    explicit_delay_ms: u64,
}

impl SymbolicPlan {
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn repeats(&self) -> u32 {
        self.repeats
    }
    pub fn expanded_event_count(&self) -> u64 {
        self.expanded_events
    }
    pub fn explicit_delay_ms(&self) -> u64 {
        self.explicit_delay_ms
    }
    /// Every plan is unresolved for execution. No release/drain is invented.
    pub fn runtime_gaps(&self) -> Vec<RuntimeGap> {
        let mut gaps = vec![
            RuntimeGap::NativeIdentityAndRouting,
            RuntimeGap::CompletionDrivenTiming,
            RuntimeGap::HeldOutputOwnershipAndCancellation,
        ];
        if self.repeats > 1 {
            gaps.push(RuntimeGap::RepeatBoundaryTiming);
        }
        if self.events.iter().any(|e| matches!(e, Event::Host(action) if matches!(action.as_ref(), HostAction::Launch { .. }))) {
            gaps.push(RuntimeGap::HostLaunchExecution);
        }
        if self.events.iter().any(|e| matches!(e, Event::Host(action) if matches!(action.as_ref(), HostAction::ClipboardText(_)))) {
            gaps.push(RuntimeGap::ClipboardAndPasteRecovery);
        }
        gaps
    }
    /// Source dispatch waits10ms between directly consecutive keyboard events.
    /// An explicit zero-delay event breaks that adjacency too. This is not an
    /// absolute timestamp and makes no claim about repetition boundaries.
    /// Native proof: delay dispatch12C201 assigns the current event to+0x80
    /// even when12AB3C skips its wait; keyboard12C2DA compares that event's type.
    pub fn source_keyboard_gap_before_us(&self, index: usize) -> Option<u32> {
        if index == 0 {
            return None;
        }
        match (self.events.get(index - 1), self.events.get(index)) {
            (Some(Event::Keyboard { .. }), Some(Event::Keyboard { .. })) => Some(10_000),
            _ => None,
        }
    }
}

fn invalid(event: Option<usize>, field: &str) -> PlanError {
    PlanError::InvalidField {
        event,
        field: field.into(),
    }
}
fn fields(
    event: Option<usize>,
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), PlanError> {
    for field in object.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(PlanError::UnsupportedField {
                event,
                field: field.clone(),
            });
        }
    }
    Ok(())
}
fn integer(
    event: Option<usize>,
    object: &Map<String, Value>,
    field: &str,
    max: u64,
) -> Result<u64, PlanError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|number| *number <= max)
        .ok_or_else(|| invalid(event, field))
}

fn decode_event(index: usize, value: &Value) -> Result<Event, PlanError> {
    let at = Some(index);
    let object = value.as_object().ok_or_else(|| invalid(at, "event"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(at, "type"))?;
    match kind {
        "launch" | "clipboard" => host::decode(index, object),
        "delay" => {
            fields(at, object, &["type", "ms"])?;
            Ok(Event::DelayMs(
                integer(at, object, "ms", u32::MAX as u64)? as u32
            ))
        }
        "keyboard" => {
            fields(
                at,
                object,
                &["type", "scancode", "flag", "modifiers", "unitId"],
            )?;
            let scancode = integer(at, object, "scancode", u16::MAX as u64)? as u16;
            let flag = integer(at, object, "flag", u16::MAX as u64)? as u16;
            let transition = match flag {
                0 | 2 | 4 => Transition::Down,
                1 | 3 | 5 => Transition::Up,
                _ => return Err(PlanError::UnsupportedKeyboardFlag { event: index, flag }),
            };
            Ok(Event::Keyboard {
                scancode,
                flag,
                transition,
                modifiers: object
                    .get("modifiers")
                    .map(|_| integer(at, object, "modifiers", u32::MAX as u64).map(|n| n as u32))
                    .transpose()?,
                unit_id: object
                    .get("unitId")
                    .map(|_| integer(at, object, "unitId", u16::MAX as u64).map(|n| n as u16))
                    .transpose()?,
            })
        }
        "mouse" => {
            let id = integer(at, object, "id", u16::MAX as u64)? as u16;
            let button = match id {
                1 | 2 => Some(Button::Left),
                4 | 8 => Some(Button::Right),
                16 | 32 => Some(Button::Middle),
                64 | 128 => Some(Button::Button4),
                256 | 512 => Some(Button::Button5),
                _ => None,
            };
            if let Some(button) = button {
                fields(at, object, &["type", "id"])?;
                let transition = match id {
                    1 | 4 | 16 | 64 | 256 => Transition::Down,
                    _ => Transition::Up,
                };
                return Ok(Event::MouseButton {
                    id,
                    button,
                    transition,
                });
            }
            if id == 1024 || id == 2048 {
                fields(at, object, &["type", "id", "data"])?;
                let raw_data = integer(at, object, "data", u16::MAX as u64)? as u16;
                if raw_data != 120 && raw_data != 65416 {
                    return Err(PlanError::UnsupportedMouse { event: index });
                }
                return Ok(Event::Wheel {
                    id,
                    axis: if id == 1024 {
                        WheelAxis::Vertical
                    } else {
                        WheelAxis::Horizontal
                    },
                    raw_data,
                    delta: raw_data as i16,
                });
            }
            Err(PlanError::UnsupportedMouse { event: index })
        }
        other => Err(PlanError::UnsupportedEvent {
            event: index,
            kind: other.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metadata() -> Value {
        json!({"guid":"synthetic-macro-id","macroPlaybackOption":"Once"})
    }
    fn plan(text: &str) -> Result<SymbolicPlan, PlanError> {
        SourceJson::parse(text, Limits::default())?.plan(
            EventLocation::RootArray,
            &PlaybackMetadata::parse(&metadata())?,
        )
    }

    #[test]
    fn source_converter_goldens_decode_without_hid_translation() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/source_macro_conversion.json"
        ))
        .unwrap();
        for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
            let text = case["output"].to_string();
            let result = plan(&text);
            if index == 3 {
                assert!(
                    matches!(result, Err(PlanError::UnsupportedEvent { kind, .. }) if kind == "randomDelay")
                );
            } else {
                let actual = result.unwrap();
                let down = Event::MouseButton {
                    id: 1,
                    button: Button::Left,
                    transition: Transition::Down,
                };
                let up = Event::MouseButton {
                    id: 2,
                    button: Button::Left,
                    transition: Transition::Up,
                };
                let expected = match index {
                    0 => vec![Event::DelayMs(2)],
                    1 => vec![Event::DelayMs(0)],
                    2 => vec![Event::DelayMs(1234)],
                    4 => vec![down],
                    5 => vec![up],
                    6 => vec![Event::MouseButton {
                        id: 4,
                        button: Button::Right,
                        transition: Transition::Down,
                    }],
                    7 => vec![Event::Wheel {
                        id: 2048,
                        axis: WheelAxis::Horizontal,
                        raw_data: 65416,
                        delta: -120,
                    }],
                    8 => vec![Event::Wheel {
                        id: 2048,
                        axis: WheelAxis::Horizontal,
                        raw_data: 120,
                        delta: 120,
                    }],
                    9 => vec![down.clone(), up.clone(), Event::DelayMs(10), down, up],
                    _ => panic!(
                        "source fixture gained a case without an independent expected vector"
                    ),
                };
                assert_eq!(actual.events, expected, "source fixture case{index}");
            }
        }
        assert_eq!(
            plan(r#"[{"type":"delay","ms":1234}]"#).unwrap().events,
            vec![Event::DelayMs(1234)]
        );
        assert_eq!(
            plan(r#"[{"type":"mouse","id":2048,"data":65416}]"#)
                .unwrap()
                .events,
            vec![Event::Wheel {
                id: 2048,
                axis: WheelAxis::Horizontal,
                raw_data: 65416,
                delta: -120
            }]
        );
    }

    #[test]
    fn keyboard_flags_preserve_native_scancode_namespace_and_gap_rule() {
        let p = plan(r#"[{"type":"keyboard","scancode":29,"flag":0},{"type":"keyboard","scancode":29,"flag":1},{"type":"delay","ms":0},{"type":"keyboard","scancode":57373,"flag":0}]"#).unwrap();
        assert_eq!(
            p.events[0],
            Event::Keyboard {
                scancode: 29,
                flag: 0,
                transition: Transition::Down,
                modifiers: None,
                unit_id: None,
            }
        );
        assert_eq!(
            p.events[1],
            Event::Keyboard {
                scancode: 29,
                flag: 1,
                transition: Transition::Up,
                modifiers: None,
                unit_id: None,
            }
        );
        assert_eq!(
            p.events[3],
            Event::Keyboard {
                scancode: 57373,
                flag: 0,
                transition: Transition::Down,
                modifiers: None,
                unit_id: None,
            }
        );
        assert_eq!(p.source_keyboard_gap_before_us(1), Some(10_000));
        assert_eq!(p.source_keyboard_gap_before_us(3), None);
        assert_eq!(p.source_keyboard_gap_before_us(0), None);
        assert_eq!(p.source_keyboard_gap_before_us(usize::MAX), None);
        assert_eq!(p.explicit_delay_ms(), 0);
        assert!(p
            .runtime_gaps()
            .contains(&RuntimeGap::HeldOutputOwnershipAndCancellation));
    }

    #[test]
    fn every_source_button_transition_and_wheel_encoding_is_explicit() {
        for (down, up, button) in [
            (1, 2, Button::Left),
            (4, 8, Button::Right),
            (16, 32, Button::Middle),
            (64, 128, Button::Button4),
            (256, 512, Button::Button5),
        ] {
            let p = plan(&json!([{"type":"mouse","id":down},{"type":"mouse","id":up}]).to_string())
                .unwrap();
            assert_eq!(
                p.events[0],
                Event::MouseButton {
                    id: down,
                    button,
                    transition: Transition::Down
                }
            );
            assert_eq!(
                p.events[1],
                Event::MouseButton {
                    id: up,
                    button,
                    transition: Transition::Up
                }
            );
        }
        for (id, axis) in [(1024, WheelAxis::Vertical), (2048, WheelAxis::Horizontal)] {
            for (raw_data, delta) in [(120, 120), (65416, -120)] {
                assert_eq!(
                    plan(&json!([{"type":"mouse","id":id,"data":raw_data}]).to_string())
                        .unwrap()
                        .events,
                    vec![Event::Wheel {
                        id,
                        axis,
                        raw_data,
                        delta
                    }]
                );
            }
        }
        for value in [
            json!({"type":"mouse","id":3}),
            json!({"type":"mouse","id":1024,"data":1}),
            json!({"type":"mouse","id":1,"data":0}),
        ] {
            assert!(plan(&json!([value]).to_string()).is_err());
        }
    }

    #[test]
    fn exact_raw_storage_survives_unknown_fields_and_projection() {
        let text = " {\"future\":{\"x\":1e3},\"appEngine\":{\"hash\":\"opaque\",\"events\":[{\"type\":\"delay\",\"ms\":2}]}} ";
        let raw = SourceJson::parse(text, Limits::default()).unwrap();
        assert_eq!(raw.original_json(), text);
        assert_eq!(raw.value()["future"]["x"], json!(1000.0));
        let group = metadata();
        assert_eq!(
            raw.plan(
                EventLocation::AppEngineEvents,
                &PlaybackMetadata::parse(&group).unwrap()
            )
            .unwrap()
            .events,
            vec![Event::DelayMs(2)]
        );
        assert_eq!(raw.original_json(), text);
    }

    #[test]
    fn unsupported_event_never_returns_partial_plan_or_skips() {
        for kind in ["randomDelay", "loop", "macro", "mouseMove", "future"] {
            let data = json!([{"type":"delay","ms":2},{"type":kind}]);
            assert!(matches!(
                plan(&data.to_string()),
                Err(PlanError::UnsupportedEvent { event: 1, .. })
            ));
        }
        let text = r#"[{"type":"keyboard","scancode":30,"flag":0,"releaseAfterMs":100}]"#;
        let raw = SourceJson::parse(text, Limits::default()).unwrap();
        assert_eq!(raw.original_json(), text);
        assert!(
            matches!(plan(text), Err(PlanError::UnsupportedField { field, .. }) if field == "releaseAfterMs")
        );
    }

    #[test]
    fn explicit_recorded_prefix_flags_preserve_identity_and_direction() {
        // Source recorder G.createKeyEvent uses output flags2(E0)/4(E1)
        // and adds1 for keyup. No prefix is inferred from scan code alone.
        for (scancode, flag, transition) in [
            (29, 2, Transition::Down),
            (29, 3, Transition::Up),
            (29, 4, Transition::Down),
            (29, 5, Transition::Up),
            (29, 0, Transition::Down),
            (29, 1, Transition::Up),
        ] {
            let text = json!([{"type":"keyboard","scancode":scancode,"flag":flag}]).to_string();
            let p = plan(&text).unwrap();
            assert_eq!(
                p.events(),
                &[Event::Keyboard {
                    scancode,
                    flag,
                    transition,
                    modifiers: None,
                    unit_id: None,
                }]
            );
        }
    }

    #[test]
    fn all_unsupported_keyboard_flags_and_invalid_numbers_are_rejected() {
        for flag in [6, 7, 8, 0x8000, 65535] {
            assert_eq!(
                plan(&json!([{"type":"keyboard","scancode":30,"flag":flag}]).to_string()),
                Err(PlanError::UnsupportedKeyboardFlag { event: 0, flag })
            );
        }
        for value in [
            json!(-1),
            json!(1.5),
            json!("1"),
            Value::Null,
            json!(u64::MAX),
        ] {
            assert!(plan(&json!([{"type":"delay","ms":value}]).to_string()).is_err());
            assert!(
                plan(&json!([{"type":"keyboard","scancode":value,"flag":0}]).to_string()).is_err()
            );
        }
        for text in [
            "[NaN]",
            "[Infinity]",
            r#"[{"type":"delay","ms":1e999}]"#,
            "[] []",
        ] {
            assert!(SourceJson::parse(text, Limits::default()).is_err());
        }
    }

    #[test]
    fn duplicate_keys_are_retained_but_never_planned() {
        let text = r#"[{"type":"delay","ms":1,"ms":2}]"#;
        let raw = SourceJson::parse(text, Limits::default()).unwrap();
        assert_eq!(raw.original_json(), text);
        assert_eq!(plan(text), Err(PlanError::DuplicateJsonKey));
    }

    #[test]
    fn native_keyboard_optional_fields_preserve_absence_zero_and_full_width() {
        for (extra, modifiers, unit_id) in [
            (json!({}), None, None),
            (json!({"modifiers":0,"unitId":0}), Some(0), Some(0)),
            (
                json!({"modifiers":u32::MAX,"unitId":u16::MAX}),
                Some(u32::MAX),
                Some(u16::MAX),
            ),
        ] {
            let mut row = json!({"type":"keyboard","scancode":29,"flag":2});
            row.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let text = json!([row]).to_string();
            let raw = SourceJson::parse(&text, Limits::default()).unwrap();
            assert_eq!(raw.original_json(), text);
            assert_eq!(
                plan(&text).unwrap().events(),
                &[Event::Keyboard {
                    scancode: 29,
                    flag: 2,
                    transition: Transition::Down,
                    modifiers,
                    unit_id
                }]
            );
        }
    }

    #[test]
    fn native_keyboard_optional_fields_refuse_invalid_values_and_duplicates() {
        for (field, maximum) in [("modifiers", u32::MAX as u64), ("unitId", u16::MAX as u64)] {
            for value in [
                Value::Null,
                json!(true),
                json!("0"),
                json!(-1),
                json!(1.5),
                json!(maximum + 1),
            ] {
                let mut row = json!({"type":"keyboard","scancode":30,"flag":0});
                row[field] = value;
                assert!(
                    matches!(plan(&json!([row]).to_string()), Err(PlanError::InvalidField { event:Some(0), field:bad }) if bad==field)
                );
            }
            let text = format!(
                r#"[{{"type":"keyboard","scancode":30,"flag":0,"{field}":0,"{field}":1}}]"#
            );
            assert_eq!(plan(&text), Err(PlanError::DuplicateJsonKey));
        }
        for field in ["delayInMs", "releaseAfterMs", "unit_id", "future"] {
            let mut row =
                json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":0,"unitId":0});
            row[field] = json!(0);
            assert!(matches!(
                plan(&json!([row]).to_string()),
                Err(PlanError::UnsupportedField { .. })
            ));
        }
    }

    #[test]
    fn source_playback_vectors_keep_finite_and_unresolved_modes_distinct() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/source_macro_conversion.json"
        ))
        .unwrap();
        for vector in vectors["playback"].as_array().unwrap() {
            let option = vector["option"].as_str().unwrap();
            let repeat = vector["output"]["downOutput"].get("repeat");
            let mut group = json!({"guid":"synthetic-macro-id","macroPlaybackOption":option});
            if let Some(repeat) = repeat {
                group["repeatCount"] = repeat.clone();
            }
            let playback = PlaybackMetadata::parse(&group).unwrap();
            assert_eq!(playback.raw(), &group);
            let source =
                SourceJson::parse(r#"[{"type":"delay","ms":3}]"#, Limits::default()).unwrap();
            let result = source.plan(EventLocation::RootArray, &playback);
            match option {
                "Once" => assert_eq!(result.unwrap().repeats(), 1),
                "NTimes" => assert_eq!(
                    result.unwrap().repeats(),
                    repeat.unwrap().as_u64().unwrap() as u32
                ),
                _ => assert_eq!(result, Err(PlanError::UnresolvedPlayback(playback.mode()))),
            }
        }
    }

    #[test]
    fn finite_repetition_is_compact_and_counts_explicit_delays_only() {
        let group = json!({"guid":"g","macroPlaybackOption":"NTimes","repeatCount":1000});
        let p = SourceJson::parse(r#"[{"type":"delay","ms":10}]"#, Limits::default())
            .unwrap()
            .plan(
                EventLocation::RootArray,
                &PlaybackMetadata::parse(&group).unwrap(),
            )
            .unwrap();
        assert_eq!(p.events().len(), 1);
        assert_eq!(p.expanded_event_count(), 1000);
        assert_eq!(p.explicit_delay_ms(), 10_000);
        assert!(p.runtime_gaps().contains(&RuntimeGap::RepeatBoundaryTiming));
        // Source's Once converter ignores repeatCount; retaining it does not
        // turn Once into an invented repeat mode.
        let once = json!({"guid":"g","macroPlaybackOption":"Once","repeatCount":"unused"});
        assert_eq!(
            PlaybackMetadata::parse(&once).unwrap().mode(),
            PlaybackMode::Once
        );
    }

    #[test]
    fn json_and_event_budgets_fail_before_large_expansion() {
        assert_eq!(
            SourceJson::parse(
                "[]",
                Limits {
                    json_bytes: 1,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            PlanError::Budget("JSON bytes")
        );
        assert_eq!(
            SourceJson::parse(
                "[0,1,2]",
                Limits {
                    json_nodes: 3,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            PlanError::Budget("JSON nodes")
        );
        assert_eq!(
            SourceJson::parse(
                "[[0]]",
                Limits {
                    json_depth: 2,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            PlanError::Budget("JSON depth")
        );
        let group = metadata();
        let playback = PlaybackMetadata::parse(&group).unwrap();
        let text = r#"[{"type":"delay","ms":2},{"type":"delay","ms":3}]"#;
        for (limits, expected) in [
            (
                Limits {
                    events: 1,
                    ..Limits::default()
                },
                "events",
            ),
            (
                Limits {
                    expanded_events: 1,
                    ..Limits::default()
                },
                "expanded events",
            ),
            (
                Limits {
                    explicit_delay_ms: 4,
                    ..Limits::default()
                },
                "explicit delay",
            ),
        ] {
            assert_eq!(
                SourceJson::parse(text, limits)
                    .unwrap()
                    .plan(EventLocation::RootArray, &playback),
                Err(PlanError::Budget(expected))
            );
        }
        assert_eq!(
            SourceJson::parse(
                "[]",
                Limits {
                    json_nodes: usize::MAX,
                    ..Limits::default()
                }
            )
            .unwrap_err(),
            PlanError::InvalidLimits
        );
    }

    #[test]
    fn overflowing_duration_and_invalid_repeat_metadata_are_not_clamped() {
        let group = json!({"guid":"g","macroPlaybackOption":"NTimes","repeatCount":u32::MAX});
        let raw = SourceJson::parse(
            r#"[{"type":"delay","ms":4294967295},{"type":"delay","ms":4294967295}]"#,
            Limits {
                expanded_events: u64::MAX,
                explicit_delay_ms: u64::MAX,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(
            raw.plan(
                EventLocation::RootArray,
                &PlaybackMetadata::parse(&group).unwrap()
            ),
            Err(PlanError::ArithmeticOverflow)
        );
        for repeat in [json!(0), json!(-1), json!(1.5), json!("2"), json!(u64::MAX)] {
            assert!(PlaybackMetadata::parse(
                &json!({"guid":"g","macroPlaybackOption":"NTimes","repeatCount":repeat})
            )
            .is_err());
        }
        assert!(PlaybackMetadata::parse(
            &json!({"guid":"g","macroPlaybackOption":"Once","futureAction":true})
        )
        .is_err());
        assert!(plan("[]").is_err());
    }
}
