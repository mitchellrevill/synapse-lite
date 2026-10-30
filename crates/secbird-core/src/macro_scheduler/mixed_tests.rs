use super::*;
use crate::source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson};
use serde_json::{json, Value};
use std::num::{NonZeroU128, NonZeroU64};

fn owner(id: u64) -> Owner {
    Owner(NonZeroU64::new(id).unwrap())
}
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
fn key(scan: u16, flag: u16) -> Value {
    json!({"type":"keyboard","scancode":scan,"flag":flag})
}
fn button(id: u16) -> Value {
    json!({"type":"mouse","id":id})
}
fn wheel(id: u16, data: u16) -> Value {
    json!({"type":"mouse","id":id,"data":data})
}
fn plan(events: Vec<Value>) -> SymbolicPlan {
    SourceJson::parse(&json!(events).to_string(), Limits::default())
        .unwrap()
        .plan(
            EventLocation::RootArray,
            &PlaybackMetadata::parse(&json!({"guid":"mixed","macroPlaybackOption":"Once"}))
                .unwrap(),
        )
        .unwrap()
}
fn context(token: u128, unit: u16, modifiers: u32) -> NativeContext {
    NativeContext {
        routing: RoutingIdentity::new(NonZeroU128::new(token).unwrap(), unit),
        modifiers,
        source_unit: Some(unit),
    }
}

#[test]
fn source_keyboard_fields_reach_packets_and_owned_cleanup_without_double_translation() {
    let events = vec![
        json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":0x430,"unitId":3}),
        json!({"type":"keyboard","scancode":32,"flag":2,"modifiers":0,"unitId":3}),
        json!({"type":"keyboard","scancode":32,"flag":3,"modifiers":0,"unitId":3}),
        json!({"type":"keyboard","scancode":30,"flag":1,"modifiers":0x430,"unitId":3}),
    ];
    let mut s = scheduler(events.clone());
    let a = take(&mut s, 0);
    assert!(
        matches!(a.packet(), InputPacket::Keyboard(p) if p.unit_id==3 && p.make_code==30 && p.extra_information==0x181)
    );
    deliver(&mut s, a, 0);
    let d = take(&mut s, 10);
    assert!(
        matches!(d.packet(), InputPacket::Keyboard(p) if p.unit_id==3 && p.make_code==32 && p.flags==2 && p.extra_information==0)
    );
    deliver(&mut s, d, 10);
    let d_up = take(&mut s, 20);
    assert!(
        matches!(d_up.packet(), InputPacket::Keyboard(p) if p.flags==3 && p.extra_information==0)
    );
    deliver(&mut s, d_up, 20);
    let a_up = take(&mut s, 30);
    assert!(
        matches!(a_up.packet(), InputPacket::Keyboard(p) if p.flags==1 && p.extra_information==0x181)
    );
    deliver(&mut s, a_up, 30);
    assert_eq!(s.poll(ms(30)).unwrap(), Poll::Finished);

    // A late uncertain down after Stop must retain the source metadata for up.
    let mut s = scheduler(events);
    let a = take(&mut s, 0);
    s.dispatch_started(a.ticket()).unwrap();
    s.stop();
    s.complete(a.ticket(), Completion::Uncertain, ms(1))
        .unwrap();
    let up = take(&mut s, 1);
    assert!(
        matches!(up.packet(), InputPacket::Keyboard(p) if p.make_code==30 && p.flags==1 && p.extra_information==0x181)
    );
    deliver(&mut s, up, 2);
    assert_eq!(s.possibly_held_count(), 0);
    assert_eq!(s.poll(ms(2)).unwrap(), stopped(true, true));
}

