//! Bounded presentation cache, independent of the Engine/HID mutex.
use crate::uniform_worker::Signal;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub value: Arc<Value>,
    pub observed_at: Instant,
}
#[derive(Default)]
struct State {
    value: Option<Arc<Value>>,
    observed_at: Option<Instant>,
    faulted: bool,
    // Retain one small signal tombstone so clearing Engine sessions cannot
    // resurrect an old healthy snapshot after stop/panic. It owns no thread.
    signal: Option<Arc<Signal>>,
    published_signal: Option<Arc<Signal>>,
    actor_generation: u64,
}
#[derive(Clone)]
pub(crate) struct Publisher(Arc<Mutex<State>>);
#[derive(Clone)]
pub(crate) struct Reader(Arc<Mutex<State>>);
pub(crate) fn channel() -> (Publisher, Reader) {
    let shared = Arc::new(Mutex::new(State::default()));
    (Publisher(shared.clone()), Reader(shared))
}
impl Publisher {
    pub(crate) fn attach_signal(&self, signal: Arc<Signal>) {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let replacement = state
            .signal
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &signal));
        if replacement {
            state.actor_generation = state.actor_generation.wrapping_add(1);
            state.value = None;
            state.observed_at = None;
            state.published_signal = None;
        }
        state.signal = Some(signal);
    }
    pub(crate) fn due(&self, now: Instant, faulted: bool) -> bool {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state
            .observed_at
            .is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(1))
            || state.faulted != faulted
    }
    #[cfg(test)]
    pub(crate) fn publish(&self, value: Value) {
        self.publish_at(value, Instant::now(), false);
    }
    pub(crate) fn publish_at(&self, value: Value, observed_at: Instant, faulted: bool) {
        let value = Arc::new(value);
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state.published_signal = state.signal.clone();
        state.value = Some(value);
        state.observed_at = Some(observed_at);
        state.faulted = faulted;
    }
}
impl Reader {
    pub(crate) fn read_device(&self, key: &str) -> Option<Snapshot> {
        let (all, observed_at, signal, generation) = {
            let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
            (
                state.value.clone()?,
                state.observed_at?,
                state.published_signal.clone(),
                state.actor_generation,
            )
        };
        // All JSON copying and live-signal sampling occur outside the tiny cache
        // lock. Reading never acquires Engine or refreshes the producer timestamp.
        let mut value = all.get(key)?.clone();
        if let Some(signal) = signal {
            let running = signal.running();
            value["worker_liveness_sampled_separately"] = json!(true);
            if let Some(uniform) = value.get_mut("uniform_host") {
                uniform["worker_enabled"] = json!(running);
                if !running {
                    uniform["worker_stopped"] = json!(!signal.thread_alive());
                    uniform["quiesce_requested"] = json!(signal.quiescing());
                    uniform["streaming"] = json!(false);
                    if signal.unexpected_exit() && uniform["error"].is_null() {
                        uniform["error"] = json!("Uniform worker exited unexpectedly; cached packet counts remain producer observations");
                    }
                }
            }
            if let Some(studio) = value.get_mut("studio") {
                studio["worker_enabled"] = json!(running);
                if !running {
                    studio["worker_stopped"] = json!(!signal.thread_alive());
                    studio["runtime_error"] = json!("Lighting worker is stopped; cached frames do not prove current playback");
                }
            }
        }
        value["worker_actor_generation"] = json!(generation);
        if self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .actor_generation
            != generation
        {
            return None;
        }
        Some(Snapshot {
            value: Arc::new(value),
            observed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_actor_cannot_reuse_an_old_healthy_publication() {
        let (publisher, reader) = channel();
        let old = Signal::running_for_test();
        publisher.attach_signal(old.clone());
        publisher.publish(
            json!({"one":{"uniform_host":{"worker_enabled":true,"streaming":true,"error":null}}}),
        );
        let old_generation =
            reader.read_device("one").unwrap().value["worker_actor_generation"].clone();
        old.stop_for_test();
        let replacement = Signal::running_for_test();
        publisher.attach_signal(replacement);
        assert!(
            reader.read_device("one").is_none(),
            "new live actor must publish its own observation"
        );
        publisher.publish(
            json!({"one":{"uniform_host":{"worker_enabled":true,"streaming":true,"error":null}}}),
        );
        let current = reader.read_device("one").unwrap();
        assert_ne!(current.value["worker_actor_generation"], old_generation);
        assert_eq!(current.value["uniform_host"]["worker_enabled"], true);
    }
    #[test]
    fn healthy_publication_is_throttled_but_fault_transitions_are_immediate() {
        let (publisher, _) = channel();
        let now = Instant::now();
        publisher.publish_at(json!({}), now, false);
        assert!(!publisher.due(now + Duration::from_millis(900), false));
        assert!(publisher.due(now + Duration::from_secs(1), false));
        assert!(publisher.due(now + Duration::from_millis(1), true));
        publisher.publish_at(json!({}), now, true);
        assert!(!publisher.due(now + Duration::from_millis(900), true));
        assert!(publisher.due(now + Duration::from_millis(1), false));
    }
    #[test]
    fn reads_preserve_producer_time_bind_device_and_need_no_engine_lock() {
        let (publisher, reader) = channel();
        let observed = Instant::now() - Duration::from_secs(2);
        publisher.publish_at(
            json!({"one":{"profile":"A"},"two":{"profile":"B"}}),
            observed,
            false,
        );
        let unrelated_engine = Mutex::new(());
        let _held = unrelated_engine.lock().unwrap();
        let value = reader.read_device("one").unwrap();
        assert_eq!(value.observed_at, observed);
        assert_eq!(value.value["profile"], "A");
        assert!(reader.read_device("missing").is_none());
        assert_eq!(reader.read_device("two").unwrap().value["profile"], "B");
        assert_eq!(reader.read_device("one").unwrap().observed_at, observed);
    }
    #[test]
    fn fault_publication_replaces_old_healthy_value_and_dead_signal_cannot_resurrect_it() {
        let (publisher, reader) = channel();
        let signal = Signal::running_for_test();
        publisher.attach_signal(signal.clone());
        publisher.publish(
            json!({"one":{"uniform_host":{"worker_enabled":true,"streaming":true,"error":null}}}),
        );
        signal.stop_for_test();
        assert_eq!(
            reader.read_device("one").unwrap().value["uniform_host"]["worker_enabled"],
            false
        );
        assert_eq!(
            reader.read_device("one").unwrap().value["uniform_host"]["streaming"],
            false
        );
        publisher.publish(json!({"one":{"uniform_host":{"error":"genuine partial OUT","accepted_packets":2,"attempted_packets":3}}}));
        let stopped = reader.read_device("one").unwrap();
        assert_eq!(
            stopped.value["uniform_host"]["error"],
            "genuine partial OUT"
        );
        assert_eq!(stopped.value["uniform_host"]["accepted_packets"], 2);
        assert_eq!(stopped.value["uniform_host"]["attempted_packets"], 3);
    }
    #[test]
    fn stopped_signal_invalidates_cached_studio_liveness_without_erasing_frame_counts() {
        let (publisher,reader)=channel();
        let signal=Signal::running_for_test();
        publisher.attach_signal(signal.clone());
        publisher.publish(json!({"one":{"studio":{"state":"applied","worker_enabled":true,"accepted_frames":25,"drained":false}}}));
        signal.stop_for_test();
        let stopped=reader.read_device("one").unwrap();
        assert_eq!(stopped.value["studio"]["worker_enabled"],false);
        assert_eq!(stopped.value["studio"]["accepted_frames"],25);
        assert_eq!(stopped.value["studio"]["drained"],false,"a liveness read must not claim batch drain");
        assert!(stopped.value["studio"]["runtime_error"].is_string());
    }
}
