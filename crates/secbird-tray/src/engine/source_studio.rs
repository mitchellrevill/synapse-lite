//! Studio uses the existing cooperative lighting worker and profile cache.
//! No per-frame thread, mode acquisition, automatic reconnect replay or saved
//! Quick effect mutation. Only the selected target's Quick writers are held;
//! explicit brightness edits persist their shared device-profile intent.
use super::*;
use crate::{
    host_studio::session::{ApplyIntent, Control, OutputSession, Poll},
    host_uniform::{Availability, Completion},
};
use secbird_core::{studio::Archive, Error};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct StudioState {
    pub(super) session: OutputSession,
    epoch: Instant,
    connection_epoch: Option<u64>,
    prepared: bool,
    error: Option<String>,
}
impl Default for StudioState {
    fn default() -> Self {
        Self {
            session: OutputSession::default(),
            epoch: Instant::now(),
            connection_epoch: Some(1),
            prepared: false,
            error: None,
        }
    }
}
impl Engine {
    #[cfg(test)]
    pub(crate) fn with_studio_output_simulator(
        path: std::path::PathBuf,
    ) -> (Self, crate::host_static::test_support::Spy) {
        HidApi::disable_device_discovery();
        let mut engine = Self::from_api(HidApi::new().unwrap());
        let (device, spy) = crate::host_static::test_support::fixture();
        spy.sim.state().ornata_layout_edition = Some([6, 0]);
        engine.devices.push(device);
        engine.simulated_only = true;
        engine.runtime_store_path = Some(path);
        engine.lighting_runtime.borrow_mut().test_environment =
            Some(crate::lighting_runtime::Environment {
                now_ms: 1000,
                idle_ms: Ok(0),
                display_off: Some(false),
                owner: crate::lighting_runtime::Owner::Clear,
            });
        engine.lighting_runtime.borrow_mut().start();
        (engine, spy)
    }
    pub(crate) fn studio_control(&self) -> Control {
        self.studio.session.control()
    }
    pub(crate) fn studio_holds(&self, key: &str) -> bool {
        self.studio.session.holds_target(key)
    }
    pub(crate) fn studio_status(&self) -> Value {
        let mut status = self.studio.session.status();
        status["worker_enabled"] = json!(self.uniform_is_enabled());
        status["frame_prepared"] = json!(self.studio.prepared);
        status["runtime_error"] = json!(self.studio.error);
        status["automatic_reconnect_replay"] = json!(false);
        status
    }
    /// Stage and save brightness intent before the existing LIVE bridge can
    /// write anything. This path never projects the saved Quick effect or frame.
    pub(crate) fn set_studio_brightness(
        &mut self,
        key: &str,
        expected_profile: &str,
        want: &secbird_core::lighting::LightingSettings,
    ) -> Result<Value> {
        if self.measurement_read_only || !self.uniform_is_enabled() {
            return Err(Error::Unsupported(
                "Studio brightness needs the running writable lighting worker".into(),
            ));
        }
        if !self.studio_holds(key) || self.studio.session.stop_requested() {
            return Err(Error::InvalidArg(
                "Studio target changed or is stopping; refresh Lighting".into(),
            ));
        }
        want.validate()?;
        let brightness = want
            .brightness
            .as_ref()
            .ok_or_else(|| Error::InvalidArg("Studio brightness settings are missing".into()))?;
        if self.studio.session.is_faulted() && brightness.projected() != 0 {
            return Err(Error::Unsupported(
                "Studio output failed; stop or explicitly reapply before raising brightness".into(),
            ));
        }
        if self.lighting_probe_store_snapshot()?.is_some() {
            return Err(Error::Unsupported(
                "Studio brightness is unavailable during an isolated lighting measurement".into(),
            ));
        }
        // This Store is a fresh local candidate, not the worker's cached object.
        // Early refusal and failed save cannot mutate live intent or device state.
        let mut candidate = self.load_store()?;
        self.validate_static_context(key, &candidate, expected_profile)?;
        let device = self
            .devices
            .iter()
            .find(|device| device.key() == key)
            .ok_or_else(|| Error::NotFound("Studio brightness target disconnected".into()))?;
        let profiles = candidate.for_device(device);
        if profiles.active != expected_profile {
            return Err(Error::InvalidArg(
                "Studio brightness active profile changed; refresh Lighting".into(),
            ));
        }
        let current = profiles
            .active()
            .lighting_settings
            .as_ref()
            .ok_or_else(|| {
                Error::InvalidArg("Studio brightness needs retained source settings".into())
            })?;
        if current.brightness.is_none() {
            return Err(Error::InvalidArg(
                "Studio brightness baseline is missing; refresh Lighting".into(),
            ));
        }
        if current.switch_off != want.switch_off || current.extra != want.extra {
            return Err(Error::InvalidArg(
                "Brightness cannot replace idle/display policy or other settings; refresh Lighting"
                    .into(),
            ));
        }
        want.store_in_profile(profiles.active_mut())?;
        secbird_core::profile_compat::clear_source_brightness_baseline(profiles.active_mut());
        let config =
            LightingConfig::from_profile(expected_profile, profiles.active(), true, self.away);
        secbird_core::profile_compat::validate_store_budget(&candidate)?;
        self.uniform_availability(&config)
            .map_err(Error::Unsupported)?;
        // Read-only mode/effect admission; no Quick-frame preflight or rendering.
        crate::host_uniform::io::preflight(device)?;
        candidate.save()?;
        self.studio.prepared = false;
        self.invalidate_uniform();
        self.lighting_runtime.borrow_mut().request_configured_live(key);
        Ok(
            json!({"execution":"host-studio-brightness","saved":true,"deferred":true,
            "configured_intent":want,"saved_hardware_unchanged":true,
            "live_policy_reconcile_pending":true,"accepted_packets":0,"attempted_packets":0,
            "optically_verified":false}),
        )
    }
    fn studio_config(&mut self, key: &str) -> std::result::Result<LightingConfig, String> {
        let path = self.uniform_path();
        let store = self.uniform.cache.refresh(&path)?;
        let profiles = store
            .devices
            .get(key)
            .or_else(|| store.devices.get("025D"))
            .ok_or_else(|| {
                "Studio needs retained device brightness and ownership settings".to_string()
            })?;
        let name = profiles.effective(self.foreground.as_deref());
        let profile = profiles
            .profiles
            .get(name)
            .ok_or_else(|| "Studio effective profile is missing".to_string())?;
        Ok(LightingConfig::from_profile(
            name,
            profile,
            !profiles.lighting_off,
            self.away,
        ))
    }
    pub(crate) fn apply_studio_saved(
        &mut self,
        intent: ApplyIntent,
        key: &str,
        archive: &Archive,
        index: usize,
        manifest: &Path,
        random_state: u32,
    ) -> Result<Value> {
        if self.measurement_read_only || !self.uniform_is_enabled() {
            return Err(Error::Unsupported(
                "Studio output needs the running lighting worker and writable device access".into(),
            ));
        }
        if self.lighting_probe_store_snapshot()?.is_some() {
            return Err(Error::Unsupported(
                "Studio apply is unavailable during an isolated lighting measurement".into(),
            ));
        }
        let connection = self.studio.connection_epoch.ok_or_else(|| {
            Error::Unsupported("Studio connection counter exhausted; restart is required".into())
        })?;
        let config = self.studio_config(key).map_err(Error::InvalidArg)?;
        // Suppression may defer first output, but unknown/conflicting ownership
        // cannot claim the Quick target, even when no frame would yet be emitted.
        self.uniform_availability(&config)
            .map_err(Error::Unsupported)?;
        let device = self
            .devices
            .iter()
            .find(|device| device.key() == key)
            .ok_or_else(|| Error::NotFound("Studio target disconnected".into()))?;
        crate::host_uniform::io::preflight(device)?;
        let previous = self.studio.session.target_key().map(str::to_owned);
        self.studio.session.apply_saved(
            intent,
            archive,
            index,
            device,
            manifest,
            connection,
            random_state,
        )?;
        self.studio_applied(previous, key);
        Ok(self.studio_status())
    }
    fn studio_applied(&mut self, previous: Option<String>, key: &str) {
        self.studio.prepared = false;
        self.studio.error = None;
        if let Some(previous) = previous {
            self.clear_uniform_action(&previous);
            self.host_static_results.borrow_mut().remove(&previous);
        }
        self.clear_uniform_action(key);
        self.host_static_results.borrow_mut().remove(key);
        self.lighting_runtime
            .borrow_mut()
            .invalidate(&[key.to_owned()]);
        if let Some(wake) = &self.uniform.wake {
            wake.unpark();
        }
    }
    /// Engine-lock acknowledgement only. The UI's cloned control requests stop
    /// first, independently, so a blocked batch can observe cancellation.
    pub(crate) fn stop_studio(&mut self) -> Result<Value> {
        self.studio_control().request_stop()?;
        let _ = self.release_stopped_studio();
        if let Some(wake) = &self.uniform.wake {
            wake.unpark();
        }
        Ok(self.studio_status())
    }
    fn release_stopped_studio(&mut self) -> bool {
        if self.studio.session.target_key().is_some() && !self.studio.session.stop_requested() {
            return false;
        }
        let previous = self.studio.session.target_key().map(str::to_owned);
        if self
            .studio
            .session
            .poll(self.studio.epoch.elapsed(), Availability::Blocked)
            != Poll::Stopped
        {
            return false;
        }
        self.studio.prepared = false;
        if let Some(key) = previous {
            self.clear_uniform_action(&key);
            self.host_static_results.borrow_mut().remove(&key);
            self.lighting_runtime.borrow_mut().invalidate(&[key]);
        }
        true
    }
    pub(crate) fn guard_studio_quick_action(&self, target: &str, path: &str) -> Result<()> {
        let Some(held) = self.studio.session.target_key() else {
            return Ok(());
        };
        let selected = target == held
            || self
                .devices
                .iter()
                .any(|device| device.key() == held && format!("{:04X}", device.pid) == target);
        if path == "/api/sync"
            || selected
                && matches!(
                    path,
                    "/api/source-effect"
                        | "/api/color"
                        | "/api/effect"
                        | "/api/off"
                        | "/api/keys"
                        | "/api/find-light"
                        | "/api/dimming"
                        | "/api/lighting-control"
                )
        {
            return Err(Error::Unsupported("Stop Studio before changing this device's Quick lighting; saved settings were not changed".into()));
        }
        Ok(())
    }
    pub(super) fn invalidate_studio(&mut self, reason: &str, handles_changed: bool) {
        let _ = self.studio_control().request_stop();
        self.release_stopped_studio();
        self.studio.error = Some(reason.into());
        if handles_changed {
            self.studio.connection_epoch = self
                .studio
                .connection_epoch
                .and_then(|epoch| epoch.checked_add(1));
        }
    }
    pub(super) fn studio_before_brightness(
        &self,
        index: usize,
        config: &mut LightingConfig,
    ) -> bool {
        if !self.studio_holds(&self.devices[index].key()) {
            return false;
        }
        if !self.studio.prepared {
            let runtime = self.lighting_runtime.borrow();
            let plan = crate::lighting_runtime::plan(
                config,
                &runtime.environment,
                runtime.running,
                runtime.stopping,
            );
            if !runtime.stopping && !plan.suppressed && plan.target.is_some_and(|target| target > 0)
            {
                config.error = Some(
                    "Studio requires an accepted frame before positive LIVE brightness".into(),
                );
            }
        }
        true
    }
    pub(super) fn tick_studio(&mut self, stopped: &AtomicBool) -> Duration {
        let wait = Duration::from_millis(250);
        if self.studio.session.target_key().is_none() {
            return wait;
        }
        if stopped.load(Ordering::Acquire) || !self.uniform_is_enabled() {
            self.invalidate_studio("lighting worker stopped", false);
            return wait;
        }
        // First acknowledge lock-independent stop. Polling Blocked cannot admit
        // a frame and never releases a pending batch.
        if self.release_stopped_studio() {
            return wait;
        }
        let key = self.studio.session.target_key().unwrap().to_owned();
        let availability = self
            .studio_config(&key)
            .and_then(|config| self.uniform_availability(&config));
        let availability = match availability {
            Ok(Availability::Ready) => {
                self.studio.error = None;
                Availability::Ready
            }
            Ok(other) => {
                self.studio.prepared = false;
                other
            }
            Err(error) => {
                self.studio.prepared = false;
                self.studio.error = Some(error);
                Availability::Blocked
            }
        };
        match self
            .studio
            .session
            .poll(self.studio.epoch.elapsed(), availability)
        {
            Poll::Wait(wait) => wait,
            Poll::Frame(ticket) => {
                let resume_live = !self.studio.prepared;
                let signal = self
                    .uniform
                    .alive
                    .as_ref()
                    .expect("enabled worker has signal")
                    .clone();
                let completion = match signal.begin_batch() {
                    None => Completion {
                        accepted_packets: 0,
                        attempted_packets: 0,
                        error: Some("Studio worker admission closed".into()),
                    },
                    Some(mut admission) => {
                        let result = match (
                            self.devices.iter_mut().find(|device| device.key() == key),
                            self.studio.connection_epoch,
                        ) {
                            (Some(device), Some(connection)) => self.studio.session.transfer(
                                &ticket,
                                device,
                                connection,
                                Instant::now() + Duration::from_millis(500),
                                || stopped.load(Ordering::Acquire),
                            ),
                            _ => Err(Error::NotFound(
                                "Studio target connection is unavailable".into(),
                            )),
                        };
                        let completion = result.unwrap_or_else(|error| Completion {
                            accepted_packets: 0,
                            attempted_packets: 0,
                            error: Some(error.to_string()),
                        });
                        admission.record(&completion);
                        // The synchronous transfer has returned; no callbacks or
                        // queued work survive releasing this admission.
                        completion
                    }
                };
                let accepted = completion.error.is_none()
                    && completion.accepted_packets == 7
                    && completion.attempted_packets == 7;
                self.studio.error = completion.error.clone();
                let complete =
                    self.studio
                        .session
                        .complete(&ticket, self.studio.epoch.elapsed(), completion);
                self.studio.prepared =
                    accepted && complete.is_ok() && !self.studio.session.is_faulted();
                if let Err(error) = complete {
                    self.studio.error = Some(error.to_string());
                }
                self.release_stopped_studio();
                if self.studio.prepared && resume_live {
                    // Apply/resume is an explicit output action, not merely a
                    // passive policy observation. A previous LIVE0 must not keep
                    // newly accepted Studio frames dark indefinitely.
                    self.lighting_runtime.borrow_mut().request_configured_live(&key);
                    self.refresh_lighting_runtime();
                }
                Duration::from_millis(40)
            }
            _ => wait,
        }
    }
}

#[cfg(test)]
mod tests;
