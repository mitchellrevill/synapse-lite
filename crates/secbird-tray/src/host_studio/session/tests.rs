use super::*;
use crate::host_static::test_support;

const PROFILE: &str =
    include_str!("../../../../secbird-core/tests/fixtures/studio-preview-profile.json");
fn prepared() -> PreparedStudio {
    let draft = Draft::open(PROFILE).unwrap();
    PreparedStudio::build(draft.read_snapshot(), crate::host_studio::tests::map(), 1).unwrap()
}
fn fixture() -> (OutputSession, Device, test_support::Spy) {
    let (device, spy) = test_support::fixture();
    let mut session = OutputSession::default();
    let intent = session.control().apply_intent().unwrap();
    session.activate(intent, &device, 41, prepared()).unwrap();
    (session, device, spy)
}
fn due(session: &mut OutputSession, millis: u64) -> Ticket {
    match session.poll(Duration::from_millis(millis), Availability::Ready) {
        Poll::Frame(ticket) => ticket,
        other => panic!("expected frame, got {other:?}"),
    }
}
fn accepted() -> Completion {
    Completion {
        accepted_packets: 7,
        attempted_packets: 7,
        error: None,
    }
}
fn send(session: &mut OutputSession, device: &mut Device, ticket: &Ticket) -> Completion {
    session
        .transfer(
            ticket,
            device,
            41,
            Instant::now() + Duration::from_secs(1),
            || false,
        )
        .unwrap()
}

#[test]
fn only_latest_apply_is_admitted_and_failed_new_request_preserves_old_scene() {
    let (mut session, device, _) = fixture();
    let before = session.status();
    let control = session.control();
    let older = control.apply_intent().unwrap();
    let latest = control.apply_intent().unwrap();
    assert!(session.activate(older, &device, 41, prepared()).is_err());
    assert_eq!(session.status()["generation"], before["generation"]);
    // A failed file/index request must not cancel the currently applied scene.
    let archive = Archive::parse(br#"{"version":1,"profiles":[]}"#).unwrap();
    assert!(session
        .apply_saved(latest, &archive, 9, &device, Path::new("absent"), 41, 1)
        .is_err());
    assert_eq!(session.status()["generation"], before["generation"]);
    let ticket = due(&mut session, 0);
    session
        .complete(&ticket, Duration::ZERO, accepted())
        .unwrap();
    assert_eq!(session.status()["accepted_frames"], 1);
    let foreign = Control::default().apply_intent().unwrap();
    assert!(session.activate(foreign, &device, 41, prepared()).is_err());
}

#[test]
fn stop_cancels_pending_apply_but_a_later_explicit_apply_can_start() {
    let (mut session, device, _) = fixture();
    let control = session.control();
    let stale = control.apply_intent().unwrap();
    control.request_stop().unwrap();
    assert!(session.activate(stale, &device, 41, prepared()).is_err());
    assert!(session.holds_target(&device.key()));
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Stopped
    );
    assert!(!session.holds_target(&device.key()));
    let later = control.apply_intent().unwrap();
    session.activate(later, &device, 42, prepared()).unwrap();
    assert_eq!(session.status()["connection_epoch"], 42);
    assert!(matches!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Frame(_)
    ));
}

#[test]
fn stop_holds_quick_target_until_inflight_completion_is_drained() {
    let (mut session, mut device, spy) = fixture();
    let key = device.key();
    assert!(!session.holds_target("some other device"));
    let ticket = due(&mut session, 0);
    session.control().request_stop().unwrap();
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Draining
    );
    assert!(session.holds_target(&key));
    assert_eq!(session.status()["drained"], false);
    let completion = send(&mut session, &mut device, &ticket);
    assert_eq!(completion.attempted_packets, 0);
    assert_eq!(spy.state.lock().unwrap().attempts, 0);
    session
        .complete(&ticket, Duration::ZERO, completion)
        .unwrap();
    assert!(
        session.holds_target(&key),
        "owner must explicitly observe the drain"
    );
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Stopped
    );
    assert_eq!(session.status()["drained"], true);
    assert!(!session.holds_target(&key));
}

