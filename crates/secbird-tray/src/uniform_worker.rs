//! One cooperative, joinable off-UI uniform worker. There is no per-frame spawn.
use crate::engine::Engine;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Arc, Mutex, TryLockError,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Metrics {
    ticks_started: AtomicU64,
    ticks_completed: AtomicU64,
    lock_skips: AtomicU64,
    tick_us_total: AtomicU64,
    last_tick_us: AtomicU64,
    max_tick_us: AtomicU64,
    park_calls: AtomicU64,
    park_requested_us_total: AtomicU64,
    park_actual_us_total: AtomicU64,
    last_park_requested_us: AtomicU64,
    last_park_actual_us: AtomicU64,
    max_park_actual_us: AtomicU64,
    batches_admitted: AtomicU64,
    batches_completed: AtomicU64,
    batches_abandoned: AtomicU64,
    last_batch_accepted: AtomicU64,
    last_batch_attempted: AtomicU64,
    last_batch_counts_known: AtomicBool,
    last_batch_failed: AtomicBool,
    stop_timeouts: AtomicU64,
}
impl Metrics {
    fn tick_completed(&self, elapsed: Duration) {
        let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.ticks_completed.fetch_add(1, Ordering::Relaxed);
        self.tick_us_total.fetch_add(micros, Ordering::Relaxed);
        self.last_tick_us.store(micros, Ordering::Relaxed);
        self.max_tick_us.fetch_max(micros, Ordering::Relaxed);
    }
    fn parked(&self, requested: Duration, actual: Duration) {
        let requested = requested.as_micros().min(u128::from(u64::MAX)) as u64;
        let actual = actual.as_micros().min(u128::from(u64::MAX)) as u64;
        self.park_calls.fetch_add(1, Ordering::Relaxed);
        self.park_requested_us_total
            .fetch_add(requested, Ordering::Relaxed);
        self.park_actual_us_total
            .fetch_add(actual, Ordering::Relaxed);
        self.last_park_requested_us
            .store(requested, Ordering::Relaxed);
        self.last_park_actual_us.store(actual, Ordering::Relaxed);
        self.max_park_actual_us.fetch_max(actual, Ordering::Relaxed);
    }
    fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "ticks_started":self.ticks_started.load(Ordering::Relaxed),
            "ticks_completed":self.ticks_completed.load(Ordering::Relaxed),
            "engine_lock_skips":self.lock_skips.load(Ordering::Relaxed),
            "tick_us_total":self.tick_us_total.load(Ordering::Relaxed),
            "last_tick_us":self.last_tick_us.load(Ordering::Relaxed),
            "max_tick_us":self.max_tick_us.load(Ordering::Relaxed),
            "park_calls":self.park_calls.load(Ordering::Relaxed),
            "park_requested_us_total":self.park_requested_us_total.load(Ordering::Relaxed),
            "park_actual_us_total":self.park_actual_us_total.load(Ordering::Relaxed),
            "last_park_requested_us":self.last_park_requested_us.load(Ordering::Relaxed),
            "last_park_actual_us":self.last_park_actual_us.load(Ordering::Relaxed),
            "max_park_actual_us":self.max_park_actual_us.load(Ordering::Relaxed),
            "diagnostic_snapshot_atomic_fields_not_simultaneous":true,
            "wait_policy":"unchanged-park_timeout-minimum8ms-no-timer-resolution-change",
            "batches_admitted":self.batches_admitted.load(Ordering::Relaxed),
            "batches_completed":self.batches_completed.load(Ordering::Relaxed),
            "batches_abandoned":self.batches_abandoned.load(Ordering::Relaxed),
            "last_batch_accepted_packets":self.last_batch_counts_known.load(Ordering::Acquire).then(||self.last_batch_accepted.load(Ordering::Relaxed)),
            "last_batch_attempted_packets":self.last_batch_counts_known.load(Ordering::Acquire).then(||self.last_batch_attempted.load(Ordering::Relaxed)),
            "last_batch_failed":self.last_batch_failed.load(Ordering::Relaxed),
            "stop_timeouts":self.stop_timeouts.load(Ordering::Relaxed)
        })
    }
}

