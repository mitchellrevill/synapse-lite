//! Cached-only Lighting observations. This module never dispatches commands,
//! refreshes hardware, reads the Store, or edits profile/gesture intent.
use crate::lighting_status::Reader;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Binding {
    pub device: String,
    pub profile: String,
}
#[derive(Clone, Debug)]
pub(super) struct Request {
    sequence: u64,
    pub binding: Binding,
}
pub(super) enum Payload {
    Snapshot(Value, Instant),
    Busy,
    Unavailable,
}
pub(super) struct Response {
    pub request: Request,
    pub payload: Payload,
}
#[derive(Default)]
pub(super) struct Change {
    pub runtime: Option<Value>,
    pub faults: Vec<String>,
    pub observed_at: Option<Instant>,
}

/// Reads the separately published cache. A busy HID/Engine mutex never prevents
/// observation, and reading does not change the producer's observation time.
pub(super) fn read(reader: &Reader, request: Request) -> Response {
    let payload = match reader.read_device(&request.binding.device) {
        Some(snapshot) => Payload::Snapshot((*snapshot.value).clone(), snapshot.observed_at),
        None => Payload::Busy,
    };
    Response { request, payload }
}

pub(super) struct Observer {
    next: Instant,
    sequence: u64,
    pending: Option<u64>,
    // Independent of Feedback.new_operation, bounded to avoid growing for the
    // lifetime of the window. Stable faults remain quiet after acknowledgement.
    seen: VecDeque<u64>,
}
impl Default for Observer {
    fn default() -> Self {
        Self {
            next: Instant::now(),
            sequence: 0,
            pending: None,
            seen: VecDeque::new(),
        }
    }
}
impl Observer {
    #[cfg(test)]
    pub fn test_response(&mut self, binding: Binding, value: Value) -> Response {
        self.sequence = self.sequence.wrapping_add(1);
        self.pending = Some(self.sequence);
        Response {
            request: Request {
                sequence: self.sequence,
                binding,
            },
            payload: Payload::Snapshot(value, Instant::now()),
        }
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn begin(&mut self, binding: Binding, now: Instant) -> Option<Request> {
        if self.pending.is_some() || now < self.next {
            return None;
        }
        self.next = now + Duration::from_secs(1);
        self.sequence = self.sequence.wrapping_add(1);
        self.pending = Some(self.sequence);
        Some(Request {
            sequence: self.sequence,
            binding,
        })
    }
    pub fn finish(&mut self, response: Response, current: Option<&Binding>) -> Change {
        if self.pending != Some(response.request.sequence) {
            return Change::default();
        }
        self.pending = None;
        let binding = response.request.binding;
        if current != Some(&binding) {
            return Change::default();
        }
        let observed_at = match &response.payload {
            Payload::Snapshot(_, time) => Some(*time),
            _ => None,
        };
        let mut change = match response.payload {
            Payload::Busy => return Change::default(),
            Payload::Unavailable => Change {
                runtime: Some(unavailable("cached-worker-unavailable")),
                faults: vec!["Lighting runtime status worker is unavailable; refresh before applying settings.".into()],
                observed_at: None,
            },
            Payload::Snapshot(mut runtime, _) => {
                if runtime.is_null() {
                    return Change {
                        runtime: Some(unavailable("cached-status-not-yet-available")),
                        faults: Vec::new(),
                        observed_at,
                    };
                }
                let wrong_profile = [
                    runtime["profile"].as_str(),
                    runtime["uniform_host"]["profile"].as_str(),
                ]
                .into_iter()
                .flatten()
                .any(|profile| profile != binding.profile);
                if wrong_profile {
                    Change {
                        runtime: Some(unavailable("cached-profile-changed-refresh-required")),
                        faults: vec!["The cached Lighting runtime belongs to another profile. Refresh the device view before applying settings; local edits were retained.".into()],
                        observed_at,
                    }
                } else {
                    // Static results can outlive the effect that produced them.
                    // Do not expose another profile's old nested context.
                    let stale_static = runtime["host_static"]["profile_name"]
                        .as_str()
                        .is_some_and(|profile| profile != binding.profile);
                    if stale_static {
                        if let Some(object) = runtime.as_object_mut() {
                            object.remove("host_static");
                        }
                    }
                    let mut faults = Vec::new();
                    for (label, value) in [
                        ("Lighting policy", &runtime["error"]),
                        ("Lighting host effect", &runtime["uniform_host"]["error"]),
                        ("Lighting configuration", &runtime["uniform_host"]["configuration_error"]),
                        ("Lighting Static", &runtime["host_static"]["error"]),
                    ] {
                        if let Some(error) = value.as_str().filter(|text| !text.is_empty()) {
                            let bounded: String = error.chars().take(1024).collect();
                            faults.push(format!("{label} failed: {bounded}"));
                        }
                    }
                    let uniform = &runtime["uniform_host"];
                    let described = uniform["error"].as_str().is_some_and(|s| !s.is_empty());
                    if uniform["state"] == "faulted" && !described {
                        faults.push("Lighting host effect stopped without an error description; explicit retry is required.".into());
                    }
                    if uniform["state"] != "faulted"
                        && uniform["worker_stopped"] == true
                        && uniform["worker_enabled"] == true
                        && uniform["opted_in"] == true
                        && !described
                    {
                        faults.push("Lighting host worker stopped while enabled; explicit retry is required.".into());
                    }
                    Change { runtime: Some(runtime), faults, observed_at }
                }
            }
        };
        change.faults.retain(|message| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            binding.hash(&mut hash);
            message.hash(&mut hash);
            let entry = hash.finish();
            if self.seen.contains(&entry) {
                return false;
            }
            if self.seen.len() == 32 {
                self.seen.pop_front();
            }
            self.seen.push_back(entry);
            true
        });
        change
    }
}
fn unavailable(reason: &str) -> Value {
    json!({"available":false,"can_execute_policies":false,"error":null,"blocked_reason":reason,"cached_observation":true})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use std::sync::{Arc, Mutex};
    fn owner(profile: &str) -> Binding {
        Binding {
            device: "025D:sim".into(),
            profile: profile.into(),
        }
    }
    fn response(request: Request, value: Value) -> Response {
        Response {
            request,
            payload: Payload::Snapshot(value, Instant::now()),
        }
    }

    #[test]
    fn one_outstanding_request_and_one_second_cadence_are_enforced() {
        let mut o = Observer::default();
        let now = Instant::now();
        let request = o.begin(owner("a"), now).unwrap();
        assert!(o.begin(owner("a"), now + Duration::from_secs(3)).is_none());
        o.finish(
            Response {
                request,
                payload: Payload::Busy,
            },
            Some(&owner("a")),
        );
        assert!(o
            .begin(owner("a"), now + Duration::from_millis(999))
            .is_none());
        assert!(o.begin(owner("a"), now + Duration::from_secs(1)).is_some());
    }
    #[test]
    fn both_profile_bindings_and_late_selection_changes_are_guarded() {
        let mut o = Observer::default();
        let now = Instant::now();
        let request = o.begin(owner("a"), now).unwrap();
        let change = o.finish(
            response(
                request,
                json!({"profile":"a","uniform_host":{"profile":"b","state":"playing"}}),
            ),
            Some(&owner("a")),
        );
        assert!(change.runtime.unwrap()["uniform_host"].is_null());
        assert_eq!(change.faults.len(), 1);
        let request = o.begin(owner("a"), now + Duration::from_secs(1)).unwrap();
        let change = o.finish(
            response(request, json!({"profile":"a","error":"stale"})),
            Some(&owner("b")),
        );
        assert!(change.runtime.is_none());
        assert!(change.faults.is_empty());
        let request = o.begin(owner("b"), now + Duration::from_secs(2)).unwrap();
        let change = o.finish(
            response(
                request,
                json!({"profile":"a","uniform_host":{"profile":"b"}}),
            ),
            Some(&owner("b")),
        );
        assert!(change.runtime.unwrap()["uniform_host"].is_null());
        assert_eq!(change.faults.len(), 1);
    }
    #[test]
    fn background_fault_remains_deduplicated_after_user_feedback_reset() {
        let mut o = Observer::default();
        let now = Instant::now();
        let mut feedback = super::super::feedback::Failures::default();
        for step in 0..3 {
            let request = o
                .begin(owner("a"), now + Duration::from_secs(step))
                .unwrap();
            let change=o.finish(response(request,json!({"profile":"a","uniform_host":{"profile":"a","state":"faulted","error":"packet rejected"}})),Some(&owner("a")));
            if step == 0 {
                assert_eq!(change.faults.len(), 1);
                assert!(feedback.record(&change.faults[0]));
                assert!(feedback.begin().is_some());
                feedback.acknowledge();
            } else {
                assert!(change.faults.is_empty());
            }
            feedback.new_operation();
        }
        assert!(!feedback.outstanding());
    }
    #[test]
    fn cached_worker_reads_no_hid_and_never_parses_invalid_profile_file() {
        let directory = std::env::temp_dir().join(format!(
            "synapse-cached-status-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("profiles.json");
        let sim = secbird_core::sim::Sim::keyboard();
        let engine = Engine::with_simulated_keyboard(&sim, file.clone()).unwrap();
        let binding = Binding {
            device: engine.devices()[0].key(),
            profile: "Default".into(),
        };
        std::fs::write(&file, b"intentionally invalid JSON").unwrap();
        let commands = sim.state().log.len();
        let engine = Arc::new(Mutex::new(engine));
        let (publisher, reader) = crate::lighting_status::channel();
        publisher.publish(json!({binding.device.clone():{"profile":"Default","uniform_host":{"profile":"Default","accepted_frames":3}}}));
        let first = reader.read_device(&binding.device).unwrap().observed_at;
        let lock = engine.lock().unwrap();
        for sequence in 1..=20 {
            let reply = read(
                &reader,
                Request {
                    sequence,
                    binding: binding.clone(),
                },
            );
            match reply.payload {
                Payload::Snapshot(value, observed_at) => {
                    assert_eq!(
                        observed_at, first,
                        "consumption cannot refresh producer time"
                    );
                    assert_eq!(value["uniform_host"]["accepted_frames"], 3);
                }
                _ => panic!("held Engine must not starve separately cached observation"),
            }
        }
        // Publishing a completed actor observation while Engine remains held
        // must become visible immediately, without any hardware/Store work.
        publisher.publish(json!({binding.device.clone():{"profile":"Default","uniform_host":{"profile":"Default","accepted_frames":7}}}));
        let reply = read(
            &reader,
            Request {
                sequence: 21,
                binding,
            },
        );
        match reply.payload {
            Payload::Snapshot(value, observed_at) => {
                assert!(observed_at >= first);
                assert_eq!(value["uniform_host"]["accepted_frames"], 7);
            }
            _ => panic!("new publication was starved by Engine"),
        }
        assert_eq!(sim.state().log.len(), commands);
        assert_eq!(std::fs::read(file).unwrap(), b"intentionally invalid JSON");
        drop(lock);
    }
}
