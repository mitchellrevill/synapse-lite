//! Mapping groups whose Synapse UI fields and native actions have matching meaning.

use super::{integer, invalid, Error, Result};
use crate::buttons::{self, Action, Input, Layer};
use crate::profile::Profile;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct KeyId {
    id: String,
    usage: u8,
}

fn key_ids() -> &'static [KeyId] {
    static KEYS: OnceLock<Vec<KeyId>> = OnceLock::new();
    KEYS.get_or_init(|| {
        serde_json::from_str(include_str!("key_ids.json")).expect("checked-in key ID data is valid")
    })
}

pub(super) fn key_usage(id: &str) -> Option<u8> {
    key_ids()
        .iter()
        .find(|key| key.id == id)
        .map(|key| key.usage)
}
fn key_id(usage: u8) -> Result<&'static str> {
    key_ids()
        .iter()
        .find(|key| key.usage == usage)
        .map(|key| key.id.as_str())
        .ok_or_else(|| {
            Error::Unsupported(format!("No verified Synapse key ID for HID usage {usage}"))
        })
}

const MOUSE_INPUTS: &[(&str, &str)] = &[
    ("LeftClick", "left"),
    ("RightClick", "right"),
    ("ScrollButton", "wheel"),
    ("ScrollUp", "scroll-up"),
    ("ScrollDown", "scroll-down"),
    ("Button4", "side-back"),
    ("Button5", "side-forward"),
    ("DKM_SB_03", "dpi"),
];

pub(super) fn setting_key(record: &Value, pid: u16) -> Option<String> {
    let input = record.get("inputID")?.as_str()?;
    let name = if pid == 605 {
        let usage = key_usage(input)?;
        let position = buttons::KEYS.iter().find(|key| key.3 == usage)?.0;
        Input::Key(position).name()
    } else {
        MOUSE_INPUTS
            .iter()
            .find(|entry| entry.0 == input)?
            .1
            .to_string()
    };
    let hypershift = match record.get("isHyperShift") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return None,
    };
    let layer = if hypershift {
        Layer::HyperShift
    } else {
        Layer::Normal
    };
    Some(buttons::setting_key(&name, layer))
}

fn input_record(key: &str, pid: u16) -> Result<Value> {
    let (input, layer) = buttons::parse_setting_key(key)?;
    let (id, kind) = match (pid, input) {
        (605, Input::Key(position)) => {
            let usage = buttons::KEYS
                .iter()
                .find(|entry| entry.0 == position)
                .ok_or_else(|| invalid("Unknown native key position"))?
                .3;
            (key_id(usage)?, "KeyInput")
        }
        (196 | 197, Input::Button(_)) => {
            let name = input.name();
            let id = MOUSE_INPUTS
                .iter()
                .find(|entry| entry.1 == name)
                .ok_or_else(|| {
                    Error::Unsupported(format!("No verified DeathAdder input ID for {name}"))
                })?
                .0;
            (
                id,
                if id.starts_with("DKM_") {
                    "DKMInput"
                } else {
                    "MouseInput"
                },
            )
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "Input '{key}' is not verified for this model"
            )))
        }
    };
    Ok(json!({"inputID":id,"inputType":kind,"isHyperShift":layer == Layer::HyperShift}))
}

pub(super) fn import(
    source: &Value,
    profile: &mut Profile,
    pid: u16,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(records) = source.get("mappings") else {
        return Ok(());
    };
    let records = records
        .as_array()
        .ok_or_else(|| invalid("Synapse mappings must be an array"))?;
    let mut seen = BTreeSet::new();
    for record in records {
        let Some(key) = setting_key(record, pid) else {
            warnings.push(format!(
                "{name}: mappings for unknown input '{}' is preserved but cannot be applied",
                record.get("inputID").and_then(Value::as_str).unwrap_or("?")
            ));
            continue;
        };
        if !seen.insert(key.clone()) {
            return Err(invalid(format!("Duplicate Synapse mapping for '{key}'")));
        }
        match action(record)? {
            Some(action) => {
                profile.buttons.insert(key, action.to_string());
            }
            None => warnings.push(format!(
                "{name}: mappings for '{key}' group '{}' is preserved but cannot be applied",
                record
                    .get("outputType")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
            )),
        }
    }
    Ok(())
}

