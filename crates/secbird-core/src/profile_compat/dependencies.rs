//! Recover missing source macro definitions without reimporting/renaming profiles.
use super::*;
pub struct MacroRecoveryReport {
    pub store: Store,
    pub added: Vec<String>,
    pub unchanged: Vec<String>,
    pub missing: Vec<String>,
}
fn records(device: &DeviceProfiles) -> Result<BTreeMap<String, Value>> {
    for (key, value) in &device.extra {
        if (key == SOURCE || key.starts_with(&format!("_synapseLiteImported:{SOURCE}")))
            && (!value.is_object()
                || !value.get("profiles").is_some_and(Value::is_array)
                || value.get("macros").is_some_and(|value| !value.is_array()))
            {
                return Err(invalid(
                    "Malformed retained source envelope; no macro definitions were changed",
                ));
            }
    }
    gui::macro_records(device)
}
/// Attach only definitions directly referenced by retained source profiles.
/// The supplied document uses the existing verified interchange codec. Profile
/// import, activation, GUID rewriting, input planning and I/O are not performed.
pub fn recover_macro_dependencies(
    current: &Store,
    target: &str,
    document: &[u8],
) -> Result<MacroRecoveryReport> {
    validate_store_budget(current)?;
    let device = current
        .devices
        .get(target)
        .ok_or_else(|| invalid("Recovery needs an exact existing device key"))?;
    let existing = records(device)?;
    let mut wanted = BTreeMap::new();
    for profile in device.profiles.values() {
        if let Some(decoded) = profile
            .extra
            .get(SOURCE)
            .and_then(|source| source.get("decoded"))
        {
            gui::refs(decoded, &mut wanted);
        }
    }
    if wanted.len() > 1024
        || wanted
            .keys()
            .any(|guid| guid.is_empty() || guid.len() > 128)
    {
        return Err(invalid("Invalid or excessive source macro references"));
    }
    let incoming = import_document(document)?;
    let mut supplied: BTreeMap<String, Value> = BTreeMap::new();
    for device in incoming.store.devices.values() {
        for (guid, record) in records(device)? {
            if guid.is_empty() || guid.len() > 128 {
                return Err(invalid("Invalid supplied macro identity"));
            }
            if supplied.get(&guid).is_some_and(|old| old != &record) {
                return Err(invalid("Supplied macro definitions conflict for one GUID"));
            }
            supplied.insert(guid, record);
        }
    }
    let mut added = vec![];
    let mut unchanged = vec![];
    let mut missing = vec![];
    for guid in wanted.keys() {
        match (existing.get(guid), supplied.get(guid)) {
            (Some(old), Some(new)) if old != new => {
                return Err(invalid(
                    "Recovery refuses to overwrite a conflicting macro identity",
                ))
            }
            (Some(_), _) => unchanged.push(guid.clone()),
            (None, Some(_)) => added.push(guid.clone()),
            (None, None) => missing.push(guid.clone()),
        }
    }
    let mut store = current.clone();
    if !added.is_empty() {
        let envelope = store
            .devices
            .get_mut(target)
            .unwrap()
            .extra
            .get_mut(SOURCE)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                invalid("Target has no canonical source envelope; import its source profile first")
            })?;
        let mut macros = envelope
            .get("macros")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for guid in &added {
            macros.push(supplied[guid].clone());
        }
        envelope.insert("macros".into(), Value::Array(macros));
    }
    validate_store_budget(&store)?;
    Ok(MacroRecoveryReport {
        store,
        added,
        unchanged,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(value: Value, name: &str) -> Value {
        json!({"name":name,"payload":STANDARD.encode(serde_json::to_vec(&value).unwrap()),"hash":synapse_hash(&value).unwrap(),"opaque":"preserved"})
    }
    fn fixture() -> (Store, Vec<u8>) {
        let profile = json!({"guid":"profile-id","name":"Kept","mappings":[{"inputID":4,"macroGroup":{"guid":"macro-id","name":"Macro","macroType":"Once"}},{"inputID":5,"macroGroup":{"guid":"missing-id","name":"Missing"}}],"future":{"keep":[1,2,3]}});
        let mut envelope = json!({"productId":196,"deviceName":"Synthetic","category":"MOUSE","profiles":[record(profile,"Kept")],"macros":[]});
        let current = import_document(&serde_json::to_vec(&envelope).unwrap())
            .unwrap()
            .store;
        envelope["macros"] = json!([
            record(
                json!({"guid":"macro-id","macroEvents":[],"macroPlaybackOption":"Once","futureMacro":true}),
                "Macro"
            ),
            record(json!({"guid":"unreferenced","macroEvents":[]}), "Unused")
        ]);
        (current, serde_json::to_vec(&envelope).unwrap())
    }
    #[test]
    fn recovery_changes_only_macro_records_and_is_idempotent() {
        let (mut current, document) = fixture();
        current
            .extra
            .insert("futureStore".into(), json!({"opaque":42}));
        let before = serde_json::to_value(&current).unwrap();
        let report = recover_macro_dependencies(&current, "00C4", &document).unwrap();
        let preview =
            gui::preview_export(&report.store.native_bytes().unwrap(), "00C4", "Kept").unwrap();
        let macros = &preview
            .rows
            .iter()
            .find(|row| row.name == "Kept")
            .unwrap()
            .macros;
        assert!(macros
            .iter()
            .any(|entry| entry.guid == "macro-id" && entry.available));
        assert!(macros
            .iter()
            .any(|entry| entry.guid == "missing-id" && !entry.available));
        assert_eq!(report.added, vec!["macro-id"]);
        assert_eq!(report.missing, vec!["missing-id"]);
        let mut after = serde_json::to_value(&report.store).unwrap();
        after["devices"]["00C4"][SOURCE]["macros"] = json!([]);
        assert_eq!(after, before);
        let exported = export_synapse4(&report.store, "00C4").unwrap();
        let exported: Value = serde_json::from_slice(&exported.bytes).unwrap();
        assert_eq!(exported["macros"].as_array().unwrap().len(), 1);
        let decoded = decode_record(&exported["macros"][0]).unwrap();
        assert_eq!(decoded["guid"], "macro-id");
        assert_eq!(decoded["futureMacro"], true);
        let again = recover_macro_dependencies(&report.store, "00C4", &document).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(again.unchanged, vec!["macro-id"]);
        assert_eq!(
            serde_json::to_value(again.store).unwrap(),
            serde_json::to_value(report.store).unwrap()
        );
    }
    #[test]
    fn invalid_hash_conflicts_and_foreign_target_fail_without_partial_changes() {
        let (current, document) = fixture();
        let before = serde_json::to_value(&current).unwrap();
        let mut bad: Value = serde_json::from_slice(&document).unwrap();
        bad["macros"][1]["hash"] = "bad".into();
        assert!(
            recover_macro_dependencies(&current, "00C4", &serde_json::to_vec(&bad).unwrap())
                .is_err()
        );
        assert!(recover_macro_dependencies(&current, "00C4:other", &document).is_err());
        let report = recover_macro_dependencies(&current, "00C4", &document).unwrap();
        let mut conflicting: Value = serde_json::from_slice(&document).unwrap();
        conflicting["macros"][0] = record(
            json!({"guid":"macro-id","macroEvents":[{"Type":"changed"}]}),
            "Macro",
        );
        assert!(recover_macro_dependencies(
            &report.store,
            "00C4",
            &serde_json::to_vec(&conflicting).unwrap()
        )
        .is_err());
        assert_eq!(serde_json::to_value(current).unwrap(), before);
    }
    #[test]
    fn malformed_retained_envelopes_and_complete_candidate_budget_refuse_losslessly() {
        let (current, document) = fixture();
        let mut malformed = current.clone();
        malformed
            .devices
            .get_mut("00C4")
            .unwrap()
            .extra
            .get_mut(SOURCE)
            .unwrap()["macros"] = json!("unknown format");
        assert!(recover_macro_dependencies(&malformed, "00C4", &document).is_err());
        let mut padded = current;
        let compact = serde_json::to_vec(&padded).unwrap().len();
        padded.extra.insert(
            "pad".into(),
            json!("x".repeat(MAX_DOCUMENT_BYTES - compact - 64)),
        );
        assert!(padded.validate_save().is_ok());
        let before = padded.native_bytes().unwrap();
        assert!(recover_macro_dependencies(&padded, "00C4", &document).is_err());
        assert_eq!(padded.native_bytes().unwrap(), before);
    }
}
