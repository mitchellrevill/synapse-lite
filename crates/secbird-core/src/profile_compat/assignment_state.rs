//! Composition of local Customize intent; never applies a profile or emits input.
use super::{invalid, mappings, mouse_assignment, Profile, Result, SOURCE};
use serde_json::{json, Map, Value};
pub const LOCAL: &str = "_synapseLiteLocalAssignments";
const MANAGED: &str = "localAssignmentInputs";

fn retire_drafts(profile: &mut Profile, pid: u16, name: &str, rows: Vec<Value>) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    if !profile.extra.contains_key(SOURCE) {
        profile.extra.insert(
            SOURCE.into(),
            super::new_native_profile(pid, name)?
                .extra
                .remove(SOURCE)
                .unwrap(),
        );
    }
    let source = profile
        .extra
        .get_mut(SOURCE)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Malformed source metadata"))?;
    let retired = source
        .entry("retiredAssignmentDrafts")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or_else(|| invalid("Unknown retired draft metadata"))?;
    for row in rows {
        if !retired.contains(&row) {
            retired.push(row);
        }
    }
    Ok(())
}

/// Sensitivity remains an inert draft. Retire only same-layer older overlays;
/// decoded source records and executable button entries are never promoted.
pub fn clear_for_sensitivity(
    profile: &mut Profile,
    pid: u16,
    name: &str,
    input: &str,
    hyper: bool,
) -> Result<()> {
    let local = local_rows(profile)?;
    let mut retired: Vec<Value> = local
        .iter()
        .filter(|r| same(r, input, hyper))
        .map(|r| json!({"kind":"local","value":r}))
        .collect();
    if let Some(pending) = profile
        .extra
        .get(SOURCE)
        .and_then(|s| s.get("pendingMouseAssignments"))
    {
        let rows = pending
            .as_array()
            .ok_or_else(|| invalid("Unknown pending mouse assignments"))?;
        if rows
            .iter()
            .any(|r| r["input_id"] == input && r["hyper"] == hyper)
        {
            mouse_assignment::effective_mappings(profile, pid)?;
            retired.extend(
                rows.iter()
                    .filter(|r| r["input_id"] == input && r["hyper"] == hyper)
                    .map(|r| json!({"kind":"mouse","value":r})),
            );
        }
    }
    retire_drafts(profile, pid, name, retired)?;
    if let Some(rows) = profile
        .extra
        .get_mut(SOURCE)
        .and_then(|s| s.get_mut("pendingMouseAssignments"))
        .and_then(Value::as_array_mut)
    {
        rows.retain(|r| r["input_id"] != input || r["hyper"] != hyper);
    }
    if let Some(Value::Array(rows)) = profile.extra.get_mut(LOCAL) {
        rows.retain(|r| !same(r, input, hyper));
        if rows.is_empty() {
            profile.extra.remove(LOCAL);
        }
    }
    Ok(())
}

/// Leaving Sensitivity consumes just this input/layer's inert draft. Keep its
/// complete old value in private history and preserve all other pending drafts.
pub fn clear_sensitivity(
    profile: &mut Profile,
    pid: u16,
    name: &str,
    input: &str,
    hyper: bool,
) -> Result<()> {
    let key = crate::sensitivity_assignment::DRAFTS;
    let Some(raw) = profile.extra.get(key) else {
        return Ok(());
    };
    let drafts: Vec<crate::sensitivity_assignment::Intent> = serde_json::from_value(raw.clone())
        .map_err(|_| invalid("Unrecognized sensitivity draft schema; original retained"))?;
    if drafts.len() > 256 {
        return Err(invalid("Sensitivity draft count exceeds its budget"));
    }
    let rows = raw
        .as_array()
        .ok_or_else(|| invalid("Unknown sensitivity drafts"))?;
    let retired = rows
        .iter()
        .filter(|r| r["input"] == input && r["hyper"] == hyper)
        .map(|r| json!({"kind":"sensitivity","value":r}))
        .collect();
    retire_drafts(profile, pid, name, retired)?;
    let rows = profile.extra.get_mut(key).unwrap().as_array_mut().unwrap();
    rows.retain(|r| r["input"] != input || r["hyper"] != hyper);
    if rows.is_empty() {
        profile.extra.remove(key);
    }
    Ok(())
}