#[test]
fn mismatched_source_keyboard_route_or_release_metadata_refuses_entire_run() {
    for (down, up) in [(0x400, 0), (0, 0x400)] {
        let run = plan(vec![
            button(1),
            json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":down,"unitId":3}),
            json!({"type":"keyboard","scancode":30,"flag":1,"modifiers":up,"unitId":3}),
            button(2),
        ]);
        assert!(matches!(
            Scheduler::with_contexts(&run, contexts(), owner(1)),
            Err(Error::InvalidSequence)
        ));
    }
    for unit in [0, 4, u16::MAX] {
        let run = plan(vec![
            json!({"type":"keyboard","scancode":30,"flag":0,"unitId":unit}),
            json!({"type":"keyboard","scancode":30,"flag":1,"unitId":unit}),
        ]);
        assert!(matches!(
            Scheduler::with_contexts(&run, contexts(), owner(1)),
            Err(Error::InvalidSequence)
        ));
    }
}

#[test]
fn equivalent_translated_source_metadata_and_sequential_changes_are_supported() {
    let mut s = scheduler(vec![
        json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":0}),
        json!({"type":"keyboard","scancode":30,"flag":1,"modifiers":0x80000000u32}),
        json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":0x400}),
        json!({"type":"keyboard","scancode":30,"flag":1,"modifiers":0x400}),
    ]);
    for (index, expected) in [0, 0, 1, 1].into_iter().enumerate() {
        let command = take(&mut s, index as u64 * 10);
        assert!(
            matches!(command.packet(),InputPacket::Keyboard(p) if p.extra_information==expected)
        );
        deliver(&mut s, command, index as u64 * 10);
    }
    assert_eq!(s.poll(ms(30)).unwrap(), Poll::Finished);
}
fn contexts() -> Contexts {
    Contexts {
        keyboard: Some(context(17, 3, 0x100)),
        mouse: Some(context(23, 7, 0x100)),
    }
}
fn scheduler(events: Vec<Value>) -> Scheduler {
    Scheduler::with_contexts(&plan(events), contexts(), owner(1)).unwrap()
}
fn take(s: &mut Scheduler, now: u64) -> Command {
    let Poll::Dispatch(command) = s.poll(ms(now)).unwrap() else {
        panic!("expected dispatch at {now}");
    };
    assert_eq!(s.poll(ms(now)).unwrap(), Poll::Pending);
    command
}
fn deliver(s: &mut Scheduler, command: Command, now: u64) {
    s.dispatch_started(command.ticket()).unwrap();
    s.complete(command.ticket(), Completion::Delivered, ms(now))
        .unwrap();
}
fn stopped(uncertain: bool, recovered: bool) -> Poll {
    Poll::Stopped {
        recovery_confirmed: recovered,
        uncertain_delivery: uncertain,
    }
}

#[test]
fn mixed_routes_timing_and_tickets_preserve_source_order() {
    let mut s = scheduler(vec![
        key(29, 2),
        button(1),
        wheel(2048, 65416),
        button(2),
        key(29, 3),
        key(30, 0),
        key(30, 1),
    ]);
    let first = take(&mut s, 0);
    assert_eq!(first.identity(), contexts().keyboard.unwrap().routing);
    deliver(&mut s, first, 100);
    let mouse = take(&mut s, 100);
    assert_eq!(mouse.identity(), contexts().mouse.unwrap().routing);
    assert_ne!(first.ticket(), mouse.ticket());
    assert_eq!(
        s.complete(first.ticket(), Completion::Delivered, ms(100)),
        Err(Error::WrongTicket)
    );
    assert_eq!(s.source_cursor(), 1);
    deliver(&mut s, mouse, 103);
    let wheel = take(&mut s, 103);
    assert_eq!(wheel.direction(), Direction::Impulse);
    assert!(
        matches!(wheel.packet(),InputPacket::Mouse(input) if input.unit_id==7&&input.button_flags==2048&&input.button_data==65416&&input.extra_information==4)
    );
    deliver(&mut s, wheel, 500);
    let up = take(&mut s, 500);
    assert!(
        matches!(up.packet(),InputPacket::Mouse(input) if input.button_flags==2&&input.extra_information==4)
    );
    deliver(&mut s, up, 510);
    // A mouse event breaks keyboard adjacency, so this key has no added gap.
    let control_up = take(&mut s, 510);
    assert_eq!(control_up.identity(), contexts().keyboard.unwrap().routing);
    deliver(&mut s, control_up, 520);
    assert_eq!(s.poll(ms(520)).unwrap(), Poll::WaitUntil(ms(530)));
    let a = take(&mut s, 530);
    deliver(&mut s, a, 530);
    assert_eq!(s.poll(ms(530)).unwrap(), Poll::WaitUntil(ms(540)));
    let a_up = take(&mut s, 540);
    deliver(&mut s, a_up, 548);
    assert_eq!(s.poll(ms(548)).unwrap(), Poll::Finished);
    assert_eq!(s.possibly_held_count(), 0);
}

