//! Paired source Type6 rows. Definition edits only; never expand or play input.
use super::*;

pub const MAX_REPEAT: u32 = 99_999;
const MAX_DEPTH: usize = 16;
const MAX_WORK: u64 = 100_000;
const MAX_ID: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub start: usize,
    pub end: usize,
    pub repeat: u32,
}

fn loop_row(event: &Value) -> bool {
    event["Type"].as_f64() == Some(6.0)
}

fn count(value: &Value) -> Result<u32> {
    let n = value
        .as_u64()
        .or_else(|| {
            let s = value.as_str()?;
            (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse().ok())
                .flatten()
        })
        .filter(|n| (1..=u64::from(MAX_REPEAT)).contains(n));
    n.map(|n| n as u32)
        .ok_or_else(|| invalid("Loop repeat must be an integer from 1 to 99999"))
}

/// Validates the entire boundary topology, including shared IDs with non-loop
/// rows. Unknown non-loop event contents are deliberately not interpreted.
pub fn pairs(events: &[Value]) -> Result<Vec<Pair>> {
    if events.len() > MAX_EVENTS {
        return Err(invalid("Macro exceeds512 event rows"));
    }
    let mut stack: Vec<(usize, u64, u32, u64)> = vec![];
    let mut seen = BTreeSet::new();
    let mut pairs = vec![];
    let mut work = 0u64;
    for (index, event) in events.iter().enumerate() {
        if !loop_row(event) {
            if event["Type"] != "actionBar" {
                if let Some(frame) = stack.last_mut() {
                    frame.3 += 1;
                } else {
                    work += 1;
                }
            } else if !stack.is_empty() {
                return Err(invalid("A loop cannot contain the action bar"));
            }
            continue;
        }
        if event["Type"].as_u64() != Some(6) {
            return Err(invalid("Loop type has an ambiguous numeric representation"));
        }
        let id = event["Id"]
            .as_u64()
            .filter(|id| *id <= MAX_ID)
            .ok_or_else(|| invalid("Loop needs an unambiguous safe integer ID"))?;
        let repeat = count(&event["Number"])?;
        let state = event["LoopEvent"]["State"]
            .as_u64()
            .filter(|s| *s <= 1)
            .ok_or_else(|| invalid("Loop boundary state must be 0 or 1"))?;
        if event
            .get("flag")
            .is_some_and(|flag| !flag.is_null() && flag.as_u64() != Some(state))
        {
            return Err(invalid("Loop flag conflicts with its boundary state"));
        }
        if events
            .iter()
            .any(|e| !loop_row(e) && e["Id"].as_f64() == Some(id as f64))
        {
            return Err(invalid("Loop ID is shared with another event type"));
        }
        if state == 0 {
            if !seen.insert(id) {
                return Err(invalid("Loop ID is reused or ambiguous"));
            }
            if stack.len() >= MAX_DEPTH {
                return Err(invalid("Loop nesting exceeds16 levels"));
            }
            stack.push((index, id, repeat, 0));
        } else {
            let (start, open_id, open_count, body) = stack
                .pop()
                .ok_or_else(|| invalid("Loop end has no matching start"))?;
            if open_id != id || open_count != repeat {
                return Err(invalid(
                    "Loop boundaries cross or their repeat counts differ",
                ));
            }
            // Include boundary overhead: even nested empty loops have a budget.
            let cost = body
                .checked_add(1)
                .and_then(|n| n.checked_mul(u64::from(repeat)))
                .and_then(|n| n.checked_add(1))
                .filter(|n| *n <= MAX_WORK)
                .ok_or_else(|| invalid("Loop work exceeds100000 bounded steps"))?;
            if let Some(parent) = stack.last_mut() {
                parent.3 += cost;
            } else {
                work += cost;
            }
            pairs.push(Pair {
                start,
                end: index,
                repeat,
            });
        }
        if work > MAX_WORK || stack.iter().any(|f| f.3 > MAX_WORK) {
            return Err(invalid("Loop work exceeds100000 bounded steps"));
        }
    }
    if !stack.is_empty() {
        return Err(invalid("Loop start has no matching end"));
    }
    if work > MAX_WORK {
        return Err(invalid("Loop work exceeds100000 bounded steps"));
    }
    Ok(pairs)
}

