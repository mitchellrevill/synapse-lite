use super::*;
use crate::{
    keyboard_symbolic_bridge::RoutingIdentity,
    macro_scheduler::{self, Completion, Contexts, NativeContext, Owner, Poll, Scheduler},
    rzcontrol_packet::InputPacket,
};
use std::{
    num::{NonZeroU128, NonZeroU64},
    time::Duration,
};

fn saved_text(text: &str) -> Store {
    let body: Value = serde_json::from_str(text).unwrap();
    let record = json!({"name":"Test macro","payload":STANDARD.encode(text),"hash":synapse_hash(&body).unwrap()});
    let mut store = Store::default();
    store.extra.insert(
        super::super::KEY.into(),
        json!({"version":1,"records":[record]}),
    );
    store
}
fn saved(body: Value) -> Store {
    saved_text(&body.to_string())
}
fn body(rows: Value) -> Value {
    json!({"guid":"test","macroEvents":rows,"macroPlaybackOption":""})
}
fn binding() -> &'static str {
    r#"{"guid":"test","macroPlaybackOption":"Once"}"#
}
fn mouse(button: u8, state: Value) -> Value {
    json!({"Type":2,"MouseEvent":{"MouseButton":button,"State":state}})
}
fn with_cache(mut body: Value, events: Value) -> Value {
    let mut cache = json!({"events":events,"mouseMoveType":"none"});
    cache["hash"] = json!(synapse_hash(&cache).unwrap());
    body["appEngine"] = cache;
    body
}
fn rewrite_cache_hash(body: &mut Value) {
    body["appEngine"]["hash"] = json!(synapse_hash(&body["appEngine"]).unwrap());
}

