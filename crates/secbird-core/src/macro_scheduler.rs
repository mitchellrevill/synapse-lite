//! Completion-driven, single-run keyboard/mouse macro execution state.
//!
//! No thread, clock reads, handles or input sender. A native adapter must prove
//! routing/ownership and report terminal completion, not submit acceptance.
pub use crate::keyboard_output_ledger::{Completion, Owner};
use crate::{keyboard_output_ledger::Failure, keyboard_symbolic_bridge::RoutingIdentity};
#[cfg(test)]
use crate::{
    rzcontrol_packet::{InputPacket, KeyboardInput},
    source_macro::SymbolicPlan,
};
#[cfg(test)]
use std::time::Duration;
mod runtime;
pub use runtime::{Command, Contexts, Direction, Poll, Scheduler, Ticket};
#[cfg(test)]
#[path = "macro_scheduler/mixed_tests.rs"]
mod mixed_tests;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    RoutingFields,
    MissingKeyboardContext,
    MissingMouseContext,
    TicketExhausted,
    UnsupportedRepetition,
    UnsupportedEvent,
    ConditionalPacket,
    InvalidSequence,
    Budget,
    ClockWentBack,
    ClockOverflow,
    WrongTicket,
    Ledger(Failure),
    MouseLedger(crate::mouse_output_ledger::Failure),
}

