use super::*;
use crate::{profile_compat, Store};
fn assignment(choice: Choice) -> Assignment {
    Assignment {
        input_id: "RightClick".into(),
        input_type: "MouseInput".into(),
        hyper: false,
        choice,
        turbo: None,
    }
}
fn profile() -> Profile {
    profile_compat::new_native_profile(196, "Fixture").unwrap()
}

#[test]
fn all_choices_have_balanced_pulses_and_existing_packet_encoding() {
    use crate::{
        keyboard_symbolic_bridge::RoutingIdentity,
        mouse_symbolic_bridge::plan_mouse_packet,
        source_macro::{Event, Transition, WheelAxis},
    };
    let identity = RoutingIdentity::new(std::num::NonZeroU128::new(7).unwrap(), 0);
    for (choice, down) in [
        (Choice::Click, 1),
        (Choice::Menu, 4),
        (Choice::ScrollButton, 16),
        (Choice::Previous, 64),
        (Choice::Next, 256),
    ] {
        let plan = choice.pulse_plan().unwrap();
        assert_eq!(plan.events().len(), 2);
        assert!(
            matches!(plan.events()[0], Event::MouseButton{id,transition:Transition::Down,..} if id==down)
        );
        assert!(
            matches!(plan.events()[1], Event::MouseButton{id,transition:Transition::Up,..} if id==down*2)
        );
        for &event in plan.events() {
            assert!(plan_mouse_packet(event, identity, 0, None).is_ok());
        }
    }
    for (choice, id, axis, delta) in [
        (Choice::ScrollUp, 1024, WheelAxis::Vertical, 120),
        (Choice::ScrollDown, 1024, WheelAxis::Vertical, -120),
        (Choice::ScrollLeft, 2048, WheelAxis::Horizontal, -120),
        (Choice::ScrollRight, 2048, WheelAxis::Horizontal, 120),
    ] {
        let plan = choice.pulse_plan().unwrap();
        assert!(
            matches!(plan.events(),[Event::Wheel{id:i,axis:a,delta:d,..}] if *i==id && *a==axis && *d==delta)
        );
        assert!(plan_mouse_packet(plan.events()[0], identity, 0, None).is_ok());
    }
    let double = Choice::DoubleClick.pulse_plan().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/source_macro_conversion.json"
    ))
    .unwrap();
    let expected = SourceJson::parse(
        &fixture["cases"][9]["output"].to_string(),
        Limits::default(),
    )
    .unwrap()
    .plan(
        EventLocation::RootArray,
        &PlaybackMetadata::parse(&json!({"guid":"fixture","macroPlaybackOption":"Once"})).unwrap(),
    )
    .unwrap();
    assert_eq!(double.events(), expected.events());
    assert_eq!(double.explicit_delay_ms(), 10);
}

#[test]
fn local_save_roundtrips_all_choices_and_preserves_source_and_native_replay() {
    for choice in [
        Choice::Click,
        Choice::Menu,
        Choice::ScrollButton,
        Choice::DoubleClick,
        Choice::ScrollUp,
        Choice::ScrollDown,
        Choice::Previous,
        Choice::Next,
        Choice::ScrollLeft,
        Choice::ScrollRight,
    ] {
        let mut p = profile();
        let record = json!({"inputID":"RightClick","inputType":"MouseInput","isHyperShift":false,"outputType":"mouseGroup","rootFuture":{"keep":1},"mouseGroup":{"mouseAssignment":"Menu","future":[3,4],"turboMode":{"isTurbo":false,"keysPerSecond":7,"future":5}}});
        p.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = json!([record]);
        let original = p.extra[SOURCE]["decoded"].clone();
        let buttons = p.buttons.clone();
        set_pending(&mut p, 196, "Fixture", assignment(choice)).unwrap();
        let reopened: Profile = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        assert_eq!(reopened.buttons, buttons); // No new firmware replay entry.
        assert_eq!(reopened.extra[SOURCE]["decoded"], original);
        let mappings = effective_mappings(&reopened, 196).unwrap();
        assert_eq!(
            mappings[0]["mouseGroup"]["mouseAssignment"],
            choice.source_name()
        );
        assert_eq!(mappings[0]["rootFuture"], record["rootFuture"]);
        assert_eq!(
            mappings[0]["mouseGroup"]["future"],
            record["mouseGroup"]["future"]
        );
        assert_eq!(mappings[0]["mouseGroup"]["turboMode"]["future"], 5);
    }
}

