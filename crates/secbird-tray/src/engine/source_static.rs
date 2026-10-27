//! Mode-aware source Static gates, local attempt ledger and integration tests.
use super::*;

impl Engine {
    pub(crate) fn validate_static_context(
        &self,
        key: &str,
        store: &Store,
        expected_profile: &str,
    ) -> Result<()> {
        self.lighting_runtime.borrow_mut().sample_owner();
        let dev = self
            .devices
            .iter()
            .find(|d| d.key() == key)
            .ok_or_else(|| secbird_core::Error::NotFound(key.into()))?;
        if dev.pid != 605
            || dev.interface != 2
            || !dev
                .spec
                .as_ref()
                .and_then(|s| s.matrix)
                .is_some_and(|m| m.rows == 6 && m.cols == 22)
        {
            return Err(secbird_core::Error::Unsupported(
                "Host Static requires exact Ornata interface2 physical6x22 matrix".into(),
            ));
        }
        let default_profiles = secbird_core::profile::DeviceProfiles::default();
        let profiles = store
            .devices
            .get(key)
            .or_else(|| store.devices.get("025D"))
            .unwrap_or(&default_profiles);
        if profiles.effective(self.foreground()) != expected_profile {
            return Err(secbird_core::Error::InvalidArg(
                "The effective profile changed; refresh Lighting".into(),
            ));
        }
        if profiles.lighting_off {
            return Err(secbird_core::Error::Unsupported(
                "Lighting ownership is disabled".into(),
            ));
        }
        let runtime = self.lighting_runtime.borrow();
        if !runtime.owner_clear() {
            return Err(secbird_core::Error::Unsupported(format!(
                "Known lighting owner is not clear: {:?}",
                runtime.environment.owner
            )));
        }
        Ok(())
    }
    /// Pure host/process/suppression gate: it never changes device brightness.
    pub(crate) fn host_static_gate(
        &self,
        key: &str,
        store: &Store,
        expected_profile: &str,
    ) -> Result<crate::host_static::Gate> {
        self.validate_static_context(key, store, expected_profile)?;
        self.lighting_runtime.borrow_mut().sample_host();
        let dev = self
            .devices
            .iter()
            .find(|d| d.key() == key)
            .ok_or_else(|| secbird_core::Error::NotFound(key.into()))?;
        if dev.pid != 605 || dev.interface != 2 {
            return Err(secbird_core::Error::Unsupported(
                "Host Static requires exact Ornata interface2".into(),
            ));
        }
        let default_profiles = secbird_core::profile::DeviceProfiles::default();
        let profiles = store
            .devices
            .get(key)
            .or_else(|| store.devices.get("025D"))
            .unwrap_or(&default_profiles);
        let name = profiles.effective(self.foreground());
        if name != expected_profile {
            return Err(secbird_core::Error::InvalidArg(
                "The effective profile changed; refresh Lighting".into(),
            ));
        }
        let config = LightingConfig::from_profile(
            name,
            &profiles.profiles[name],
            !profiles.lighting_off,
            self.away,
        );
        let runtime = self.lighting_runtime.borrow();
        let decision = crate::lighting_runtime::plan(
            &config,
            &runtime.environment,
            runtime.running,
            runtime.stopping,
        );
        if let Some(reason) = decision.blocked {
            return Err(secbird_core::Error::Unsupported(format!(
                "Host Static blocked: {reason}"
            )));
        }
        if runtime.stopping {
            return Err(secbird_core::Error::Unsupported(
                "Host Static runtime is stopping".into(),
            ));
        }
        if !config.source_known {
            return Ok(crate::host_static::Gate::NeedsBaseline {
                environment: runtime.environment.clone(),
                away_percent: self.away,
            });
        }
        if !config.enabled
            || decision.suppressed
            || decision.target == Some(0)
            || self.away.is_some()
        {
            return Ok(crate::host_static::Gate::Deferred(
                "brightness/policy/global-away suppression".into(),
            ));
        }
        let status = runtime.status();
        if let Some(error) = status.get(key).and_then(|v| v["error"].as_str()) {
            return Err(secbird_core::Error::Unsupported(format!(
                "Host Static brightness bridge failed: {error}"
            )));
        }
        Ok(crate::host_static::Gate::Ready)
    }

