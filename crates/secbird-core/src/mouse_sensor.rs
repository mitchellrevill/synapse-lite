//! Installed196/197 Smart Tracking, recovered from the selected Orca5Plus path.
//! Capture exact registers; verify readback and restore them on write/save failure.
use crate::{commands as cmd, Device, Error, Result, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CalibrationPreferences {
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracking_distance_mm: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationRaw {
    pub proximity_state: u8,
    pub lift_mode: u8,
    pub lift_height: u8,
}
impl CalibrationRaw {
    /// Preserve non-UI modes and distance3 rather than fabricating a radio value.
    pub fn tracking_distance_mm(self) -> Option<u16> {
        (self.lift_mode == 1).then_some(u16::from(self.lift_height) + 1)
    }
    pub fn for_distance(mm: u8) -> Result<Self> {
        if !matches!(mm, 1 | 2) {
            return Err(Error::InvalidArg(
                "Tracking distance must be1mm or2mm".into(),
            ));
        }
        Ok(Self {
            proximity_state: 1,
            lift_mode: 1,
            lift_height: mm - 1,
        })
    }
}

fn model(dev: &Device) -> Result<()> {
    if !matches!(dev.pid, 196 | 197) {
        return Err(Error::Unsupported(
            "Smart Tracking is verified only for DeathAdder V3 HyperSpeed".into(),
        ));
    }
    Ok(())
}
impl Device {
    /// Reapply explicit device-wide intent on connection/resume, without saving
    /// profiles or coercing a non-UI checkpoint. Named profile switches do not
    /// call this path. An already matching register requires no SET.
    pub fn apply_tracking_distance(&self, mm: u8) -> Result<CalibrationRaw> {
        if self.is_query_only() {
            return Err(Error::Unsupported(
                "Read-only handle refuses Calibration replay".into(),
            ));
        }
        model(self)?;
        let desired = CalibrationRaw::for_distance(mm)?;
        let original = self.calibration_raw()?;
        if original == desired {
            return Ok(desired);
        }
        if let Err(error) = self.apply_calibration_raw(desired) {
            return match self.restore_calibration_raw(original) {
                Ok(()) => Err(Error::Unsupported(format!(
                    "{error}; original calibration restored"
                ))),
                Err(recovery) => Err(Error::Unsupported(format!("{error}; {recovery}"))),
            };
        }
        Ok(desired)
    }
    pub fn calibration_raw(&self) -> Result<CalibrationRaw> {
        model(self)?;
        let state = self.get(cmd::PROXIMITY_STATE, 3, &[0, 4, 0])?;
        let lift = self.get(cmd::PROXIMITY_LIFT, 4, &[0, 4, 0, 0])?;
        if state.0[5] != 3
            || lift.0[5] != 4
            || state.args()[..2] != [0, 4]
            || lift.args()[..2] != [0, 4]
            || state.args()[2] > 1
        {
            return Err(Error::Unsupported(
                "Unexpected Smart Tracking register response".into(),
            ));
        }
        Ok(CalibrationRaw {
            proximity_state: state.args()[2],
            lift_mode: lift.args()[2],
            lift_height: lift.args()[3],
        })
    }

    /// Exact recovery path, not a UI API for arbitrary calibration modes.
    pub fn restore_calibration_raw(&self, raw: CalibrationRaw) -> Result<()> {
        model(self)?;
        if raw.proximity_state > 1 {
            return Err(Error::InvalidArg("Invalid proximity state".into()));
        }
        let state = self.transact(cmd::PROXIMITY_STATE, 3, &[0, 4, raw.proximity_state]);
        let lift = self.transact(
            cmd::PROXIMITY_LIFT,
            4,
            &[0, 4, raw.lift_mode, raw.lift_height],
        );
        // A lost acknowledgement does not prove either mutation or recovery.
        // Independent GETs decide whether the exact original state returned.
        match self.calibration_raw() {
            Ok(observed) if observed==raw => Ok(()),
            observed => Err(Error::Unsupported(format!("Calibration recovery was not verified: state={:?}, lift={:?}, readback={observed:?}",state.err(),lift.err()))),
        }
    }

    fn apply_calibration_raw(&self, raw: CalibrationRaw) -> Result<()> {
        model(self)?;
        let state = self.transact(cmd::PROXIMITY_STATE, 3, &[0, 4, raw.proximity_state])?;
        if state.args()[..3] != [0, 4, raw.proximity_state] {
            return Err(Error::Unsupported(
                "Smart Tracking state acknowledgement differs".into(),
            ));
        }
        let lift = self.transact(
            cmd::PROXIMITY_LIFT,
            4,
            &[0, 4, raw.lift_mode, raw.lift_height],
        )?;
        if lift.args()[..4] != [0, 4, raw.lift_mode, raw.lift_height] {
            return Err(Error::Unsupported(
                "Smart Tracking distance acknowledgement differs".into(),
            ));
        }
        if self.calibration_raw()? != raw {
            return Err(Error::Unsupported(
                "Mouse did not retain the requested tracking distance".into(),
            ));
        }
        Ok(())
    }
}

/// Device-wide intent: never change the active profile or its opaque source data.
pub fn set_tracking_distance(
    dev: &Device,
    store: &mut Store,
    expected_device: &str,
    mm: u8,
) -> Result<CalibrationRaw> {
    if dev.is_query_only() {
        return Err(Error::Unsupported(
            "Read-only handle refuses calibration changes".into(),
        ));
    }
    if expected_device != dev.key() {
        return Err(Error::InvalidArg(
            "Selected mouse changed; refresh Calibration".into(),
        ));
    }
    model(dev)?;
    let desired = CalibrationRaw::for_distance(mm)?;
    let mut candidate = store.clone();
    let prefs = candidate
        .for_device(dev)
        .sensor_calibration
        .get_or_insert_with(Default::default);
    prefs.tracking_distance_mm = Some(mm);
    candidate.validate_save()?;
    let original = dev.calibration_raw()?;
    let outcome = dev
        .apply_calibration_raw(desired)
        .and_then(|_| candidate.save());
    if let Err(error) = outcome {
        return match dev.restore_calibration_raw(original) {
            Ok(()) => Err(Error::Unsupported(format!(
                "{error}; original calibration restored"
            ))),
            Err(recovery) => Err(Error::Unsupported(format!("{error}; {recovery}"))),
        };
    }
    *store = candidate;
    Ok(desired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sim::Sim, Db};
    use serde_json::json;

    fn fixture() -> (Sim, Device, Store, std::path::PathBuf) {
        let sim = Sim::wireless_mouse();
        sim.state().sensor_calibration = Some([0, 0, 2]);
        let db = Db::builtin().unwrap();
        let dev = sim.open(196, db.get(196).cloned()).unwrap();
        let folder = std::env::temp_dir().join(format!(
            "sensor-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        store
            .for_device(&dev)
            .active_mut()
            .extra
            .insert("foreignSensorData".into(), json!({"keep":[1,2,3]}));
        store.save().unwrap();
        sim.state().log.clear();
        (sim, dev, store, folder)
    }
    fn cleanup(folder: std::path::PathBuf) {
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn calibration_is_device_wide_and_verified_without_changing_named_profiles() {
        let (sim, dev, mut store, folder) = fixture();
        let profiles = store.for_device(&dev).profiles.clone();
        let result = set_tracking_distance(&dev, &mut store, &dev.key(), 2).unwrap();
        assert_eq!(result.tracking_distance_mm(), Some(2));
        assert_eq!(sim.state().sensor_calibration, Some([1, 1, 1]));
        assert_eq!(store.for_device(&dev).profiles, profiles);
        assert_eq!(
            store
                .for_device(&dev)
                .sensor_calibration
                .as_ref()
                .unwrap()
                .tracking_distance_mm,
            Some(2)
        );
        let reloaded = Store::load(&folder.join("profiles.json")).unwrap();
        assert_eq!(
            reloaded.devices[&dev.key()]
                .sensor_calibration
                .as_ref()
                .unwrap()
                .tracking_distance_mm,
            Some(2)
        );
        cleanup(folder);
    }
    #[test]
    fn failed_lift_write_restores_exact_non_ui_mode_and_keeps_store_unchanged() {
        let (sim, dev, mut store, folder) = fixture();
        let before = std::fs::read(folder.join("profiles.json")).unwrap();
        sim.state().fail_next_calibration_lift = true;
        assert!(set_tracking_distance(&dev, &mut store, &dev.key(), 1).is_err());
        assert_eq!(
            dev.calibration_raw().unwrap(),
            CalibrationRaw {
                proximity_state: 0,
                lift_mode: 0,
                lift_height: 2
            }
        );
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        assert!(store.for_device(&dev).sensor_calibration.is_none());
        cleanup(folder);
    }
    #[test]
    fn filesystem_failure_restores_sensor_and_invalid_requests_do_not_reach_it() {
        let (sim, dev, mut store, folder) = fixture();
        let before = std::fs::read(folder.join("profiles.json")).unwrap();
        assert!(set_tracking_distance(&dev, &mut store, "stale", 1).is_err());
        assert!(set_tracking_distance(&dev, &mut store, &dev.key(), 3).is_err());
        assert!(sim.state().log.is_empty());
        std::fs::create_dir(folder.join("profiles.json.tmp")).unwrap();
        assert!(set_tracking_distance(&dev, &mut store, &dev.key(), 2).is_err());
        assert_eq!(sim.state().sensor_calibration, Some([0, 0, 2]));
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), before);
        std::fs::remove_dir(folder.join("profiles.json.tmp")).unwrap();
        assert_eq!(
            CalibrationRaw {
                proximity_state: 1,
                lift_mode: 1,
                lift_height: 2
            }
            .tracking_distance_mm(),
            Some(3)
        );
        cleanup(folder);
    }
}
