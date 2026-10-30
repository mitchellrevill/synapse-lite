//! Device-lifetime Rotation ownership. No hooks or replacement driver installation.
use super::*;
use secbird_core::{
    dynamic_sensitivity::Rotation,
    sensor_rotation_control::{read_rotation, RotationController},
};
use serde_json::{json, Value};
impl Engine {
    pub(crate) fn rotation_snapshot(&mut self, id: &str, profile: &str) -> Result<Value> {
        let index = self
            .devices
            .iter()
            .position(|d| d.key() == id)
            .ok_or_else(|| secbird_core::Error::InvalidArg("Selected mouse disconnected".into()))?;
        let device = &self.devices[index];
        let mut store = self.load_store()?;
        if store.for_device(device).active != profile {
            return Err(secbird_core::Error::InvalidArg(
                "Selected profile changed".into(),
            ));
        }
        let intent = store.for_device(device).active().rotation.clone();
        let observed = if self.measurement_read_only {
            read_rotation(device)?
        } else {
            if !self.sensor_rotation.contains_key(id) {
                self.sensor_rotation
                    .insert(id.into(), RotationController::open(device)?);
            }
            self.sensor_rotation.get_mut(id).unwrap().observe(device)?
        };
        Ok(json!({"available":true,"observed":observed,"intent":intent}))
    }
    pub(crate) fn set_rotation(&mut self, id: &str, profile: &str, intent: Rotation) -> Result<()> {
        if self.measurement_read_only {
            return Err(secbird_core::Error::Unsupported(
                "Read-only measurement refuses Rotation".into(),
            ));
        }
        intent.validate()?;
        // The filter state is shared with Synapse. Never intentionally compete.
        #[cfg(not(test))]
        if !secbird_core::debloat::synapse_running().is_empty() {
            return Err(secbird_core::Error::Unsupported(
                "Close Razer Synapse before changing Rotation".into(),
            ));
        }
        let device =
            self.devices.iter().find(|d| d.key() == id).ok_or_else(|| {
                secbird_core::Error::InvalidArg("Selected mouse disconnected".into())
            })?;
        let mut candidate = self.load_store()?;
        if candidate.for_device(device).active != profile {
            return Err(secbird_core::Error::InvalidArg(
                "Selected profile changed; refresh Advanced".into(),
            ));
        }
        candidate.for_device(device).active_mut().rotation = Some(intent.clone());
        candidate.validate_save()?;
        if !self.sensor_rotation.contains_key(id) {
            self.sensor_rotation
                .insert(id.into(), RotationController::open(device)?);
        }
        self.sensor_rotation
            .get_mut(id)
            .unwrap()
            .apply_with_commit(device, &intent, || candidate.save())?;
        Ok(())
    }
    pub(super) fn replay_rotation(&mut self, id: &str, profile: &str) -> Result<()> {
        let device = self
            .devices
            .iter()
            .find(|d| d.key() == id)
            .ok_or_else(|| secbird_core::Error::InvalidArg("Mouse disconnected".into()))?;
        let mut store = self.load_store()?;
        let intent = store
            .for_device(device)
            .profiles
            .get(profile)
            .and_then(|p| p.rotation.as_ref());
        if let Some(intent) = intent {
            #[cfg(not(test))]
            if !secbird_core::debloat::synapse_running().is_empty() {
                return Err(secbird_core::Error::Unsupported(
                    "Rotation not applied while Synapse is running".into(),
                ));
            }
            if !self.sensor_rotation.contains_key(id) {
                self.sensor_rotation
                    .insert(id.into(), RotationController::open(device)?);
            }
            self.sensor_rotation
                .get_mut(id)
                .unwrap()
                .apply(device, intent)?;
        } else if let Some(controller) = self.sensor_rotation.get_mut(id) {
            controller.restore(device)?;
        }
        Ok(())
    }
    pub(super) fn shutdown_rotation(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        for (id, mut controller) in self.sensor_rotation.drain() {
            if let Some(device) = self.devices.iter().find(|d| d.key() == id) {
                if let Err(error) = controller.restore(device) {
                    failures.push(format!("Rotation restoration: {error}"));
                }
            }
        }
        failures
    }
    /// Arrival/resume is distinct from a named/foreground profile switch.
    /// Replay device-global Calibration only here, without resaving the store.
    pub(crate) fn apply_connected(&mut self, keys: &[String]) -> Vec<ApplyReport> {
        if self.measurement_read_only {
            return Vec::new();
        }
        let mut reports = self.apply(keys);
        // apply() reports in connected-device order, including failures. Match
        // that order, never the display name shared by identical mouse models.
        for (device, report) in self
            .devices
            .iter()
            .filter(|device| keys.contains(&device.key()))
            .zip(&mut reports)
        {
            let replay = (|| -> Result<()> {
                if matches!(device.pid, 196 | 197) {
                    device.require_mouse_identity()?;
                }
                let mut store = self.load_store()?;
                let preference = store
                    .for_device(device)
                    .sensor_calibration
                    .as_ref()
                    .and_then(|p| p.tracking_distance_mm);
                if let Some(mm) = preference {
                    #[cfg(test)]
                    let clear = if self.simulated_only {
                        self.synapse.is_empty()
                    } else {
                        secbird_core::debloat::synapse_running().is_empty()
                    };
                    #[cfg(not(test))]
                    let clear = secbird_core::debloat::synapse_running().is_empty();
                    if !clear {
                        return Err(secbird_core::Error::Unsupported(
                            "Calibration not reapplied while Synapse is running".into(),
                        ));
                    }
                    device.apply_tracking_distance(mm)?;
                }
                Ok(())
            })();
            if let Err(error) = replay {
                report.failures.push(format!("Calibration: {error}"));
            }
        }
        reports
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        commands as cmd, mouse_sensor::CalibrationPreferences, profile::Profile, sim::Sim,
    };
    use std::path::PathBuf;
    fn fixture() -> (Engine, Sim, PathBuf) {
        let sim = Sim::wireless_mouse();
        sim.state().sensor_calibration = Some([0, 0, 4]);
        let mut engine = Engine::with_simulated_mouse(&sim).unwrap();
        let path = std::env::temp_dir().join(format!(
            "sensor-lifecycle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        engine.runtime_store_path = Some(path.join("profiles.json"));
        (engine, sim, path)
    }
    fn seed(engine: &Engine, folder: &std::path::Path, alias: bool) {
        let dev = &engine.devices[0];
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        let key = if alias {
            format!("00C5:{}", dev.serial)
        } else {
            dev.key()
        };
        let prefs = store.profiles_for(&key, if alias { 197 } else { 196 });
        prefs.sensor_calibration = Some(CalibrationPreferences {
            tracking_distance_mm: Some(2),
            ..Default::default()
        });
        prefs
            .active_mut()
            .extra
            .insert("opaque".into(), json!({"keep":true}));
        prefs.profiles.insert("Other".into(), Profile::default());
        store.save().unwrap();
    }
    fn cleanup(folder: PathBuf) {
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn connection_replays_global_calibration_but_profile_switch_does_not() {
        let (mut engine, sim, folder) = fixture();
        seed(&engine, &folder, true);
        let key = engine.devices[0].key();
        let bytes = std::fs::read(folder.join("profiles.json")).unwrap();
        assert!(engine.apply_connected(std::slice::from_ref(&key))[0]
            .failures
            .is_empty());
        assert_eq!(sim.state().sensor_calibration, Some([1, 1, 1]));
        assert_eq!(
            std::fs::read(folder.join("profiles.json")).unwrap(),
            bytes,
            "Connection replay must not resave or migrate aliases on disk"
        );
        let count = sim
            .state()
            .log
            .iter()
            .filter(|c| **c == cmd::PROXIMITY_LIFT)
            .count();
        assert!(engine.apply_connected(std::slice::from_ref(&key))[0]
            .failures
            .is_empty());
        assert_eq!(
            sim.state()
                .log
                .iter()
                .filter(|c| **c == cmd::PROXIMITY_LIFT)
                .count(),
            count,
            "Matching settings should not be rewritten"
        );
        sim.state().sensor_calibration = Some([0, 0, 4]);
        assert!(engine.switch_profile(&key, "Other").unwrap()[0]
            .failures
            .is_empty());
        assert_eq!(
            sim.state().sensor_calibration,
            Some([0, 0, 4]),
            "Named profile selection must not replay global Calibration"
        );
        assert!(engine.apply_connected(std::slice::from_ref(&key))[0]
            .failures
            .is_empty());
        assert_eq!(sim.state().sensor_calibration, Some([1, 1, 1]));
        cleanup(folder);
    }
    #[test]
    fn calibration_replay_failure_restores_checkpoint_and_owner_or_readonly_refuses_set() {
        let (mut engine, sim, folder) = fixture();
        seed(&engine, &folder, false);
        let key = engine.devices[0].key();
        let before = std::fs::read(folder.join("profiles.json")).unwrap();
        sim.state().fail_next_calibration_lift = true;
        let report = engine.apply_connected(std::slice::from_ref(&key));
        assert!(report[0]
            .failures
            .iter()
            .any(|s| s.contains("original calibration restored")));
        assert_eq!(sim.state().sensor_calibration, Some([0, 0, 4]));
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        sim.state().log.clear();
        engine.synapse.push("RazerAppEngine.exe".into());
        let report = engine.apply_connected(std::slice::from_ref(&key));
        assert!(report[0]
            .failures
            .iter()
            .any(|s| s.contains("while Synapse is running")));
        assert!(!sim
            .state()
            .log
            .iter()
            .any(|c| *c == cmd::PROXIMITY_LIFT || *c == cmd::PROXIMITY_STATE));
        engine.enforce_measurement_read_only().unwrap();
        sim.state().log.clear();
        assert!(engine.apply_connected(&[key]).is_empty());
        assert!(sim.state().log.is_empty());
        cleanup(folder);
    }
    #[test]
    fn rotation_replay_resolves_same_serial_connection_alias_and_preserves_lifetime_baseline() {
        let (mut engine, sim, folder) = fixture();
        {
            let mut s = sim.state();
            s.firmware_extended = Some([1, 10, 1, 0]);
            s.sensor_rotation = Some([1, 0, 0]);
        }
        let device = &engine.devices[0];
        let key = device.key();
        let alias = format!("00C5:{}", device.serial);
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        let profiles = store.profiles_for(&alias, 197);
        profiles.active_mut().rotation = Some(Rotation {
            is_enabled: true,
            value: 22,
            ..Default::default()
        });
        profiles.profiles.insert(
            "Other".into(),
            Profile {
                rotation: Some(Rotation {
                    is_enabled: true,
                    value: -12,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        store.save().unwrap();
        let before = std::fs::read(folder.join("profiles.json")).unwrap();
        assert!(engine.apply(&[key.clone()])[0].failures.is_empty());
        assert_eq!(sim.state().sensor_rotation, Some([1, 1, 22]));
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        assert!(engine.switch_profile(&key, "Other").unwrap()[0]
            .failures
            .is_empty());
        assert_eq!(sim.state().sensor_rotation, Some([1, 1, (-12i8) as u8]));
        assert!(engine.shutdown_rotation().is_empty());
        assert_eq!(sim.state().sensor_rotation, Some([1, 0, 0]));
        cleanup(folder);
    }
    #[test]
    fn identical_model_names_never_route_calibration_to_the_other_physical_mouse() {
        let (mut engine, first, folder) = fixture();
        seed(&engine, &folder, false);
        let second = Sim::wireless_mouse();
        {
            let mut s = second.state();
            s.serial = "OTHER-MOUSE".into();
            s.sensor_calibration = Some([0, 0, 3]);
        }
        let db = secbird_core::Db::builtin().unwrap();
        engine
            .devices
            .push(second.open(196, db.get(196).cloned()).unwrap());
        assert_eq!(engine.devices[0].name(), engine.devices[1].name());
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        store.for_device(&engine.devices[1]).sensor_calibration = Some(CalibrationPreferences {
            tracking_distance_mm: Some(1),
            ..Default::default()
        });
        store.save().unwrap();
        let keys = engine.all_keys();
        let reports = engine.apply_connected(&keys);
        assert_eq!(reports.len(), 2);
        assert!(reports.iter().all(|r| r.failures.is_empty()));
        assert_eq!(first.state().sensor_calibration, Some([1, 1, 1]));
        assert_eq!(second.state().sensor_calibration, Some([1, 1, 0]));
        first.state().sensor_calibration = Some([0, 0, 4]);
        second.state().sensor_calibration = Some([0, 0, 3]);
        let reports = engine.apply_connected(&[keys[1].clone()]);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].failures.is_empty());
        assert_eq!(first.state().sensor_calibration, Some([0, 0, 4]));
        assert_eq!(second.state().sensor_calibration, Some([1, 1, 0]));
        cleanup(folder);
    }

    #[test]
    fn wireless_lifecycle_releases_handles_and_replays_exact_serial_without_resaving() {
        let (mut engine, sim, folder) = fixture();
        seed(&engine, &folder, false);
        {
            let mut hardware = sim.state();
            hardware.firmware_extended = Some([1,10,1,0]);
            hardware.sensor_rotation = Some([1,0,0]);
        }
        let mut store = engine.load_store().unwrap();
        store.for_device(&engine.devices[0]).active_mut().rotation = Some(Rotation {
            is_enabled: true, value: 22, ..Default::default()
        });
        store.save().unwrap();
        let original_key = engine.devices[0].key();
        assert!(engine.apply_connected(&[original_key])[0].failures.is_empty());
        assert_eq!(sim.state().sensor_rotation, Some([1,1,22]));
        let before = std::fs::read(folder.join("profiles.json")).unwrap();
        let db = secbird_core::Db::builtin().unwrap();
        let unrelated = Sim::wireless_mouse();
        unrelated.state().serial = "UNRELATED-RECEIVER".into();
        let old_key = engine.devices[0].key();
        let old_token = engine.devices[0].connection_id();
        engine.live_dpi.insert(old_key.clone(), (1200,1200));
        engine.foreground_profiles.insert(old_key.clone(), "Default".into());
        engine.relight.insert(old_key.clone(), Instant::now()+RELIGHT_FOR);
        engine.release_connections();
        assert_eq!(sim.state().sensor_rotation, Some([1,0,0]));
        assert!(engine.sensor_rotation.is_empty());
        assert!(engine.live_dpi.is_empty());
        assert!(engine.foreground_profiles.is_empty());
        assert!(engine.relight.is_empty());
        engine.devices.clear(); // unplug, no reachable mouse
        assert!(engine.connection_replay_keys(&HashSet::new()).is_empty());
        sim.state().available = false;
        assert!(sim.open(197, db.get(197).cloned()).is_none()); // sleep is not presence
        sim.state().available = true;
        sim.state().sensor_calibration = Some([0,0,4]);
        engine.devices.push(sim.open(197, db.get(197).cloned()).unwrap());
        engine.devices.push(unrelated.open(197, db.get(197).cloned()).unwrap());
        assert_ne!(engine.devices[0].connection_id(), old_token);
        let key = engine.devices[0].key();
        let reports = engine.apply_connected(std::slice::from_ref(&key));
        assert!(reports[0].failures.is_empty(), "{:?}", reports[0].failures);
        assert_eq!(sim.state().sensor_calibration, Some([1,1,1]));
        assert_eq!(sim.state().sensor_rotation, Some([1,1,22]));
        assert!(unrelated.state().log.iter().all(|c| c.id & 0x80 != 0));
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        // A same-key reopen must still get the Calibration replay, even when
        // there was no intermediate rescan that observed the unplug.
        let previous = engine.all_keys().into_iter().collect();
        engine.release_connections();
        engine.devices.clear();
        engine.devices.push(sim.open(197, db.get(197).cloned()).unwrap());
        assert_eq!(engine.connection_replay_keys(&previous), vec![key.clone()]);
        sim.state().sensor_calibration = Some([0,0,4]);
        sim.state().fail_next_calibration_lift = true;
        let reports = engine.apply_connected(&[key]);
        assert!(!reports[0].failures.is_empty());
        assert_eq!(sim.state().sensor_calibration, Some([0,0,4]));
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        cleanup(folder);
    }
}