fn action(record: &Value) -> Result<Option<Action>> {
    if record.get("mapping").is_some()
        || record.get("isInterDevice").and_then(Value::as_bool) == Some(true)
    {
        return Ok(None);
    }
    let output = record
        .get("outputType")
        .and_then(Value::as_str)
        .unwrap_or("");
    let group = &record[output];
    let turbo = |group: &Value| -> Result<Option<u8>> {
        let Some(turbo) = group.get("turboMode") else {
            return Ok(None);
        };
        if turbo.get("isTurbo").and_then(Value::as_bool) != Some(true) {
            return Ok(None);
        }
        Ok(Some(
            integer(turbo.get("keysPerSecond"), 1, 255, "keysPerSecond")? as u8,
        ))
    };
    Ok(Some(match output {
        "disableGroup" => Action::Disabled,
        "hyperShiftGroup" => Action::HyperShift,
        "mouseGroup" => {
            let assignment = group
                .get("mouseAssignment")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("mouseGroup has no mouseAssignment"))?;
            let button = match assignment {
                "Click" => 1,
                "Menu" => 2,
                "ScrollButton" => 3,
                "Previous" => 4,
                "Next" => 5,
                "ScrollUp" => 9,
                "ScrollDown" => 10,
                "ScrollLeft" => 104,
                "ScrollRight" => 105,
                "DoubleClick" if turbo(group)?.is_none() => return Ok(Some(Action::DoubleClick)),
                _ => return Ok(None),
            };
            match turbo(group)? {
                Some(rate) if button <= 5 => Action::TurboMouse { button, rate },
                Some(_) => return Ok(None),
                None => Action::Mouse(button),
            }
        }
        "keyboardGroup" => {
            if group
                .get("keyGroup")
                .is_some_and(|value| value.as_u64() != Some(0))
            {
                return Ok(None);
            }
            let usage = match &group["key"] {
                Value::String(id) => match key_usage(id) {
                    Some(usage) => usage,
                    None => return Ok(None),
                },
                Value::Object(object)
                    if object.get("pageID").and_then(Value::as_str) == Some("7")
                        && object
                            .get("flag")
                            .is_none_or(|value| value.as_u64() == Some(0)) =>
                {
                    let Some(usage) = object
                        .get("HID")
                        .and_then(Value::as_str)
                        .and_then(|value| value.parse::<u8>().ok())
                    else {
                        return Ok(None);
                    };
                    if key_id(usage).is_err() {
                        return Ok(None);
                    }
                    usage
                }
                _ => return Ok(None),
            };
            let modifiers = group
                .get("modifiers")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("keyboardGroup.modifiers must be an array"))?;
            let mut bits = 0;
            for id in modifiers {
                let Some(usage @ 224..=231) = id.as_str().and_then(key_usage) else {
                    return Ok(None);
                };
                bits |= 1 << (usage - 224);
            }
            match turbo(group)? {
                Some(rate) => Action::TurboKey {
                    modifiers: bits,
                    usage,
                    rate,
                },
                None => Action::Key {
                    modifiers: bits,
                    usage,
                },
            }
        }
        "sensitivityGroup" => match group.get("sensitivityAssignment").and_then(Value::as_str) {
            Some("DPI_Up") => Action::DpiUp,
            Some("DPI_Down") => Action::DpiDown,
            Some("DPI_CycleUp") => Action::DpiCycleUp,
            Some("DPI_CycleDown") => Action::DpiCycleDown,
            Some("DPI_Clutch") => {
                let x = integer(group.get("x"), 100, 26_000, "DPI clutch x")? as u16;
                let y = integer(group.get("y"), 100, 26_000, "DPI clutch y")? as u16;
                let independent = group
                    .get("independent")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("DPI clutch independent must be boolean"))?;
                Action::DpiClutch {
                    x,
                    y: if independent { y } else { x },
                }
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    }))
}