pub fn selection(events: &[Value], index: usize) -> Result<Pair> {
    pairs(events)?
        .into_iter()
        .find(|p| p.start == index || p.end == index)
        .ok_or_else(|| invalid("Select a verified loop boundary"))
}

fn parsed(rows: &[Box<RawValue>]) -> Result<Vec<Value>> {
    rows.iter()
        .map(|row| {
            let value: Value =
                serde_json::from_str(row.get()).map_err(|e| invalid(e.to_string()))?;
            if loop_row(&value) {
                let fields: Fields =
                    serde_json::from_str(row.get()).map_err(|e| invalid(e.to_string()))?;
                let loop_event = fields
                    .0
                    .iter()
                    .find(|(k, _)| k == "LoopEvent")
                    .ok_or_else(|| invalid("Loop event descriptor is missing"))?;
                let _: Fields =
                    serde_json::from_str(loop_event.1.get()).map_err(|e| invalid(e.to_string()))?;
            }
            Ok(value)
        })
        .collect()
}

pub(super) fn validate(rows: &[Box<RawValue>]) -> Result<()> {
    pairs(&parsed(rows)?)?;
    Ok(())
}

pub(super) fn wrap(
    rows: &mut Vec<Box<RawValue>>,
    start: usize,
    end: usize,
    repeat: u32,
) -> Result<()> {
    count(&json!(repeat))?;
    if start > end || end > rows.len() || rows.len() + 2 > MAX_EVENTS {
        return Err(invalid("Loop range exceeds macro bounds"));
    }
    let events = parsed(rows)?;
    let existing = pairs(&events)?;
    if events
        .get(start..end)
        .unwrap()
        .iter()
        .any(|e| e["Type"] == "actionBar")
        || (start == 0 && events.first().is_some_and(|e| e["Type"] == "actionBar"))
    {
        return Err(invalid("Loop boundaries must follow the action bar"));
    }
    for p in existing {
        if (start <= p.start && p.start < end) != (start <= p.end && p.end < end) {
            return Err(invalid("Select both boundaries of each enclosed loop"));
        }
    }
    let mut highest = 0;
    for event in &events {
        if let Some(id) = event.get("Id").filter(|v| !v.is_null()) {
            highest = highest.max(
                id.as_u64()
                    .filter(|id| *id < MAX_ID)
                    .ok_or_else(|| invalid("Cannot allocate a safe source event ID"))?,
            );
        }
    }
    let id = highest + 1;
    let boundary = |state| {
        raw(
            &json!({"Type":6,"Id":id,"Number":repeat,"LoopEvent":{"State":state},"flag":state,"selected":false,"isPairing":false}),
        )
    };
    rows.insert(end, boundary(1)?);
    rows.insert(start, boundary(0)?);
    validate(rows)
}

pub(super) fn set_repeat(rows: &mut [Box<RawValue>], index: usize, repeat: u32) -> Result<()> {
    count(&json!(repeat))?;
    let pair = selection(&parsed(rows)?, index)?;
    for index in [pair.start, pair.end] {
        let mut fields: Fields =
            serde_json::from_str(rows[index].get()).map_err(|e| invalid(e.to_string()))?;
        let field = fields
            .0
            .iter_mut()
            .find(|(k, _)| k == "Number")
            .ok_or_else(|| invalid("Loop repeat is missing"))?;
        // Retain the source's string/number representation and all other tokens.
        let old: Value = serde_json::from_str(field.1.get()).map_err(|e| invalid(e.to_string()))?;
        field.1 = if old.is_string() {
            raw(&repeat.to_string())?
        } else {
            raw(&repeat)?
        };
        rows[index] = raw(&fields)?;
    }
    validate(rows)
}

