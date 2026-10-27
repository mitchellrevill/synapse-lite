//! GUI-only source profile selection. Existing interchange entry points are unchanged.
//! Preview and document construction perform no storage or device operations.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Synapse4,
    NativeBackup,
}
#[derive(Clone, Debug)]
pub struct MacroChoice {
    pub guid: String,
    pub name: String,
    pub available: bool,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub device_key: String,
    pub name: String,
    pub guid: Option<String>,
    pub macros: Vec<MacroChoice>,
}
#[derive(Clone, Debug, Default)]
pub struct Choice {
    pub selected: bool,
    pub macros: BTreeSet<String>,
}
#[derive(Debug)]
pub struct Preview {
    pub kind: Kind,
    pub rows: Vec<Row>,
    pub warnings: Vec<String>,
    pub has_source_warning: bool,
    store: Store,
    backup: Vec<u8>,
    export_target: Option<String>,
}
fn family(key: &str) -> String {
    let p = key.split(':').next().unwrap_or(key).to_ascii_uppercase();
    if p == "00C5" {
        "00C4".into()
    } else {
        p
    }
}
/// Resolve only an exact physical identity or a model-only legacy entry.
/// Receiver/device aliases require the same serial; a different unit is never a fallback.
pub fn stored_device_key(store: &Store, target: &str) -> Option<String> {
    if store.devices.contains_key(target) {
        return Some(target.into());
    }
    let (pid, serial) = target
        .split_once(':')
        .map_or((target, None), |(p, s)| (p, Some(s)));
    if let Some(serial) = serial {
        let peer = match pid {
            "00C4" => Some("00C5"),
            "00C5" => Some("00C4"),
            _ => None,
        };
        if let Some(peer) = peer {
            let key = format!("{peer}:{serial}");
            if store.devices.contains_key(&key) {
                return Some(key);
            }
        }
    }
    if store.devices.contains_key(pid) {
        return Some(pid.into());
    }
    let peer = match pid {
        "00C4" => Some("00C5"),
        "00C5" => Some("00C4"),
        _ => None,
    };
    peer.filter(|p| store.devices.contains_key(*p))
        .map(str::to_string)
}
fn refs(value: &Value, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(o) => {
            if let Some(g) = o.get("macroGroup").and_then(Value::as_object) {
                if let Some(id) = g.get("guid").and_then(Value::as_str) {
                    out.entry(id.into()).or_insert_with(|| {
                        g.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("Macro")
                            .into()
                    });
                }
            }
            for v in o.values() {
                refs(v, out)
            }
        }
        Value::Array(a) => {
            for v in a {
                refs(v, out)
            }
        }
        _ => {}
    }
}
fn envelopes(device: &DeviceProfiles) -> Vec<&Value> {
    device
        .extra
        .iter()
        .filter(|(k, v)| {
            (*k == SOURCE || k.starts_with(&format!("_synapseLiteImported:{SOURCE}")))
                && v.get("profiles").is_some()
        })
        .map(|(_, v)| v)
        .collect()
}
fn macro_records(device: &DeviceProfiles) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for envelope in envelopes(device) {
        if let Some(a) = envelope.get("macros").and_then(Value::as_array) {
            for entry in a {
                let decoded = decode_record(entry)?;
                let guid = decoded
                    .get("guid")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Macro has no GUID"))?;
                match out.get(guid){Some(old)if old!=entry=>return Err(invalid("Retained source documents contain conflicting macro identities; use a full native backup")),_=>{out.insert(guid.into(),entry.clone());}}
            }
        }
    }
    Ok(out)
}
fn rows(store: &Store, target: Option<&str>) -> Result<Vec<Row>> {
    let mut out = Vec::new();
    for (key, device) in &store.devices {
        if target.is_some_and(|t| t != key) {
            continue;
        }
        let records = macro_records(device).unwrap_or_default();
        let first = out.len();
        for (name, p) in &device.profiles {
            let decoded = p.extra.get(SOURCE).and_then(|v| v.get("decoded"));
            let mut ids = BTreeMap::new();
            if let Some(d) = decoded {
                refs(d, &mut ids)
            }
            let macros = ids
                .into_iter()
                .map(|(guid, fallback)| {
                    let name = records
                        .get(&guid)
                        .and_then(|r| r.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or(&fallback)
                        .to_string();
                    MacroChoice {
                        available: records.contains_key(&guid),
                        guid,
                        name,
                    }
                })
                .collect();
            out.push(Row {
                device_key: key.clone(),
                name: name.clone(),
                guid: decoded
                    .and_then(|d| d.get("guid"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                macros,
            });
        }
        // Retained source records contain list order even though native storage
        // uses a name-sorted map. Bind by GUID so a rename cannot reorder a row.
        let mut order = BTreeMap::new();
        for envelope in envelopes(device) {
            for record in envelope["profiles"].as_array().into_iter().flatten() {
                if let Ok(decoded) = decode_record(record) {
                    if let Some(guid) = decoded["guid"].as_str() {
                        let next = order.len();
                        order.entry(guid.to_string()).or_insert(next);
                    }
                }
            }
        }
        out[first..].sort_by_key(|row| {
            row.guid
                .as_ref()
                .and_then(|guid| order.get(guid))
                .copied()
                .unwrap_or(usize::MAX)
        });
    }
    Ok(out)
}
fn preview_warnings(store: &Store, target: Option<&str>) -> Vec<String> {
    store
        .devices
        .iter()
        .filter(|(k, _)| target.is_none_or(|t| t == k.as_str()))
        .filter_map(|(_, d)| macro_records(d).err().map(|e| e.to_string()))
        .collect()
}
pub fn preview_import(bytes: &[u8]) -> Result<Preview> {
    // Bound the original document before parsing/filtering; corrupt records do
    // not provide a way around the existing resource limits.
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid("Profile document exceeds 2 MiB"));
    }
    let raw = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if raw.starts_with(b"PK") {
        return Err(Error::Unsupported(
            "Synapse 3 ZIP migration is not implemented; original archive was not modified".into(),
        ));
    }
    check_json_complexity(raw)?;
    let mut value: Value =
        serde_json::from_slice(raw).map_err(|e| invalid(format!("Invalid profile JSON: {e}")))?;
    let kind = if value.get("devices").is_some() {
        Kind::NativeBackup
    } else {
        Kind::Synapse4
    };
    let mut skipped = Vec::new();
    let mut report = if kind == Kind::NativeBackup {
        import_document(bytes)?
    } else {
        let pid = value
            .get("productId")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("Synapse export has no numeric productId"))?;
        let category = match pid {
            196 | 197 => "MOUSE",
            605 => "KEYBOARD",
            _ => {
                return Err(invalid(
                    "GUI source import is verified only for products196,197,605",
                ))
            }
        };
        if value.get("category").and_then(Value::as_str) != Some(category) {
            return Err(invalid("Synapse productId and category do not match"));
        }
        let profiles = value
            .get("profiles")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Synapse export has no profiles array"))?;
        let macros = value
            .get("macros")
            .map(|v| {
                v.as_array()
                    .ok_or_else(|| invalid("Synapse macros must be an array"))
            })
            .transpose()?;
        let encoded: usize = profiles
            .iter()
            .chain(macros.into_iter().flatten())
            .filter_map(|r| r.get("payload").and_then(Value::as_str))
            .map(str::len)
            .sum();
        if encoded > MAX_DECODED_BYTES * 4 / 3 {
            return Err(invalid(
                "Combined decoded Synapse payloads exceed the 1 MiB memory budget",
            ));
        }
        // Gs.processData catches each profile/macro decode independently. Keep
        // that GUI behavior separate from strict interchange/CLI validation.
        for field in ["profiles", "macros"] {
            if let Some(records) = value.get_mut(field).and_then(Value::as_array_mut) {
                let mut accepted = Vec::new();
                for (i, record) in records.drain(..).enumerate() {
                    match decode_record(&record) {
                        Ok(_) => accepted.push(record),
                        Err(error) => skipped.push(format!(
                            "Excluded invalid source {field} record{}: {error}",
                            i + 1
                        )),
                    }
                }
                *records = accepted;
            }
        }
        if value["profiles"].as_array().unwrap().is_empty() {
            ImportReport {
                store: Store::default(),
                warnings: Vec::new(),
            }
        } else {
            import_synapse4(value.clone())?
        }
    };
    let mut rows = rows(&report.store, None)?;
    if kind == Kind::Synapse4 {
        let order: Vec<_> = value
            .get("profiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| r.get("name").and_then(Value::as_str))
            .collect();
        rows.sort_by_key(|row| {
            order
                .iter()
                .position(|name| *name == row.name)
                .unwrap_or(usize::MAX)
        });
        for row in &mut rows {
            row.macros.retain(|m| m.available);
        }
    }
    let has_source_warning = !skipped.is_empty();
    report.warnings.extend(skipped);
    report
        .warnings
        .extend(preview_warnings(&report.store, None));
    Ok(Preview {
        kind,
        rows,
        warnings: report.warnings,
        has_source_warning,
        store: report.store,
        backup: bytes.to_vec(),
        export_target: None,
    })
}

pub fn preview_export(backup: &[u8], target: &str, active: &str) -> Result<Preview> {
    let report = import_document(backup)?;
    let mut store = report.store;
    let key = match stored_device_key(&store, target) {
        Some(k) => k,
        None => {
            let pid = u16::from_str_radix(target.split(':').next().unwrap_or(target), 16)
                .map_err(|_| invalid("Bad device identity"))?;
            let mut d = DeviceProfiles::default();
            d.profiles.clear();
            d.profiles
                .insert(active.into(), new_native_profile(pid, active)?);
            d.active = active.into();
            store.devices.insert(target.into(), d);
            target.into()
        }
    };
    let mut rows = rows(&store, Some(&key))?;
    if rows.iter().any(|r| r.name == "General") && rows.iter().any(|r| r.name == "Factory Default")
    {
        rows.retain(|r| r.name != "Factory Default");
    }
    let warnings = preview_warnings(&store, Some(&key));
    Ok(Preview {
        kind: Kind::Synapse4,
        rows,
        warnings,
        has_source_warning: false,
        store,
        backup: backup.to_vec(),
        export_target: Some(key),
    })
}
impl Preview {
    pub fn initial_choices(&self, active: Option<&str>) -> Vec<Choice> {
        self.rows
            .iter()
            .map(|r| Choice {
                selected: self.kind == Kind::NativeBackup
                    || active.is_none_or(|name| r.name == name),
                macros: r
                    .macros
                    .iter()
                    .filter(|m| m.available)
                    .map(|m| m.guid.clone())
                    .collect(),
            })
            .collect()
    }
    /// Keep the unmodified all-device snapshot when export rows use an in-memory
    /// selected-device migration/default; native format never exports that overlay.
    pub fn preserve_native_backup(mut self, original: &[u8]) -> Result<Self> {
        let checked = import_document(original)?;
        let value: Value = serde_json::from_slice(original).map_err(|e| invalid(e.to_string()))?;
        if value.get("devices").is_none() {
            return Err(invalid("A full native backup snapshot is required"));
        }
        validate_store_budget(&checked.store)?;
        self.backup = original.to_vec();
        Ok(self)
    }
    pub fn full_native_backup(&self) -> &[u8] {
        &self.backup
    }
    pub fn compatible(&self, target: &str) -> bool {
        self.kind == Kind::NativeBackup
            || self
                .rows
                .iter()
                .all(|r| family(&r.device_key) == family(target))
    }
    fn check(&self, choices: &[Choice]) -> Result<()> {
        if choices.len() != self.rows.len() || !choices.iter().any(|c| c.selected) {
            return Err(invalid("Select at least one profile"));
        }
        for (row, choice) in self.rows.iter().zip(choices) {
            if choice
                .macros
                .iter()
                .any(|id| !row.macros.iter().any(|m| m.available && &m.guid == id))
            {
                return Err(invalid("Macro selection no longer matches this preview"));
            }
        }
        Ok(())
    }
}
fn unique_name(existing: &BTreeSet<String>, base: &str) -> String {
    if !base.trim().is_empty() && !existing.contains(base) {
        return base.into();
    }
    for n in 1usize.. {
        let name = format!("{base} ({n})");
        if !existing.contains(&name) {
            return name;
        }
    }
    unreachable!()
}
fn record(mut original: Value, decoded: &Value, name: &str) -> Result<Value> {
    let o = original
        .as_object_mut()
        .ok_or_else(|| invalid("Source record is not an object"))?;
    o.insert("name".into(), json!(name));
    o.insert("hash".into(), json!(synapse_hash(decoded)?));
    o.insert(
        "payload".into(),
        json!(STANDARD.encode(serde_json::to_vec(decoded).map_err(|e| invalid(e.to_string()))?)),
    );
    Ok(original)
}
fn rewrite_refs(
    value: &mut Value,
    renames: &BTreeMap<String, (String, String)>,
    excluded: &BTreeSet<String>,
) {
    match value {
        Value::Object(o) => {
            if let Some(g) = o.get_mut("macroGroup").and_then(Value::as_object_mut) {
                if let Some((id, name)) = g
                    .get("guid")
                    .and_then(Value::as_str)
                    .and_then(|id| renames.get(id))
                {
                    g.insert("guid".into(), json!(id));
                    g.insert("name".into(), json!(name));
                }
            }
            for (key, v) in o.iter_mut() {
                if matches!(key.as_str(), "mappings" | "mapping") {
                    if let Some(a) = v.as_array_mut() {
                        a.retain(|item| {
                            !item
                                .get("macroGroup")
                                .and_then(|g| g.get("guid"))
                                .and_then(Value::as_str)
                                .is_some_and(|id| excluded.contains(id))
                        });
                    }
                }
                rewrite_refs(v, renames, excluded);
            }
        }
        Value::Array(a) => {
            for v in a {
                rewrite_refs(v, renames, excluded)
            }
        }
        _ => {}
    }
}
/// Construct an immutable selected source document. Native backups intentionally
/// retain their established all-device semantics, including every opaque field.
pub fn selected_import(
    preview: &Preview,
    choices: &[Choice],
    current: &Store,
    target: &str,
) -> Result<ExportReport> {
    preview.check(choices)?;
    if preview.kind == Kind::NativeBackup {
        if choices.iter().any(|c| !c.selected) {
            return Err(invalid("Native backups import all devices/profiles/metadata; partial selection is not supported"));
        }
        return Ok(ExportReport{bytes:preview.backup.clone(),warnings:vec!["Full native backup: all devices, profiles and opaque metadata are retained; import does not activate hardware profiles".into()]});
    }
    if !preview.compatible(target) {
        return Err(invalid("This source profile belongs to another device family; choose that device or use a full native backup"));
    }
    let incoming = preview
        .store
        .devices
        .values()
        .next()
        .ok_or_else(|| invalid("Preview has no device"))?;
    let mut envelope = incoming
        .extra
        .get(SOURCE)
        .cloned()
        .ok_or_else(|| invalid("Source envelope is missing"))?;
    let destination = stored_device_key(current, target).and_then(|key| current.devices.get(&key));
    let mut names: BTreeSet<String> = destination
        .into_iter()
        .flat_map(|d| d.profiles.keys().cloned())
        .collect();
    let existing_macros = destination
        .map(macro_records)
        .transpose()?
        .unwrap_or_default();
    let mut macro_names: BTreeSet<String> = existing_macros
        .values()
        .filter_map(|r| r["name"].as_str().map(str::to_string))
        .collect();
    let incoming_macros = macro_records(incoming)?;
    let selected_ids: BTreeSet<String> = choices
        .iter()
        .filter(|c| c.selected)
        .flat_map(|c| c.macros.iter().cloned())
        .collect();
    let mut rewritten = BTreeMap::new();
    let mut macros = Vec::new();
    for id in &selected_ids {
        let original = incoming_macros
            .get(id)
            .ok_or_else(|| invalid("Selected macro payload is missing"))?;
        let mut decoded = decode_record(original)?;
        let name = unique_name(&macro_names, original["name"].as_str().unwrap_or("Macro"));
        macro_names.insert(name.clone());
        let new_id = if existing_macros.contains_key(id) {
            settings::new_guid()?
        } else {
            id.clone()
        };
        decoded["guid"] = json!(new_id);
        if decoded.get("name").is_some() {
            decoded["name"] = json!(name)
        }
        rewritten.insert(id.clone(), (new_id, name.clone()));
        macros.push(record(original.clone(), &decoded, &name)?);
    }
    let mut profiles = Vec::new();
    for (row, choice) in preview.rows.iter().zip(choices).filter(|(_, c)| c.selected) {
        let p = &preview.store.devices[&row.device_key].profiles[&row.name];
        let source = p
            .extra
            .get(SOURCE)
            .ok_or_else(|| invalid("Missing retained profile"))?;
        let mut decoded = source["decoded"].clone();
        let name = unique_name(&names, &row.name);
        names.insert(name.clone());
        decoded["guid"] = json!(settings::new_guid()?);
        decoded["name"] = json!(name);
        let mut referenced = BTreeMap::new();
        refs(&decoded, &mut referenced);
        let excluded = referenced
            .into_keys()
            .filter(|id| !choice.macros.contains(id))
            .collect();
        rewrite_refs(&mut decoded, &rewritten, &excluded);
        profiles.push(record(source["entry"].clone(), &decoded, &name)?);
    }
    envelope["profiles"] = Value::Array(profiles);
    envelope["macros"] = Value::Array(macros);
    let bytes = serde_json::to_vec_pretty(&envelope).map_err(|e| invalid(e.to_string()))?;
    let checked = import_document(&bytes)?;
    let mut warnings = preview.warnings.clone();
    for warning in checked.warnings {
        if !warnings.contains(&warning) {
            warnings.push(warning)
        }
    }
    Ok(ExportReport { bytes, warnings })
}
fn composed_source(device: &mut DeviceProfiles) -> Result<()> {
    let all: Vec<Value> = envelopes(device).into_iter().cloned().collect();
    if all.len() < 2 {
        return Ok(());
    }
    let mut combined = all[0].clone();
    let mut records: BTreeMap<String, Value> = BTreeMap::new();
    for envelope in &all {
        for (k, v) in envelope
            .as_object()
            .ok_or_else(|| invalid("Source envelope is not an object"))?
        {
            if matches!(
                k.as_str(),
                "profiles" | "macros" | "deviceName" | "productId"
            ) {
                continue;
            }
            match combined.get(k) {
                Some(old) if old != v => {
                    return Err(invalid(
                        "Opaque source-envelope fields conflict; export a full native backup",
                    ))
                }
                None => {
                    combined[k] = v.clone();
                }
                _ => {}
            }
        }
        if let Some(macros) = envelope["macros"].as_array() {
            for entry in macros {
                let d = decode_record(entry)?;
                let id = d["guid"]
                    .as_str()
                    .ok_or_else(|| invalid("Macro has no GUID"))?;
                if records.get(id).is_some_and(|old| old != entry) {
                    return Err(invalid(
                        "Macro identities conflict; export a full native backup",
                    ));
                }
                records.insert(id.into(), entry.clone());
            }
        }
    }
    combined["macros"] = Value::Array(records.into_values().collect());
    device
        .extra
        .retain(|k, _| !k.starts_with(&format!("_synapseLiteImported:{SOURCE}")));
    device.extra.insert(SOURCE.into(), combined);
    Ok(())
}
/// Export only selected profiles/macros, using the same verified codec. Source
/// document composition occurs only in this temporary export candidate; the full
/// native backup keeps all original documents and extension fields unchanged.
pub fn selected_export(preview: &Preview, choices: &[Choice]) -> Result<ExportReport> {
    preview.check(choices)?;
    let target = preview
        .export_target
        .as_deref()
        .ok_or_else(|| invalid("This is not an export preview"))?;
    let mut store: Store = serde_json::from_slice(
        &serde_json::to_vec(&preview.store).map_err(|e| invalid(e.to_string()))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let device = store
        .devices
        .get_mut(target)
        .ok_or_else(|| invalid("Export device is missing"))?;
    let selected: BTreeSet<_> = preview
        .rows
        .iter()
        .zip(choices)
        .filter(|(_, c)| c.selected)
        .map(|(r, _)| r.name.as_str())
        .collect();
    device
        .profiles
        .retain(|name, _| selected.contains(name.as_str()));
    if !device.profiles.contains_key(&device.active) {
        device.active = device.profiles.keys().next().cloned().unwrap_or_default()
    }
    composed_source(device)?;
    let mut report = export_synapse4(&store, target)?;
    let mut envelope: Value =
        serde_json::from_slice(&report.bytes).map_err(|e| invalid(e.to_string()))?;
    if let Some(profiles) = envelope.get_mut("profiles").and_then(Value::as_array_mut) {
        profiles.sort_by_key(|record| {
            preview
                .rows
                .iter()
                .position(|row| record["name"].as_str() == Some(&row.name))
                .unwrap_or(usize::MAX)
        });
    }
    let ids: BTreeSet<_> = choices
        .iter()
        .filter(|c| c.selected)
        .flat_map(|c| c.macros.iter().cloned())
        .collect();
    if let Some(macros) = envelope.get_mut("macros").and_then(Value::as_array_mut) {
        let mut selected = Vec::new();
        for entry in macros.drain(..) {
            let d = decode_record(&entry)?;
            if d["guid"].as_str().is_some_and(|id| ids.contains(id)) {
                selected.push(entry)
            }
        }
        *macros = selected;
    }
    report.bytes = serde_json::to_vec_pretty(&envelope).map_err(|e| invalid(e.to_string()))?;
    if report.bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid("Selected export exceeds 2 MiB"));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source_document(names: &[&str], with_macro: bool) -> Vec<u8> {
        let mut profiles = Vec::new();
        for name in names {
            let mut d = settings::default_profile(605, name).unwrap();
            d["opaque"] = json!({"retain":[1,2]});
            if with_macro {
                d["mappings"] = json!([{"inputID":"DKM_F1","outputType":"future-macro","macroGroup":{"guid":"macro-fixture","name":"Macro"},"future":7}]);
            }
            profiles.push(record(json!({"entry-extra":9}), &d, name).unwrap());
        }
        let macros = if with_macro {
            vec![record(
                json!({"macro-extra":8}),
                &json!({"guid":"macro-fixture","macroEvents":[],"macroPlaybackOption":1}),
                "Macro",
            )
            .unwrap()]
        } else {
            Vec::new()
        };
        serde_json::to_vec(&json!({"productId":605,"category":"KEYBOARD","deviceName":"Fixture","profiles":profiles,"macros":macros,"envelope-extra":{"retain":true}})).unwrap()
    }
    fn clone_store(store: &Store) -> Store {
        serde_json::from_value(serde_json::to_value(store).unwrap()).unwrap()
    }
    #[test]
    fn gui_partial_decode_keeps_valid_records_in_order_and_cli_stays_strict() {
        let mut doc: Value =
            serde_json::from_slice(&source_document(&["Zulu", "Bad", "Alpha"], true)).unwrap();
        doc["profiles"][1]["hash"] = json!("bad-hash");
        let mut bad_macro = doc["macros"][0].clone();
        bad_macro["hash"] = json!("bad-hash");
        doc["macros"].as_array_mut().unwrap().push(bad_macro);
        let bytes = serde_json::to_vec(&doc).unwrap();
        assert!(import_document(&bytes).is_err());
        let preview = preview_import(&bytes).unwrap();
        assert!(preview.has_source_warning);
        assert_eq!(
            preview
                .rows
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["Zulu", "Alpha"]
        );
        assert!(preview
            .rows
            .iter()
            .all(|r| r.macros.len() == 1 && r.macros[0].available));
        assert_eq!(
            preview
                .warnings
                .iter()
                .filter(|w| w.starts_with("Excluded invalid source"))
                .count(),
            2
        );
        assert_eq!(preview.full_native_backup(), bytes);
        let selected = selected_import(
            &preview,
            &preview.initial_choices(None),
            &Store::default(),
            "025D:sim",
        )
        .unwrap();
        let selected: Value = serde_json::from_slice(&selected.bytes).unwrap();
        assert_eq!(selected["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(selected["macros"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn gui_invalid_macro_is_excluded_and_missing_binding_removed_on_import() {
        let mut doc: Value = serde_json::from_slice(&source_document(&["Work"], true)).unwrap();
        doc["macros"][0]["hash"] = json!("bad-hash");
        let bytes = serde_json::to_vec(&doc).unwrap();
        assert!(import_document(&bytes).is_err());
        let preview = preview_import(&bytes).unwrap();
        assert!(preview.has_source_warning);
        assert!(preview.rows[0].macros.is_empty());
        let selected = selected_import(
            &preview,
            &preview.initial_choices(None),
            &Store::default(),
            "025D:sim",
        )
        .unwrap();
        let selected: Value = serde_json::from_slice(&selected.bytes).unwrap();
        assert_eq!(selected["macros"], json!([]));
        assert_eq!(
            decode_record(&selected["profiles"][0]).unwrap()["mappings"],
            json!([])
        );
        assert!(selected["profiles"][0]["payload"].is_string());
    }
    #[test]
    fn gui_all_invalid_source_is_empty_while_native_backup_is_always_strict() {
        let mut doc: Value = serde_json::from_slice(&source_document(&["Bad"], false)).unwrap();
        doc["profiles"][0]["hash"] = json!("bad-hash");
        let preview = preview_import(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(preview.rows.is_empty());
        assert!(preview.has_source_warning);
        assert!(selected_import(&preview, &[], &Store::default(), "025D:sim").is_err());
        let bad_native =
            br#"{"devices":{"025D":{"active":"Bad","profiles":[]}},"opaque":{"retain":true}}"#;
        assert!(preview_import(bad_native).is_err());
        assert!(preview_import(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
    }
    #[test]
    fn import_preview_preserves_nonlexical_source_array_order() {
        let bytes = source_document(&["Zulu", "Alpha"], false);
        let preview = preview_import(&bytes).unwrap();
        assert_eq!(
            preview
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Zulu", "Alpha"]
        );
        assert_eq!(preview.full_native_backup(), bytes);
    }
    #[test]
    fn native_export_retains_source_order_across_rename_delete_and_new_profile() {
        let mut store = import_document(&source_document(&["Zulu", "Alpha", "Removed"], false))
            .unwrap()
            .store;
        let device = store.devices.get_mut("025D").unwrap();
        device.rename("Zulu", "Omega").unwrap();
        device.profiles.remove("Removed");
        device.profiles.insert(
            "Aardvark".into(),
            new_native_profile(605, "Aardvark").unwrap(),
        );
        let backup = serde_json::to_vec(&store).unwrap();
        let preview = preview_export(&backup, "025D:unit", "Omega").unwrap();
        assert_eq!(
            preview
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["Omega", "Alpha", "Aardvark"]
        );
        let output = selected_export(&preview, &preview.initial_choices(None)).unwrap();
        let document: Value = serde_json::from_slice(&output.bytes).unwrap();
        assert_eq!(
            document["profiles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|record| record["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Omega", "Alpha", "Aardvark"]
        );
        assert_eq!(
            decode_record(&document["profiles"][0]).unwrap()["opaque"],
            json!({"retain":[1,2]})
        );
        assert_eq!(preview.full_native_backup(), backup);
    }
    #[test]
    fn source_preview_is_read_only_and_import_copies_identity_without_activating() {
        let bytes = source_document(&["Work", "Other"], false);
        let preview = preview_import(&bytes).unwrap();
        assert_eq!(preview.full_native_backup(), bytes);
        let current = import_document(&source_document(&["Work"], false))
            .unwrap()
            .store;
        let before = serde_json::to_value(&current).unwrap();
        let active = current.devices["025D"].active.clone();
        let mut choices = preview.initial_choices(None);
        let other = preview.rows.iter().position(|r| r.name == "Other").unwrap();
        choices[other].selected = false;
        let output = selected_import(&preview, &choices, &current, "025D:unit").unwrap();
        let incoming = import_document(&output.bytes).unwrap().store;
        let profile = &incoming.devices["025D"].profiles["Work (1)"];
        assert_ne!(
            profile.extra[SOURCE]["decoded"]["guid"],
            current.devices["025D"].profiles["Work"].extra[SOURCE]["decoded"]["guid"]
        );
        assert_eq!(
            profile.extra[SOURCE]["decoded"]["opaque"],
            json!({"retain":[1,2]})
        );
        let mut merged = clone_store(&current);
        merged.import_for(incoming, &["025D:unit".into()]);
        assert_eq!(merged.devices["025D"].active, active);
        assert_eq!(serde_json::to_value(&current).unwrap(), before);
        assert!(!output.bytes.is_empty());
        assert_eq!(preview.rows.len(), 2);
    }
    #[test]
    fn macro_choices_filter_payload_and_binding_and_collision_rewrites_both() {
        let bytes = source_document(&["Work"], true);
        let preview = preview_import(&bytes).unwrap();
        let mut choices = preview.initial_choices(None);
        assert_eq!(preview.rows[0].macros[0].guid, "macro-fixture");
        choices[0].macros.clear();
        let output = selected_import(&preview, &choices, &Store::default(), "025D:unit").unwrap();
        let d: Value = serde_json::from_slice(&output.bytes).unwrap();
        assert_eq!(d["macros"].as_array().unwrap().len(), 0);
        assert_eq!(
            decode_record(&d["profiles"][0]).unwrap()["mappings"],
            json!([])
        );
        let existing = import_document(&bytes).unwrap().store;
        let output = selected_import(
            &preview,
            &preview.initial_choices(None),
            &existing,
            "025D:unit",
        )
        .unwrap();
        let d: Value = serde_json::from_slice(&output.bytes).unwrap();
        let m = decode_record(&d["macros"][0]).unwrap();
        let p = decode_record(&d["profiles"][0]).unwrap();
        assert_ne!(m["guid"], "macro-fixture");
        assert_eq!(d["macros"][0]["name"], "Macro (1)");
        assert_eq!(p["mappings"][0]["macroGroup"]["guid"], m["guid"]);
        assert_eq!(d["macros"][0]["macro-extra"], 8);
    }
    #[test]
    fn selected_export_keeps_quick_effects_and_does_not_change_native_backup() {
        let report = import_document(&source_document(&["Work", "Other"], true)).unwrap();
        let backup = serde_json::to_vec_pretty(&report.store).unwrap();
        let p = preview_export(&backup, "025D:unit", "Work").unwrap();
        let mut choices = p.initial_choices(Some("Work"));
        for c in &mut choices {
            c.macros.clear()
        }
        let output = selected_export(&p, &choices).unwrap();
        let d: Value = serde_json::from_slice(&output.bytes).unwrap();
        assert_eq!(d["profiles"].as_array().unwrap().len(), 1);
        assert_eq!(d["profiles"][0]["name"], "Work");
        assert!(decode_record(&d["profiles"][0])
            .unwrap()
            .get("quickEffects")
            .is_some());
        assert_eq!(d["macros"], json!([]));
        assert_eq!(p.full_native_backup(), backup);
    }
    #[test]
    fn full_native_backup_retains_foreign_devices_and_opaque_fields() {
        let original = json!({"global-extra":{"keep":true},"devices":{"025D:a":{"active":"One","profiles":{"One":{"opaque":[1,2]}}},"FFFF:foreign":{"active":"Two","profiles":{"Two":{"other":7}},"device-extra":9}}});
        let bytes = serde_json::to_vec(&original).unwrap();
        let p = preview_import(&bytes).unwrap();
        assert_eq!(p.kind, Kind::NativeBackup);
        assert_eq!(p.rows.len(), 2);
        let mut choices = p.initial_choices(None);
        let out = selected_import(&p, &choices, &Store::default(), "025D:a").unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&out.bytes).unwrap(),
            original
        );
        choices[0].selected = false;
        assert!(selected_import(&p, &choices, &Store::default(), "025D:a").is_err());
    }
    #[test]
    fn stored_lookup_never_uses_another_serial_and_allows_same_serial_aliases() {
        let mut s = Store::default();
        s.devices
            .insert("025D:other".into(), DeviceProfiles::default());
        assert!(stored_device_key(&s, "025D:selected").is_none());
        s.devices
            .insert("00C4:unit".into(), DeviceProfiles::default());
        assert_eq!(
            stored_device_key(&s, "00C5:unit").as_deref(),
            Some("00C4:unit")
        );
        assert!(stored_device_key(&s, "00C5:another").is_none());
        s.devices.insert("025D".into(), DeviceProfiles::default());
        assert_eq!(
            stored_device_key(&s, "025D:selected").as_deref(),
            Some("025D")
        );
    }
    #[test]
    fn owner_family_empty_selection_and_source_names_are_explicit() {
        let p = preview_import(&source_document(&["Work"], false)).unwrap();
        assert!(!p.compatible("00C4:x"));
        let mut c = p.initial_choices(None);
        assert!(selected_import(&p, &c, &Store::default(), "00C4:x").is_err());
        c[0].selected = false;
        assert!(selected_import(&p, &c, &Store::default(), "025D:x").is_err());
        let names = BTreeSet::from(["Work".into(), "Work (1)".into(), "Work (3)".into()]);
        assert_eq!(unique_name(&names, "Work"), "Work (2)");
        assert_eq!(unique_name(&names, "work"), "work");
    }
}