/// Explicit native fields, separate from the opaque endpoint routing context.
/// None retains the recovered source default UnitId0; it never borrows the
/// routing UnitId. No Default implementation grants an implicit context.
#[derive(Clone, Copy)]
pub struct NativeContext {
    pub routing: RoutingIdentity,
    pub modifiers: u32,
    pub source_unit: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson};
    use serde_json::{json, Value};
    use std::num::{NonZeroU128, NonZeroU64};
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }
    fn source(events: Value, repeats: u32) -> SymbolicPlan {
        let metadata = if repeats == 1 {
            json!({"guid":"test","macroPlaybackOption":"Once"})
        } else {
            json!({"guid":"test","macroPlaybackOption":"NTimes","repeatCount":repeats})
        };
        SourceJson::parse(&events.to_string(), Limits::default())
            .unwrap()
            .plan(
                EventLocation::RootArray,
                &PlaybackMetadata::parse(&metadata).unwrap(),
            )
            .unwrap()
    }
    fn key(scan: u16, flag: u16) -> Value {
        json!({"type":"keyboard","scancode":scan,"flag":flag})
    }
    fn scheduler(events: Vec<Value>) -> Scheduler {
        Scheduler::new(
            &source(json!(events), 1),
            NativeContext {
                routing: RoutingIdentity::new(NonZeroU128::new(7).unwrap(), 3),
                modifiers: 0,
                source_unit: Some(3),
            },
            Owner(NonZeroU64::new(1).unwrap()),
        )
        .unwrap()
    }
    fn next(s: &mut Scheduler, now: u64, direction: Direction) -> Command {
        let Poll::Dispatch(command) = s.poll(ms(now)).unwrap() else {
            panic!("expected one command")
        };
        assert_eq!(command.direction(), direction);
        command
    }
    fn deliver(s: &mut Scheduler, command: Command, now: u64) {
        s.dispatch_started(command.ticket()).unwrap();
        s.complete(command.ticket(), Completion::Delivered, ms(now))
            .unwrap();
    }

    #[test]
    fn keyboard_gap_starts_after_terminal_completion_and_never_reissues_command() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        assert_eq!(s.poll(ms(50)).unwrap(), Poll::Pending);
        assert_eq!(s.source_cursor(), 0);
        deliver(&mut s, down, 100);
        assert_eq!(s.poll(ms(100)).unwrap(), Poll::WaitUntil(ms(110)));
        assert_eq!(s.poll(ms(109)).unwrap(), Poll::WaitUntil(ms(110)));
        let up = next(&mut s, 110, Direction::Up);
        deliver(&mut s, up, 400);
        assert_eq!(s.poll(ms(400)).unwrap(), Poll::Finished);
        assert_eq!(s.source_cursor(), 2);
        assert_eq!(s.possibly_held_count(), 0);
    }
    #[test]
    fn explicit_delays_and_zero_delay_break_keyboard_adjacency() {
        for delay in [0, 20] {
            let mut s = scheduler(vec![
                key(30, 0),
                json!({"type":"delay","ms":delay}),
                key(30, 1),
            ]);
            let down = next(&mut s, 0, Direction::Down);
            deliver(&mut s, down, 100);
            if delay > 0 {
                assert_eq!(s.poll(ms(100)).unwrap(), Poll::WaitUntil(ms(120)));
            }
            let up = next(&mut s, 100 + delay, Direction::Up);
            deliver(&mut s, up, 150);
            assert_eq!(s.poll(ms(150)).unwrap(), Poll::Finished);
        }
    }
    #[test]
    fn stop_during_delay_releases_held_key_before_any_later_source_event() {
        let mut s = scheduler(vec![
            key(30, 0),
            json!({"type":"delay","ms":1000}),
            key(30, 1),
            key(32, 0),
            key(32, 1),
        ]);
        let down = next(&mut s, 0, Direction::Down);
        deliver(&mut s, down, 5);
        assert_eq!(s.poll(ms(5)).unwrap(), Poll::WaitUntil(ms(1005)));
        s.stop();
        let up = next(&mut s, 6, Direction::Up);
        assert_eq!(
            up.packet(),
            InputPacket::Keyboard(KeyboardInput {
                unit_id: 3,
                make_code: 30,
                flags: 1,
                reserved: 0,
                extra_information: 0
            })
        );
        deliver(&mut s, up, 7);
        assert_eq!(
            s.poll(ms(7)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
    }
    #[test]
    fn stop_before_dispatch_stales_down_but_late_dispatched_down_requires_release() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        s.stop();
        assert!(s.dispatch_started(down.ticket()).is_err());
        assert_eq!(
            s.poll(ms(1)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        s.dispatch_started(down.ticket()).unwrap();
        s.stop();
        assert_eq!(s.poll(ms(1)).unwrap(), Poll::Pending);
        s.complete(down.ticket(), Completion::Delivered, ms(2))
            .unwrap();
        let up = next(&mut s, 2, Direction::Up);
        deliver(&mut s, up, 3);
        assert_eq!(
            s.poll(ms(3)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
    }
    #[test]
    fn uncertain_completion_latches_stop_and_exhausted_recovery_never_reports_drained() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        s.dispatch_started(down.ticket()).unwrap();
        s.complete(down.ticket(), Completion::Uncertain, ms(1))
            .unwrap();
        for now in 2..5 {
            let up = next(&mut s, now, Direction::Up);
            s.dispatch_started(up.ticket()).unwrap();
            s.complete(up.ticket(), Completion::Uncertain, ms(now))
                .unwrap();
        }
        assert_eq!(
            s.poll(ms(5)),
            Err(Error::Ledger(Failure::RecoveryUnconfirmed))
        );
        assert_eq!(s.possibly_held_count(), 1);
    }
    #[test]
    fn confirmed_recovery_preserves_historical_uncertainty() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        s.dispatch_started(down.ticket()).unwrap();
        s.complete(down.ticket(), Completion::Uncertain, ms(1))
            .unwrap();
        let up = next(&mut s, 2, Direction::Up);
        deliver(&mut s, up, 3);
        assert_eq!(
            s.poll(ms(3)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: true
            }
        );
    }
    #[test]
    fn stale_duplicate_and_wrong_phase_callbacks_cannot_advance_source() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        assert_eq!(
            s.complete(down.ticket(), Completion::Delivered, ms(99)),
            Err(Error::Ledger(Failure::WrongPhase))
        );
        s.dispatch_started(down.ticket()).unwrap();
        assert_eq!(
            s.dispatch_started(down.ticket()),
            Err(Error::Ledger(Failure::WrongPhase))
        );
        s.complete(down.ticket(), Completion::Delivered, ms(1))
            .unwrap();
        let up = next(&mut s, 11, Direction::Up);
        assert_eq!(
            s.complete(down.ticket(), Completion::Delivered, Duration::ZERO),
            Err(Error::WrongTicket)
        );
        assert_eq!(s.source_cursor(), 1);
        deliver(&mut s, up, 12);
        assert_eq!(s.poll(ms(12)).unwrap(), Poll::Finished);
    }
    #[test]
    fn regressed_completion_clock_stops_but_retains_terminal_delivery_for_release() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 100, Direction::Down);
        s.dispatch_started(down.ticket()).unwrap();
        assert_eq!(s.poll(ms(150)).unwrap(), Poll::Pending);
        assert_eq!(
            s.complete(down.ticket(), Completion::Delivered, ms(120)),
            Err(Error::ClockWentBack)
        );
        let up = next(&mut s, 150, Direction::Up);
        deliver(&mut s, up, 151);
        assert_eq!(
            s.poll(ms(151)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
    }
    #[test]
    fn refusal_before_dispatch_does_not_invent_held_output_or_retry_forever() {
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        s.refused_before_dispatch(down.ticket()).unwrap();
        assert_eq!(
            s.poll(ms(1)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
        let mut s = scheduler(vec![key(30, 0), key(30, 1)]);
        let down = next(&mut s, 0, Direction::Down);
        deliver(&mut s, down, 1);
        s.stop();
        let up = next(&mut s, 2, Direction::Up);
        s.refused_before_dispatch(up.ticket()).unwrap();
        assert_eq!(
            s.poll(ms(3)),
            Err(Error::Ledger(Failure::RecoveryUnconfirmed))
        );
        assert_eq!(
            s.poll(ms(3)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: false,
                uncertain_delivery: false
            }
        );
        assert_eq!(s.possibly_held_count(), 1);
    }
    #[test]
    fn unsupported_sequences_refuse_before_any_output_and_storage_is_bounded() {
        let identity = RoutingIdentity::new(NonZeroU128::new(7).unwrap(), 3);
        let owner = Owner(NonZeroU64::new(1).unwrap());
        let context = NativeContext {
            routing: identity,
            modifiers: 0,
            source_unit: Some(3),
        };
        for events in [
            vec![key(30, 0)],
            vec![key(30, 1)],
            vec![key(30, 0), key(30, 0), key(30, 1)],
            vec![key(29, 4), key(29, 5)],
            vec![json!({"type":"mouse","id":1})],
        ] {
            let raw = SourceJson::parse(&json!(events).to_string(), Limits::default()).unwrap();
            let metadata = json!({"guid":"test","macroPlaybackOption":"Once"});
            let plan = raw
                .plan(
                    EventLocation::RootArray,
                    &PlaybackMetadata::parse(&metadata).unwrap(),
                )
                .unwrap();
            assert!(Scheduler::new(&plan, context, owner).is_err());
        }
        assert!(matches!(
            Scheduler::new(&source(json!([key(30, 0), key(30, 1)]), 2), context, owner),
            Err(Error::UnsupportedRepetition)
        ));
        let events = vec![json!({"type":"delay","ms":0}); 512];
        let mut s = scheduler(events);
        assert!(s.retained_step_bytes() <= 32768);
        assert_eq!(s.poll(Duration::ZERO).unwrap(), Poll::Finished);
        let mut s = scheduler(vec![json!({"type":"delay","ms":1})]);
        assert_eq!(s.poll(Duration::MAX), Err(Error::ClockOverflow));
        assert_eq!(
            s.poll(Duration::MAX).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
    }
    #[test]
    fn source_unit_is_never_inferred_from_route_and_explicit_metadata_survives() {
        let plan = source(json!([key(30, 0), key(30, 1)]), 1);
        let owner = Owner(NonZeroU64::new(1).unwrap());
        let route = RoutingIdentity::new(NonZeroU128::new(7).unwrap(), 3);
        for source_unit in [None, Some(2)] {
            assert!(matches!(
                Scheduler::new(
                    &plan,
                    NativeContext {
                        routing: route,
                        modifiers: 0,
                        source_unit
                    },
                    owner
                ),
                Err(Error::RoutingFields)
            ));
        }
        let mut s = Scheduler::new(
            &plan,
            NativeContext {
                routing: route,
                modifiers: 0x100,
                source_unit: Some(3),
            },
            owner,
        )
        .unwrap();
        let command = next(&mut s, 0, Direction::Down);
        assert!(matches!(
            command.packet(),
            InputPacket::Keyboard(KeyboardInput {
                unit_id: 3,
                extra_information: 4,
                ..
            })
        ));
        deliver(&mut s, command, 1);
        s.stop();
        let command = next(&mut s, 1, Direction::Up);
        assert!(matches!(
            command.packet(),
            InputPacket::Keyboard(KeyboardInput {
                unit_id: 3,
                extra_information: 4,
                ..
            })
        ));
        deliver(&mut s, command, 2);
        let route = RoutingIdentity::new(NonZeroU128::new(7).unwrap(), 0);
        assert!(Scheduler::new(
            &plan,
            NativeContext {
                routing: route,
                modifiers: 0,
                source_unit: None
            },
            owner
        )
        .is_ok());
    }
}
#[cfg(test)]
mod chord_tests {
    use super::*;
    #[test]
    fn extended_chord_retains_route_and_prefix_through_owned_cleanup() {
        use crate::source_macro::{EventLocation, Limits, PlaybackMetadata, SourceJson};
        use std::num::{NonZeroU128, NonZeroU64};
        let raw=SourceJson::parse(r#"[{"type":"keyboard","scancode":29,"flag":2},{"type":"keyboard","scancode":30,"flag":0},{"type":"keyboard","scancode":30,"flag":1},{"type":"keyboard","scancode":29,"flag":3}]"#,Limits::default()).unwrap();
        let group = serde_json::json!({"guid":"chord","macroPlaybackOption":"Once"});
        let plan = raw
            .plan(
                EventLocation::RootArray,
                &PlaybackMetadata::parse(&group).unwrap(),
            )
            .unwrap();
        let mut s = Scheduler::new(
            &plan,
            NativeContext {
                routing: RoutingIdentity::new(NonZeroU128::new(7).unwrap(), 3),
                modifiers: 0,
                source_unit: Some(3),
            },
            Owner(NonZeroU64::new(1).unwrap()),
        )
        .unwrap();
        for (now, scan, flags) in [(0, 29, 2), (10, 30, 0)] {
            let Poll::Dispatch(command) = s.poll(Duration::from_millis(now)).unwrap() else {
                panic!("missing chord down")
            };
            assert!(
                matches!(command.packet(),InputPacket::Keyboard(input) if input.unit_id==3&&input.make_code==scan&&input.flags==flags)
            );
            s.dispatch_started(command.ticket()).unwrap();
            s.complete(
                command.ticket(),
                Completion::Delivered,
                Duration::from_millis(now),
            )
            .unwrap();
        }
        s.stop();
        for scan in [29, 30] {
            let Poll::Dispatch(command) = s.poll(Duration::from_millis(11)).unwrap() else {
                panic!("missing owned release")
            };
            assert!(
                matches!(command.packet(),InputPacket::Keyboard(input) if input.unit_id==3&&input.make_code==scan&&input.flags==if scan==29{3}else{1})
            );
            s.dispatch_started(command.ticket()).unwrap();
            s.complete(
                command.ticket(),
                Completion::Delivered,
                Duration::from_millis(11),
            )
            .unwrap();
        }
        assert_eq!(
            s.poll(Duration::from_millis(11)).unwrap(),
            Poll::Stopped {
                recovery_confirmed: true,
                uncertain_delivery: false
            }
        );
    }
}