#[test]
fn cancellation_during_rows_drains_without_retry_phase_advance_or_restoration_claim() {
    let (mut session, mut device, spy) = fixture();
    let control = session.control();
    let ticket = due(&mut session, 0);
    let completion = session
        .transfer(
            &ticket,
            &mut device,
            41,
            Instant::now() + Duration::from_secs(1),
            || {
                if spy.state.lock().unwrap().attempts == 3 {
                    control.request_stop().unwrap();
                }
                false
            },
        )
        .unwrap();
    assert_eq!(
        (completion.accepted_packets, completion.attempted_packets),
        (3, 3)
    );
    assert!(completion.error.is_some());
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Draining
    );
    session
        .complete(&ticket, Duration::from_millis(1), completion)
        .unwrap();
    assert_eq!(session.status()["frame_index"], 0);
    assert_eq!(session.status()["prior_rgb_restored"], false);
    assert_eq!(
        session.poll(Duration::from_secs(1), Availability::Ready),
        Poll::Stopped
    );
    assert_eq!(spy.state.lock().unwrap().attempts, 3);
    assert_eq!(spy.state.lock().unwrap().mode, 3);
}

#[test]
fn a_ticket_can_transfer_only_once_and_stale_completion_cannot_change_new_generation() {
    let (mut session, mut device, spy) = fixture();
    let first = due(&mut session, 0);
    let completion = send(&mut session, &mut device, &first);
    assert!(completion.error.is_none());
    assert!(session
        .transfer(
            &first,
            &mut device,
            41,
            Instant::now() + Duration::from_secs(1),
            || false
        )
        .is_err());
    assert_eq!(spy.state.lock().unwrap().attempts, 7);
    assert!(session
        .activate(
            session.control().apply_intent().unwrap(),
            &device,
            41,
            prepared()
        )
        .is_err());
    session
        .complete(&first, Duration::from_millis(5), completion)
        .unwrap();
    let intent = session.control().apply_intent().unwrap();
    session.activate(intent, &device, 41, prepared()).unwrap();
    let next = due(&mut session, 40);
    assert_ne!(next.generation, first.generation);
    assert!(session
        .complete(&first, Duration::from_millis(50), accepted())
        .is_err());
    assert_eq!(
        session.poll(Duration::from_millis(50), Availability::Ready),
        Poll::InFlight
    );
    assert_eq!(session.status()["accepted_frames"], 0);
    session
        .complete(&next, Duration::from_millis(50), accepted())
        .unwrap();
}

#[test]
fn owner_block_and_idle_freeze_phase_and_late_polls_never_catch_up() {
    let (mut session, _, _) = fixture();
    let first = due(&mut session, 0);
    session
        .complete(&first, Duration::from_millis(39), accepted())
        .unwrap();
    assert_eq!(
        session.poll(Duration::from_millis(39), Availability::Ready),
        Poll::Wait(Duration::from_millis(1))
    );
    assert_eq!(
        session.poll(Duration::from_secs(1), Availability::Suppressed),
        Poll::Suppressed
    );
    assert_eq!(
        session.poll(Duration::from_secs(200), Availability::Blocked),
        Poll::Blocked
    );
    let second = due(&mut session, 200_000);
    assert_eq!(second.frame_index, 1);
    assert_eq!(
        session.poll(Duration::from_secs(999), Availability::Ready),
        Poll::InFlight
    );
    session
        .complete(&second, Duration::from_secs(999), accepted())
        .unwrap();
    let third = due(&mut session, 999_000);
    assert_eq!(third.frame_index, 2);
}

#[test]
fn reconnect_epoch_refuses_same_key_and_path_before_any_device_io() {
    let (mut session, mut device, spy) = fixture();
    let ticket = due(&mut session, 0);
    spy.state.lock().unwrap().commands.clear();
    let completion = session
        .transfer(
            &ticket,
            &mut device,
            42,
            Instant::now() + Duration::from_secs(1),
            || false,
        )
        .unwrap();
    assert_eq!(completion.attempted_packets, 0);
    assert!(completion
        .error
        .as_deref()
        .unwrap()
        .contains("connection changed"));
    assert!(spy.state.lock().unwrap().commands.is_empty());
    session
        .complete(&ticket, Duration::ZERO, completion)
        .unwrap();
    assert_eq!(
        session.poll(Duration::from_secs(100), Availability::Ready),
        Poll::Faulted
    );
    assert!(session.holds_target(&device.key()));
}