pub fn prepare_layer(
    profile: &Profile,
    pid: u16,
    name: &str,
    input: &str,
    hyper: bool,
) -> Result<Profile> {
    let mut next = profile.clone();
    clear_sensitivity(&mut next, pid, name, input, hyper)?;
    prepare_input(&next, pid, name, input)
}

pub fn validate_input(pid: u16, input: &str, kind: &str) -> Result<()> {
    let row = json!({"inputID":input,"isHyperShift":false});
    let expected = if pid == 605 {
        "KeyInput"
    } else if input == "DKM_SB_03" {
        "DKMInput"
    } else {
        "MouseInput"
    };
    if !matches!(pid, 605 | 196 | 197)
        || kind != expected
        || pid != 605 && input == "LeftClick"
        || mappings::setting_key(&row, pid).is_none()
    {
        return Err(invalid("Protected or unverified assignment input"));
    }
    Ok(())
}

fn same(row: &Value, input: &str, hyper: bool) -> bool {
    row["inputID"] == input && row["isHyperShift"].as_bool().unwrap_or(false) == hyper
}
fn local_rows(profile: &Profile) -> Result<Vec<Value>> {
    let rows = match profile.extra.get(LOCAL) {
        None => vec![],
        Some(Value::Array(rows)) => rows.clone(),
        Some(_) => {
            return Err(invalid(
                "Unknown local assignment format; original retained",
            ))
        }
    };
    if rows.len() > 256 {
        return Err(invalid("Local assignment count exceeds its budget"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for row in &rows {
        let id = row["inputID"]
            .as_str()
            .ok_or_else(|| invalid("Local input identity is malformed"))?;
        if !row.is_object()
            || !row["inputType"].is_string()
            || row.get("isHyperShift").is_some_and(|v| !v.is_boolean())
            || !matches!(
                row["outputType"].as_str(),
                Some("defaultGroup" | "disableGroup")
            )
            || !seen.insert((id, row["isHyperShift"].as_bool().unwrap_or(false)))
        {
            return Err(invalid("Ambiguous local assignment; original retained"));
        }
    }
    Ok(rows)
}
pub fn effective_mappings(profile: &Profile, pid: u16) -> Result<Vec<Value>> {
    let mut records = mouse_assignment::effective_mappings(profile, pid)?;
    for local in local_rows(profile)? {
        let id = local["inputID"].as_str().unwrap();
        validate_input(pid, id, local["inputType"].as_str().unwrap())?;
        let hyper = local["isHyperShift"].as_bool().unwrap_or(false);
        if records.iter().filter(|r| same(r, id, hyper)).count() > 1
            || records.iter().any(|r| {
                r["inputID"] == id && r.get("isHyperShift").is_some_and(|v| !v.is_boolean())
            })
        {
            return Err(invalid("Ambiguous source assignment; original retained"));
        }
        if let Some(record) = records.iter_mut().find(|r| same(r, id, hyper)) {
            let object = record
                .as_object_mut()
                .ok_or_else(|| invalid("Malformed source mapping"))?;
            for (key, value) in local.as_object().unwrap() {
                object.insert(key.clone(), value.clone());
            }
        } else {
            records.push(local);
        }
    }
    Ok(records)
}

/// Retire overlays for this physical input before another category takes over.
/// Both layers are materialized together so source Hypershift pair removal is
/// not undone. Unrelated inputs and their pending metadata stay byte-equivalent.
pub fn prepare_input(profile: &Profile, pid: u16, name: &str, input: &str) -> Result<Profile> {
    if profile
        .extra
        .get(SOURCE)
        .and_then(|s| s["decoded"]["mappings"].as_array())
        .is_some_and(|rows| {
            rows.iter().any(|r| {
                r["inputID"] == input
                    && (r.get("isHyperShift").is_some_and(|v| !v.is_boolean())
                        || r.get("mapping").is_some()
                        || r["isInterDevice"] == true)
            }) || [false, true]
                .iter()
                .any(|hyper| rows.iter().filter(|r| same(r, input, *hyper)).count() > 1)
        })
    {
        return Err(invalid(
            "Ambiguous or structured source input; original retained",
        ));
    }
    let local = local_rows(profile)?;
    let mouse = profile
        .extra
        .get(SOURCE)
        .and_then(|v| v.get("pendingMouseAssignments"));
    if mouse.is_some_and(|v| !v.is_array()) {
        return Err(invalid("Unknown pending mouse assignments"));
    }
    let affected = local.iter().any(|r| r["inputID"] == input)
        || mouse.and_then(Value::as_array).is_some_and(|rows| {
            rows.iter()
                .any(|r| r["input_id"] == input || r["inputID"] == input)
        });
    if !affected {
        return Ok(profile.clone());
    }
    let paired_clear = profile
        .extra
        .get(SOURCE)
        .and_then(|s| s["decoded"]["mappings"].as_array())
        .is_some_and(|rows| {
            rows.iter()
                .any(|r| same(r, input, false) && r["outputType"] == "hyperShiftGroup")
        })
        && mouse.and_then(Value::as_array).is_some_and(|rows| {
            rows.iter()
                .any(|r| r["input_id"] == input && r["hyper"] == false)
                && !rows
                    .iter()
                    .any(|r| r["input_id"] == input && r["hyper"] == true)
        });
    let effective = effective_mappings(profile, pid)?;
    let mut next = profile.clone();
    if !next.extra.contains_key(SOURCE) {
        next.extra.insert(
            SOURCE.into(),
            super::new_native_profile(pid, name)?
                .extra
                .remove(SOURCE)
                .unwrap(),
        );
    }
    let source = next
        .extra
        .get_mut(SOURCE)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Malformed source metadata"))?;
    let decoded = source
        .get_mut("decoded")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("Malformed source payload"))?;
    let records = decoded
        .entry("mappings")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or_else(|| invalid("Malformed source mappings"))?;
    let retired: Vec<Value> = records
        .iter()
        .filter(|r| {
            r["inputID"] == input
                && !effective
                    .iter()
                    .any(|e| same(e, input, r["isHyperShift"].as_bool().unwrap_or(false)))
        })
        .cloned()
        .collect();
    let position = records
        .iter()
        .position(|r| r["inputID"] == input)
        .unwrap_or(records.len());
    records.retain(|r| r["inputID"] != input);
    for (offset, row) in effective
        .into_iter()
        .filter(|r| r["inputID"] == input)
        .enumerate()
    {
        records.insert((position + offset).min(records.len()), row);
    }
    let retired_layers: Vec<bool> = retired
        .iter()
        .map(|r| r["isHyperShift"].as_bool().unwrap_or(false))
        .collect();
    if !retired.is_empty() {
        let history = source
            .entry("retiredAssignmentRecords")
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(|| invalid("Unknown retired assignment metadata"))?;
        for row in retired {
            if !history.contains(&row) {
                history.push(row);
            }
        }
    }
    if let Some(rows) = source
        .get_mut("pendingMouseAssignments")
        .and_then(Value::as_array_mut)
    {
        rows.retain(|r| r["input_id"] != input && r["inputID"] != input);
    }
    if let Some(Value::Array(rows)) = next.extra.get_mut(LOCAL) {
        rows.retain(|r| r["inputID"] != input);
        if rows.is_empty() {
            next.extra.remove(LOCAL);
        }
    }
    for hyper in [false, true] {
        let authored = local.iter().any(|r| same(r, input, hyper))
            || mouse.and_then(Value::as_array).is_some_and(|rows| {
                rows.iter()
                    .any(|r| r["input_id"] == input && r["hyper"] == hyper)
            });
        let removed = retired_layers.contains(&hyper) || paired_clear && hyper;
        if authored || removed {
            mark_local(&mut next, pid, input, hyper)?;
        }
    }
    Ok(next)
}
pub fn mark_local(profile: &mut Profile, pid: u16, input: &str, hyper: bool) -> Result<()> {
    let record = json!({"inputID":input,"isHyperShift":hyper});
    let key = mappings::setting_key(&record, pid)
        .ok_or_else(|| invalid("Unverified assignment input"))?;
    profile.buttons.remove(&key);
    if let Some(source) = profile.extra.get_mut(SOURCE) {
        let source = source
            .as_object_mut()
            .ok_or_else(|| invalid("Malformed source metadata"))?;
        let rows = source
            .entry(MANAGED)
            .or_insert(json!([]))
            .as_array_mut()
            .ok_or_else(|| invalid("Unknown local ownership metadata"))?;
        if rows.len() > 256
            || rows.iter().any(|r| {
                !r.is_object() || !r["inputID"].is_string() || !r["isHyperShift"].is_boolean()
            })
        {
            return Err(invalid(
                "Malformed local ownership metadata; original retained",
            ));
        }
        if !rows.iter().any(|r| same(r, input, hyper)) {
            rows.push(record);
        }
    }
    Ok(())
}
pub(super) fn managed(profile: &Profile, row: &Value) -> bool {
    profile
        .extra
        .get(SOURCE)
        .and_then(|v| v[MANAGED].as_array())
        .is_some_and(|rows| {
            rows.iter().any(|r| {
                r["inputID"] == row["inputID"]
                    && r["isHyperShift"].as_bool()
                        == Some(row["isHyperShift"].as_bool().unwrap_or(false))
            })
        })
}
pub(super) fn export_local(
    profile: &Profile,
    decoded: &mut Map<String, Value>,
    pid: u16,
) -> Result<()> {
    let local = local_rows(profile)?;
    if local.is_empty() {
        return Ok(());
    }
    if decoded.contains_key("mapping") || decoded.contains_key("nativeMapping") {
        return Err(invalid(
            "Native assignment cache rewriting is unverified; retain a native backup",
        ));
    }
    let effective = effective_mappings(profile, pid)?;
    let rows = decoded
        .entry("mappings")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or_else(|| invalid("Malformed exported mappings"))?;
    for local in local {
        let input = local["inputID"].as_str().unwrap();
        let hyper = local["isHyperShift"].as_bool().unwrap_or(false);
        let row = effective
            .iter()
            .find(|r| same(r, input, hyper))
            .unwrap()
            .clone();
        let position = rows
            .iter()
            .position(|r| same(r, input, hyper))
            .unwrap_or(rows.len());
        rows.retain(|r| !same(r, input, hyper));
        rows.insert(position.min(rows.len()), row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retiring_mouse_overlay_preserves_removed_hypershift_record_and_other_intent() {
        let mut profile = super::super::new_native_profile(605, "Fixture").unwrap();
        profile.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = json!([
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"hyperShiftGroup","opaque":1},
            {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"keep":17}},
            {"inputID":"KEY_B","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"other":19}}
        ]);
        for input in ["KEY_A", "KEY_B"] {
            mouse_assignment::set_pending(
                &mut profile,
                605,
                "Fixture",
                mouse_assignment::Assignment {
                    input_id: input.into(),
                    input_type: "KeyInput".into(),
                    hyper: false,
                    choice: mouse_assignment::Choice::Click,
                    turbo: None,
                },
            )
            .unwrap();
        }
        let other = profile.extra[SOURCE]["pendingMouseAssignments"][1].clone();
        let prepared = prepare_input(&profile, 605, "Fixture", "KEY_A").unwrap();
        assert_eq!(
            prepared.extra[SOURCE]["pendingMouseAssignments"],
            json!([other])
        );
        assert_eq!(
            prepared.extra[SOURCE]["retiredAssignmentRecords"][0]["futureGroup"],
            json!({"keep":17})
        );
        let rows = effective_mappings(&prepared, 605).unwrap();
        assert!(!rows.iter().any(|r| same(r, "KEY_A", true)));
        assert_eq!(
            rows.iter().find(|r| same(r, "KEY_B", true)).unwrap()["futureGroup"],
            json!({"other":19})
        );
        assert_eq!(
            profile.extra[SOURCE]["decoded"]["mappings"][1]["futureGroup"],
            json!({"keep":17})
        );
    }
    #[test]
    fn malformed_local_and_source_ownership_refuse_without_mutation() {
        for rows in [
            json!({"future":true}),
            json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":"false","outputType":"disableGroup"}]),
            json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup"}]),
        ] {
            let mut profile = super::super::new_native_profile(605, "Fixture").unwrap();
            profile.extra.insert(LOCAL.into(), rows);
            let before = profile.clone();
            assert!(prepare_input(&profile, 605, "Fixture", "KEY_A").is_err());
            assert_eq!(profile, before);
        }
    }
}