#[test]
fn source_export_import_retains_assignment_layers_and_opaque_leaves() {
    let mut store = Store::default();
    let p = store.profiles_for("00C4", 196).active_mut();
    *p = profile();
    p.extra.get_mut(SOURCE).unwrap()["decoded"]["futureRoot"] = json!({"keep":[1,2]});
    set_pending(p, 196, "Default", assignment(Choice::DoubleClick)).unwrap();
    let mut hyper = assignment(Choice::ScrollRight);
    hyper.hyper = true;
    set_pending(p, 196, "Default", hyper).unwrap();
    let export = profile_compat::export_synapse4(&store, "00C4").unwrap();
    let imported = profile_compat::import_document(&export.bytes).unwrap();
    let p = imported.store.devices["00C4"].active();
    let records = effective_mappings(p, 196).unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| r["inputID"] == "RightClick")
            .count(),
        2
    );
    assert_eq!(
        records.iter().find(|r| r["isHyperShift"] == true).unwrap()["mouseGroup"]
            ["mouseAssignment"],
        "ScrollRight"
    );
    assert_eq!(
        p.extra[SOURCE]["decoded"]["futureRoot"],
        json!({"keep":[1,2]})
    );
}

#[test]
fn restrictions_and_bad_source_refuse_without_mutating_profile() {
    for (pid, id, kind, turbo) in [
        (196, "LeftClick", "MouseInput", None),
        (196, "ScrollUp", "MouseInput", Some(7)),
        (197, "DKM_SB_03", "DKMInput", Some(7)),
        (605, "KEY_A", "MouseInput", None),
        (1, "RightClick", "MouseInput", None),
        (196, "Button4", "MouseInput", Some(21)),
    ] {
        let mut p = profile();
        let before = p.clone();
        let a = Assignment {
            input_id: id.into(),
            input_type: kind.into(),
            turbo,
            ..assignment(Choice::Click)
        };
        assert!(set_pending(&mut p, pid, "Fixture", a).is_err());
        assert_eq!(p, before);
    }
    for records in [
        json!([{"inputID":"RightClick","inputType":"MouseInput"},{"inputID":"RightClick","inputType":"MouseInput"}]),
        json!([{"inputID":"RightClick","inputType":"MouseInput","mapping":[{}]}]),
        json!([{"inputID":"RightClick","inputType":"MouseInput","isHyperShift":0}]),
        json!([{"inputID":"RightClick","inputType":"MouseInput","mouseGroup":3}]),
    ] {
        let mut p = profile();
        p.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = records;
        let before = p.clone();
        assert!(set_pending(&mut p, 196, "Fixture", assignment(Choice::Menu)).is_err());
        assert_eq!(p, before);
    }
    assert!(Choice::parse("BossKey").is_err());
    assert!(Choice::parse("RepeatScrollUp").is_err());
}

#[test]
fn replacing_normal_hypershift_removes_only_its_paired_secondary() {
    let mut p = profile();
    let normal = json!({"inputID":"RightClick","inputType":"MouseInput","isHyperShift":false,"outputType":"hyperShiftGroup","keep":8});
    let secondary = json!({"inputID":"RightClick","inputType":"MouseInput","isHyperShift":true,"outputType":"hyperShiftGroup","foreign":[1,2]});
    let unrelated = json!({"inputID":"Button4","inputType":"MouseInput","isHyperShift":true,"outputType":"futureGroup","keep":9});
    p.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = json!([normal, secondary, unrelated]);
    set_pending(&mut p, 196, "Fixture", assignment(Choice::Menu)).unwrap();
    let records = effective_mappings(&p, 196).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[1], unrelated);
    assert_eq!(records[0]["keep"], 8);
    assert_eq!(p.extra[SOURCE]["decoded"]["mappings"][1], secondary);
}

#[test]
fn supported_keyboard_and_turbo_intent_remain_local_and_bounded() {
    let mut p = profile_compat::new_native_profile(605, "Keyboard").unwrap();
    let a = Assignment {
        input_id: "KEY_A".into(),
        input_type: "KeyInput".into(),
        turbo: Some(20),
        ..assignment(Choice::DoubleClick)
    };
    set_pending(&mut p, 605, "Keyboard", a).unwrap();
    assert_eq!(
        effective_mappings(&p, 605).unwrap()[0]["mouseGroup"]["turboMode"]["keysPerSecond"],
        20
    );
    assert!(p.buttons.is_empty());
}

#[test]
fn later_explicit_secondary_survives_further_normal_edits() {
    let mut p = profile();
    p.extra.get_mut(SOURCE).unwrap()["decoded"]["mappings"] = json!([
        {"inputID":"RightClick","inputType":"MouseInput","isHyperShift":false,"outputType":"hyperShiftGroup"},
        {"inputID":"RightClick","inputType":"MouseInput","isHyperShift":true,"outputType":"hyperShiftGroup"}]);
    set_pending(&mut p, 196, "Fixture", assignment(Choice::Menu)).unwrap();
    let mut secondary = assignment(Choice::ScrollUp);
    secondary.hyper = true;
    set_pending(&mut p, 196, "Fixture", secondary).unwrap();
    set_pending(&mut p, 196, "Fixture", assignment(Choice::DoubleClick)).unwrap();
    let maps = effective_mappings(&p, 196).unwrap();
    assert_eq!(maps.len(), 2);
    assert_eq!(
        maps.iter().find(|r| r["isHyperShift"] == true).unwrap()["mouseGroup"]["mouseAssignment"],
        "ScrollUp"
    );
}