#[test]
fn partial_outputs_mode_zero_and_worker_cancel_latch_without_retries() {
    for fail_at in [0, 3, 6, 7, 8] {
        let (mut session, mut device, spy) = fixture();
        if fail_at < 7 {
            spy.state.lock().unwrap().fail_at = Some(fail_at);
        }
        if fail_at == 7 {
            spy.state.lock().unwrap().mode = 0;
        }
        let ticket = due(&mut session, 0);
        let completion = session
            .transfer(
                &ticket,
                &mut device,
                41,
                Instant::now() + Duration::from_secs(1),
                || fail_at == 8,
            )
            .unwrap();
        assert!(completion.error.is_some());
        let counts = (completion.accepted_packets, completion.attempted_packets);
        if fail_at < 7 {
            assert_eq!(counts, (fail_at, fail_at + 1));
        } else {
            assert_eq!(counts, (0, 0));
        }
        session
            .complete(&ticket, Duration::from_millis(1), completion)
            .unwrap();
        assert_eq!(session.status()["frame_index"], 0);
        for _ in 0..5 {
            assert_eq!(
                session.poll(Duration::from_secs(500), Availability::Ready),
                Poll::Faulted
            );
        }
        assert_eq!(spy.state.lock().unwrap().attempts, counts.1);
    }
}

#[test]
fn backward_clock_and_counter_exhaustion_fail_closed() {
    let (mut session, _, _) = fixture();
    let ticket = due(&mut session, 10);
    session
        .complete(&ticket, Duration::from_millis(9), accepted())
        .unwrap();
    assert_eq!(
        session.poll(Duration::from_secs(1), Availability::Ready),
        Poll::Faulted
    );
    assert_eq!(session.status()["frame_index"], 0);
    let control = session.control();
    control.0.latest.store(u64::MAX, Ordering::Release);
    assert!(control.apply_intent().is_err());
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Stopped
    );
    assert!(control.request_stop().is_err());
    let (mut session, _, _) = fixture();
    session.active.as_mut().unwrap().next_sequence = u64::MAX;
    assert_eq!(
        session.poll(Duration::ZERO, Availability::Ready),
        Poll::Faulted
    );
}

#[test]
fn applied_revision_uses_saved_json_and_draft_edits_do_not_change_output() {
    let (mut session, mut device, spy) = fixture();
    let before = session.status();
    assert_eq!(before["composition_id"], "synthetic");
    let mut editor = Draft::open(PROFILE).unwrap();
    assert_eq!(
        before["composition_sha256"],
        digest(editor.read_snapshot().json().as_bytes()).unwrap()
    );
    editor.rename_layer("fire", "Unsaved editor title").unwrap();
    assert_ne!(
        digest(editor.read_snapshot().json().as_bytes()).unwrap(),
        before["composition_sha256"]
    );
    assert_eq!(
        session.status()["composition_sha256"],
        before["composition_sha256"]
    );
    let (gaming, macros, brightness, effects) = {
        let sim = spy.sim.state();
        (
            sim.game_mode,
            sim.macros.clone(),
            sim.brightness.clone(),
            sim.effects.clone(),
        )
    };
    let ticket = due(&mut session, 0);
    let completion = send(&mut session, &mut device, &ticket);
    session
        .complete(&ticket, Duration::from_millis(1), completion)
        .unwrap();
    let sim = spy.sim.state();
    assert_eq!(sim.game_mode, gaming);
    assert_eq!(sim.macros, macros);
    assert_eq!(sim.brightness, brightness);
    assert_eq!(sim.effects, effects);
    assert_eq!(spy.state.lock().unwrap().mode, 3);
    assert!(sim
        .matrix
        .as_ref()
        .unwrap()
        .2
        .iter()
        .any(|pixel| *pixel != [0, 0, 0]));
}

#[test]
fn stop_is_available_while_the_owner_mutex_is_held_elsewhere() {
    let (session, _, _) = fixture();
    let control = session.control();
    let owner = std::sync::Mutex::new(session);
    let mut locked = owner.lock().unwrap();
    let join = std::thread::spawn(move || control.request_stop());
    join.join().unwrap().unwrap();
    assert_eq!(
        locked.poll(Duration::ZERO, Availability::Ready),
        Poll::Stopped
    );
}

#[test]
fn ticket_from_another_session_with_identical_numeric_counters_is_refused() {
    let (mut first, _, _) = fixture();
    let (mut second, mut device, spy) = fixture();
    let old = due(&mut first, 0);
    let current = due(&mut second, 0);
    assert_eq!(old.generation, current.generation);
    assert_eq!(old.sequence, current.sequence);
    assert_ne!(
        old, current,
        "the issuing control identity must be retained"
    );
    assert!(second.complete(&old, Duration::ZERO, accepted()).is_err());
    assert!(second
        .transfer(
            &old,
            &mut device,
            41,
            Instant::now() + Duration::from_secs(1),
            || false
        )
        .is_err());
    assert_eq!(spy.state.lock().unwrap().attempts, 0);
    assert_eq!(
        second.poll(Duration::ZERO, Availability::Ready),
        Poll::InFlight
    );
}

