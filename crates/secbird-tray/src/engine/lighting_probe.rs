//! Read-only admission and immutable configuration boundary for an explicit
//! lighting-only runtime measurement. Ordinary application runs have no pin.
use super::*;
use secbird_core::Error;
use serde_json::{json, Value};
use std::io::Read;

const MAX_PROBE_STORE_BYTES: u64 = 2 * 1024 * 1024;

pub(super) struct Boundary {
    // Exact canonical serialization avoids a new hash dependency/collision rule.
    // The already-enforced native budget bounds this additional retained copy.
    approved_store: Vec<u8>,
    error: Option<String>,
}

impl Engine {
    /// Probe-only fresh read with a hard byte cap, independent of timestamps,
    /// file length metadata and the ordinary worker's parsed Store cache.
    fn read_probe_store(&self) -> Result<Store> {
        #[cfg(test)]
        let path = self
            .runtime_store_path
            .clone()
            .unwrap_or_else(|| config_dir().join("profiles.json"));
        #[cfg(not(test))]
        let path = config_dir().join("profiles.json");
        let file = std::fs::File::open(&path).map_err(|error| Error::Db(error.to_string()))?;
        let mut bytes = Vec::new();
        file.take(MAX_PROBE_STORE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| Error::Db(error.to_string()))?;
        if bytes.len() as u64 > MAX_PROBE_STORE_BYTES {
            return Err(Error::InvalidArg(
                "Lighting probe Store exceeds 2MiB".into(),
            ));
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        serde_json::from_slice(bytes).map_err(|error| Error::Db(error.to_string()))
    }
    /// A probe must inspect current parsed content on every admission. Normal
    /// runs return None without file I/O and retain their existing cache policy.
    pub(super) fn lighting_probe_store_snapshot(&self) -> Result<Option<Store>> {
        if self.lighting_probe.borrow().is_none() {
            return Ok(None);
        }
        if let Some(error) = self.lighting_probe_error() {
            return Err(Error::Unsupported(error));
        }
        let store = self.read_probe_store().map_err(|error| {
            Error::Unsupported(self.latch_lighting_probe_error(&error.to_string()))
        })?;
        self.validate_lighting_probe_store(&store)?;
        Ok(Some(store))
    }
    /// GET-only: no generic apply, initialization, migration, save or opt-in.
    /// Admission is prospective; it does not start the brightness runtime.
    pub(crate) fn preflight_lighting_probe(&self) -> Result<Value> {
        self.preflight_probe(crate::runtime_probe::ProbeKind::Uniform)
    }
    /// Separate Wave-only admission. The worker will independently prepare its
    /// current layout again; this read-only observation never renews a lease.
    pub(crate) fn preflight_wave_probe(&self) -> Result<Value> {
        self.preflight_probe(crate::runtime_probe::ProbeKind::Wave)
    }
    fn preflight_probe(&self, kind: crate::runtime_probe::ProbeKind) -> Result<Value> {
        if self.lighting_probe.borrow().is_some() {
            return Err(Error::InvalidArg("Lighting probe was already admitted; restart rather than replacing its configuration pin".into()));
        }
        let store = self.read_probe_store()?;
        let (key, name) = match kind {
            crate::runtime_probe::ProbeKind::Uniform => {
                crate::runtime_probe::validate_store(&store)?
            }
            crate::runtime_probe::ProbeKind::Wave => {
                crate::runtime_probe::validate_wave_store(&store)?
            }
        };
        let candidates: Vec<_> = self.devices.iter().filter(|dev| dev.pid == 605).collect();
        if candidates.len() != 1 {
            return Err(Error::Unsupported(
                "Lighting probe requires exactly one connected Ornata candidate".into(),
            ));
        }
        let dev = candidates[0];
        if dev.key() != key
            || dev.interface != 2
            || !dev
                .spec
                .as_ref()
                .and_then(|spec| spec.matrix)
                .is_some_and(|matrix| matrix.rows == 6 && matrix.cols == 22)
        {
            return Err(Error::Unsupported("Lighting probe requires the exact configured Ornata025D/interface2 physical6x22 device".into()));
        }
        let profiles = &store.devices[&key];
        if profiles.lighting_off || profiles.active != name {
            return Err(Error::Unsupported("Lighting probe requires active lighting ownership and the persisted active profile".into()));
        }
        let profile = &profiles.profiles[&name];
        let program = crate::host_uniform::Program::from_profile(profile, 605)?
            .filter(|program| program.opted_in(profile))
            .ok_or_else(|| Error::Unsupported("Lighting probe requires existing exact opted-in Spectrum or fixed Breathing intent".into()))?;
        let config = LightingConfig::from_profile(&name, profile, true, self.away);
        self.lighting_runtime.borrow_mut().sample_admission();
        let runtime = self.lighting_runtime.borrow();
        let plan = crate::lighting_runtime::plan(&config, &runtime.environment, true, false);
        if !config.source_known || !config.enabled || plan.suppressed || !plan.can_execute {
            return Err(Error::Unsupported(format!(
                "Lighting probe is not presently runnable without initialization: {:?}",
                plan.blocked
            )));
        }
        drop(runtime);
        let mode = dev.ornata_mode_readback()?;
        let effect = dev.ornata_effect_readback(0, 0)?;
        if mode.mode() != 3
            || !effect.selectors_match()
            || effect.declared_size() < 6
            || effect.effect_id() != 8
            || effect.flags() != 0
            || effect.rate() != 0
            || effect.color_count() != 0
        {
            return Err(Error::Unsupported("Lighting probe requires existing Driver_Mode3 and profile0/region0 CustomFrame8 flags0/rate0/count0".into()));
        }
        let live = dev.ornata_brightness_readback(0, 0)?;
        let saved = dev.ornata_brightness_readback(1, 0)?;
        let expected_raw = config.inferred_live_raw.or_else(|| {
            config
                .baseline()
                .map(|percent| (u16::from(percent) * 255 / 100) as u8)
        });
        if !live.selectors_match()
            || !saved.selectors_match()
            || live.raw_brightness() == 0
            || expected_raw != Some(live.raw_brightness())
        {
            return Err(Error::Unsupported("Lighting probe requires nonzero LIVE brightness already matching existing configured intent; no bootstrap/brightness edit is performed".into()));
        }
        let wave_preparation = if let crate::host_uniform::Program::Wave(choice) = program {
            let path = crate::native_editor::assets_dir().join("DeviceManifest_605_0.json");
            Some(crate::host_wave::preparation::prepare(dev, choice, &path)?)
        } else {
            None
        };
        // Recheck the known process gate after the control queries. This is not
        // an interprocess lease; the worker continues its ordinary fresh gates.
        self.lighting_runtime.borrow_mut().sample_owner();
        if !self.lighting_runtime.borrow().owner_clear() {
            return Err(Error::Unsupported(
                "A known competing lighting owner appeared during probe admission".into(),
            ));
        }
        let approved_store =
            serde_json::to_vec(&store).map_err(|error| Error::Db(error.to_string()))?;
        *self.lighting_probe.borrow_mut() = Some(Boundary {
            approved_store,
            error: None,
        });
        let mut admission = json!({
            "key":key,"profile":name,"source_id":program.source_id(),"native_id":program.native_id(),
            "mode":mode.mode(),"mode_parameter":mode.param(),"effect":effect.effect_id(),
            "live_raw":live.raw_brightness(),"saved_raw":saved.raw_brightness(),
            "configuration_pinned":true,"initialization_performed":false,"get_only":true
        });
        if let Some(prepared) = wave_preparation {
            admission["active_wave_probe"] = json!(true);
            admission["preflight_wave_preparation"] = prepared.metadata_at(Instant::now());
            admission["worker_prepares_independently"] = json!(true);
        }
        Ok(admission)
    }

    /// The first refusal is sticky. Returning the file to its old contents or
    /// calling preflight again cannot quietly authorize another session.
    pub(super) fn latch_lighting_probe_error(&self, reason: &str) -> String {
        let mut boundary = self.lighting_probe.borrow_mut();
        if let Some(boundary) = boundary.as_mut() {
            boundary
                .error
                .get_or_insert_with(|| format!("Lighting probe stopped until exit: {reason}"))
                .clone()
        } else {
            reason.to_owned()
        }
    }
    pub(super) fn lighting_probe_error(&self) -> Option<String> {
        self.lighting_probe
            .borrow()
            .as_ref()
            .and_then(|boundary| boundary.error.clone())
    }
    pub(super) fn validate_lighting_probe_store(&self, store: &Store) -> Result<()> {
        let borrowed = self.lighting_probe.borrow();
        let Some(boundary) = borrowed.as_ref() else {
            return Ok(());
        };
        if let Some(error) = &boundary.error {
            return Err(Error::Unsupported(error.clone()));
        }
        let current = serde_json::to_vec(store).map_err(|error| Error::Db(error.to_string()))?;
        let changed = current != boundary.approved_store;
        // Release the RefCell read before latching through a mutable borrow.
        drop(borrowed);
        if changed {
            let message = self.latch_lighting_probe_error("configured content changed");
            return Err(Error::Unsupported(message));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host_static::test_support::{self, Spy},
        host_uniform::Program,
        lighting_runtime::{Environment, Owner},
    };
    use secbird_core::{
        effects::Effect,
        lighting::{Brightness, LightingSettings},
        Profile,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        engine: Engine,
        spy: Spy,
        path: std::path::PathBuf,
        key: String,
    }
    impl Fixture {
        fn use_wave(&self) {
            let mut store = Store::load(&self.path).unwrap();
            let profile = store.devices.get_mut(&self.key).unwrap().active_mut();
            secbird_core::profile_compat::set_source_quick_effect(
                profile,
                605,
                "Probe",
                4,
                &json!({"direction": 1}),
            )
            .unwrap();
            crate::host_uniform::Program::clear_opt_in(profile);
            profile.lighting.get_mut("backlight").unwrap().effect =
                Some(crate::host_wave::Program::Left.native_projection());
            crate::host_wave::Program::Left.opt_in(profile).unwrap();
            crate::runtime_probe::validate_wave_store(&store).unwrap();
            store.save().unwrap();
        }
        fn new() -> Self {
            HidApi::disable_device_discovery();
            let mut engine = Engine::from_api(HidApi::new().unwrap());
            let (dev, spy) = test_support::fixture();
            let key = dev.key();
            let path = std::env::temp_dir()
                .join(format!(
                    "lighting-probe-{}-{}",
                    std::process::id(),
                    SEQ.fetch_add(1, Ordering::Relaxed)
                ))
                .join("profiles.json");
            let mut profile = Profile::default();
            secbird_core::profile_compat::set_source_quick_effect(
                &mut profile,
                605,
                "Probe",
                3,
                &json!({}),
            )
            .unwrap();
            // Synthetic minimal Lighting envelope; no real user data is read.
            let quick = secbird_core::profile_compat::source_quick_effects(&profile)
                .unwrap()
                .clone();
            profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"] =
                json!({"name":"Probe","guid":"SYNTHETIC-PROBE","quickEffects":quick});
            let zone = profile.lighting.entry("backlight".into()).or_default();
            zone.effect = Some(Effect::Spectrum);
            zone.brightness = Some(100);
            LightingSettings {
                brightness: Some(Brightness {
                    enabled: true,
                    value: 100,
                    ..Default::default()
                }),
                ..Default::default()
            }
            .store_in_profile(&mut profile)
            .unwrap();
            Program::Spectrum.opt_in(&mut profile).unwrap();
            let mut store = Store::load(&path).unwrap();
            let profiles = store.for_device(&dev);
            profiles.active = "Probe".into();
            profiles.profiles = std::collections::BTreeMap::from([("Probe".into(), profile)]);
            crate::runtime_probe::validate_store(&store).unwrap();
            store.save().unwrap();
            spy.sim.state().brightness.insert(5, 255);
            spy.sim.state().live_brightness.insert(5, 255);
            engine.devices.push(dev);
            engine.simulated_only = true;
            engine.runtime_store_path = Some(path.clone());
            engine.lighting_runtime.borrow_mut().test_environment = Some(Environment {
                now_ms: 1000,
                idle_ms: Ok(0),
                display_off: Some(false),
                owner: Owner::Clear,
            });
            spy.state.lock().unwrap().commands.clear();
            Self {
                engine,
                spy,
                path,
                key,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(self.path.parent().unwrap()).unwrap();
        }
    }
    #[test]
    fn wave_admission_refuses_wrong_scope_owner_and_unavailable_identity_without_set() {
        for scenario in ["scope", "owner", "mode", "identity"] {
            let mut f = Fixture::new();
            f.use_wave();
            let before = std::fs::read(&f.path).unwrap();
            match scenario {
                "scope" => f.engine.devices[0].interface = 1,
                "owner" => {
                    f.engine
                        .lighting_runtime
                        .borrow_mut()
                        .test_environment
                        .as_mut()
                        .unwrap()
                        .owner = Owner::Competing(vec!["synthetic competing owner".into()])
                }
                "mode" => f.spy.state.lock().unwrap().mode = 0,
                "identity" => (), // Mock device has no identity response unless explicitly configured.
                _ => unreachable!(),
            }
            assert!(f.engine.preflight_wave_probe().is_err(), "{scenario}");
            assert_eq!(std::fs::read(&f.path).unwrap(), before);
            assert!(f.engine.lighting_probe.borrow().is_none());
            assert!(!f.engine.lighting_runtime.borrow().running);
            let spy = f.spy.state.lock().unwrap();
            assert_eq!(spy.attempts, 0);
            assert!(spy.commands.iter().all(|command| command.id & 0x80 != 0));
        }
    }

    #[test]
    #[ignore = "requires explicit SECBIRD_ORNATA_MANIFEST_TEST_PATH; mock transport only, never opens hardware"]
    fn wave_preflight_authenticates_the_explicit_asset_without_starting_or_writing() {
        let path = std::path::PathBuf::from(
            std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH")
                .expect("explicit pinned manifest path"),
        );
        assert_eq!(path.file_name().unwrap(), "DeviceManifest_605_0.json");
        let old = std::env::var_os("SYNAPSE_LITE_ASSETS");
        std::env::set_var("SYNAPSE_LITE_ASSETS", path.parent().unwrap());
        let f = Fixture::new();
        f.use_wave();
        f.spy.sim.state().ornata_layout_edition = Some([6, 0]);
        let before = std::fs::read(&f.path).unwrap();
        let admission = f.engine.preflight_wave_probe().unwrap();
        match old {
            Some(value) => std::env::set_var("SYNAPSE_LITE_ASSETS", value),
            None => std::env::remove_var("SYNAPSE_LITE_ASSETS"),
        }
        assert_eq!(admission["source_id"], 4);
        assert_eq!(admission["native_id"], 2);
        assert_eq!(admission["worker_prepares_independently"], true);
        assert_eq!(admission["preflight_wave_preparation"]["layout_id"], 6);
        assert!(admission["preflight_wave_preparation"]["firmware_id"].is_null());
        assert_eq!(
            admission["preflight_wave_preparation"]["authenticated_manifest_sha256"],
            secbird_core::ornata_spatial::SOURCE_MANIFEST_SHA256
        );
        assert_eq!(std::fs::read(&f.path).unwrap(), before);
        assert!(f.engine.lighting_probe.borrow().is_some());
        assert!(!f.engine.lighting_runtime.borrow().running);
        assert!(!f.engine.uniform_is_enabled());
        let spy = f.spy.state.lock().unwrap();
        assert_eq!(spy.attempts, 0);
        assert_eq!(
            spy.commands
                .iter()
                .filter(|command| command.class == 0 && command.id == 0x86)
                .count(),
            1
        );
        assert!(spy.commands.iter().all(|command| command.id & 0x80 != 0));
    }

    #[test]
    #[ignore = "requires explicit SECBIRD_ORNATA_MANIFEST_TEST_PATH; real worker with mock transport only"]
    fn admitted_wave_uses_the_ordinary_worker_and_reports_authenticated_clean_stop() {
        let path = std::path::PathBuf::from(
            std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH")
                .expect("explicit pinned manifest path"),
        );
        assert_eq!(path.file_name().unwrap(), "DeviceManifest_605_0.json");
        // Run this explicit test alone: assets_dir is a process-wide setting.
        let old = std::env::var_os("SYNAPSE_LITE_ASSETS");
        std::env::set_var("SYNAPSE_LITE_ASSETS", path.parent().unwrap());
        let mut f = Fixture::new();
        f.use_wave();
        f.spy.sim.state().ornata_layout_edition = Some([6, 0]);
        let before = std::fs::read(&f.path).unwrap();
        f.engine.preflight_wave_probe().unwrap();
        let replacement = Engine::from_api(HidApi::new().unwrap());
        let engine = std::sync::Arc::new(std::sync::Mutex::new(std::mem::replace(
            &mut f.engine,
            replacement,
        )));
        engine.lock().unwrap().start_lighting_runtime();
        let mut worker = crate::uniform_worker::Worker::start(engine.clone()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let observed = loop {
            if let Ok(engine) = engine.try_lock() {
                let runtime = engine.lighting_runtime_status();
                let status = crate::runtime_probe::readiness_for(
                    crate::runtime_probe::ProbeKind::Wave,
                    &runtime[&f.key],
                    "Probe",
                );
                if status["ready"] == true {
                    break status;
                }
            }
            if Instant::now() >= deadline {
                assert!(worker.stop(Duration::from_secs(2)));
                panic!("Mock Wave worker did not produce authenticated readiness");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(worker.stop(Duration::from_secs(2)));
        let mut engine = engine.lock().unwrap();
        let runtime = engine.lighting_runtime_status();
        let stopped = crate::runtime_probe::readiness_for(
            crate::runtime_probe::ProbeKind::Wave,
            &runtime[&f.key],
            "Probe",
        );
        let report =
            crate::runtime_probe::stop_report(Some((&observed, 0)), true, 0, Some(&stopped));
        assert_eq!(report["clean_stop"], true);
        assert_eq!(report["authenticated_layout_ready_at_observation"], true);
        assert_eq!(report["source_id"], 4);
        assert_eq!(report["native_id"], 2);
        engine.set_uniform_worker(false, None, None);
        assert!(engine.shutdown_lighting_runtime().is_empty());
        assert_eq!(std::fs::read(&f.path).unwrap(), before);
        let spy = f.spy.state.lock().unwrap();
        assert!(spy.attempts >= 21);
        assert_eq!(spy.attempts % 7, 0);
        assert_eq!(
            spy.commands
                .iter()
                .filter(|command| command.class == 0 && command.id == 0x86)
                .count(),
            2,
            "Preflight and the independent worker preparation each query identity once"
        );
        // Unlike GET-only admission, the ordinary worker may reconcile the
        // configured LIVE brightness after a completed frame. It stays 255;
        // no saved brightness, mode, input or firmware effect path is invoked.
        assert!(spy.commands.iter().all(|command| command.id & 0x80 != 0
            || command.class == 15 && matches!(command.id, 2..=4)));
        assert_eq!(f.spy.sim.state().live_brightness[&5], 255);
        assert_eq!(f.spy.sim.state().brightness[&5], 255);
        drop(spy);
        match old {
            Some(value) => std::env::set_var("SYNAPSE_LITE_ASSETS", value),
            None => std::env::remove_var("SYNAPSE_LITE_ASSETS"),
        }
    }
    #[test]
    fn admission_is_get_only_and_preserves_disk_opt_in_and_runtime_state() {
        let f = Fixture::new();
        let before = std::fs::read(&f.path).unwrap();
        let result = f.engine.preflight_lighting_probe().unwrap();
        assert_eq!(result["key"], f.key);
        assert_eq!(result["mode"], 3);
        assert!(result["mode_parameter"].is_null());
        assert_eq!(result["get_only"], true);
        assert_eq!(std::fs::read(&f.path).unwrap(), before);
        assert!(!f.engine.lighting_runtime.borrow().running);
        assert_eq!(f.engine.lighting_runtime.borrow().status(), json!({}));
        assert!(!f.engine.uniform_is_enabled());
        let spy = f.spy.state.lock().unwrap();
        assert_eq!(spy.attempts, 0);
        assert_eq!(spy.commands.len(), 4);
        assert!(spy.commands.iter().all(|command| command.id & 0x80 != 0));
    }
    #[test]
    fn admission_preserves_validated_raw_bootstrap_without_initialization() {
        for (saved, live, percent) in [(128, 128, 51), (0, 186, 73)] {
            let f = Fixture::new();
            let mut store = Store::load(&f.path).unwrap();
            let profile = store.devices.get_mut(&f.key).unwrap().active_mut();
            let mut settings = profile.lighting_settings.clone().unwrap();
            settings.brightness.as_mut().unwrap().value = percent;
            settings.store_in_profile(profile).unwrap();
            secbird_core::profile_compat::record_ornata_brightness_baseline(profile, saved, live)
                .unwrap();
            store.save().unwrap();
            let before = std::fs::read(&f.path).unwrap();
            f.spy.sim.state().brightness.insert(5, saved);
            f.spy.sim.state().live_brightness.insert(5, live);
            let result = f.engine.preflight_lighting_probe().unwrap();
            assert_eq!(result["live_raw"], live);
            assert_eq!(result["saved_raw"], saved);
            assert_eq!(std::fs::read(&f.path).unwrap(), before);
            assert_eq!(f.spy.sim.state().live_brightness[&5], live);
            assert!(f
                .spy
                .state
                .lock()
                .unwrap()
                .commands
                .iter()
                .all(|command| command.id & 0x80 != 0));
        }
    }
    #[test]
    fn scope_configuration_owner_and_control_refusals_never_send_set_or_create_a_pin() {
        for reason in [
            "scope",
            "nonlighting",
            "owner",
            "unopted",
            "mode",
            "custom",
            "live-mismatch",
            "extra-device",
        ] {
            let mut f = Fixture::new();
            match reason {
                "scope" => f.engine.devices[0].interface = 0,
                "nonlighting" | "unopted" | "extra-device" => {
                    let mut store = Store::load(&f.path).unwrap();
                    if reason == "nonlighting" {
                        store.devices.get_mut(&f.key).unwrap().active_mut().polling = Some(1000);
                    }
                    if reason == "unopted" {
                        Program::clear_opt_in(store.devices.get_mut(&f.key).unwrap().active_mut());
                    }
                    if reason == "extra-device" {
                        store
                            .devices
                            .insert("00C4:OTHER-SYNTHETIC".into(), Default::default());
                    }
                    store.save().unwrap();
                }
                "owner" => {
                    f.engine
                        .lighting_runtime
                        .borrow_mut()
                        .test_environment
                        .as_mut()
                        .unwrap()
                        .owner = Owner::Competing(vec!["RazerAppEngine.exe".into()])
                }
                "mode" => f.spy.state.lock().unwrap().mode = 0,
                "custom" => {
                    f.spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
                }
                _ => {
                    f.spy.sim.state().live_brightness.insert(5, 30);
                }
            }
            let before = std::fs::read(&f.path).unwrap();
            assert!(f.engine.preflight_lighting_probe().is_err(), "{reason}");
            assert!(f.engine.lighting_probe.borrow().is_none());
            assert_eq!(std::fs::read(&f.path).unwrap(), before);
            let spy = f.spy.state.lock().unwrap();
            assert_eq!(spy.attempts, 0);
            assert!(spy.commands.iter().all(|command| command.id & 0x80 != 0));
        }
    }
    #[test]
    fn same_size_change_cannot_hide_behind_an_approved_store_with_matching_cache_stamp() {
        let mut f = Fixture::new();
        f.engine.preflight_lighting_probe().unwrap();
        let original = std::fs::read(&f.path).unwrap();
        let approved = Store::load(&f.path).unwrap();
        f.engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        f.engine.start_lighting_runtime();
        f.engine.tick_uniform(&AtomicBool::new(false));
        let commands = f.spy.state.lock().unwrap().commands.len();
        let mut changed = Store::load(&f.path).unwrap();
        changed
            .devices
            .get_mut(&f.key)
            .unwrap()
            .active_mut()
            .extra
            .get_mut("_synapseLiteSynapse4")
            .unwrap()["decoded"]["name"] = json!("Other");
        crate::runtime_probe::validate_store(&changed).unwrap();
        changed.save().unwrap();
        assert_eq!(
            std::fs::metadata(&f.path).unwrap().len(),
            original.len() as u64
        );
        // Stronger than restoring mtime: force a cache hit with the old approved
        // Store and the changed file's actual current path/stamp/length.
        f.engine.seed_unchanged_uniform_cache_for_test(approved);
        f.engine.tick_uniform(&AtomicBool::new(false));
        assert!(f.engine.lighting_probe_error().is_some());
        std::fs::write(&f.path, original).unwrap();
        f.engine.tick_uniform(&AtomicBool::new(false));
        f.engine.refresh_lighting_runtime();
        assert_eq!(f.spy.state.lock().unwrap().commands.len(), commands);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
    }
    #[test]
    fn fresh_probe_reads_are_bounded_and_semantic_whitespace_changes_are_allowed() {
        let mut f = Fixture::new();
        f.engine.preflight_lighting_probe().unwrap();
        let parsed: Value = serde_json::from_slice(&std::fs::read(&f.path).unwrap()).unwrap();
        let mut rewritten = vec![0xef, 0xbb, 0xbf];
        rewritten.extend_from_slice(b" \n");
        rewritten.extend(serde_json::to_vec(&parsed).unwrap());
        rewritten.extend_from_slice(b" \n");
        std::fs::write(&f.path, rewritten).unwrap();
        assert!(f.engine.lighting_probe_store_snapshot().unwrap().is_some());
        let commands = f.spy.state.lock().unwrap().commands.len();
        std::fs::write(&f.path, vec![b' '; MAX_PROBE_STORE_BYTES as usize + 1]).unwrap();
        f.engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        f.engine.tick_uniform(&AtomicBool::new(false));
        f.engine.refresh_lighting_runtime();
        assert!(f.engine.lighting_probe_error().unwrap().contains("2MiB"));
        assert_eq!(f.spy.state.lock().unwrap().commands.len(), commands);
        assert_eq!(f.spy.state.lock().unwrap().attempts, 0);
        let f = Fixture::new();
        std::fs::remove_file(&f.path).unwrap();
        assert!(
            f.engine.lighting_probe_store_snapshot().unwrap().is_none(),
            "ordinary runtime does no extra probe file read"
        );
    }
    #[test]
    fn changed_or_invalid_store_latches_before_frame_and_brightness_until_exit() {
        for invalid_json in [false, true] {
            let mut f = Fixture::new();
            f.engine.preflight_lighting_probe().unwrap();
            let before = std::fs::read(&f.path).unwrap();
            f.engine.set_uniform_worker(
                true,
                None,
                Some(crate::uniform_worker::Signal::running_for_test()),
            );
            f.engine.start_lighting_runtime();
            f.engine.tick_uniform(&AtomicBool::new(false));
            assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
            let commands = f.spy.state.lock().unwrap().commands.len();
            if invalid_json {
                std::fs::write(&f.path, b"not valid JSON").unwrap();
            } else {
                let mut store = Store::load(&f.path).unwrap();
                store
                    .devices
                    .get_mut(&f.key)
                    .unwrap()
                    .active_mut()
                    .buttons
                    .insert("synthetic".into(), "key:a".into());
                store.save().unwrap();
            }
            f.engine.refresh_lighting_runtime();
            f.engine.invalidate_uniform();
            f.engine.tick_uniform(&AtomicBool::new(false));
            std::fs::write(&f.path, &before).unwrap();
            f.engine.invalidate_uniform();
            f.engine.refresh_lighting_runtime();
            f.engine.tick_uniform(&AtomicBool::new(false));
            assert!(f.engine.preflight_lighting_probe().is_err());
            assert_eq!(f.spy.state.lock().unwrap().commands.len(), commands);
            assert_eq!(f.spy.state.lock().unwrap().attempts, 7);
            assert!(f
                .engine
                .lighting_probe
                .borrow()
                .as_ref()
                .unwrap()
                .error
                .is_some());
        }
    }
}