const QUIESCE: u8 = 1;
const IN_BATCH: u8 = 2;
pub(crate) struct Signal {
    alive: AtomicBool,
    stop: AtomicBool,
    admission: AtomicU8,
    unexpected_exit: AtomicBool,
    metrics: Metrics,
}
/// A local worker scheduling admission, not a device ownership/restore token.
pub(crate) struct BatchAdmission {
    signal: Arc<Signal>,
    recorded: bool,
}
impl BatchAdmission {
    pub(crate) fn record(&mut self, completion: &crate::host_uniform::Completion) {
        let metrics = &self.signal.metrics;
        metrics
            .last_batch_accepted
            .store(completion.accepted_packets as u64, Ordering::Relaxed);
        metrics
            .last_batch_attempted
            .store(completion.attempted_packets as u64, Ordering::Relaxed);
        metrics
            .last_batch_failed
            .store(completion.error.is_some(), Ordering::Relaxed);
        metrics
            .last_batch_counts_known
            .store(true, Ordering::Release);
        metrics.batches_completed.fetch_add(1, Ordering::Relaxed);
        self.recorded = true;
    }
}
impl Drop for BatchAdmission {
    fn drop(&mut self) {
        if !self.recorded {
            self.signal
                .metrics
                .batches_abandoned
                .fetch_add(1, Ordering::Relaxed);
            self.signal
                .metrics
                .last_batch_counts_known
                .store(false, Ordering::Release);
            self.signal
                .metrics
                .last_batch_failed
                .store(true, Ordering::Relaxed);
        }
        self.signal
            .admission
            .fetch_and(!IN_BATCH, Ordering::Release);
    }
}
impl Signal {
    pub(crate) fn metrics(&self) -> serde_json::Value {
        let mut value = self.metrics.status();
        value["quiesce_requested"] = serde_json::json!(self.quiescing());
        value["cancel_requested"] = serde_json::json!(self.stop.load(Ordering::Acquire));
        value["batch_active"] =
            serde_json::json!(self.admission.load(Ordering::Acquire) & IN_BATCH != 0);
        value["thread_alive"] = serde_json::json!(self.thread_alive());
        value["unexpected_exit"] = serde_json::json!(self.unexpected_exit());
        value
    }
    pub(crate) fn thread_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    pub(crate) fn running(&self) -> bool {
        self.alive.load(Ordering::Acquire)
            && !self.stop.load(Ordering::Acquire)
            && !self.quiescing()
    }
    pub(crate) fn quiescing(&self) -> bool {
        self.admission.load(Ordering::Acquire) & QUIESCE != 0
    }
    pub(crate) fn unexpected_exit(&self) -> bool {
        self.unexpected_exit.load(Ordering::Acquire)
    }
    pub(crate) fn begin_batch(self: &Arc<Self>) -> Option<BatchAdmission> {
        if self.stop.load(Ordering::Acquire) {
            return None;
        }
        self.admission
            .compare_exchange(0, IN_BATCH, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        self.metrics
            .last_batch_counts_known
            .store(false, Ordering::Release);
        self.metrics
            .batches_admitted
            .fetch_add(1, Ordering::Relaxed);
        Some(BatchAdmission {
            signal: self.clone(),
            recorded: false,
        })
    }
    fn request_quiesce(&self) {
        self.admission.fetch_or(QUIESCE, Ordering::AcqRel);
    }

    #[cfg(test)]
    pub(crate) fn stop_for_test(&self) {
        self.stop.store(true, Ordering::Release);
    }
    #[cfg(test)]
    pub(crate) fn running_for_test() -> Arc<Self> {
        Arc::new(Self {
            alive: AtomicBool::new(true),
            stop: AtomicBool::new(false),
            admission: AtomicU8::new(0),
            unexpected_exit: AtomicBool::new(false),
            metrics: Metrics::default(),
        })
    }
}
struct AliveGuard(Arc<Signal>);
impl Drop for AliveGuard {
    fn drop(&mut self) {
        if !self.0.quiescing() || std::thread::panicking() {
            self.0.unexpected_exit.store(true, Ordering::Release);
        }
        self.0.alive.store(false, Ordering::Release);
    }
}
pub(crate) struct Worker {
    signal: Arc<Signal>,
    handle: Option<JoinHandle<()>>,
}
fn try_tick<T>(
    shared: &Mutex<T>,
    signal: &Signal,
    tick: impl FnOnce(&mut T) -> Duration,
) -> Duration {
    match shared.try_lock() {
        Ok(mut value) => tick(&mut value),
        Err(TryLockError::WouldBlock) => {
            signal.metrics.lock_skips.fetch_add(1, Ordering::Relaxed);
            Duration::from_millis(10)
        }
        Err(TryLockError::Poisoned(_)) => {
            signal.stop.store(true, Ordering::Release);
            Duration::from_millis(250)
        }
    }
}
impl Worker {
    pub(crate) fn start(engine: Arc<Mutex<Engine>>) -> Result<Self, String> {
        if engine.lock().map_err(|_| "Read-only worker admission could not inspect poisoned Engine")?.is_measurement_read_only() {
            return Err("Read-only measurement cannot start the uniform worker".into());
        }
        let shared = engine.clone();
        let mut worker = Self::spawn(move |signal| {
            try_tick(&shared, signal, |engine| {
                let wait = engine.tick_uniform(&signal.stop);
                engine.publish_lighting_status(signal.quiescing());
                wait
            })
        })?;
        let wake = worker.handle.as_ref().unwrap().thread().clone();
        match engine.lock() {
            Ok(mut engine) => {
                engine.set_uniform_worker(true, Some(wake.clone()), Some(worker.signal.clone()));
                wake.unpark();
            }
            Err(_) => {
                worker.stop(Duration::from_secs(1));
                return Err("Uniform worker could not initialize a poisoned Engine".into());
            }
        }
        Ok(worker)
    }
    fn spawn(mut tick: impl FnMut(&Signal) -> Duration + Send + 'static) -> Result<Self, String> {
        let signal = Arc::new(Signal {
            alive: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            admission: AtomicU8::new(0),
            unexpected_exit: AtomicBool::new(false),
            metrics: Metrics::default(),
        });
        let cancelled = signal.clone();
        let handle = thread::Builder::new()
            .name("Ornata uniform".into())
            .spawn(move || {
                let _alive = AliveGuard(cancelled.clone());
                cancelled.alive.store(true, Ordering::Release);
                while !cancelled.stop.load(Ordering::Acquire) && !cancelled.quiescing() {
                    cancelled
                        .metrics
                        .ticks_started
                        .fetch_add(1, Ordering::Relaxed);
                    let tick_started = Instant::now();
                    let delay = tick(&cancelled)
                        .clamp(Duration::from_millis(8), Duration::from_millis(250));
                    cancelled.metrics.tick_completed(tick_started.elapsed());
                    if cancelled.stop.load(Ordering::Acquire) || cancelled.quiescing() {
                        break;
                    }
                    // At least 8ms between batches gives pending UI/server lock users
                    // an opportunity. Timing is a local policy, not certified 25fps.
                    let park_started = Instant::now();
                    thread::park_timeout(delay);
                    cancelled.metrics.parked(delay, park_started.elapsed());
                }
            })
            .map_err(|e| format!("Uniform worker could not start: {e}"))?;
        Ok(Self {
            signal,
            handle: Some(handle),
        })
    }
    pub(crate) fn log_shutdown_diagnostics(&self) {
        secbird_core::log::append(&format!(
            "uniform worker stop diagnostics: {}",
            self.signal.metrics()
        ));
    }
    #[cfg(test)]
    pub(crate) fn signal_for_test(&self) -> Arc<Signal> {
        self.signal.clone()
    }
    /// Close local admission and let the current batch finish. Call outside the
    /// Engine lock. Only timeout requests immediate cooperative cancellation;
    /// HID syscalls cannot be interrupted. On timeout do not acquire Engine for
    /// cleanup. A joined thread does not erase any recorded frame failure.
    pub(crate) fn stop(&mut self, timeout: Duration) -> bool {
        self.signal.request_quiesce();
        let Some(handle) = &self.handle else {
            return !self.signal.unexpected_exit();
        };
        handle.thread().unpark();
        let deadline = Instant::now() + timeout;
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                self.signal
                    .metrics
                    .stop_timeouts
                    .fetch_add(1, Ordering::Relaxed);
                self.signal.stop.store(true, Ordering::Release);
                handle.thread().unpark();
                return false;
            }
            thread::sleep(
                Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        self.handle.take().unwrap().join().is_ok() && !self.signal.unexpected_exit()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.signal.request_quiesce();
        self.signal.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, mpsc, Condvar};
    #[test]
    fn diagnostic_wait_aggregates_keep_requested_and_actual_times_distinct() {
        let measured = Metrics::default();
        measured.parked(Duration::from_millis(8), Duration::from_millis(12));
        measured.parked(Duration::from_millis(250), Duration::from_millis(1));
        measured.tick_completed(Duration::from_micros(75));
        let value = measured.status();
        assert_eq!(value["park_calls"], 2);
        assert_eq!(value["park_requested_us_total"], 258000);
        assert_eq!(value["park_actual_us_total"], 13000);
        assert_eq!(value["last_park_requested_us"], 250000);
        assert_eq!(value["last_park_actual_us"], 1000);
        assert_eq!(value["max_park_actual_us"], 12000);
        assert_eq!(value["tick_us_total"], 75);
    }
    #[test]
    fn quiesce_closes_local_admission_without_cancelling_an_admitted_batch() {
        let signal = Signal::running_for_test();
        let mut batch = signal.begin_batch().unwrap();
        signal.request_quiesce();
        assert!(!signal.running());
        assert!(!signal.stop.load(Ordering::Acquire));
        assert!(signal.begin_batch().is_none());
        batch.record(&crate::host_uniform::Completion {
            accepted_packets: 7,
            attempted_packets: 7,
            error: None,
        });
        drop(batch);
        assert!(signal.begin_batch().is_none());
        assert_eq!(signal.metrics()["batch_active"], false);
        assert_eq!(signal.metrics()["last_batch_accepted_packets"], 7);
        for _ in 0..32 {
            let signal = Signal::running_for_test();
            let rendezvous = Arc::new(std::sync::Barrier::new(2));
            let other = signal.clone();
            let wait = rendezvous.clone();
            let admitted = thread::spawn(move || {
                wait.wait();
                other.begin_batch()
            });
            rendezvous.wait();
            signal.request_quiesce();
            drop(admitted.join().unwrap());
            assert!(signal.begin_batch().is_none());
            assert!(signal.metrics()["batches_admitted"].as_u64().unwrap() <= 1);
        }
    }
    #[test]
    fn last_healthy_publication_cannot_hide_an_actual_worker_panic() {
        let (publisher, reader) = crate::lighting_status::channel();
        let (release, wait) = mpsc::channel();
        let mut worker = Worker::spawn(move |_| {
            wait.recv().unwrap();
            panic!("injected after healthy publication")
        })
        .unwrap();
        publisher.attach_signal(worker.signal.clone());
        publisher.publish(serde_json::json!({"one":{"uniform_host":{"worker_enabled":true,"streaming":true,"error":null}}}));
        let observed = reader.read_device("one").unwrap().observed_at;
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while !worker.handle.as_ref().unwrap().is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!worker.stop(Duration::from_secs(1)));
        let snapshot = reader.read_device("one").unwrap();
        assert_eq!(snapshot.observed_at, observed);
        assert_eq!(snapshot.value["uniform_host"]["worker_enabled"], false);
        assert_eq!(snapshot.value["uniform_host"]["worker_stopped"], true);
        assert!(snapshot.value["uniform_host"]["error"]
            .as_str()
            .unwrap()
            .contains("unexpectedly"));
    }
    #[test]
    fn one_worker_runs_multiple_ticks_and_joins_once() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counted = ticks.clone();
        let mut worker = Worker::spawn(move |_| {
            counted.fetch_add(1, Ordering::Relaxed);
            Duration::from_millis(1)
        })
        .unwrap();
        let limit = Instant::now() + Duration::from_secs(1);
        while ticks.load(Ordering::Relaxed) < 3 && Instant::now() < limit {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(worker.stop(Duration::from_secs(1)));
        let count = ticks.load(Ordering::Relaxed);
        assert!(count >= 3);
        let measured = worker.signal.metrics();
        assert_eq!(measured["ticks_completed"], count);
        assert_eq!(measured["ticks_started"], count);
        assert!(measured["park_calls"].as_u64().unwrap() > 0);
        assert_eq!(measured["last_park_requested_us"], 8000);
        assert!(worker.stop(Duration::ZERO));
        assert_eq!(ticks.load(Ordering::Relaxed), count);
    }
    #[test]
    fn busy_ui_lock_is_skipped_instead_of_waited_on() {
        let shared = Arc::new(Mutex::new(0usize));
        let held = shared.lock().unwrap();
        let checked = shared.clone();
        let ticks = Arc::new(AtomicUsize::new(0));
        let counted = ticks.clone();
        let mut worker = Worker::spawn(move |stop| {
            try_tick(&checked, stop, |value| {
                *value += 1;
                counted.fetch_add(1, Ordering::Relaxed);
                Duration::from_millis(8)
            })
        })
        .unwrap();
        thread::sleep(Duration::from_millis(30));
        assert_eq!(ticks.load(Ordering::Relaxed), 0);
        drop(held);
        let limit = Instant::now() + Duration::from_secs(1);
        while ticks.load(Ordering::Relaxed) == 0 && Instant::now() < limit {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(worker.stop(Duration::from_secs(1)));
        assert!(ticks.load(Ordering::Relaxed) > 0);
        assert!(
            worker.signal.metrics()["engine_lock_skips"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    #[test]
    fn normal_stop_panic_and_poison_never_leave_false_worker_ready() {
        let mut normal = Worker::spawn(|_| Duration::from_millis(8)).unwrap();
        let limit = Instant::now() + Duration::from_secs(1);
        while !normal.signal.running() && Instant::now() < limit {
            thread::yield_now();
        }
        assert!(normal.signal.running());
        assert!(normal.stop(Duration::from_secs(1)));
        assert!(!normal.signal.running());
        let mut panic = Worker::spawn(|_| panic!("injected worker failure")).unwrap();
        let limit = Instant::now() + Duration::from_secs(1);
        while !panic.handle.as_ref().unwrap().is_finished() && Instant::now() < limit {
            thread::yield_now();
        }
        assert!(!panic.signal.running());
        assert!(!panic.stop(Duration::from_secs(1)));
        assert!(!panic.signal.running());
        let value = Arc::new(Mutex::new(0));
        let poisoned = value.clone();
        let _ = thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("injected mutex poison");
        })
        .join();
        let mut poison =
            Worker::spawn(move |stop| try_tick(&value, stop, |_| Duration::from_millis(8)))
                .unwrap();
        let limit = Instant::now() + Duration::from_secs(1);
        while !poison.handle.as_ref().unwrap().is_finished() && Instant::now() < limit {
            thread::yield_now();
        }
        assert!(!poison.signal.running());
        assert!(
            !poison.stop(Duration::from_secs(1)),
            "poisoned Engine exit is not a clean stop"
        );
    }
    #[test]
    fn stop_timeout_does_not_join_an_uninterruptible_call_or_take_engine_lock() {
        let blocked = Arc::new((Mutex::new(false), Condvar::new()));
        let release = blocked.clone();
        let (started, seen) = mpsc::channel();
        let mut worker = Worker::spawn(move |_| {
            started.send(()).ok();
            let (lock, wake) = &*release;
            let mut done = lock.lock().unwrap();
            while !*done {
                done = wake.wait(done).unwrap();
            }
            Duration::from_millis(8)
        })
        .unwrap();
        seen.recv_timeout(Duration::from_secs(1)).unwrap();
        let now = Instant::now();
        assert!(!worker.stop(Duration::from_millis(10)));
        assert!(now.elapsed() < Duration::from_millis(100));
        {
            let (lock, wake) = &*blocked;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
        assert!(worker.stop(Duration::from_secs(1)));
    }
}