fn action_fields(action: Action) -> Result<Value> {
    let mouse = |button| match button {
        1 => Some("Click"),
        2 => Some("Menu"),
        3 => Some("ScrollButton"),
        4 => Some("Previous"),
        5 => Some("Next"),
        9 => Some("ScrollUp"),
        10 => Some("ScrollDown"),
        104 => Some("ScrollLeft"),
        105 => Some("ScrollRight"),
        _ => None,
    };
    let keyboard = |modifiers: u8, usage: u8, rate: Option<u8>| -> Result<Value> {
        let mut ids = Vec::new();
        for bit in 0..8 {
            if modifiers & (1 << bit) != 0 {
                ids.push(key_id(224 + bit)?);
            }
        }
        let mut group = json!({"key":key_id(usage)?,"keyGroup":0,"modifiers":ids});
        group["turboMode"] = json!({"isTurbo":rate.is_some(),"keysPerSecond":rate.unwrap_or(7)});
        Ok(json!({"outputType":"keyboardGroup","keyboardGroup":group}))
    };
    Ok(match action {
        Action::Disabled => json!({"outputType":"disableGroup"}),
        Action::HyperShift => json!({"outputType":"hyperShiftGroup"}),
        Action::Mouse(button) => {
            json!({"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":mouse(button).ok_or_else(|| invalid("Unknown mouse action"))?,"turboMode":{"isTurbo":false,"keysPerSecond":7}}})
        }
        Action::DoubleClick => {
            json!({"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"DoubleClick","turboMode":{"isTurbo":false,"keysPerSecond":7}}})
        }
        Action::TurboMouse { button, rate } if button <= 5 && rate > 0 => {
            json!({"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":mouse(button).ok_or_else(|| invalid("Unknown turbo mouse action"))?,"turboMode":{"isTurbo":true,"keysPerSecond":rate}}})
        }
        Action::Key { modifiers, usage } => keyboard(modifiers, usage, None)?,
        Action::TurboKey {
            modifiers,
            usage,
            rate,
        } if rate > 0 => keyboard(modifiers, usage, Some(rate))?,
        Action::DpiUp => {
            json!({"outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_Up"}})
        }
        Action::DpiDown => {
            json!({"outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_Down"}})
        }
        Action::DpiCycleUp => {
            json!({"outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_CycleUp"}})
        }
        Action::DpiCycleDown => {
            json!({"outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_CycleDown"}})
        }
        Action::DpiClutch { x, y }
            if (100..=26_000).contains(&x) && (100..=26_000).contains(&y) =>
        {
            json!({"outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_Clutch","x":x,"y":y,"independent":x!=y}})
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "Native action '{action}' has no verified Synapse translation; export refused"
            )))
        }
    })
}

pub(super) fn export(
    profile: &Profile,
    decoded: &mut serde_json::Map<String, Value>,
    pid: u16,
) -> Result<()> {
    let original = decoded
        .get("mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut records = Vec::new();
    let mut represented = BTreeSet::new();
    for mut record in original {
        if super::keyboard_assignments::local_only(profile, &record) || super::assignment_state::managed(profile,&record) {
            if setting_key(&record, pid).is_some_and(|key|profile.buttons.contains_key(&key)) {
                return Err(invalid("Local keyboard intent conflicts with a newer native button projection; retain a native backup and resolve the assignment"));
            }
            records.push(record);
            continue;
        }
        let Some(key) = setting_key(&record, pid) else {
            records.push(record);
            continue;
        };
        if let Some(text) = profile.buttons.get(&key) {
            let fields = action_fields(text.parse()?)?;
            merge_mapping(&mut record, fields)?;
            represented.insert(key);
            records.push(record);
        } else if action(&record)?.is_none() {
            // This record was never translated, so absence from native buttons is not
            // evidence the user deleted it. Preserve its entire foreign representation.
            records.push(record);
        }
    }
    for (key, text) in &profile.buttons {
        if represented.contains(key) {
            continue;
        }
        let mut record = input_record(key, pid)?;
        merge_mapping(&mut record, action_fields(text.parse()?)?)?;
        records.push(record);
    }
    decoded.insert("mappings".into(), Value::Array(records));
    Ok(())
}

fn merge_mapping(record: &mut Value, fields: Value) -> Result<()> {
    let group = fields
        .get("outputType")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Translated mapping has no output type"))?;
    if let Some(group_fields) = fields.get(group) {
        let object = record
            .as_object_mut()
            .ok_or_else(|| invalid("Mapping must be an object"))?;
        let existing = object.entry(group).or_insert(json!({}));
        merge_fields(existing, group_fields)?;
    }
    record
        .as_object_mut()
        .ok_or_else(|| invalid("Mapping must be an object"))?
        .insert("outputType".into(), json!(group));
    Ok(())
}

fn merge_fields(target: &mut Value, fields: &Value) -> Result<()> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| invalid("Mapping group must be an object"))?;
    for (key, value) in fields
        .as_object()
        .ok_or_else(|| invalid("Translated mapping group must be an object"))?
    {
        if let Some(existing) = object
            .get_mut(key)
            .filter(|existing| existing.is_object() && value.is_object())
        {
            merge_fields(existing, value)?;
        } else {
            object.insert(key.clone(), value.clone());
        }
    }
    Ok(())
}