    pub(crate) fn device_mut(&mut self, key: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.key() == key)
    }
    pub(super) fn apply_source_static(
        dev: &mut Device,
        color: secbird_core::Rgb,
    ) -> Result<serde_json::Value> {
        let mode = dev.ornata_mode_readback()?;
        match mode.mode() {
            0 => dev
                .set_static_color(5, color)
                .map(|_| serde_json::json!({"execution":"firmware"})),
            3 => crate::host_static::send_uniform(dev, color),
            value => Err(secbird_core::Error::Unsupported(format!(
                "Source Static mode{value} is unsupported"
            ))),
        }
    }
    pub(crate) fn static_is_prepared(
        &self,
        key: &str,
        name: &str,
        color: secbird_core::Rgb,
    ) -> bool {
        self.host_static_results
            .borrow()
            .get(key)
            .is_some_and(|(p, c, v)| {
                p == name
                    && *c == color
                    && v["execution"] == "host-static"
                    && (v["usb_out_complete"] == true || v["frame_already_prepared"] == true)
                    && v["deferred"] != true
                    && !v["error"].is_string()
            })
    }
    pub(crate) fn record_host_static(
        &self,
        key: &str,
        name: &str,
        color: secbird_core::Rgb,
        result: &Result<serde_json::Value>,
    ) {
        let mut value = match result {
            Ok(v) => v.clone(),
            Err(e) => serde_json::json!({"error":e.to_string(),"automatic_retry":false}),
        };
        value["profile_name"] = serde_json::json!(name);
        value["configured_color"] = serde_json::json!(color.to_string());
        if value["execution"] == "host-static"
            && !value["error"].is_string()
            && value["live_policy_reconcile_pending"] != false
        {
            value["live_policy_reconcile_pending"] = serde_json::json!(true);
        }
        self.host_static_results
            .borrow_mut()
            .insert(key.into(), (name.into(), color, value));
    }
}

