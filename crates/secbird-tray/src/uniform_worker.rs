//! One cooperative, joinable off-UI uniform worker. There is no per-frame spawn.
use crate::engine::Engine;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, TryLockError,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) struct Signal {
    alive: AtomicBool,
    stop: AtomicBool,
}
impl Signal {
    pub(crate) fn running(&self) -> bool {
        self.alive.load(Ordering::Acquire) && !self.stop.load(Ordering::Acquire)
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
        })
    }
}
struct AliveGuard(Arc<Signal>);
impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.0.alive.store(false, Ordering::Release);
    }
}
pub(crate) struct Worker {
    signal: Arc<Signal>,
    handle: Option<JoinHandle<()>>,
}
fn try_tick<T>(
    shared: &Mutex<T>,
    stop: &AtomicBool,
    tick: impl FnOnce(&mut T) -> Duration,
) -> Duration {
    match shared.try_lock() {
        Ok(mut value) => tick(&mut value),
        Err(TryLockError::WouldBlock) => Duration::from_millis(10),
        Err(TryLockError::Poisoned(_)) => {
            stop.store(true, Ordering::Release);
            Duration::from_millis(250)
        }
    }
}
impl Worker {
    pub(crate) fn start(engine: Arc<Mutex<Engine>>) -> Result<Self, String> {
        let shared = engine.clone();
        let mut worker =
            Self::spawn(move |stop| try_tick(&shared, stop, |engine| engine.tick_uniform(stop)))?;
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
    fn spawn(
        mut tick: impl FnMut(&AtomicBool) -> Duration + Send + 'static,
    ) -> Result<Self, String> {
        let signal = Arc::new(Signal {
            alive: AtomicBool::new(false),
            stop: AtomicBool::new(false),
        });
        let cancelled = signal.clone();
        let handle = thread::Builder::new()
            .name("Ornata uniform".into())
            .spawn(move || {
                let _alive = AliveGuard(cancelled.clone());
                cancelled.alive.store(true, Ordering::Release);
                while !cancelled.stop.load(Ordering::Acquire) {
                    let delay = tick(&cancelled.stop)
                        .clamp(Duration::from_millis(8), Duration::from_millis(250));
                    if cancelled.stop.load(Ordering::Acquire) {
                        break;
                    }
                    // At least 8ms between batches gives pending UI/server lock users
                    // an opportunity. Timing is a local policy, not certified 25fps.
                    thread::park_timeout(delay);
                }
            })
            .map_err(|e| format!("Uniform worker could not start: {e}"))?;
        Ok(Self {
            signal,
            handle: Some(handle),
        })
    }
    /// Call outside the Engine lock. In-flight HID syscalls cannot be interrupted;
    /// on timeout the caller must not block acquiring that same engine for cleanup.
    pub(crate) fn stop(&mut self, timeout: Duration) -> bool {
        self.signal.stop.store(true, Ordering::Release);
        let Some(handle) = &self.handle else {
            return true;
        };
        handle.thread().unpark();
        let deadline = Instant::now() + timeout;
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(
                Duration::from_millis(5).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        self.handle.take().unwrap().join().is_ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop(Duration::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, mpsc, Condvar};
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
        assert!(poison.stop(Duration::from_secs(1)));
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
