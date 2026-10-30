//! Local desired mouse assignments. No device, input sender or driver access.
//! Retain pending intent beside the source payload; firmware replay never sees it.
use super::{invalid, Profile, Result, SOURCE};
use crate::source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson, SymbolicPlan};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

const PENDING: &str = "pendingMouseAssignments";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    Click,
    Menu,
    ScrollButton,
    DoubleClick,
    ScrollUp,
    ScrollDown,
    Previous,
    Next,
    ScrollLeft,
    ScrollRight,
}
impl Choice {
    pub fn parse(value: &str) -> Result<Self> {
        serde_json::from_value(json!(value)).map_err(|_| invalid("Unsupported mouse function"))
    }
    pub fn source_name(self) -> &'static str {
        match self {
            Self::Click => "Click",
            Self::Menu => "Menu",
            Self::ScrollButton => "ScrollButton",
            Self::DoubleClick => "DoubleClick",
            Self::ScrollUp => "ScrollUp",
            Self::ScrollDown => "ScrollDown",
            Self::Previous => "Previous",
            Self::Next => "Next",
            Self::ScrollLeft => "ScrollLeft",
            Self::ScrollRight => "ScrollRight",
        }
    }
    /// A bounded pulse for the existing source planner/packet APIs, never a sender.
    /// Double-click's 10ms inter-click gap is pinned by source converter fixture9.
    /// Held/Turbo trigger scheduling requires the independently gated runtime.
    pub fn pulse_plan(self) -> Result<SymbolicPlan> {
        let pair = |id| {
            vec![
                json!({"type":"mouse","id":id}),
                json!({"type":"mouse","id":id*2}),
            ]
        };
        let events = match self {
            Self::Click => pair(1),
            Self::Menu => pair(4),
            Self::ScrollButton => pair(16),
            Self::Previous => pair(64),
            Self::Next => pair(256),
            Self::DoubleClick => {
                let mut events = pair(1);
                events.push(json!({"type":"delay","ms":10}));
                events.extend(pair(1));
                events
            }
            Self::ScrollUp | Self::ScrollDown | Self::ScrollLeft | Self::ScrollRight => {
                let id = if matches!(self, Self::ScrollLeft | Self::ScrollRight) {
                    2048
                } else {
                    1024
                };
                let data = if matches!(self, Self::ScrollDown | Self::ScrollLeft) {
                    65416
                } else {
                    120
                };
                vec![json!({"type":"mouse","id":id,"data":data})]
            }
        };
        SourceJson::parse(&json!(events).to_string(), Limits::default())
            .and_then(|source| {
                source.plan(
                    EventLocation::RootArray,
                    &PlaybackMetadata::parse(
                        &json!({"guid":"assignment-pulse","macroPlaybackOption":"Once"}),
                    )?,
                )
            })
            .map_err(|error| invalid(error.to_string()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub input_id: String,
    pub input_type: String,
    pub hyper: bool,
    pub choice: Choice,
    pub turbo: Option<u8>,
}
impl Assignment {
    pub fn validate(&self, pid: u16) -> Result<()> {
        let valid_input = match pid {
            196 | 197 => match self.input_id.as_str() {
                "RightClick" | "ScrollButton" | "ScrollUp" | "ScrollDown" | "Button4"
                | "Button5" => self.input_type == "MouseInput",
                "DKM_SB_03" => self.input_type == "DKMInput",
                _ => false, // Source primary LeftClick is protected.
            },
            605 => {
                self.input_type == "KeyInput" && super::source_key_usage(&self.input_id).is_some()
            }
            _ => false,
        };
        if !valid_input {
            return Err(invalid(
                "Mouse Function is unavailable for this input/model",
            ));
        }
        if let Some(rate) = self.turbo {
            if !(1..=20).contains(&rate)
                || matches!(
                    self.input_id.as_str(),
                    "ScrollUp" | "ScrollDown" | "DKM_SB_03"
                )
            {
                return Err(invalid(
                    "Turbo is unavailable for this input or outside 1..20",
                ));
            }
        }
        Ok(())
    }
}

fn pending(profile: &Profile) -> Result<Vec<Assignment>> {
    let changes: Vec<Assignment> = match profile
        .extra
        .get(SOURCE)
        .and_then(|source| source.get(PENDING))
    {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|_| invalid("Invalid retained mouse assignments")),
        None => Ok(vec![]),
    }?;
    if changes.len() > 256 {
        return Err(invalid("Mouse assignment budget exceeded"));
    }
    Ok(changes)
}
fn matches(record: &Value, id: &str, hyper: bool) -> bool {
    record["inputID"] == id
        && record
            .get("isHyperShift")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            == hyper
}
fn source_records(profile: &Profile) -> Result<Vec<Value>> {
    let Some(source) = profile.extra.get(SOURCE) else {
        return Ok(vec![]);
    };
    let decoded = source
        .get("decoded")
        .filter(|decoded| decoded.is_object())
        .ok_or_else(|| invalid("Source profile payload must be an object"))?;
    match decoded.get("mappings") {
        Some(Value::Array(records)) => Ok(records.clone()),
        Some(_) => Err(invalid("Source mappings must be an array")),
        None => Ok(vec![]),
    }
}
fn overlay(profile: &Profile, records: &mut Vec<Value>, pid: u16) -> Result<()> {
    let original = source_records(profile)?;
    let changes = pending(profile)?;
    let mut seen = std::collections::BTreeSet::new();
    for change in &changes {
        change.validate(pid)?;
        if original.iter().any(|record| {
            record["inputID"] == change.input_id
                && record
                    .get("isHyperShift")
                    .is_some_and(|value| !value.is_boolean())
        }) {
            return Err(invalid(
                "Source mapping layer is ambiguous; original retained",
            ));
        }
        if !seen.insert((&change.input_id, change.hyper)) {
            return Err(invalid("Duplicate pending mouse assignment"));
        }
        let originals: Vec<_> = original
            .iter()
            .filter(|r| matches(r, &change.input_id, change.hyper))
            .collect();
        if originals.len() > 1 {
            return Err(invalid("Ambiguous source mapping; original retained"));
        }
        if originals.iter().any(|r| {
            r.get("mapping").is_some()
                || r["isInterDevice"] == true
                || r.get("isHyperShift").is_some_and(|v| !v.is_boolean())
                || r["inputType"] != change.input_type
        }) {
            return Err(invalid(
                "Structured/inter-device source mapping cannot be edited here",
            ));
        }
        // Shipped popup removes the paired secondary when replacing normal Hypershift.
        let remove_pair = !change.hyper
            && originals
                .iter()
                .any(|r| r["outputType"] == "hyperShiftGroup")
            && !changes
                .iter()
                .any(|other| other.input_id == change.input_id && other.hyper);
        if remove_pair
            && original
                .iter()
                .filter(|r| matches(r, &change.input_id, true))
                .count()
                > 1
        {
            return Err(invalid(
                "Ambiguous paired source mapping; original retained",
            ));
        }
        let mut record = originals.first().map(|r| (*r).clone()).unwrap_or_else(|| json!({"inputID":change.input_id,"inputType":change.input_type,"isHyperShift":change.hyper}));
        // Preserve unknown root/group/Turbo leaves while replacing only authored fields.
        let object = record
            .as_object_mut()
            .ok_or_else(|| invalid("Mapping must be an object"))?;
        object.insert("outputType".into(), json!("mouseGroup"));
        if object.contains_key("isDefault") {
            object.insert("isDefault".into(), json!(false));
        }
        let group = object
            .entry("mouseGroup")
            .or_insert(json!({}))
            .as_object_mut()
            .ok_or_else(|| invalid("Mouse group must be an object"))?;
        group.insert("mouseAssignment".into(), json!(change.choice.source_name()));
        let turbo = group
            .entry("turboMode")
            .or_insert(json!({}))
            .as_object_mut()
            .ok_or_else(|| invalid("Turbo group must be an object"))?;
        turbo.insert("isTurbo".into(), json!(change.turbo.is_some()));
        if let Some(rate) = change.turbo {
            turbo.insert("keysPerSecond".into(), json!(rate));
        } else {
            turbo.entry("keysPerSecond").or_insert(json!(7));
        }
        let position = records
            .iter()
            .position(|r| matches(r, &change.input_id, change.hyper))
            .unwrap_or(records.len());
        records.retain(|r| {
            !(matches(r, &change.input_id, change.hyper)
                || remove_pair && matches(r, &change.input_id, true))
        });
        records.insert(position.min(records.len()), record);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

pub fn effective_mappings(profile: &Profile, pid: u16) -> Result<Vec<Value>> {
    let mut records = source_records(profile)?;
    overlay(profile, &mut records, pid)?;
    Ok(records)
}

/// Atomic in-memory update. Caller must compare its full profile baseline and save
/// under the existing store guard. Does not add firmware replay entries.
pub fn set_pending(
    profile: &mut Profile,
    pid: u16,
    name: &str,
    assignment: Assignment,
) -> Result<()> {
    assignment.validate(pid)?;
    let mut next = profile.clone();
    if !next.extra.contains_key(SOURCE) {
        let mut source = super::new_native_profile(pid, name)?;
        next.extra
            .insert(SOURCE.into(), source.extra.remove(SOURCE).unwrap());
    }
    let mut changes = pending(&next)?;
    // Replacing normal Hypershift clears its paired pending secondary as well.
    let paired = !assignment.hyper
        && effective_mappings(&next, pid)?.iter().any(|r| {
            matches(r, &assignment.input_id, false) && r["outputType"] == "hyperShiftGroup"
        });
    changes.retain(|other| {
        other.input_id != assignment.input_id || (!paired && other.hyper != assignment.hyper)
    });
    changes.push(assignment);
    if changes.len() > 256 {
        return Err(invalid("Mouse assignment budget exceeded"));
    }
    next.extra
        .get_mut(SOURCE)
        .unwrap()
        .as_object_mut()
        .ok_or_else(|| invalid("Source metadata must be an object"))?
        .insert(PENDING.into(), json!(changes));
    effective_mappings(&next, pid)?;
    *profile = next;
    Ok(())
}

pub(super) fn export_pending(
    profile: &Profile,
    decoded: &mut Map<String, Value>,
    pid: u16,
) -> Result<()> {
    if pending(profile)?.is_empty() {
        return Ok(());
    }
    let mut records = decoded
        .get("mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    overlay(profile, &mut records, pid)?;
    decoded.insert("mappings".into(), json!(records));
    Ok(())
}