#[test]
fn all_static_mouse_vectors_survive_scheduling_and_owned_cleanup() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/native-mouse-packets-static-derived.json"
    ))
    .unwrap();
    let vectors = fixture["vectors"].as_array().unwrap();
    for pair in vectors[..10].chunks_exact(2) {
        let down = pair[0]["id"].as_u64().unwrap() as u16;
        let up = pair[1]["id"].as_u64().unwrap() as u16;
        let mut s = Scheduler::with_contexts(
            &plan(vec![button(down), button(up)]),
            Contexts {
                keyboard: None,
                mouse: Some(context(44, 4660, 1911)),
            },
            owner(4),
        )
        .unwrap();
        let command = take(&mut s, 0);
        let expected = |value: &Value| {
            value["packet_hex"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .map(|hex| u8::from_str_radix(hex, 16).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(command.packet().to_bytes().as_slice(), expected(&pair[0]));
        deliver(&mut s, command, 1);
        s.stop();
        let command = take(&mut s, 1);
        assert_eq!(command.packet().to_bytes().as_slice(), expected(&pair[1]));
        deliver(&mut s, command, 2);
        assert_eq!(s.poll(ms(2)).unwrap(), stopped(false, true));
    }
    for vector in &vectors[10..] {
        let event = wheel(
            vector["id"].as_u64().unwrap() as u16,
            vector["raw_data"].as_u64().unwrap() as u16,
        );
        let unit = vector["unit"].as_u64().unwrap() as u16;
        let modifiers = vector["modifiers"].as_u64().unwrap() as u32;
        let mut s = Scheduler::with_contexts(
            &plan(vec![event]),
            Contexts {
                keyboard: None,
                mouse: Some(context(44, unit, modifiers)),
            },
            owner(4),
        )
        .unwrap();
        let command = take(&mut s, 0);
        let expected: Vec<_> = vector["packet_hex"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .map(|hex| u8::from_str_radix(hex, 16).unwrap())
            .collect();
        assert_eq!(command.packet().to_bytes().as_slice(), expected);
        deliver(&mut s, command, 100);
        assert_eq!(s.poll(ms(100)).unwrap(), Poll::Finished);
        assert_eq!(s.possibly_held_count(), 0);
    }
}

#[test]
fn stop_during_delay_drains_both_classes_on_their_original_routes() {
    let mut s = scheduler(vec![
        key(29, 2),
        button(64),
        json!({"type":"delay","ms":1000}),
        button(128),
        key(29, 3),
    ]);
    let command = take(&mut s, 0);
    deliver(&mut s, command, 1);
    let command = take(&mut s, 1);
    deliver(&mut s, command, 2);
    assert_eq!(s.poll(ms(2)).unwrap(), Poll::WaitUntil(ms(1002)));
    s.stop();
    let key = take(&mut s, 3);
    assert!(
        matches!(key.packet(),InputPacket::Keyboard(input) if input.flags==3&&input.unit_id==3&&input.extra_information==4)
    );
    assert_eq!(key.identity(), contexts().keyboard.unwrap().routing);
    deliver(&mut s, key, 4);
    let mouse = take(&mut s, 4);
    assert!(
        matches!(mouse.packet(),InputPacket::Mouse(input) if input.button_flags==128&&input.unit_id==7&&input.extra_information==4)
    );
    assert_eq!(mouse.identity(), contexts().mouse.unwrap().routing);
    deliver(&mut s, mouse, 5);
    assert_eq!(s.poll(ms(5)).unwrap(), stopped(false, true));
}

#[test]
fn late_uncertain_mouse_down_waits_then_recovers_without_future_wheel() {
    let mut s = scheduler(vec![
        key(30, 0),
        button(16),
        wheel(1024, 120),
        button(32),
        key(30, 1),
    ]);
    let key = take(&mut s, 0);
    deliver(&mut s, key, 1);
    let down = take(&mut s, 1);
    s.dispatch_started(down.ticket()).unwrap();
    s.stop();
    assert_eq!(s.poll(ms(2)).unwrap(), Poll::Pending);
    s.complete(down.ticket(), Completion::Uncertain, ms(3))
        .unwrap();
    for now in 3..5 {
        let command = take(&mut s, now);
        assert_eq!(command.direction(), Direction::Up);
        deliver(&mut s, command, now);
    }
    assert_eq!(s.poll(ms(5)).unwrap(), stopped(true, true));
    assert_eq!(s.source_cursor(), 1);
}

#[test]
fn undispatched_mouse_down_is_stale_and_needs_no_mouse_release() {
    let mut s = scheduler(vec![key(30, 0), button(256), button(512), key(30, 1)]);
    let key = take(&mut s, 0);
    deliver(&mut s, key, 0);
    let mouse = take(&mut s, 0);
    s.stop();
    assert_eq!(s.dispatch_started(mouse.ticket()), Err(Error::WrongTicket));
    let up = take(&mut s, 0);
    assert!(matches!(up.packet(), InputPacket::Keyboard(_)));
    deliver(&mut s, up, 0);
    assert_eq!(s.poll(ms(0)).unwrap(), stopped(false, true));
}

#[test]
fn wheel_stop_and_uncertainty_never_replay_or_invert_the_wheel() {
    for phase in 0..3 {
        let mut s = scheduler(vec![wheel(2048, 65416), key(30, 0), key(30, 1)]);
        let command = take(&mut s, 0);
        if phase == 0 {
            s.stop();
            assert_eq!(
                s.dispatch_started(command.ticket()),
                Err(Error::WrongTicket)
            );
        } else {
            s.dispatch_started(command.ticket()).unwrap();
            s.stop();
            assert_eq!(s.poll(ms(1)).unwrap(), Poll::Pending);
            s.complete(
                command.ticket(),
                if phase == 1 {
                    Completion::Delivered
                } else {
                    Completion::Uncertain
                },
                ms(2),
            )
            .unwrap();
        }
        for _ in 0..3 {
            assert_eq!(s.poll(ms(3)).unwrap(), stopped(phase == 2, true));
        }
        assert_eq!(s.source_cursor(), 0);
        assert_eq!(s.possibly_held_count(), 0);
    }
}

#[test]
fn uncertain_wheel_drains_previously_held_keys_and_buttons() {
    let mut s = scheduler(vec![
        key(30, 0),
        button(1),
        wheel(1024, 120),
        button(2),
        key(30, 1),
    ]);
    for now in 0..2 {
        let down = take(&mut s, now);
        deliver(&mut s, down, now);
    }
    let wheel = take(&mut s, 2);
    assert_eq!(wheel.direction(), Direction::Impulse);
    s.dispatch_started(wheel.ticket()).unwrap();
    s.complete(wheel.ticket(), Completion::Uncertain, ms(3))
        .unwrap();
    assert_eq!(s.source_cursor(), 2);
    for now in 3..5 {
        let release = take(&mut s, now);
        assert_eq!(release.direction(), Direction::Up);
        deliver(&mut s, release, now);
    }
    assert_eq!(s.poll(ms(5)).unwrap(), stopped(true, true));
    assert_eq!(s.possibly_held_count(), 0);
}

#[test]
fn exhausted_keyboard_recovery_still_releases_the_mouse() {
    let mut s = scheduler(vec![key(30, 0), button(1), button(2), key(30, 1)]);
    for now in 0..2 {
        let command = take(&mut s, now);
        deliver(&mut s, command, now);
    }
    s.stop();
    for now in 2..5 {
        let command = take(&mut s, now);
        assert!(matches!(command.packet(), InputPacket::Keyboard(_)));
        s.dispatch_started(command.ticket()).unwrap();
        s.complete(command.ticket(), Completion::Uncertain, ms(now))
            .unwrap();
    }
    let mouse = take(&mut s, 5);
    assert!(matches!(mouse.packet(),InputPacket::Mouse(input) if input.button_flags==2));
    deliver(&mut s, mouse, 5);
    assert_eq!(
        s.poll(ms(6)),
        Err(Error::Ledger(Failure::RecoveryUnconfirmed))
    );
    assert_eq!(s.poll(ms(6)).unwrap(), stopped(true, false));
    assert_eq!(s.possibly_held_count(), 1);
}

#[test]
fn refused_keyboard_cleanup_does_not_strand_mouse_cleanup() {
    let mut s = scheduler(vec![key(30, 0), button(4), button(8), key(30, 1)]);
    for now in 0..2 {
        let command = take(&mut s, now);
        deliver(&mut s, command, now);
    }
    s.stop();
    let key = take(&mut s, 2);
    s.refused_before_dispatch(key.ticket()).unwrap();
    let mouse = take(&mut s, 3);
    assert!(matches!(mouse.packet(),InputPacket::Mouse(input) if input.button_flags==8));
    deliver(&mut s, mouse, 3);
    assert_eq!(
        s.poll(ms(4)),
        Err(Error::Ledger(Failure::RecoveryUnconfirmed))
    );
    assert_eq!(s.poll(ms(4)).unwrap(), stopped(false, false));
    assert_eq!(s.possibly_held_count(), 1);
}

#[test]
fn refused_releases_do_not_strand_other_keys_or_buttons_in_the_same_class() {
    let mut s = scheduler(vec![
        key(30, 0),
        key(32, 0),
        button(1),
        button(4),
        button(8),
        button(2),
        key(32, 1),
        key(30, 1),
    ]);
    let down = take(&mut s, 0);
    deliver(&mut s, down, 0);
    assert_eq!(s.poll(ms(0)).unwrap(), Poll::WaitUntil(ms(10)));
    for _ in 0..3 {
        let down = take(&mut s, 10);
        deliver(&mut s, down, 10);
    }
    assert_eq!(s.possibly_held_count(), 4);
    s.stop();
    let first_key = take(&mut s, 11);
    assert!(matches!(first_key.packet(),InputPacket::Keyboard(input) if input.make_code==30));
    s.refused_before_dispatch(first_key.ticket()).unwrap();
    let second_key = take(&mut s, 11);
    assert!(
        matches!(second_key.packet(),InputPacket::Keyboard(input) if input.make_code==32&&input.flags==1)
    );
    deliver(&mut s, second_key, 11);
    let first_button = take(&mut s, 11);
    assert!(matches!(first_button.packet(),InputPacket::Mouse(input) if input.button_flags==2));
    s.refused_before_dispatch(first_button.ticket()).unwrap();
    let second_button = take(&mut s, 11);
    assert!(matches!(second_button.packet(),InputPacket::Mouse(input) if input.button_flags==8));
    deliver(&mut s, second_button, 11);
    assert_eq!(
        s.poll(ms(11)),
        Err(Error::Ledger(Failure::RecoveryUnconfirmed))
    );
    assert_eq!(s.poll(ms(11)).unwrap(), stopped(false, false));
    assert_eq!(s.possibly_held_count(), 2);
    assert_eq!(
        s.dispatch_started(first_key.ticket()),
        Err(Error::WrongTicket)
    );
    assert_eq!(
        s.dispatch_started(first_button.ticket()),
        Err(Error::WrongTicket)
    );
}

#[test]
fn mouse_clock_regression_and_overflow_preserve_owned_recovery() {
    let mut s = scheduler(vec![button(1), button(2)]);
    let down = take(&mut s, 100);
    s.dispatch_started(down.ticket()).unwrap();
    assert_eq!(s.poll(ms(150)).unwrap(), Poll::Pending);
    assert_eq!(
        s.complete(down.ticket(), Completion::Delivered, ms(120)),
        Err(Error::ClockWentBack)
    );
    let up = take(&mut s, 150);
    deliver(&mut s, up, 151);
    assert_eq!(s.poll(ms(151)).unwrap(), stopped(false, true));
    let mut s = scheduler(vec![button(1), json!({"type":"delay","ms":1}), button(2)]);
    let Poll::Dispatch(down) = s.poll(Duration::MAX).unwrap() else {
        panic!()
    };
    s.dispatch_started(down.ticket()).unwrap();
    s.complete(down.ticket(), Completion::Delivered, Duration::MAX)
        .unwrap();
    assert_eq!(s.poll(Duration::MAX), Err(Error::ClockOverflow));
    let Poll::Dispatch(up) = s.poll(Duration::MAX).unwrap() else {
        panic!()
    };
    s.dispatch_started(up.ticket()).unwrap();
    s.complete(up.ticket(), Completion::Delivered, Duration::MAX)
        .unwrap();
    assert_eq!(s.poll(Duration::MAX).unwrap(), stopped(false, true));
}

#[test]
fn missing_and_mismatched_contexts_refuse_without_borrowing_the_other_route() {
    let mouse = plan(vec![button(1), button(2)]);
    assert!(matches!(
        Scheduler::new(&mouse, contexts().keyboard.unwrap(), owner(1)),
        Err(Error::MissingMouseContext)
    ));
    let keys = plan(vec![key(30, 0), key(30, 1)]);
    assert!(matches!(
        Scheduler::with_contexts(
            &keys,
            Contexts {
                keyboard: None,
                mouse: contexts().mouse
            },
            owner(1)
        ),
        Err(Error::MissingKeyboardContext)
    ));
    let mut wrong = contexts();
    wrong.mouse.as_mut().unwrap().source_unit = None;
    assert!(matches!(
        Scheduler::with_contexts(&mouse, wrong, owner(1)),
        Err(Error::RoutingFields)
    ));
    let mut zero = context(23, 0, 0);
    zero.source_unit = None;
    let s = Scheduler::with_contexts(
        &mouse,
        Contexts {
            keyboard: None,
            mouse: Some(zero),
        },
        owner(1),
    )
    .unwrap();
    assert!(s.contexts().mouse.unwrap().source_unit.is_none());
}

#[test]
fn unbalanced_and_repeated_mouse_makes_refuse_but_explicit_clicks_remain_distinct() {
    for events in [
        vec![button(1)],
        vec![button(2)],
        vec![button(1), button(1), button(2)],
        vec![button(1), button(8)],
    ] {
        assert!(matches!(
            Scheduler::with_contexts(&plan(events), contexts(), owner(1)),
            Err(Error::InvalidSequence)
        ));
    }
    let mut s = scheduler(vec![button(1), button(2), button(1), button(2)]);
    for now in 0..4 {
        let command = take(&mut s, now);
        deliver(&mut s, command, now);
    }
    assert_eq!(s.source_cursor(), 4);
    assert_eq!(s.poll(ms(4)).unwrap(), Poll::Finished);
}

#[test]
fn wheel_storage_is_bounded_and_callbacks_from_another_run_are_inert() {
    let mut s = scheduler(vec![wheel(1024, 120); 512]);
    assert!(s.retained_step_bytes() <= 32768);
    for now in 0..512 {
        let command = take(&mut s, now);
        deliver(&mut s, command, now);
    }
    assert_eq!(s.poll(ms(512)).unwrap(), Poll::Finished);
    let run = plan(vec![wheel(1024, 120)]);
    let mut old = Scheduler::with_contexts(&run, contexts(), owner(1)).unwrap();
    let mut new = Scheduler::with_contexts(&run, contexts(), owner(2)).unwrap();
    let stale = take(&mut old, 0);
    let current = take(&mut new, 0);
    assert_ne!(stale.ticket(), current.ticket());
    assert_eq!(
        new.dispatch_started(stale.ticket()),
        Err(Error::WrongTicket)
    );
    assert_eq!(new.source_cursor(), 0);
    deliver(&mut new, current, 0);
    assert_eq!(new.poll(ms(0)).unwrap(), Poll::Finished);
}
