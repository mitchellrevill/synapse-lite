use super::*;
use library::{
    host_actions::Action,
    input_events::{Gesture, Input},
    Change,
};
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    time::Instant,
};
struct Fixture {
    folder: PathBuf,
    path: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!(
            "host-macro-worker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("profiles.json");
        let store = library::edit(
            &Store::default(),
            Change::Create {
                guid: "worker".into(),
                name: "Worker".into(),
            },
        )
        .unwrap();
        let mut fixture = Self {
            folder,
            path,
            store,
        };
        fixture.write();
        fixture
    }
    fn write(&mut self) {
        std::fs::write(&self.path, self.store.native_bytes().unwrap()).unwrap();
    }
    fn edit(&mut self, change: Change) {
        self.store = library::edit(&self.store, change).unwrap();
        self.write();
    }
    fn action(&mut self, index: usize) {
        self.edit(Change::InsertAction {
            guid: "worker".into(),
            index,
            action: Action::Command("inert test data".into()),
        });
    }
    fn delay(&mut self, index: usize, ms: u32) {
        self.edit(Change::InsertDelay {
            guid: "worker".into(),
            index,
            milliseconds: ms,
        });
    }
    fn request(&self) -> Request {
        Request {
            path: self.path.clone(),
            guid: "worker".into(),
            revision: library::read(&self.store).unwrap().entries[0]
                .revision
                .clone(),
            read_only: false,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let target = self.folder.canonicalize().unwrap();
        let temp = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            target.starts_with(&temp)
                && target != temp
                && target
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("host-macro-worker-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}
fn accepted() -> Result<native::Receipt, native::Error> {
    Ok(native::Receipt {
        route: native::Route::CreateProcess,
        process_id: Some(42),
    })
}
fn finished(registry: &Registry, session: &Session) -> Report {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match registry.poll(session) {
            Poll::Finished(report) => return report,
            Poll::Missing => panic!("lost session"),
            _ => {}
        }
        assert!(Instant::now() < deadline, "worker did not drain");
        thread::sleep(Duration::from_millis(1));
    }
}
fn running(registry: &Registry, session: &Session) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(registry.poll(session),Poll::Running(snapshot) if snapshot.total>0) {
            return;
        }
        assert!(Instant::now() < deadline, "worker did not start");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn macro_worker_delay_begins_after_launch_acceptance_and_preserves_saved_bytes() {
    let mut fixture = Fixture::new();
    fixture.action(1);
    fixture.delay(2, 40);
    let before = std::fs::read(&fixture.path).unwrap();
    let completion = Mutex::new(None);
    let report = run_request(
        &fixture.request(),
        &Stop::default(),
        &Progress::new(),
        &|_, _| {
            thread::sleep(Duration::from_millis(30));
            *completion.lock().unwrap() = Some(Instant::now());
            accepted()
        },
    );
    assert_eq!(
        report,
        Report {
            outcome: Outcome::Finished,
            launches: 1,
            completed: 3,
            uncertain: false
        }
    );
    assert!(completion.into_inner().unwrap().unwrap().elapsed() >= Duration::from_millis(88));
    assert_eq!(std::fs::read(&fixture.path).unwrap(), before);
}
#[test]
fn macro_worker_late_input_or_clipboard_refuses_whole_plan_before_any_launch() {
    for text in [false, true] {
        let mut fixture = Fixture::new();
        fixture.action(1);
        fixture.edit(if text {
            Change::InsertAction {
                guid: "worker".into(),
                index: 2,
                action: Action::Text("not pasted".into()),
            }
        } else {
            Change::InsertInput {
                guid: "worker".into(),
                index: 2,
                input: Input::Keyboard("KEY_A".into()),
                gesture: Gesture::Tap,
            }
        });
        let count = AtomicUsize::new(0);
        let before = std::fs::read(&fixture.path).unwrap();
        let report = run_request(
            &fixture.request(),
            &Stop::default(),
            &Progress::new(),
            &|_, _| {
                count.fetch_add(1, Ordering::Relaxed);
                accepted()
            },
        );
        assert!(matches!(report.outcome, Outcome::Failed(_)));
        assert_eq!(report.launches, 0);
        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert_eq!(std::fs::read(&fixture.path).unwrap(), before);
    }
}
#[test]
fn macro_worker_stop_wakes_long_delay_and_releases_admission_only_after_join() {
    let mut fixture = Fixture::new();
    fixture.delay(1, 10000);
    fixture.action(2);
    let request = fixture.request();
    let registry = Registry::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let child = calls.clone();
    let session = registry
        .spawn(move |stop, progress| {
            run_request(&request, stop, progress, &|_, _| {
                child.fetch_add(1, Ordering::Relaxed);
                accepted()
            })
        })
        .unwrap();
    running(&registry, &session);
    session.stop();
    assert_eq!(finished(&registry, &session).outcome, Outcome::Stopped);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let next = registry
        .spawn(|_, progress| progress.report(Outcome::Finished, false))
        .unwrap();
    assert!(matches!(registry.poll(&session), Poll::Missing));
    assert_eq!(finished(&registry, &next).outcome, Outcome::Finished);
}
#[test]
fn macro_worker_stop_during_inflight_launch_keeps_gate_closed_and_reports_accepted_launch() {
    let mut fixture = Fixture::new();
    fixture.action(1);
    fixture.action(2);
    let request = fixture.request();
    let registry = Registry::default();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let session = registry
        .spawn(move |stop, progress| {
            run_request(&request, stop, progress, &|_, _| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                accepted()
            })
        })
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    session.stop();
    assert!(registry
        .spawn(|_, p| p.report(Outcome::Finished, false))
        .is_err());
    assert!(matches!(registry.poll(&session),Poll::Running(snapshot) if snapshot.stopping));
    release_tx.send(()).unwrap();
    let report = finished(&registry, &session);
    assert_eq!(report.outcome, Outcome::Stopped);
    assert_eq!(report.launches, 1);
    assert_eq!(report.completed, 1);
}
#[test]
fn macro_worker_readonly_and_stale_revision_do_not_execute_or_rewrite_source() {
    let mut fixture = Fixture::new();
    fixture.action(1);
    let old = fixture.request();
    fixture.edit(Change::Rename {
        guid: "worker".into(),
        name: "External change".into(),
    });
    let bytes = std::fs::read(&fixture.path).unwrap();
    let calls = AtomicUsize::new(0);
    let report = run_request(&old, &Stop::default(), &Progress::new(), &|_, _| {
        calls.fetch_add(1, Ordering::Relaxed);
        accepted()
    });
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let mut readonly = fixture.request();
    readonly.read_only = true;
    assert!(start(readonly).is_err());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), bytes);
}
#[test]
fn macro_worker_native_failure_keeps_error_and_never_dispatches_later_actions() {
    let mut fixture = Fixture::new();
    fixture.action(1);
    fixture.action(2);
    let calls = AtomicUsize::new(0);
    let stop = Stop::default();
    let progress = Progress::new();
    let report = run_request(&fixture.request(), &stop, &progress, &|_, _| {
        calls.fetch_add(1, Ordering::Relaxed);
        Err(native::Error::PathNotFound)
    });
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(report.launches, 0);
    assert!(stop.state.lock().unwrap().native.is_none());
}
#[test]
fn macro_worker_orphan_join_handle_remains_registered_and_can_be_reaped() {
    let registry = Registry::default();
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let session = registry
        .spawn(move |_, progress| {
            release_rx.recv().unwrap();
            progress.report(Outcome::Finished, false)
        })
        .unwrap();
    session.stop();
    drop(session);
    assert!(registry
        .spawn(|_, p| p.report(Outcome::Finished, false))
        .is_err());
    release_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let next = loop {
        match registry.spawn(|_, p| p.report(Outcome::Finished, false)) {
            Ok(session) => break session,
            Err(_) => {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
        }
    };
    assert_eq!(finished(&registry, &next).outcome, Outcome::Finished);
}
#[test]
fn macro_worker_panic_is_uncertain_and_not_a_false_zero_launch_success() {
    let registry = Registry::default();
    let session = registry
        .spawn(|_, progress| {
            progress.launches.store(1, Ordering::Release);
            panic!("controlled worker fixture")
        })
        .unwrap();
    let report = finished(&registry, &session);
    assert!(matches!(report.outcome, Outcome::Failed(_)));
    assert!(report.uncertain);
    assert_eq!(report.launches, 1);
}
#[test]
fn macro_worker_stop_before_preparation_skips_file_and_backend_work() {
    let registry = Registry::default();
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let session = registry
        .spawn(move |stop, progress| {
            release_rx.recv().unwrap();
            run_request(
                &Request {
                    path: PathBuf::from("missing-unused-source"),
                    guid: "unused".into(),
                    revision: "unused".into(),
                    read_only: false,
                },
                stop,
                progress,
                &|_, _| panic!("no dispatch expected"),
            )
        })
        .unwrap();
    session.stop();
    release_tx.send(()).unwrap();
    assert_eq!(finished(&registry, &session).outcome, Outcome::Stopped);
}
#[test]
fn macro_worker_registry_contention_does_not_misreport_missing_session_or_block_stop() {
    let registry = Registry::default();
    let session = registry
        .spawn(|stop, progress| {
            stop.wait(Duration::from_secs(10));
            progress.report(Outcome::Stopped, false)
        })
        .unwrap();
    let guard = registry.run.lock().unwrap();
    assert!(matches!(registry.poll(&session), Poll::Pending));
    assert!(registry
        .spawn(|_, p| p.report(Outcome::Finished, false))
        .is_err());
    session.stop();
    drop(guard);
    assert_eq!(finished(&registry, &session).outcome, Outcome::Stopped);
}
