//! Opted-in uniform playback state. Scheduling is local, not a recovered clock.
use super::*;
use crate::host_uniform::{self, ActionTrace, Availability, Completion, Playback, Poll, Program};
use crate::lighting_runtime::Config;
use secbird_core::{Error, Profile};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

struct Session {
    profile: String,
    program: Program,
    playback: Playback,
    controls: host_uniform::io::StreamState,
    prepared: bool,
    pending_live: bool,
    mode: Option<u8>,
    status: Value,
    started: Instant,
}
#[derive(Default)]
struct StoreCache {
    path: Option<PathBuf>,
    stamp: Option<(Option<SystemTime>, u64)>,
    store: Option<Store>,
    error: Option<String>,
    reloads: u64,
}
impl StoreCache {
    fn refresh(&mut self, path: &Path) -> std::result::Result<&Store, String> {
        let stamp = match std::fs::metadata(path) {
            Ok(m) => Some((m.modified().ok(), m.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if self.path.as_deref() != Some(path)
            || self.stamp != stamp
            || self.store.is_none() && self.error.is_none()
        {
            self.reloads = self.reloads.saturating_add(1);
            self.path = Some(path.into());
            self.stamp = stamp;
            match Store::load(path).and_then(|s| {
                secbird_core::profile_compat::validate_store_budget(&s)?;
                Ok(s)
            }) {
                Ok(store) => {
                    self.store = Some(store);
                    self.error = None;
                }
                Err(error) => {
                    self.store = None;
                    self.error = Some(error.to_string());
                }
            }
        }
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.store.as_ref().expect("successful cache refresh"))
    }
    fn invalidate(&mut self) {
        self.path = None;
    }
}
struct Fault {
    error: String,
    completion: Option<Completion>,
}
pub(super) struct UniformState {
    pub enabled: bool,
    alive: Option<std::sync::Arc<crate::uniform_worker::Signal>>,
    epoch: Instant,
    generation: u64,
    sessions: HashMap<String, Session>,
    faults: HashMap<String, Fault>,
    cache: StoreCache,
    pub wake: Option<std::thread::Thread>,
    error: Option<String>,
}
impl Default for UniformState {
    fn default() -> Self {
        Self {
            enabled: false,
            alive: None,
            epoch: Instant::now(),
            generation: 0,
            sessions: HashMap::new(),
            faults: HashMap::new(),
            cache: StoreCache::default(),
            wake: None,
            error: None,
        }
    }
}
impl UniformState {
    fn session(&mut self, key: &str, name: &str, program: Program) -> &mut Session {
        let new = self
            .sessions
            .get(key)
            .is_none_or(|s| s.profile != name || s.program != program);
        if new {
            self.generation = self.generation.wrapping_add(1);
            self.sessions.insert(
                key.into(),
                Session {
                    profile: name.into(),
                    program,
                    playback: Playback::new(program, self.generation, self.epoch.elapsed()),
                    controls: host_uniform::io::StreamState::default(),
                    prepared: false,
                    pending_live: true,
                    mode: None,
                    status: json!({"state":"pending-first-frame"}),
                    started: Instant::now(),
                },
            );
        }
        self.sessions.get_mut(key).unwrap()
    }
    fn invalidate(&mut self) {
        self.cache.invalidate();
        if let Some(wake) = &self.wake {
            wake.unpark();
        }
    }
}
#[derive(Clone)]
struct Candidate {
    key: String,
    name: String,
    program: Program,
    opted: bool,
    config: Config,
}
impl Engine {
    fn uniform_path(&self) -> PathBuf {
        #[cfg(test)]
        if let Some(path) = &self.runtime_store_path {
            return path.clone();
        }
        config_dir().join("profiles.json")
    }
    #[cfg(test)]
    pub(super) fn seed_unchanged_uniform_cache_for_test(&mut self, approved: Store) {
        let path = self.uniform_path();
        let metadata = std::fs::metadata(&path).unwrap();
        self.uniform.cache.path = Some(path);
        self.uniform.cache.stamp = Some((metadata.modified().ok(), metadata.len()));
        self.uniform.cache.store = Some(approved);
        self.uniform.cache.error = None;
    }
    pub(crate) fn set_uniform_worker(
        &mut self,
        enabled: bool,
        wake: Option<std::thread::Thread>,
        alive: Option<std::sync::Arc<crate::uniform_worker::Signal>>,
    ) {
        let actor_changed = match (&self.uniform.alive, &alive) {
            (Some(previous), Some(next)) => !std::sync::Arc::ptr_eq(previous, next),
            (None, Some(_)) => true,
            _ => false,
        };
        if enabled && actor_changed {
            // A new local worker starts a new playback generation. In particular,
            // a ticket planned just before quiesce must not stay InFlight forever.
            // Device-bound failures survive this reset and still prohibit retry.
            self.uniform.sessions.clear();
        }
        if let Some(signal) = &alive {
            self.lighting_publisher.attach_signal(signal.clone());
        }
        self.uniform.enabled = enabled;
        self.uniform.alive = if enabled { alive } else { None };
        self.uniform.wake = wake;
        self.uniform.invalidate();
        if !enabled {
            self.uniform.sessions.clear();
        }
    }
    pub(super) fn invalidate_uniform(&mut self) {
        self.uniform.invalidate();
    }
    pub(super) fn reset_uniform_handles(&mut self) {
        self.uniform.sessions.clear();
        self.uniform.faults.clear();
        self.uniform.invalidate();
    }
    pub(crate) fn uniform_is_enabled(&self) -> bool {
        self.uniform.enabled
            && self
                .uniform
                .alive
                .as_ref()
                .is_some_and(|signal| signal.running())
    }
    pub(crate) fn uniform_settings_changed(&mut self, key: &str) {
        if let Some(session) = self.uniform.sessions.get_mut(key) {
            session.pending_live = true;
        }
        self.uniform.invalidate();
    }
    pub(crate) fn clear_uniform_action(&mut self, key: &str) {
        self.uniform.sessions.remove(key);
        self.uniform.faults.remove(key);
        self.uniform.invalidate();
    }
    pub(crate) fn uniform_action_record(
        &mut self,
        key: &str,
        name: &str,
        program: Program,
        result: &Result<Value>,
        trace: Option<&ActionTrace>,
    ) {
        // Explicit attempts always get a fresh generation, even for an unchanged
        // program. A stale failure/in-flight ticket must not poison recovery.
        self.uniform.sessions.remove(key);
        self.uniform.invalidate();
        if result.is_ok() {
            self.uniform.faults.remove(key);
        }
        let epoch = self.uniform.epoch;
        let session = self.uniform.session(key, name, program);
        let now = epoch.elapsed();
        match result {
            Ok(value) if value["execution"] == "firmware" => {
                session.mode = Some(0);
                session.prepared = false;
                session.pending_live = false;
                session.status = json!({"state":"normal-mode-firmware","streaming":false});
            }
            Ok(value) => {
                session.mode = Some(3);
                session.pending_live = value["live_policy_reconcile_pending"] != false;
                if value["accepted_packets"] == 7
                    && value["attempted_packets"] == 7
                    && value["control_state_verified"] == true
                {
                    if let Poll::Frame(ticket) = session.playback.poll(now, Availability::Ready) {
                        let _ = session.playback.complete(
                            ticket,
                            now,
                            Completion {
                                accepted_packets: 7,
                                attempted_packets: 7,
                                error: None,
                            },
                        );
                        session.prepared = true;
                    }
                }
                session.status = value.clone();
            }
            Err(error) => {
                let completion = trace.and_then(|t| t.completion.clone());
                if let Poll::Frame(ticket) = session.playback.poll(now, Availability::Ready) {
                    let _ = session.playback.complete(
                        ticket,
                        now,
                        completion.clone().unwrap_or(Completion {
                            accepted_packets: 0,
                            attempted_packets: 0,
                            error: Some(error.to_string()),
                        }),
                    );
                }
                session.prepared = false;
                session.pending_live = false;
                session.status =
                    json!({"state":"faulted","error":error.to_string(),"automatic_retry":false});
                self.uniform.faults.insert(
                    key.into(),
                    Fault {
                        error: error.to_string(),
                        completion,
                    },
                );
            }
        }
    }
    pub(crate) fn latch_uniform_action_failure(
        &mut self,
        key: &str,
        error: &Error,
        trace: &ActionTrace,
    ) {
        if trace.output_attempted {
            self.uniform.faults.insert(
                key.into(),
                Fault {
                    error: error.to_string(),
                    completion: trace.completion.clone(),
                },
            );
            self.uniform.invalidate();
        }
    }
    fn uniform_availability(&self, config: &Config) -> std::result::Result<Availability, String> {
        let mut runtime = self.lighting_runtime.borrow_mut();
        runtime.sample_host();
        let plan = crate::lighting_runtime::plan(
            config,
            &runtime.environment,
            runtime.running,
            runtime.stopping,
        );
        if let Some(reason) = plan.blocked {
            return Err(reason);
        }
        if !config.source_known {
            return Err("retained source brightness is required".into());
        }
        if !config.enabled || plan.suppressed || plan.target == Some(0) || self.away.is_some() {
            Ok(Availability::Suppressed)
        } else {
            Ok(Availability::Ready)
        }
    }
    /// Worker-only entry point. No catch-up burst; at most one frame per device.
    pub(crate) fn tick_uniform(&mut self, stopped: &AtomicBool) -> Duration {
        if !self.uniform_is_enabled() || stopped.load(Ordering::Acquire) {
            return Duration::from_millis(250);
        }
        let mut state = std::mem::take(&mut self.uniform);
        let path = self.uniform_path();
        let candidates = (|| -> std::result::Result<Vec<Candidate>, String> {
            let probe_store = self.lighting_probe_store_snapshot()
                .map_err(|error| error.to_string())?;
            let store = match probe_store.as_ref() {
                Some(store) => store,
                None => state.cache.refresh(&path)
                    .map_err(|error| self.latch_lighting_probe_error(&error))?,
            };
            let mut result = Vec::new();
            for dev in self
                .devices
                .iter()
                .filter(|d| d.pid == 605 && d.interface == 2)
            {
                let key = dev.key();
                let Some(profiles) = store
                    .devices
                    .get(&key)
                    .or_else(|| store.devices.get("025D"))
                else {
                    continue;
                };
                let name = profiles.effective(self.foreground()).to_string();
                let profile = &profiles.profiles[&name];
                if let Ok(Some(program)) = Program::from_profile(profile, dev.pid) {
                    result.push(Candidate {
                        key,
                        name: name.clone(),
                        program,
                        opted: program.opted_in(profile),
                        config: Config::from_profile(
                            &name,
                            profile,
                            !profiles.lighting_off,
                            self.away,
                        ),
                    });
                }
            }
            Ok(result)
        })();
        let candidates = match candidates {
            Ok(c) => {
                state.error = None;
                c
            }
            Err(error) => {
                state.error = Some(error);
                self.uniform = state;
                return Duration::from_millis(250);
            }
        };
        let active: HashSet<_> = candidates
            .iter()
            .filter(|c| c.opted)
            .map(|c| c.key.clone())
            .collect();
        state.sessions.retain(|key, _| active.contains(key));
        let mut wait = Duration::from_millis(250);
        let mut refresh_live = false;
        let epoch = state.epoch;
        for candidate in candidates.into_iter().filter(|c| c.opted) {
            if state.faults.contains_key(&candidate.key) {
                continue;
            }
            if stopped.load(Ordering::Acquire) {
                break;
            }
            let gate_started = Instant::now();
            let availability = self.uniform_availability(&candidate.config);
            let gate_us = gate_started.elapsed().as_micros() as u64;
            let worker_signal = state.alive.clone();
            let session = state.session(&candidate.key, &candidate.name, candidate.program);
            let now = epoch.elapsed();
            match availability {
                Err(reason) => {
                    session.prepared = false;
                    session.controls.invalidate();
                    session.playback.poll(now, Availability::Blocked);
                    session.status = json!({"state":"blocked","reason":reason,"streaming":false});
                    continue;
                }
                Ok(Availability::Suppressed) => {
                    session.prepared = false;
                    session.controls.invalidate();
                    session.playback.poll(now, Availability::Suppressed);
                    session.status =
                        json!({"state":"suppressed","deferred":true,"streaming":false});
                    continue;
                }
                Ok(_) => {}
            }
            if session.mode == Some(0) {
                continue;
            }
            if session.mode.is_none() {
                let Some(device) = self.devices.iter().find(|d| d.key() == candidate.key) else {
                    continue;
                };
                match device.ornata_mode_readback() {
                    Ok(mode) if mode.mode() == 0 => {
                        session.mode = Some(0);
                        session.status = json!({"state":"normal-mode-firmware","streaming":false});
                        continue;
                    }
                    Ok(mode) if mode.mode() == 3 => session.mode = Some(3),
                    Ok(mode) => {
                        state.faults.insert(
                            candidate.key.clone(),
                            Fault {
                                error: format!("Unsupported uniform mode{}", mode.mode()),
                                completion: None,
                            },
                        );
                        continue;
                    }
                    Err(error) => {
                        state.faults.insert(
                            candidate.key.clone(),
                            Fault {
                                error: error.to_string(),
                                completion: None,
                            },
                        );
                        continue;
                    }
                }
            }
            match session.playback.poll(epoch.elapsed(), Availability::Ready) {
                Poll::Frame(ticket) => {
                    let Some(device) = self.devices.iter_mut().find(|d| d.key() == candidate.key)
                    else {
                        continue;
                    };
                    let Some(signal) = worker_signal.as_ref() else { break; };
                    let Some(mut admission) = signal.begin_batch() else {
                        session.status = json!({"state":"quiescing","streaming":false});
                        break;
                    };
                    let submitted = session.controls.submit(
                        device,
                        ticket,
                        Instant::now() + Duration::from_millis(500),
                        || stopped.load(Ordering::Acquire),
                    );
                    admission.record(&submitted.completion);
                    drop(admission);
                    let success = submitted.completion.error.is_none()
                        && submitted.completion.accepted_packets == 7;
                    let fault = if !success {
                        Some(Fault {
                            error: submitted
                                .completion
                                .error
                                .clone()
                                .unwrap_or_else(|| "Incomplete uniform OUT".into()),
                            completion: Some(submitted.completion.clone()),
                        })
                    } else {
                        None
                    };
                    let error = submitted.completion.error.clone();
                    let accepted = submitted.completion.accepted_packets;
                    let attempted = submitted.completion.attempted_packets;
                    session
                        .playback
                        .complete(ticket, epoch.elapsed(), submitted.completion.clone())
                        .ok();
                    if success && !session.prepared {
                        session.prepared = true;
                        session.pending_live = true;
                        refresh_live = true;
                    }
                    session.status = json!({"state":if success {"playing"}else{"faulted"},"streaming":success,"source_id":candidate.program.source_id(),"native_id":candidate.program.native_id(),"palette_index":ticket.palette_index,"frame_count":session.playback.frame_count(),"accepted_frames":session.playback.accepted_frames(),"accepted_packets":accepted,"attempted_packets":attempted,"error":error,"custom_frame_control_verified":submitted.control_verified,"mode_before":submitted.observed_mode,"parameter_before":submitted.observed_parameter,"mode_after":submitted.mode_after,"parameter_after":submitted.parameter_after,"parameter_unchanged":submitted.parameter_unchanged(),"optically_verified":false,"automatic_retry":false,"last_submit_us":submitted.elapsed.as_micros() as u64,"last_mode_get_us":submitted.mode_query_elapsed.as_micros() as u64,"last_seven_out_calls_us":submitted.usb_out_elapsed.as_micros() as u64,"last_gate_us":gate_us,"nominal_interval_ms":40,"cadence_certified":false,"phase_policy":"local-freeze-no-catchup"});
                    if let Some(fault) = fault {
                        state.faults.insert(candidate.key.clone(), fault);
                    }
                    wait = wait.min(Duration::from_millis(8));
                }
                Poll::Wait(remaining) => wait = wait.min(remaining.max(Duration::from_millis(8))),
                Poll::Faulted => {}
                _ => {}
            }
        }
        self.uniform = state;
        if refresh_live && self.uniform_is_enabled() {
            self.refresh_lighting_runtime();
        }
        wait
    }
    /// Called before the existing LIVE brightness reconcile. It never submits a
    /// frame on the UI thread, and holds a positive resume until the worker is ready.
    pub(super) fn uniform_before_brightness(
        &mut self,
        index: usize,
        profile: &Profile,
        name: &str,
        config: &mut Config,
    ) {
        // Shutdown releases only the existing LIVE policy lease; it never waits
        // for an animation frame, a worker or an exact previous visual phase.
        if self.lighting_runtime.borrow().stopping {
            return;
        }
        let dev = &self.devices[index];
        let program = match Program::from_profile(profile, dev.pid) {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(error) => {
                config.error = Some(error.to_string());
                return;
            }
        };
        if dev.interface != 2 {
            return;
        }
        let key = dev.key();
        if !self.lighting_runtime.borrow().owner_clear() {
            return;
        }
        if !self.uniform_is_enabled() || !program.opted_in(profile) {
            match dev.ornata_mode_readback() {
                Ok(mode) if mode.mode() == 0 => return,
                Ok(_) => config.error = Some(
                    "Uniform host playback requires explicit effect selection and a running worker"
                        .into(),
                ),
                Err(error) => {
                    config.error = Some(format!("Uniform mode preflight failed: {error}"))
                }
            };
            return;
        }
        let faulted = self.uniform.faults.contains_key(&key);
        let session = self.uniform.session(&key, name, program);
        if session.mode == Some(0) {
            return;
        }
        let runtime = self.lighting_runtime.borrow();
        let plan = crate::lighting_runtime::plan(
            config,
            &runtime.environment,
            runtime.running,
            runtime.stopping,
        );
        drop(runtime);
        if plan.suppressed || !config.enabled || plan.target == Some(0) {
            session.controls.invalidate();
            session.prepared = false;
            if session.pending_live {
                self.lighting_runtime
                    .borrow_mut()
                    .request_configured_live(&key);
                session.pending_live = false;
            }
            return;
        }
        if faulted || session.playback.failure().is_some() {
            config.error=Some("Uniform playback failed; reselect the effect or reconnect before brightness resume".into());
            return;
        }
        if !session.prepared {
            config.error =
                Some("Uniform first frame is pending; brightness resume deferred".into());
            return;
        }
        if session.pending_live {
            self.lighting_runtime
                .borrow_mut()
                .request_configured_live(&key);
            session.pending_live = false;
        }
    }
    pub(super) fn uniform_publication_faulted(&self) -> bool {
        self.uniform.error.is_some()
            || !self.uniform.faults.is_empty()
            || self.lighting_probe_error().is_some()
    }
    pub(crate) fn log_uniform_shutdown_diagnostics(&self) {
        // Local private diagnostics retain actual errors/counts before cleanup
        // clears sessions. Public RAM metadata remains sanitized separately.
        let detail = json!({
            "uniform":self.uniform_status(),
            "brightness_policy":self.lighting_runtime.borrow().status(),
            "probe_error":self.lighting_probe_error(),
            "worker_signal":self.uniform.alive.as_ref().map(|signal|signal.metrics())
        });
        secbird_core::log::append(&format!("uniform shutdown cached diagnostics: {detail}"));
    }
    pub(super) fn uniform_status(&self) -> Value {
        let mut result = serde_json::Map::new();
        for (key, session) in &self.uniform.sessions {
            let mut value = session.status.clone();
            value["profile"] = json!(session.profile);
            value["opted_in"] = json!(true);
            value["worker_enabled"] = json!(self.uniform_is_enabled());
            value["worker_timing"] = self.uniform.alive.as_ref().map(|signal| signal.metrics()).unwrap_or(Value::Null);
            if !self.uniform_is_enabled() {
                value["streaming"] = json!(false);
                value["worker_stopped"] = json!(self.uniform.alive.as_ref().is_none_or(|signal|!signal.thread_alive()));
                value["quiesce_requested"] = json!(self.uniform.alive.as_ref().is_some_and(|signal|signal.quiescing()));
            }
            value["configuration_reloads"] = json!(self.uniform.cache.reloads);
            value["control_observation"] = session.controls.status();
            value["session_elapsed_us"] = json!(session.started.elapsed().as_micros() as u64);
            value["accepted_frames"] = json!(session.playback.accepted_frames());
            result.insert(key.clone(), value);
        }
        for (key, fault) in &self.uniform.faults {
            let profile = self
                .uniform
                .sessions
                .get(key)
                .map(|session| session.profile.as_str());
            result.insert(
                key.clone(),
                json!({
                    "profile": profile,
                    "state": "faulted",
                    "streaming": false,
                    "worker_enabled": self.uniform_is_enabled(),
                    "error": fault.error,
                    "accepted_packets": fault.completion.as_ref().map(|c| c.accepted_packets),
                    "attempted_packets": fault.completion.as_ref().map(|c| c.attempted_packets),
                    "automatic_retry": false,
                    "device_bound_latch": true
                }),
            );
        }
        if let Some(error) = &self.uniform.error {
            for value in result.values_mut() {
                value["configuration_error"] = json!(error);
            }
        }
        Value::Object(result)
    }
    pub(super) fn uniform_shadow(
        &self,
        dev: &Device,
        profiles: &mut secbird_core::profile::DeviceProfiles,
        name: &str,
    ) -> Vec<String> {
        if dev.pid != 605 || dev.interface != 2 || profiles.lighting_off {
            return Vec::new();
        }
        let profile = &profiles.profiles[name];
        let program = match Program::from_profile(profile, dev.pid) {
            Ok(None) => return Vec::new(),
            value => value,
        };
        let mode = match dev.ornata_mode_readback() {
            Ok(mode) => mode.mode(),
            Err(error) => {
                profiles.lighting_off = true;
                return vec![format!("Uniform mode preflight: {error}")];
            }
        };
        if mode == 0 {
            return Vec::new();
        }
        if mode != 3 {
            profiles.lighting_off = true;
            return vec![format!("Uniform mode{mode} is unsupported")];
        }
        let program = match program {
            Ok(Some(p)) => p,
            Err(error) => {
                profiles.lighting_off = true;
                return vec![error.to_string()];
            }
            _ => unreachable!(),
        };
        if !self.uniform_is_enabled() || !program.opted_in(profile) {
            profiles.lighting_off = true;
            return vec!["Select Spectrum or fixed Breathing explicitly in Lighting to authorize the host worker".into()];
        }
        let profile = profiles.profiles.get_mut(name).unwrap();
        let zone = profile.lighting.get_mut("backlight").unwrap();
        zone.color = None;
        zone.effect = None;
        zone.brightness = None;
        if let Some(settings) = profile.lighting_settings.as_mut() {
            settings.brightness = None;
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host_static::test_support::{self, Spy},
        lighting_runtime::{Environment, Owner},
    };
    use secbird_core::{lighting::SwitchOffLighting, Rgb};
    use std::sync::atomic::AtomicUsize;
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        engine: Engine,
        spy: Spy,
        path: PathBuf,
        key: String,
    }
    impl Fixture {
        fn new(opted: bool) -> Self {
            HidApi::disable_device_discovery();
            let mut engine = Engine::from_api(HidApi::new().unwrap());
            let (dev, spy) = test_support::fixture();
            let key = dev.key();
            let path = std::env::temp_dir()
                .join(format!(
                    "uniform-controller-{}-{}",
                    std::process::id(),
                    SEQ.fetch_add(1, Ordering::Relaxed)
                ))
                .join("profiles.json");
            let mut profile =
                secbird_core::profile_compat::new_native_profile(605, "Default").unwrap();
            let mut settings = profile.lighting_settings.clone().unwrap();
            settings.switch_off = Some(SwitchOffLighting {
                display_off: true,
                ..Default::default()
            });
            settings.store_in_profile(&mut profile).unwrap();
            if opted {
                Program::Spectrum.opt_in(&mut profile).unwrap();
            }
            let mut store = Store::load(&path).unwrap();
            store
                .for_device(&dev)
                .profiles
                .insert("Default".into(), profile);
            store.save().unwrap();
            spy.sim.state().brightness.insert(5, 255);
            spy.sim.state().live_brightness.insert(5, 0);
            engine.devices.push(dev);
            engine.simulated_only = true;
            engine.runtime_store_path = Some(path.clone());
            engine.lighting_runtime.borrow_mut().test_environment = Some(Environment {
                now_ms: 1000,
                idle_ms: Ok(0),
                display_off: Some(false),
                owner: Owner::Clear,
            });
            engine.lighting_runtime.borrow_mut().start();
            engine.set_uniform_worker(
                true,
                None,
                Some(crate::uniform_worker::Signal::running_for_test()),
            );
            Self {
                engine,
                spy,
                path,
                key,
            }
        }
        fn advance(&mut self) {
            self.engine.uniform.epoch = self
                .engine
                .uniform
                .epoch
                .checked_sub(Duration::from_millis(60))
                .unwrap();
        }
        fn tick(&mut self) {
            self.engine.tick_uniform(&AtomicBool::new(false));
        }
        fn actual_action(&mut self, program: Program) -> (Result<Value>, ActionTrace) {
            let mut playback = Playback::new(program, 999, Duration::ZERO);
            let ticket = match playback.poll(Duration::ZERO, Availability::Ready) {
                Poll::Frame(t) => t,
                _ => unreachable!(),
            };
            let submitted = host_uniform::io::submit(
                &mut self.engine.devices[0],
                ticket,
                Instant::now() + Duration::from_secs(1),
                || false,
            );
            let trace = ActionTrace {
                output_attempted: true,
                requested: Some(program),
                completion: Some(submitted.completion.clone()),
            };
            let result = if let Some(error) = &submitted.completion.error {
                Err(Error::InvalidArg(error.clone()))
            } else {
                Ok(
                    json!({"execution":"host-uniform","accepted_packets":7,"attempted_packets":7,"control_state_verified":true}),
                )
            };
            (result, trace)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
        }
    }
    #[test]
    fn cached_controls_refresh_on_resume_owner_reentry_program_change_and_reconnect() {
        let mut f = Fixture::new(true);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 2);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 2);
        let status = f.engine.uniform_status();
        assert_eq!(status[&f.key]["custom_frame_control_verified"], false);
        assert_eq!(status[&f.key]["mode_before"], 3);
        assert!(status[&f.key]["mode_after"].is_null());
        assert_eq!(
            status[&f.key]["control_observation"]["maximum_first_packet_admission_age_ms"],
            500
        );
        assert_eq!(
            status[&f.key]["control_observation"]["observation_is_ownership_lease"],
            false
        );
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(true);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(false);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 4);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
        assert_eq!(f.engine.uniform_status()[&f.key]["palette_index"], 2);
        let attempts = f.spy.state.lock().unwrap().attempts;
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .owner = Owner::Competing(vec!["RazerAppEngine.exe".into()]);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, attempts);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .owner = Owner::Clear;
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 6);
        let mut store = Store::load(&f.path).unwrap();
        let profile = store.for_device(&f.engine.devices[0]).active_mut();
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(secbird_core::effects::Effect::Breathing(vec![
                secbird_core::Rgb(1, 2, 3),
            ]));
        Program::Breathing {
            first: secbird_core::Rgb(1, 2, 3),
            second: None,
        }
        .opt_in(profile)
        .unwrap();
        store.save().unwrap();
        f.engine.uniform_settings_changed(&f.key);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 8);
        f.engine.reset_uniform_handles();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().effects_read, 10);
    }
    #[derive(Default)]
    struct RowGateState {
        next_row: usize,
        entered: bool,
        released: bool,
    }
    type RowGate = std::sync::Arc<(std::sync::Mutex<RowGateState>,std::sync::Condvar)>;
    type SharedEngine = std::sync::Arc<std::sync::Mutex<Engine>>;
    struct BlockedRow {
        spy: Spy,
        gate: RowGate,
    }
    impl secbird_core::transport::Transport for BlockedRow {
        fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
            if data[7] == 15 && data[8] == 3 {
                let (mutex, wake) = &*self.gate;
                let mut state = mutex.lock().unwrap();
                let row = state.next_row;
                state.next_row += 1;
                if row == 2 {
                    state.entered = true;
                    wake.notify_all();
                    while !state.released {
                        state = wake.wait(state).unwrap();
                    }
                }
            }
            secbird_core::transport::Transport::send_feature_report(&self.spy, data)
        }
        fn get_feature_report(&self, data: &mut [u8]) -> secbird_core::Result<usize> {
            secbird_core::transport::Transport::get_feature_report(&self.spy, data)
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    fn blocked_worker(f: &mut Fixture) -> (crate::uniform_worker::Worker, SharedEngine, RowGate) {
        let gate = std::sync::Arc::new((
            std::sync::Mutex::new(RowGateState::default()),
            std::sync::Condvar::new(),
        ));
        let old = f.engine.devices.remove(0);
        let device = Device::open(
            Box::new(BlockedRow {
                spy: f.spy.clone(),
                gate: gate.clone(),
            }),
            605,
            2,
            std::ffi::CString::new("sim:blocked-row").unwrap(),
            &[0x1f],
            old.spec,
        )
        .unwrap();
        f.engine.devices.push(device);
        let engine = std::mem::replace(&mut f.engine, Engine::from_api(HidApi::new().unwrap()));
        let shared = std::sync::Arc::new(std::sync::Mutex::new(engine));
        let worker = crate::uniform_worker::Worker::start(shared.clone()).unwrap();
        let (mutex, wake) = &*gate;
        let state = mutex.lock().unwrap();
        let (state, wait) = wake
            .wait_timeout_while(state, Duration::from_secs(2), |state| !state.entered)
            .unwrap();
        assert!(state.entered && !wait.timed_out());
        drop(state);
        (worker, shared, gate)
    }
    fn release_row(gate: &RowGate) {
        let (mutex, wake) = &**gate;
        mutex.lock().unwrap().released = true;
        wake.notify_all();
    }
    #[test]
    fn graceful_stop_during_actual_row_finishes_one_batch_and_keeps_genuine_failure() {
        for fail in [false, true] {
            let mut f = Fixture::new(true);
            if fail {
                f.spy.state.lock().unwrap().fail_at = Some(2);
            }
            let (mut worker, shared, gate) = blocked_worker(&mut f);
            let signal = worker.signal_for_test();
            let observed = signal.clone();
            let release = std::thread::spawn(move || {
                let limit = Instant::now() + Duration::from_secs(2);
                while !observed.quiescing() && Instant::now() < limit {
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(observed.quiescing());
                release_row(&gate);
            });
            assert!(worker.stop(Duration::from_secs(2)));
            release.join().unwrap();
            let engine = shared.lock().unwrap();
            let value = engine.uniform_status();
            if fail {
                assert_eq!(value[&f.key]["accepted_packets"], 2);
                assert_eq!(value[&f.key]["attempted_packets"], 3);
                assert!(value[&f.key]["error"].is_string());
            } else {
                assert_eq!(value[&f.key]["accepted_frames"], 1);
                assert!(!value[&f.key]["error"].is_string());
                assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
                assert!(!engine.lighting_runtime_status()[&f.key]["error"].is_string());
            }
            let metrics = signal.metrics();
            assert_eq!(metrics["batches_admitted"], 1);
            assert_eq!(metrics["batches_completed"], 1);
            assert_eq!(
                metrics["last_batch_accepted_packets"],
                if fail { 2 } else { 7 }
            );
            assert_eq!(
                metrics["last_batch_attempted_packets"],
                if fail { 3 } else { 7 }
            );
            assert_eq!(metrics["last_batch_failed"], fail);
            assert_eq!(metrics["stop_timeouts"], 0);
        }
    }
    #[test]
    fn immediate_cancel_mid_row_reproduces_partial_fault_instead_of_hiding_it() {
        let mut f = Fixture::new(true);
        let (mut worker, shared, gate) = blocked_worker(&mut f);
        let signal = worker.signal_for_test();
        signal.stop_for_test();
        let observed = signal.clone();
        let release = std::thread::spawn(move || {
            let limit = Instant::now() + Duration::from_secs(2);
            while !observed.quiescing() && Instant::now() < limit {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(observed.quiescing());
            release_row(&gate);
        });
        assert!(worker.stop(Duration::from_secs(2)));
        release.join().unwrap();
        let engine = shared.lock().unwrap();
        let value = engine.uniform_status();
        assert_eq!(value[&f.key]["accepted_packets"], 3);
        assert_eq!(value[&f.key]["attempted_packets"], 3);
        assert!(value[&f.key]["error"]
            .as_str()
            .unwrap()
            .contains("Cancelled"));
        assert_eq!(signal.metrics()["last_batch_failed"], true);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
    }
    #[test]
    fn blocked_hid_stop_timeout_is_bounded_and_does_not_acquire_engine_for_cleanup() {
        let mut f = Fixture::new(true);
        let (mut worker, shared, gate) = blocked_worker(&mut f);
        let signal = worker.signal_for_test();
        let started = Instant::now();
        assert!(!worker.stop(Duration::from_secs(2)));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(
            shared.try_lock().is_err(),
            "HID call remains blocked underEngine; caller must not acquire it"
        );
        assert_eq!(signal.metrics()["stop_timeouts"], 1);
        release_row(&gate);
        assert!(worker.stop(Duration::from_secs(1)));
        let engine = shared.lock().unwrap();
        let value = engine.uniform_status();
        assert_eq!(value[&f.key]["accepted_packets"], 3);
        assert_eq!(value[&f.key]["attempted_packets"], 3);
        assert!(value[&f.key]["error"].is_string());
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
    }
    #[test]
    fn replacement_worker_discards_unsubmitted_ticket_but_preserves_device_faults() {
        let mut f = Fixture::new(true);
        let epoch = f.engine.uniform.epoch;
        let session = f
            .engine
            .uniform
            .session(&f.key, "Default", Program::Spectrum);
        let now = epoch.elapsed();
        assert!(matches!(
            session.playback.poll(now, Availability::Ready),
            Poll::Frame(_)
        ));
        assert!(matches!(
            session.playback.poll(now, Availability::Ready),
            Poll::InFlight
        ));
        f.engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        assert!(f.engine.uniform.sessions.is_empty());
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        assert_eq!(f.engine.uniform_status()[&f.key]["accepted_frames"], 1);
        let mut f = Fixture::new(true);
        f.spy.state.lock().unwrap().fail_at = Some(2);
        f.tick();
        assert!(f.engine.uniform.faults.contains_key(&f.key));
        f.engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        assert!(f.engine.uniform.sessions.is_empty());
        assert!(f.engine.uniform.faults.contains_key(&f.key));
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
        assert_eq!(f.engine.uniform_status()[&f.key]["accepted_packets"], 2);
        assert_eq!(f.engine.uniform_status()[&f.key]["attempted_packets"], 3);
    }
    #[test]
    fn initial_frame_precedes_live_brightness_and_cache_is_reused() {
        let mut f = Fixture::new(true);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
        let commands = f.spy.state.lock().unwrap().commands.clone();
        assert!(
            commands
                .iter()
                .position(|c| *c == secbird_core::commands::MATRIX_FRAME)
                .unwrap()
                < commands
                    .iter()
                    .position(|c| *c == secbird_core::commands::MATRIX_BRIGHTNESS)
                    .unwrap()
        );
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 14);
        assert_eq!(f.engine.uniform.cache.reloads, 1);
        assert!(!f.spy.sim.state().effects.contains_key(&5));
    }
    #[test]
    fn unmarked_default_and_suppressed_programs_do_not_submit() {
        let mut f = Fixture::new(false);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert!(f.engine.uniform.sessions.is_empty());
        let mut f = Fixture::new(true);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(true);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(false);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
    }
    #[test]
    fn inferred_raw_baseline_survives_effect_changes_and_policy_resume() {
        let mut f = Fixture::new(true);
        f.spy.sim.state().brightness.insert(5, 128);
        f.spy.sim.state().live_brightness.insert(5, 128);
        let mut store = Store::load(&f.path).unwrap();
        let profile = store.for_device(&f.engine.devices[0]).active_mut();
        let mut settings = profile.lighting_settings.clone().unwrap();
        settings.brightness.as_mut().unwrap().value = 51;
        settings.store_in_profile(profile).unwrap();
        secbird_core::profile_compat::record_ornata_brightness_baseline(profile, 128, 128).unwrap();
        let zone = profile.lighting.get_mut("backlight").unwrap();
        zone.color = Some(secbird_core::Rgb(1, 2, 3));
        zone.effect = None;
        secbird_core::profile_compat::set_source_quick_effect(
            profile,
            605,
            "Default",
            1,
            &json!({"color1":"#010203"}),
        )
        .unwrap();
        store.save().unwrap();
        let mut initial =
            crate::host_static::send_uniform(&mut f.engine.devices[0], secbird_core::Rgb(1, 2, 3))
                .unwrap();
        initial["live_policy_reconcile_pending"] = json!(false);
        f.engine
            .record_host_static(&f.key, "Default", secbird_core::Rgb(1, 2, 3), &Ok(initial));
        f.engine.request_lighting_refresh();
        let mut observed = vec![f.spy.sim.state().live_brightness[&5]];
        store
            .for_device(&f.engine.devices[0])
            .active_mut()
            .lighting
            .get_mut("backlight")
            .unwrap()
            .color = Some(secbird_core::Rgb(4, 5, 6));
        store.save().unwrap();
        f.engine.request_lighting_refresh();
        observed.push(f.spy.sim.state().live_brightness[&5]);
        for program in [
            Program::Spectrum,
            Program::Breathing {
                first: secbird_core::Rgb(1, 2, 3),
                second: None,
            },
        ] {
            let profile = store.for_device(&f.engine.devices[0]).active_mut();
            let zone = profile.lighting.get_mut("backlight").unwrap();
            zone.color = None;
            zone.effect = Some(match program {
                Program::Spectrum => secbird_core::effects::Effect::Spectrum,
                Program::Breathing { first, .. } => {
                    secbird_core::effects::Effect::Breathing(vec![first])
                }
                Program::Wave(_) => unreachable!("uniform fixture"),
            });
            program.opt_in(profile).unwrap();
            store.save().unwrap();
            f.engine.uniform_settings_changed(&f.key);
            f.engine.request_lighting_refresh();
            f.tick();
            observed.push(f.spy.sim.state().live_brightness[&5]);
        }
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(true);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(false);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.advance();
        f.tick();
        observed.push(f.spy.sim.state().live_brightness[&5]);
        assert_eq!(observed, vec![128; 5]);
        assert_eq!(f.spy.sim.state().brightness[&5], 128);
        // An explicit brightness edit clears bootstrap provenance, even when
        // the user chooses the same displayed51%. It now means configured51%.
        secbird_core::profile_compat::clear_source_brightness_baseline(
            store.for_device(&f.engine.devices[0]).active_mut(),
        );
        store.save().unwrap();
        f.engine.uniform_settings_changed(&f.key);
        f.engine.request_lighting_refresh();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 130);
        assert_eq!(f.spy.sim.state().brightness[&5], 128);
    }
    #[test]
    fn suppressed_intent_change_prepares_latest_program_before_brightness_resume() {
        let mut f = Fixture::new(true);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(true);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        let mut store = Store::load(&f.path).unwrap();
        let profile = store.for_device(&f.engine.devices[0]).active_mut();
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(secbird_core::effects::Effect::Breathing(vec![
                secbird_core::Rgb(0, 0, 255),
            ]));
        let mut settings = profile.lighting_settings.clone().unwrap();
        settings.brightness.as_mut().unwrap().value = 49;
        settings.store_in_profile(profile).unwrap();
        let program = Program::Breathing {
            first: secbird_core::Rgb(0, 0, 255),
            second: None,
        };
        program.opt_in(profile).unwrap();
        store.save().unwrap();
        f.engine.uniform_settings_changed(&f.key);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(false);
        f.engine.request_lighting_refresh();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 124);
        assert!(f
            .spy
            .sim
            .state()
            .matrix
            .as_ref()
            .unwrap()
            .2
            .iter()
            .all(|rgb| *rgb == [0, 0, 0]));
        // Source one-color Breathing retains black for its first25% of175
        // entries. Advance actual accepted frames rather than skipping that phase.
        for _ in 0..45 {
            f.advance();
            f.tick();
        }
        assert!(f
            .spy
            .sim
            .state()
            .matrix
            .as_ref()
            .unwrap()
            .2
            .iter()
            .all(|rgb| rgb[0] == 0 && rgb[1] == 0 && rgb[2] > 0));
    }
    #[test]
    fn explicit_same_program_success_replaces_a_failed_playback() {
        let mut f = Fixture::new(true);
        f.spy.state.lock().unwrap().fail_at = Some(2);
        let (result, trace) = f.actual_action(Program::Spectrum);
        assert!(result.is_err());
        f.engine
            .uniform_action_record(&f.key, "Default", Program::Spectrum, &result, Some(&trace));
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
        f.spy.state.lock().unwrap().fail_at = None;
        let (result, trace) = f.actual_action(Program::Spectrum);
        result.as_ref().unwrap();
        f.engine
            .uniform_action_record(&f.key, "Default", Program::Spectrum, &result, Some(&trace));
        assert!(f.engine.uniform.faults.is_empty());
        assert!(f.engine.uniform.sessions[&f.key]
            .playback
            .failure()
            .is_none());
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 17);
        assert_eq!(
            f.engine.uniform.sessions[&f.key].playback.accepted_frames(),
            2
        );
    }
    #[test]
    fn normal_mode_dormant_has_no_pending_ticket_and_explicit_driver_action_recovers() {
        let mut f = Fixture::new(true);
        f.spy.state.lock().unwrap().mode = 0;
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert_eq!(f.engine.uniform.sessions[&f.key].mode, Some(0));
        let now = f.engine.uniform.epoch.elapsed();
        assert!(matches!(
            f.engine
                .uniform
                .sessions
                .get_mut(&f.key)
                .unwrap()
                .playback
                .poll(now, Availability::Ready),
            Poll::Frame(_)
        ));
        f.spy.state.lock().unwrap().mode = 3;
        let (result, trace) = f.actual_action(Program::Spectrum);
        f.engine
            .uniform_action_record(&f.key, "Default", Program::Spectrum, &result, Some(&trace));
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 14);
        assert!(f.engine.uniform.faults.is_empty());
    }
    #[test]
    fn partial_new_program_does_not_automatically_repaint_old_saved_program() {
        let mut f = Fixture::new(true);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        let before = std::fs::read(&f.path).unwrap();
        f.spy.state.lock().unwrap().fail_at = Some(9);
        let requested = Program::Breathing {
            first: Rgb(255, 0, 0),
            second: None,
        };
        let (result, trace) = f.actual_action(requested);
        assert!(result.is_err());
        f.engine
            .uniform_action_record(&f.key, "Default", requested, &result, Some(&trace));
        assert_eq!(std::fs::read(&f.path).unwrap(), before);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 10);
        assert_eq!(f.engine.uniform_status()[&f.key]["accepted_packets"], 2);
        assert_eq!(f.engine.uniform_status()[&f.key]["attempted_packets"], 3);
        f.spy.state.lock().unwrap().fail_at = None;
        let (result, trace) = f.actual_action(Program::Spectrum);
        f.engine
            .uniform_action_record(&f.key, "Default", Program::Spectrum, &result, Some(&trace));
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 24);
    }
    #[test]
    fn partial_static_choice_latches_device_with_unknown_counts_and_no_old_stream_repaint() {
        let mut f = Fixture::new(true);
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        let before = std::fs::read(&f.path).unwrap();
        f.spy.state.lock().unwrap().fail_at = Some(9);
        let error = crate::host_static::send_uniform(&mut f.engine.devices[0], Rgb(16, 32, 48))
            .unwrap_err();
        // Static's existing error is human-readable; no packet-count parsing.
        let trace = ActionTrace {
            output_attempted: true,
            requested: None,
            completion: None,
        };
        f.engine
            .latch_uniform_action_failure(&f.key, &error, &trace);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 10);
        assert_eq!(std::fs::read(&f.path).unwrap(), before);
        let status = f.engine.uniform_status();
        assert_eq!(status[&f.key]["state"], "faulted");
        assert!(status[&f.key]["accepted_packets"].is_null());
        assert!(status[&f.key]["attempted_packets"].is_null());
    }
    #[test]
    fn invalid_choice_before_output_does_not_stop_existing_stream() {
        let mut f = Fixture::new(true);
        f.tick();
        f.engine.latch_uniform_action_failure(
            &f.key,
            &Error::Unsupported("invalid/unsupported choice with no I/O".into()),
            &ActionTrace::default(),
        );
        assert!(f.engine.uniform.faults.is_empty());
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 14);
    }
    #[test]
    fn shutdown_releases_live_policy_without_repainting_a_faulted_frame() {
        let mut f = Fixture::new(true);
        f.tick();
        f.spy.state.lock().unwrap().fail_at = Some(9);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 10);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .display_off = Some(true);
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.engine.set_uniform_worker(false, None, None);
        assert!(f.engine.shutdown_lighting_runtime().is_empty());
        assert_eq!(f.spy.state.lock().unwrap().attempts, 10);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
    }
    #[test]
    fn stale_enabled_flag_does_not_resurrect_a_stopped_worker() {
        let mut f = Fixture::new(true);
        f.tick();
        f.engine.uniform.alive.as_ref().unwrap().stop_for_test();
        assert!(f.engine.uniform.enabled);
        assert!(!f.engine.uniform_is_enabled());
        let status = f.engine.uniform_status();
        assert_eq!(status[&f.key]["worker_enabled"], false);
        assert_eq!(status[&f.key]["streaming"], false);
        f.advance();
        f.tick();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
    }
    #[test]
    fn aggregate_budget_failure_and_owner_block_do_no_frame_io() {
        let mut f = Fixture::new(true);
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .owner = Owner::Competing(vec!["RazerAppEngine.exe".into()]);
        f.tick();
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
        let mut f = Fixture::new(true);
        let mut store = Store::load(&f.path).unwrap();
        store
            .extra
            .insert("oversized".into(), json!("x".repeat(2 * 1024 * 1024)));
        std::fs::write(&f.path, serde_json::to_vec(&store).unwrap()).unwrap();
        f.tick();
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
        assert!(f.engine.uniform.error.is_some());
    }
}