#[cfg(test)]
mod host_static_tests {
    use super::*;
    use crate::{
        host_static::test_support::{self, Spy},
        lighting_runtime::{Environment, Owner},
    };
    use secbird_core::{
        commands as cmd,
        lighting::{Brightness, LightingSettings, SwitchOffLighting},
        Rgb,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        engine: Engine,
        spy: Spy,
        path: std::path::PathBuf,
        key: String,
    }
    impl Fixture {
        fn new() -> Self {
            HidApi::disable_device_discovery();
            let mut engine = Engine::from_api(HidApi::new().unwrap());
            let (dev, spy) = test_support::fixture();
            let key = dev.key();
            let folder = std::env::temp_dir().join(format!(
                "source-static-engine-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let path = folder.join("profiles.json");
            let mut store = Store::load(&path).unwrap();
            let mut profile =
                secbird_core::profile_compat::new_native_profile(605, "Default").unwrap();
            secbird_core::profile_compat::set_source_quick_effect(
                &mut profile,
                605,
                "Default",
                1,
                &serde_json::json!({"color1":"#010203"}),
            )
            .unwrap();
            let z = profile.lighting.entry("backlight".into()).or_default();
            z.color = Some(Rgb(1, 2, 3));
            z.effect = None;
            z.off = false;
            LightingSettings {
                brightness: Some(Brightness {
                    enabled: true,
                    value: 41,
                    ..Default::default()
                }),
                switch_off: Some(SwitchOffLighting {
                    display_off: true,
                    idle_enabled: true,
                    idle_minutes: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }
            .store_in_profile(&mut profile)
            .unwrap();
            store
                .for_device(&dev)
                .profiles
                .insert("Default".into(), profile);
            store.save().unwrap();
            spy.sim.state().brightness.insert(5, 186);
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
            Self {
                engine,
                spy,
                path,
                key,
            }
        }
        fn edit(&self, f: impl FnOnce(&mut Store, &Device)) {
            let mut store = Store::load(&self.path).unwrap();
            f(&mut store, &self.engine.devices[0]);
            store.save().unwrap();
        }
        fn enabled(&self, enabled: bool) {
            self.edit(|store, dev| {
                let p = store.for_device(dev).active_mut();
                let mut settings = p.lighting_settings.clone().unwrap();
                settings.brightness.as_mut().unwrap().enabled = enabled;
                settings.store_in_profile(p).unwrap();
            });
        }
        fn color(&self, color: Rgb) {
            self.edit(|store, dev| {
                store
                    .for_device(dev)
                    .active_mut()
                    .lighting
                    .get_mut("backlight")
                    .unwrap()
                    .color = Some(color)
            });
        }
        fn uniform(&self, color: [u8; 3]) -> bool {
            self.spy
                .sim
                .state()
                .matrix
                .as_ref()
                .unwrap()
                .2
                .iter()
                .all(|c| *c == color)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
        }
    }
    #[test]
    fn activation_uses_latest_native_color_before_live_brightness_and_never_saved_effect() {
        let mut f = Fixture::new();
        f.color(Rgb(9, 8, 7));
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0].failures.is_empty(), "{:?}", reports[0].failures);
        assert!(f.uniform([9, 8, 7]));
        assert_eq!(f.spy.sim.state().brightness[&5], 186);
        assert_eq!(
            f.spy.sim.state().live_brightness[&5],
            104,
            "{}",
            f.engine.lighting_runtime_status()
        );
        assert!(!f.spy.sim.state().effects.contains_key(&5));
        let commands = f.spy.state.lock().unwrap().commands.clone();
        let first_frame = commands
            .iter()
            .position(|c| *c == cmd::MATRIX_FRAME)
            .unwrap();
        let first_brightness = commands
            .iter()
            .position(|c| *c == cmd::MATRIX_BRIGHTNESS)
            .unwrap();
        assert!(first_frame < first_brightness);
        let attempts = f.spy.state.lock().unwrap().attempts;
        f.engine.refresh_frames();
        assert_eq!(
            f.spy.state.lock().unwrap().attempts,
            attempts,
            "unchanged Static is not streamed repeatedly"
        );
        f.engine.host_static_results.borrow_mut().clear();
        f.engine.apply(std::slice::from_ref(&f.key));
        assert_eq!(
            f.spy.state.lock().unwrap().attempts,
            attempts + 7,
            "a reconnect/explicit activation rechecks and renders"
        );
    }
    #[test]
    fn off_intent_retains_latest_color_and_resume_prepares_it_before_brightness() {
        let mut f = Fixture::new();
        f.enabled(false);
        f.engine.apply(std::slice::from_ref(&f.key));
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        f.color(Rgb(4, 5, 6));
        f.engine.request_lighting_refresh();
        f.color(Rgb(7, 8, 9));
        f.engine.request_lighting_refresh();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        f.spy.state.lock().unwrap().commands.clear();
        f.enabled(true);
        f.engine.request_lighting_refresh();
        assert!(f.uniform([7, 8, 9]));
        let commands = f.spy.state.lock().unwrap().commands.clone();
        assert!(
            commands
                .iter()
                .position(|c| *c == cmd::MATRIX_FRAME)
                .unwrap()
                < commands
                    .iter()
                    .position(|c| *c == cmd::MATRIX_BRIGHTNESS)
                    .unwrap()
        );
        assert_eq!(f.spy.sim.state().brightness[&5], 186);
        assert_eq!(
            f.spy.sim.state().live_brightness[&5],
            104,
            "{}",
            f.engine.lighting_runtime_status()
        );
    }
    #[test]
    fn failed_frame_never_raises_brightness_or_retries_on_ticks() {
        let mut f = Fixture::new();
        f.spy.state.lock().unwrap().fail_at = Some(2);
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0]
            .failures
            .iter()
            .any(|s| s.contains("accepted2 attempted3")));
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
        f.engine.request_lighting_refresh();
        f.engine.refresh_frames();
        assert_eq!(f.spy.state.lock().unwrap().attempts, 3);
        f.spy.state.lock().unwrap().fail_at = None;
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0].failures.is_empty(), "{:?}", reports[0].failures);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 10);
    }
    #[test]
    fn owner_budget_and_native_overrides_do_not_send_static_frames() {
        let mut f = Fixture::new();
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .owner = Owner::Competing(vec!["RazerAppEngine.exe".into()]);
        let store = Store::load(&f.path).unwrap();
        assert!(f
            .engine
            .validate_static_context(&f.key, &store, "Default")
            .is_err());
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
        f.engine
            .lighting_runtime
            .borrow_mut()
            .test_environment
            .as_mut()
            .unwrap()
            .owner = Owner::Clear;
        let valid_bytes = std::fs::read(&f.path).unwrap();
        let mut oversized = Store::load(&f.path).unwrap();
        oversized.extra.insert("large".into(),serde_json::json!("x".repeat(2 * 1024 * 1024)));
        // Model an externally supplied oversized file, bypassing validating Save.
        std::fs::write(&f.path,serde_json::to_vec(&oversized).unwrap()).unwrap();
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(!reports[0].failures.is_empty());
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
        std::fs::write(&f.path, valid_bytes).unwrap();
        f.edit(|store, dev| {
            store.extra.remove("large");
            store
                .for_device(dev)
                .active_mut()
                .lighting
                .get_mut("backlight")
                .unwrap()
                .off = true;
        });
        let store = Store::load(&f.path).unwrap();
        assert_eq!(
            store.devices[&f.key].active().source_static_color(605),
            None
        );
        f.edit(|store, dev| {
            let p = store.for_device(dev).active_mut();
            p.lighting.get_mut("backlight").unwrap().off = false;
            p.keys = vec![Rgb(1, 1, 1); 132];
        });
        let store = Store::load(&f.path).unwrap();
        assert_eq!(
            store.devices[&f.key].active().source_static_color(605),
            None
        );
    }
    #[test]
    fn bootstrap_observation_does_not_round_or_reapply_initial_raw_live_brightness() {
        let mut f = Fixture::new();
        f.spy.sim.state().brightness.insert(5, 128);
        f.spy.sim.state().live_brightness.insert(5, 128);
        f.edit(|store, dev| {
            let p = store.for_device(dev).active_mut();
            let mut settings = p.lighting_settings.clone().unwrap();
            settings.brightness.as_mut().unwrap().value = 51;
            settings.store_in_profile(p).unwrap();
            secbird_core::profile_compat::record_ornata_brightness_baseline(p, 128, 128).unwrap();
        });
        let mut response =
            crate::host_static::send_uniform(&mut f.engine.devices[0], Rgb(1, 2, 3)).unwrap();
        response["live_policy_reconcile_pending"] = serde_json::json!(false);
        f.engine
            .record_host_static(&f.key, "Default", Rgb(1, 2, 3), &Ok(response));
        f.engine.request_lighting_refresh();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 128);
        assert_eq!(f.spy.sim.state().brightness[&5], 128);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
        assert!(!f
            .spy
            .state
            .lock()
            .unwrap()
            .commands
            .contains(&cmd::MATRIX_BRIGHTNESS));
    }
    #[test]
    fn first_unpersisted_default_requests_verified_baseline_without_mutating_guard_store() {
        let f = Fixture::new();
        let store = Store::default();
        assert!(matches!(
            f.engine
                .host_static_gate(&f.key, &store, "Default")
                .unwrap(),
            crate::host_static::Gate::NeedsBaseline { .. }
        ));
        assert!(store.devices.is_empty());
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
        assert!(f.engine.host_static_gate(&f.key, &store, "stale").is_err());
        assert!(f.spy.state.lock().unwrap().commands.is_empty());
    }
    #[test]
    fn startup_off_intent_darkens_without_painting_and_does_not_repeat_live_writes() {
        let mut f = Fixture::new();
        f.enabled(false);
        f.spy.sim.state().live_brightness.insert(5, 186);
        f.engine.start_lighting_runtime();
        assert_eq!(f.spy.sim.state().live_brightness[&5], 0);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        let writes = f
            .spy
            .state
            .lock()
            .unwrap()
            .commands
            .iter()
            .filter(|c| **c == cmd::MATRIX_BRIGHTNESS)
            .count();
        assert_eq!(writes, 1);
        f.engine.refresh_lighting_runtime();
        assert_eq!(
            f.spy
                .state
                .lock()
                .unwrap()
                .commands
                .iter()
                .filter(|c| **c == cmd::MATRIX_BRIGHTNESS)
                .count(),
            writes
        );
        assert_eq!(
            f.engine.lighting_runtime_status()[&f.key]["host_static"]["deferred"],
            true
        );
    }
    #[test]
    fn normal_mode_preserves_saved_brightness_and_shutdown_does_not_render_deferred_color() {
        let mut f = Fixture::new();
        f.spy.state.lock().unwrap().mode = 0;
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0].failures.is_empty(), "{:?}", reports[0].failures);
        assert_eq!(f.spy.sim.state().brightness[&5], 104);
        assert_eq!(f.spy.sim.state().live_brightness[&5], 104);
        assert!(f.spy.sim.state().effects.contains_key(&5));
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        let mut f = Fixture::new();
        f.enabled(false);
        f.engine.apply(std::slice::from_ref(&f.key));
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert!(f.engine.shutdown_lighting_runtime().is_empty());
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
    }
    #[test]
    fn standalone_core_refuses_driver_static_before_any_lighting_brightness_write() {
        let f = Fixture::new();
        let mut store = Store::load(&f.path).unwrap();
        let mut profile = store.for_device(&f.engine.devices[0]).active().clone();
        profile.game_mode = None;
        let outcomes = profile.apply(&f.engine.devices[0]);
        assert!(outcomes.iter().any(|o| o
            .result
            .as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("running tray host-frame runtime"))));
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        assert!(!f.spy.sim.state().log.contains(&cmd::MATRIX_BRIGHTNESS));
        assert!(!f.spy.sim.state().effects.contains_key(&5));
    }
}