#[test]
fn all_independent_source_conversion_cases_match_without_inventing_descriptors() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/macro-definition-conversion.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 501);
    assert_eq!(KEYS.len(), 150);
    for case in cases {
        let expected = case["output"].as_array().unwrap();
        let input = case["input_json"]
            .as_str()
            .map(|text| serde_json::from_str::<Value>(text).unwrap())
            .unwrap_or_else(|| case["input"].clone());
        let converted = convert(&input);
        if expected
            .iter()
            .any(|event| event["type"] == "keyboard" && event["scancode"].is_null())
        {
            assert!(converted.is_err(), "{}", case["name"]);
        } else {
            assert_eq!(
                json!(converted.unwrap()),
                case["output"],
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn json_float_spellings_preserve_js_numeric_lookup_and_cache_equality() {
    let rows:Value=serde_json::from_str(r#"[{"Type":1.0,"KeyEvent":{"Makecode":65.0,"State":0.0},"ScanCode":48.0,"flag":0.0},{"Type":1.0,"KeyEvent":{"Makecode":65.0,"State":1.0},"ScanCode":48.0,"flag":0.0}]"#).unwrap();
    let events:Value=serde_json::from_str(r#"[{"type":"keyboard","scancode":48.0,"flag":0.0},{"type":"keyboard","scancode":48.0,"flag":1.0}]"#).unwrap();
    let store = saved(with_cache(body(rows), events));
    let before = store.native_bytes().unwrap();
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(prepared.cache_status(), CacheStatus::Verified);
    assert_eq!(
        serde_json::from_str::<Value>(prepared.native_json()).unwrap(),
        json!([{"type":"keyboard","scancode":48,"flag":0},{"type":"keyboard","scancode":48,"flag":1}])
    );
    assert_eq!(store.native_bytes().unwrap(), before);
    // Source strict equality does not coerce a string flag into numeric zero.
    assert_eq!(
        keyboard(&json!({"KeyEvent":{"Makecode":65,"State":0},"ScanCode":48,"flag":"0"})).unwrap(),
        [json!({"type":"keyboard","scancode":30,"flag":0})]
    );
    assert!(keyboard(&json!({"KeyEvent":{"Makecode":"65","State":0}})).is_err());
}

#[test]
fn all_125_authored_keys_compile_to_the_independent_native_outputs() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/macro-authored-key-events.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let store = saved(body(case["events"].clone()));
        let prepared = prepare(&store, binding()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(prepared.native_json()).unwrap(),
            case["output"],
            "{}",
            case["key"]
        );
        assert_eq!(prepared.source_rows(), [0, 1]);
        assert_eq!(prepared.cache_status(), CacheStatus::Generated);
    }
}

#[test]
fn authored_saved_definition_runs_through_the_routed_state_model() {
    use super::super::{Change, Gesture, Input};
    let mut store = super::super::edit(
        &Store::default(),
        Change::Create {
            guid: "test".into(),
            name: "Pipeline".into(),
        },
    )
    .unwrap();
    for (index, input, gesture) in [
        (1, Input::Keyboard("KEY_RIGHT_CTRL".into()), Gesture::Down),
        (2, Input::Mouse(0), Gesture::Down),
        (3, Input::Mouse(8), Gesture::Tap),
        (4, Input::Mouse(0), Gesture::Up),
        (5, Input::Keyboard("KEY_RIGHT_CTRL".into()), Gesture::Up),
    ] {
        store = super::super::edit(
            &store,
            Change::InsertInput {
                guid: "test".into(),
                index,
                input,
                gesture,
            },
        )
        .unwrap();
    }
    store = super::super::edit(
        &store,
        Change::InsertDelay {
            guid: "test".into(),
            index: 6,
            milliseconds: 250,
        },
    )
    .unwrap();
    store = super::super::edit(
        &store,
        Change::InsertInput {
            guid: "test".into(),
            index: 7,
            input: Input::Mouse(5),
            gesture: Gesture::Tap,
        },
    )
    .unwrap();
    let before = store.native_bytes().unwrap();
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(prepared.source_row_count(), 8);
    assert_eq!(prepared.source_rows(), [1, 2, 3, 4, 5, 6, 7, 7, 7, 7, 7]);
    assert_eq!(prepared.plan().explicit_delay_ms(), 260);
    let context = |token, unit| NativeContext {
        routing: RoutingIdentity::new(NonZeroU128::new(token).unwrap(), unit),
        modifiers: 0,
        source_unit: Some(unit),
    };
    let mut scheduler = Scheduler::with_contexts(
        prepared.plan(),
        Contexts {
            keyboard: Some(context(1, 3)),
            mouse: Some(context(2, 7)),
        },
        Owner(NonZeroU64::new(1).unwrap()),
    )
    .unwrap();
    let mut now = Duration::ZERO;
    let mut dispatched = 0;
    for _ in 0..32 {
        match scheduler.poll(now).unwrap() {
            Poll::Dispatch(command) => {
                match command.packet() {
                    InputPacket::Keyboard(_) => assert_eq!(command.identity().unit_id(), 3),
                    InputPacket::Mouse(_) => assert_eq!(command.identity().unit_id(), 7),
                }
                scheduler.dispatch_started(command.ticket()).unwrap();
                now += Duration::from_millis(1);
                scheduler
                    .complete(command.ticket(), Completion::Delivered, now)
                    .unwrap();
                dispatched += 1;
            }
            Poll::WaitUntil(deadline) => now = deadline,
            Poll::Finished => break,
            other => panic!("Unexpected {other:?}"),
        }
    }
    assert_eq!(dispatched, 9);
    assert_eq!(scheduler.poll(now).unwrap(), Poll::Finished);
    assert_eq!(scheduler.possibly_held_count(), 0);
    assert_eq!(store.native_bytes().unwrap(), before);
}

#[test]
fn matching_cache_requires_its_own_hash_and_exact_complete_projection() {
    let native = json!([{"type":"delay","ms":10}]);
    let good = with_cache(body(json!([{"Type":0,"Number":"0.01"}])), native.clone());
    let store = saved(good.clone());
    assert_eq!(
        prepare(&store, binding()).unwrap().cache_status(),
        CacheStatus::Verified
    );
    for bad in [
        {
            let mut b = good.clone();
            b["appEngine"]["hash"] = json!("wrong");
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"]["events"][0]["ms"] = json!(11);
            rewrite_cache_hash(&mut b);
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"]["events"][0]["unit"] = json!(7);
            rewrite_cache_hash(&mut b);
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"]["timer"] = json!(1);
            rewrite_cache_hash(&mut b);
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"]["mouseMoveType"] = json!("relative");
            rewrite_cache_hash(&mut b);
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"] = Value::Null;
            b
        },
        {
            let mut b = good.clone();
            b["appEngine"].as_object_mut().unwrap().remove("hash");
            b
        },
    ] {
        let store = saved(bad);
        let before = store.native_bytes().unwrap();
        assert!(prepare(&store, binding()).is_err());
        assert_eq!(store.native_bytes().unwrap(), before);
    }
    let mut without_type = good;
    without_type["appEngine"]
        .as_object_mut()
        .unwrap()
        .remove("mouseMoveType");
    rewrite_cache_hash(&mut without_type);
    assert!(prepare(&saved(without_type), binding()).is_ok());
    assert!(super::super::edit(
        &store,
        super::super::Change::InsertDelay {
            guid: "test".into(),
            index: 1,
            milliseconds: 1
        }
    )
    .is_err());
}

#[test]
fn duplicate_keys_anywhere_refuse_without_collapsing_saved_data() {
    for text in [
        r#"{"guid":"other","guid":"test","macroEvents":[{"Type":0,"Number":"0.1"}]}"#,
        r#"{"guid":"test","macroEvents":[{"Type":0,"Type":0,"Number":"0.1"}]}"#,
        r#"{"guid":"test","macroEvents":[{"Type":1,"KeyEvent":{"Makecode":65,"Makecode":66,"State":0}}]}"#,
        r#"{"guid":"test","macroEvents":[{"Type":0,"Number":"0.1"}],"future":{"x":0,"x":1}}"#,
    ] {
        let store = saved_text(text);
        let before = store.native_bytes().unwrap();
        let error = prepare(&store, binding()).err().unwrap().to_string();
        assert!(error.contains("Duplicate"), "{error}");
        assert_eq!(store.native_bytes().unwrap(), before);
    }
    let store = saved(body(json!([{"Type":0,"Number":0}])));
    assert!(prepare(
        &store,
        r#"{"guid":"other","guid":"test","macroPlaybackOption":"Once"}"#
    )
    .is_err());
}

#[test]
fn source_hash_binding_identity_and_definition_conflicts_are_enforced() {
    let mut store = saved(body(json!([{"Type":0,"Number":0}])));
    assert!(prepare(&store, r#"{"guid":"missing","macroPlaybackOption":"Once"}"#).is_err());
    assert!(prepare(&store, r#"{"guid":"TEST","macroPlaybackOption":"Once"}"#).is_err());
    store.extra.get_mut(super::super::KEY).unwrap()["records"][0]["hash"] = json!("changed");
    assert!(prepare(&store, binding()).is_err());
    let mut store = saved(body(json!([{"Type":0,"Number":0}])));
    let mut other = store.extra[super::super::KEY]["records"][0].clone();
    other["name"] = json!("Conflicting name");
    store.extra.get_mut(super::super::KEY).unwrap()["records"]
        .as_array_mut()
        .unwrap()
        .push(other);
    assert!(prepare(&store, binding()).is_err());
}

#[test]
fn unsupported_actions_phases_and_modes_never_return_a_partial_plan() {
    for row in [
        json!({"Type":0,"Number":{"min":0.1,"max":0.2}}),
        json!({"Type":1,"KeyEvent":{"Makecode":65,"State":6}}),
        json!({"Type":3}),
        json!({"Type":4,"Text":"Do not paste"}),
        json!({"Type":5,"TextCommand":"Do not run"}),
        json!({"Type":6}),
        json!({"Type":7,"guid":"child"}),
        json!({"Type":8}),
        json!({"Type":9}),
        json!({"Type":10}),
        json!({"Type":11}),
        json!({"Type":0,"Number":"0.1","phase":1}),
    ] {
        let store = saved(body(json!([{"Type":0,"Number":0},row])));
        let before = store.native_bytes().unwrap();
        assert!(prepare(&store, binding()).is_err());
        assert_eq!(store.native_bytes().unwrap(), before);
    }
    let store = saved(body(json!([{"Type":0,"Number":0}])));
    for mode in [
        "ContinuousHeld",
        "ContinuousToggle",
        "Sequence",
        "Queue",
        "Phased",
        "PhasedMacro",
    ] {
        assert!(prepare(
            &store,
            &json!({"guid":"test","macroPlaybackOption":mode}).to_string()
        )
        .is_err());
    }
    let repeated = prepare(
        &store,
        r#"{"guid":"test","macroPlaybackOption":"NTimes","repeatCount":3}"#,
    )
    .unwrap();
    assert_eq!(repeated.plan().repeats(), 3);
    let context = NativeContext {
        routing: RoutingIdentity::new(NonZeroU128::new(1).unwrap(), 0),
        modifiers: 0,
        source_unit: None,
    };
    assert!(matches!(
        Scheduler::new(repeated.plan(), context, Owner(NonZeroU64::new(1).unwrap())),
        Err(macro_scheduler::Error::UnsupportedRepetition)
    ));
}

#[test]
fn legacy_null_state_order_is_preserved_without_fabricating_a_balanced_run() {
    let store = saved(body(json!([mouse(0, Value::Null)])));
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(prepared.native_json()).unwrap(),
        json!([{"type":"mouse","id":2},{"type":"mouse","id":1}])
    );
    let context = NativeContext {
        routing: RoutingIdentity::new(NonZeroU128::new(1).unwrap(), 0),
        modifiers: 0,
        source_unit: None,
    };
    assert!(matches!(
        Scheduler::with_contexts(
            prepared.plan(),
            Contexts {
                keyboard: None,
                mouse: Some(context)
            },
            Owner(NonZeroU64::new(1).unwrap())
        ),
        Err(macro_scheduler::Error::InvalidSequence)
    ));
    let store = saved(body(
        json!([{"Type":1,"KeyEvent":{"Makecode":285,"State":null,"Type":"Modifiers"}}]),
    ));
    assert_eq!(
        serde_json::from_str::<Value>(prepare(&store, binding()).unwrap().native_json()).unwrap(),
        json!([{"type":"keyboard","scancode":29,"flag":0},{"type":"keyboard","scancode":29,"flag":1}])
    );
}

#[test]
fn source_lookup_fallback_and_null_placeholder_rules_are_distinct() {
    assert_eq!(
        keyboard(&json!({"KeyEvent":{"Makecode":65530,"State":0},"ScanCode":125,"flag":0}))
            .unwrap(),
        [json!({"type":"keyboard","scancode":125,"flag":0})]
    );
    assert!(
        keyboard(&json!({"KeyEvent":{"Makecode":65530,"State":null},"ScanCode":125,"flag":0}))
            .is_err()
    );
    for rows in [
        json!([{"Type":"actionBar"}]),
        json!([{"Type":1,"KeyEvent":{"Makecode":null}}]),
        json!([{"Type":2,"MouseEvent":{"MouseButton":null}}]),
        json!([mouse(5, Value::Null)]),
        json!([mouse(8, json!(1))]),
    ] {
        assert!(prepare(&saved(body(rows)), binding()).is_err());
    }
    let store = saved(body(
        json!([{"Type":"actionBar"},mouse(8,json!(1)),{"Type":0,"Number":0}]),
    ));
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(prepared.source_rows(), [2]);
}

#[test]
fn opaque_large_numbers_and_original_profiles_survive_planning_and_source_export() {
    let mut store = crate::profile_compat::import_document(include_bytes!(
        "../../../../tests/fixtures/synthetic.synapse4"
    ))
    .unwrap()
    .store;
    let extra = saved_text(
        r#"{"guid":"test","macroEvents":[{"Type":"actionBar"},{"Type":0,"Number":"0.1"}],"future":9.9900e+100,"integer":9007199254740993123456789}"#,
    );
    store.extra.insert(
        super::super::KEY.into(),
        extra.extra[super::super::KEY].clone(),
    );
    let key = store.devices.keys().next().unwrap().clone();
    let before = store.native_bytes().unwrap();
    let exported = crate::profile_compat::export_synapse4(&store, &key)
        .unwrap()
        .bytes;
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(prepared.name(), "Test macro");
    assert_eq!(prepared.plan().explicit_delay_ms(), 100);
    assert!(!prepared.plan().runtime_gaps().is_empty());
    assert_eq!(store.native_bytes().unwrap(), before);
    assert_eq!(
        crate::profile_compat::export_synapse4(&store, &key)
            .unwrap()
            .bytes,
        exported
    );
}

#[test]
fn prepared_snapshot_has_the_exact_saved_record_revision() {
    let store = saved(body(json!([{"Type":0,"Number":0}])));
    let record = &store.extra[super::super::KEY]["records"][0];
    let prepared = prepare(&store, binding()).unwrap();
    assert_eq!(prepared.revision(), revision(record).unwrap());
    let changed = super::super::edit(
        &store,
        super::super::Change::SetDelay {
            guid: "test".into(),
            index: 0,
            milliseconds: 2,
        },
    )
    .unwrap();
    let new = prepare(&changed, binding()).unwrap();
    assert_ne!(prepared.revision(), new.revision());
    assert_eq!(prepared.plan().explicit_delay_ms(), 0);
    assert_eq!(new.plan().explicit_delay_ms(), 2);
}

#[test]
fn row_expansion_json_and_repetition_bounds_refuse_atomically() {
    let store = saved(body(json!(vec![json!({"Type":0,"Number":0}); 512])));
    assert_eq!(
        prepare(&store, binding()).unwrap().plan().events().len(),
        512
    );
    assert!(prepare(
        &saved(body(json!(vec![json!({"Type":0,"Number":0}); 513]))),
        binding()
    )
    .is_err());
    let expanded = saved(body(json!(vec![mouse(5, json!(0)); 103])));
    assert!(prepare(&expanded, binding()).is_err());
    assert!(prepare(&store, &(" ".repeat(4096) + binding())).is_err());
    assert!(prepare(
        &store,
        r#"{"guid":"test","macroPlaybackOption":"NTimes","repeatCount":100000}"#
    )
    .is_err());
    let nested = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert!(unique::parse(&nested, unique::DEFINITION).is_err());
    assert!(unique::parse(
        &format!("[{}]", vec!["0"; 32768].join(",")),
        unique::DEFINITION
    )
    .is_err());
    assert!(unique::parse(&" ".repeat(1024 * 1024 + 1), unique::DEFINITION).is_err());
}

#[test]
fn fixed_delay_rounding_matches_javascript_including_negative_half_to_zero() {
    for (seconds, expected) in [
        (0.0004999999999999999, 0),
        (0.0005, 1),
        (-0.0005, 0),
        (0.0015, 2),
    ] {
        assert_eq!(
            delay(&json!({"Number":seconds})).unwrap(),
            [json!({"type":"delay","ms":expected})]
        );
    }
    for value in [
        json!(true),
        json!("Infinity"),
        json!("0x10"),
        json!(-0.001),
        json!(1e20),
    ] {
        assert!(delay(&json!({"Number":value})).is_err());
    }
}