pub(super) fn delete(rows: &mut Vec<Box<RawValue>>, index: usize) -> Result<()> {
    let pair = selection(&parsed(rows)?, index)?;
    rows.remove(pair.end);
    rows.remove(pair.start);
    validate(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(text: &str) -> Value {
        let body: Value = serde_json::from_str(text).unwrap();
        json!({"name":"Loops","payload":STANDARD.encode(text),"hash":synapse_hash(&body).unwrap(),"foreignRecord":true})
    }
    fn base() -> Value {
        encoded(
            r#"{"guid":"loops","macroEvents":[{"Type":"actionBar","selected":false},{"Type":0,"Number":"0.1","opaque":9.9900e+100},{"future":9007199254740993123456789},{"Type":0,"Number":"0.2"}],"macroPlaybackOption":{"future":true},"opaque":1.2300e-7}"#,
        )
    }
    fn wrap(record: &Value, index: usize, end: usize, repeat: u32) -> Result<Value> {
        edit_definition(
            record,
            Change::WrapLoop {
                guid: "loops".into(),
                index,
                end,
                repeat,
            },
        )
    }
    fn change(record: &Value, index: usize, repeat: u32) -> Result<Value> {
        edit_definition(
            record,
            Change::SetLoopRepeat {
                guid: "loops".into(),
                index,
                repeat,
            },
        )
    }
    fn remove(record: &Value, index: usize) -> Result<Value> {
        edit_definition(
            record,
            Change::DeleteEvent {
                guid: "loops".into(),
                index,
            },
        )
    }
    fn decoded(record: &Value) -> String {
        String::from_utf8(
            STANDARD
                .decode(record["payload"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn authored_pairs_match_independently_replayed_source_helpers() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/macro-loop-source.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 9);
        for case in cases {
            let highest = case["highest"].as_u64().unwrap();
            let repeat = case["repeat"].as_u64().unwrap() as u32;
            let source = record(
                &json!({"guid":"loops","macroEvents":[{"Type":"actionBar","Id":highest}]}),
                "Source",
            )
            .unwrap();
            let authored = wrap(&source, 1, 1, repeat).unwrap();
            let events = describe(&authored).unwrap().events;
            assert_eq!(json!(&events[1..]), case["events"]);
            assert_eq!(
                case["output"],
                json!([{"type":"loop","flag":0,"repeat":repeat},{"type":"loop","flag":1}])
            );
        }
    }

    #[test]
    fn nested_loops_update_either_boundary_and_delete_only_the_pair() {
        let original = base();
        let outer = wrap(&original, 1, 4, 2).unwrap();
        let nested = wrap(&outer, 2, 4, 3).unwrap();
        let entry = describe(&nested).unwrap();
        let topology = pairs(&entry.events).unwrap();
        assert_eq!(
            topology,
            vec![
                Pair {
                    start: 2,
                    end: 5,
                    repeat: 3
                },
                Pair {
                    start: 1,
                    end: 7,
                    repeat: 2
                }
            ]
        );
        assert_ne!(entry.events[1]["Id"], entry.events[2]["Id"]);
        let changed = change(&nested, 5, 4).unwrap();
        let events = describe(&changed).unwrap().events;
        assert_eq!(events[2]["Number"], 4);
        assert_eq!(events[5]["Number"], 4);
        for i in [0, 1, 3, 4, 6, 7] {
            assert_eq!(events[i], entry.events[i]);
        }
        let removed = remove(&changed, 2).unwrap();
        assert_eq!(
            describe(&removed).unwrap().events,
            describe(&outer).unwrap().events
        );
        assert_eq!(
            describe(&remove(&outer, 5).unwrap()).unwrap().events,
            describe(&original).unwrap().events
        );
        let text = decoded(&changed);
        for raw in ["9.9900e+100", "9007199254740993123456789", "1.2300e-7"] {
            assert!(text.contains(raw));
        }
        assert_eq!(changed["foreignRecord"], true);
    }

    #[test]
    fn source_string_counts_unknown_boundary_fields_and_numeric_tokens_survive() {
        let source = encoded(
            r#"{"guid":"loops","macroEvents":[{"Type":6,"Id":7,"Number":"2","LoopEvent":{"State":0,"raw":1.200e-3},"flag":null,"selected":true,"future":9.9900e+100},{"future":true},{"Type":6,"Id":7,"Number":"2","LoopEvent":{"State":1},"selected":false,"isPairing":true}]}"#,
        );
        let next = change(&source, 2, 5).unwrap();
        let entry = describe(&next).unwrap();
        assert_eq!(entry.events[0]["Number"], "5");
        assert_eq!(entry.events[2]["Number"], "5");
        assert_eq!(entry.events[0]["flag"], Value::Null);
        assert_eq!(entry.events[0]["selected"], true);
        assert_eq!(entry.events[2]["isPairing"], true);
        assert!(decoded(&next).contains("1.200e-3"));
        assert!(decoded(&next).contains("9.9900e+100"));
    }

    #[test]
    fn malformed_crossed_ambiguous_and_duplicate_boundaries_are_retained_but_not_edited() {
        let valid = describe(&wrap(&base(), 1, 4, 2).unwrap()).unwrap().events;
        let mut cases = vec![];
        for (index, field, value) in [
            (1, "Type", json!(6.0)),
            (1, "Id", json!(null)),
            (1, "Id", json!(1.0)),
            (1, "Number", json!(0)),
            (1, "Number", json!(1.5)),
            (5, "Number", json!(3)),
            (1, "flag", json!(1)),
        ] {
            let mut events = valid.clone();
            events[index][field] = value;
            cases.push(events);
        }
        let mut events = valid.clone();
        events[1]["Type"] = json!(6.0);
        events[5]["Type"] = json!(6.0);
        cases.push(events);
        let mut events = valid.clone();
        events[1]["LoopEvent"]["State"] = Value::Null;
        cases.push(events);
        let mut events = valid.clone();
        events.swap(1, 5);
        cases.push(events);
        let mut events = valid.clone();
        events.remove(5);
        cases.push(events);
        let mut events = valid.clone();
        events.push(events[5].clone());
        cases.push(events);
        let mut events = valid.clone();
        events[2]["Id"] = events[1]["Id"].clone();
        cases.push(events);
        let nested = wrap(&wrap(&base(), 1, 4, 2).unwrap(), 2, 4, 2).unwrap();
        let mut events = describe(&nested).unwrap().events;
        events.swap(5, 7);
        cases.push(events);
        for events in cases {
            let source =
                record(&json!({"guid":"loops","macroEvents":events}), "Malformed").unwrap();
            let before = source.clone();
            assert!(change(&source, 1, 2).is_err());
            assert!(remove(&source, 1).is_err());
            assert!(wrap(&source, 1, 1, 2).is_err());
            assert_eq!(source, before);
            assert!(
                describe(&source).is_ok(),
                "Malformed source must still be readable/exportable"
            );
        }
        for row in [
            r#"{"Type":6,"Id":1,"Id":2,"Number":1,"LoopEvent":{"State":0}}"#,
            r#"{"Type":6,"Id":1,"Number":1,"LoopEvent":{"State":0,"State":1}}"#,
        ] {
            let source = encoded(&format!(
                r#"{{"guid":"loops","macroEvents":[{row},{{"Type":6,"Id":1,"Number":1,"LoopEvent":{{"State":1}}}}]}}"#
            ));
            assert!(remove(&source, 0).is_err());
        }
    }

    #[test]
    fn range_work_depth_row_id_and_native_cache_guards_refuse_without_mutation() {
        let source = base();
        for (start, end, repeat) in [
            (4, 1, 1),
            (0, 2, 1),
            (1, 9, 1),
            (1, 4, 0),
            (1, 4, 100000),
            (1, 4, 99999),
        ] {
            assert!(wrap(&source, start, end, repeat).is_err());
        }
        assert!(wrap(&source, 4, 4, 99996).is_ok()); // bounded empty loop
        assert!(wrap(&source, 4, 4, 99997).is_err());
        // The total also includes rows after the loop's closing boundary.
        assert!(wrap(&source, 1, 1, 99997).is_err());
        let outer = wrap(&source, 1, 4, 2).unwrap();
        assert!(wrap(&outer, 1, 3, 2).is_err());
        assert!(wrap(&outer, 3, 6, 2).is_err());
        let mut nested = source.clone();
        for _ in 0..16 {
            let len = describe(&nested).unwrap().events.len();
            nested = wrap(&nested, 1, len, 1).unwrap();
        }
        assert!(wrap(&nested, 1, describe(&nested).unwrap().events.len(), 1).is_err());
        let mut body: Value = serde_json::from_str(&decoded(&source)).unwrap();
        body["macroEvents"] = json!((0..511).map(|_| json!({"future":true})).collect::<Vec<_>>());
        assert!(wrap(&record(&body, "Large").unwrap(), 0, 0, 1).is_err());
        for id in [json!(MAX_ID), json!(-1), json!("1")] {
            let mut body: Value = serde_json::from_str(&decoded(&source)).unwrap();
            body["macroEvents"][1]["Id"] = id;
            assert!(wrap(&record(&body, "ID").unwrap(), 1, 2, 1).is_err());
        }
        let mut body: Value = serde_json::from_str(&decoded(&outer)).unwrap();
        body["appEngine"] = json!({"events":[]});
        let cached = record(&body, "Cached").unwrap();
        assert!(wrap(&cached, 2, 3, 1).is_err());
        assert!(change(&cached, 1, 3).is_err());
        assert!(remove(&cached, 1).is_err());
        assert!(edit_definition(
            &outer,
            Change::MoveEvent {
                guid: "loops".into(),
                index: 1,
                direction: -1
            }
        )
        .is_err());
    }

    #[test]
    fn loop_store_save_and_profile_interchange_preserve_profiles_and_raw_payload() {
        let mut store =
            import_document(include_bytes!("../../../tests/fixtures/synthetic.synapse4"))
                .unwrap()
                .store;
        let entry = read(&store).unwrap().entries.remove(0);
        let id = entry.guid.clone();
        let before = store.clone();
        let rows = entry.events.len();
        // Fixture contains source input rows; no action bar is enclosed.
        store = edit(
            &store,
            Change::WrapLoop {
                guid: id.clone(),
                index: 0,
                end: rows,
                repeat: 2,
            },
        )
        .unwrap();
        let device = store.devices.keys().next().unwrap();
        assert_eq!(
            store.devices[device].profiles,
            before.devices[device].profiles
        );
        assert_eq!(store.devices[device].active, before.devices[device].active);
        let output = export_synapse4(&store, device).unwrap();
        let imported = import_document(&output.bytes).unwrap().store;
        let saved = read(&store)
            .unwrap()
            .entries
            .into_iter()
            .find(|e| e.guid == id)
            .unwrap();
        let restored = read(&imported)
            .unwrap()
            .entries
            .into_iter()
            .find(|e| e.guid == id)
            .unwrap();
        assert_eq!(saved.encoded_record, restored.encoded_record);
        assert_eq!(pairs(&restored.events).unwrap().len(), 1);
        let invalid = Change::SetLoopRepeat {
            guid: id,
            index: 0,
            repeat: 100000,
        };
        assert!(edit(&store, invalid).is_err());
        let mut oversized = store.clone();
        oversized.extra.insert(
            "opaqueLargeField".into(),
            json!("x".repeat(2 * 1024 * 1024)),
        );
        let before = serde_json::to_value(&oversized).unwrap();
        let id = saved.guid.clone();
        assert!(edit(
            &oversized,
            Change::SetLoopRepeat {
                guid: id,
                index: 0,
                repeat: 3
            }
        )
        .is_err());
        assert_eq!(serde_json::to_value(&oversized).unwrap(), before);
    }

    #[test]
    fn wrapping_nested_macro_rows_preserves_the_export_dependency_closure() {
        let mut source: Value =
            serde_json::from_slice(include_bytes!("../../../tests/fixtures/synthetic.synapse4"))
                .unwrap();
        let mut macro_body = decode_record(&source["macros"][0]).unwrap();
        macro_body["macroPlaybackOption"] = json!("");
        source["macros"][0] = record(&macro_body, "Loop parent").unwrap();
        let mut profile = decode_record(&source["profiles"][0]).unwrap();
        profile["mappings"] = json!([{"inputID":"DKM_F1","outputType":"future-macro","macroGroup":{"guid":macro_body["guid"],"name":"Loop parent"}}]);
        let name = source["profiles"][0]["name"].as_str().unwrap().to_owned();
        source["profiles"][0] = record(&profile, &name).unwrap();
        let mut store = import_document(&serde_json::to_vec(&source).unwrap())
            .unwrap()
            .store;
        let owner = read(&store).unwrap().entries.remove(0);
        let body = decode_record(&owner.encoded_record).unwrap();
        let child=record(&json!({"guid":"loop-child","macroEvents":[],"macroPlaybackOption":body["macroPlaybackOption"]}),"Child").unwrap();
        store
            .extra
            .insert(KEY.into(), json!({"version":1,"records":[child.clone()]}));
        store = edit(
            &store,
            Change::InsertNested {
                guid: owner.guid.clone(),
                index: owner.events.len(),
                target: "loop-child".into(),
            },
        )
        .unwrap();
        let parent = read(&store)
            .unwrap()
            .entries
            .into_iter()
            .find(|entry| entry.guid == owner.guid)
            .unwrap();
        let context: Vec<_> = read(&store)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.encoded_record)
            .collect();
        let draft = edit_definition_with_library(
            &parent.encoded_record,
            Change::WrapLoop {
                guid: owner.guid.clone(),
                index: 0,
                end: parent.events.len(),
                repeat: 3,
            },
            &context,
        )
        .unwrap();
        store = edit(
            &store,
            Change::Replace {
                guid: owner.guid.clone(),
                expected_revision: parent.revision,
                record: draft,
            },
        )
        .unwrap();
        let device = store.devices.keys().next().unwrap();
        let exported = export_synapse4(&store, device).unwrap();
        let preview = gui::preview_export(
            &serde_json::to_vec(&store).unwrap(),
            device,
            &store.devices[device].active,
        )
        .unwrap();
        let selected = gui::selected_export(&preview, &preview.initial_choices(None)).unwrap();
        for bytes in [&exported.bytes, &selected.bytes] {
            let restored = import_document(bytes).unwrap().store;
            let library = read(&restored).unwrap();
            assert!(library.missing.is_empty());
            assert_eq!(library.dependency_issues, 0);
            let restored_child = library
                .entries
                .iter()
                .find(|entry| entry.guid == "loop-child")
                .unwrap();
            assert_eq!(restored_child.encoded_record, child);
            let restored_owner = library
                .entries
                .iter()
                .find(|entry| entry.guid == owner.guid)
                .unwrap();
            assert_eq!(pairs(&restored_owner.events).unwrap().len(), 1);
            assert!(restored_owner
                .events
                .iter()
                .any(|event| event["Type"] == 7 && event["guid"] == "loop-child"));
        }
    }
}