#[test]
fn stop_during_a_blocked_syscall_finishes_that_row_then_drains_without_another_out() {
    use secbird_core::Transport;
    use std::sync::{mpsc, Mutex};
    struct BlockFirstRow {
        inner: test_support::Spy,
        once: AtomicBool,
        entered: mpsc::SyncSender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl Transport for BlockFirstRow {
        fn send_feature_report(&self, data: &[u8]) -> Result<()> {
            if data[7..9] == [15, 3] && self.once.swap(false, Ordering::AcqRel) {
                self.entered.send(()).unwrap();
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            self.inner.send_feature_report(data)
        }
        fn get_feature_report(&self, data: &mut [u8]) -> Result<usize> {
            self.inner.get_feature_report(data)
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    let (mut session, original, spy) = fixture();
    let control = session.control();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let transport = BlockFirstRow {
        inner: spy.clone(),
        once: AtomicBool::new(true),
        entered: entered_tx,
        release: Mutex::new(release_rx),
    };
    let mut device = Device::open(
        Box::new(transport),
        605,
        2,
        original.path.clone(),
        &[0x1f],
        original.spec.clone(),
    )
    .unwrap();
    let ticket = due(&mut session, 0);
    let worker = std::thread::spawn(move || {
        let completion = session
            .transfer(
                &ticket,
                &mut device,
                41,
                Instant::now() + Duration::from_secs(5),
                || false,
            )
            .unwrap();
        assert_eq!(
            session.poll(Duration::ZERO, Availability::Ready),
            Poll::Draining
        );
        session
            .complete(&ticket, Duration::from_millis(1), completion.clone())
            .unwrap();
        assert_eq!(session.status()["frame_index"], 0);
        assert_eq!(
            session.poll(Duration::from_millis(1), Availability::Ready),
            Poll::Stopped
        );
        completion
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    control.request_stop().unwrap();
    assert!(
        !worker.is_finished(),
        "stop is admission cancellation, not syscall interruption"
    );
    release_tx.send(()).unwrap();
    let completion = worker.join().unwrap();
    assert_eq!(
        (completion.accepted_packets, completion.attempted_packets),
        (1, 1)
    );
    assert!(completion.error.is_some());
    assert_eq!(spy.state.lock().unwrap().attempts, 1);
    assert_eq!(spy.state.lock().unwrap().mode, 3);
}

#[test]
#[ignore = "requires explicit pinned private manifest and actual exported composition; MOCK output only"]
fn actual_saved_archive_applies_in_a_session_without_touching_source_or_quick_settings() {
    let manifest = std::path::PathBuf::from(
        std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH").expect("select pinned asset"),
    );
    let input = std::path::PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select actual export"),
    );
    let bytes = std::fs::read(&input).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let (mut device, spy) = test_support::fixture();
    spy.sim.state().ornata_layout_edition = Some([6, 0]);
    let before = {
        let sim = spy.sim.state();
        (
            sim.effects.clone(),
            sim.brightness.clone(),
            sim.macros.clone(),
            sim.game_mode,
        )
    };
    let mut session = OutputSession::default();
    session
        .apply_saved(
            session.control().apply_intent().unwrap(),
            &archive,
            0,
            &device,
            &manifest,
            41,
            1,
        )
        .unwrap();
    assert_eq!(session.target_key(), Some(device.key().as_str()));
    assert_eq!(
        spy.state.lock().unwrap().attempts,
        0,
        "apply preparation itself never outputs"
    );
    for frame in 0..25 {
        let ticket = due(&mut session, frame * 40);
        let completion = send(&mut session, &mut device, &ticket);
        assert!(completion.error.is_none());
        session
            .complete(&ticket, Duration::from_millis(frame * 40 + 1), completion)
            .unwrap();
    }
    assert_eq!(session.status()["accepted_frames"], 25);
    assert_eq!(spy.state.lock().unwrap().attempts, 175);
    session.control().request_stop().unwrap();
    assert_eq!(
        session.poll(Duration::from_secs(1), Availability::Ready),
        Poll::Stopped
    );
    let after = {
        let sim = spy.sim.state();
        (
            sim.effects.clone(),
            sim.brightness.clone(),
            sim.macros.clone(),
            sim.game_mode,
        )
    };
    assert_eq!(before, after);
    assert_eq!(spy.state.lock().unwrap().mode, 3);
    assert_eq!(std::fs::read(input).unwrap(), bytes);
}
