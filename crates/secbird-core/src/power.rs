//! Verified DeathAdder V3 HyperSpeed power threshold protocol.
//!
//! The shipped UI edits percent in steps of five; the device stores one raw byte.
//! Percent conversion is lossy outside that UI grid. Save and restore the raw byte
//! when temporarily changing the device, including values outside the UI range.
use crate::{commands as cmd, Device, Error, Result};

/// Source getter uses floor(raw / 255 * 100).
pub const fn critical_percent(raw: u8) -> u8 {
    (raw as u16 * 100 / 255) as u8
}

/// Source setter uses ceil(percent / 100 * 255), for the rendered 5..100 step-5 UI.
pub fn critical_raw_for_percent(percent: u8) -> Result<u8> {
    if !(5..=100).contains(&percent) || percent % 5 != 0 {
        return Err(Error::InvalidArg(
            "low-power threshold must be 5-100% in steps of 5".into(),
        ));
    }
    Ok((u16::from(percent) * 255).div_ceil(100) as u8)
}

impl Device {
    fn require_power_model(&self) -> Result<()> {
        if matches!(self.pid, 196 | 197) {
            Ok(())
        } else {
            Err(Error::Unsupported(
                "Power controls are verified only for DeathAdder V3 HyperSpeed".into(),
            ))
        }
    }

    /// Exact device sleep seconds for this model, independent of a connection's
    /// legacy wireless-device flag. The source Power page reads hardware directly.
    pub fn device_sleep_seconds(&self) -> Result<u16> {
        self.require_power_model()?;
        let report = self.get(cmd::IDLE_TIME, cmd::IDLE_TIME_SIZE, &[])?;
        if report.0[5] != cmd::IDLE_TIME_SIZE {
            return Err(Error::Unsupported(
                "unexpected device sleep response length".into(),
            ));
        }
        Ok(u16::from_be_bytes([report.args()[0], report.args()[1]]))
    }

    /// Source Power-page sleep action. It does not alter any named profile.
    pub fn set_device_sleep_minutes(&self, minutes: u16) -> Result<()> {
        self.require_power_model()?;
        if !(1..=15).contains(&minutes) {
            return Err(Error::InvalidArg(
                "device sleep must be 1-15 minutes".into(),
            ));
        }
        let seconds = minutes * 60;
        self.require_mouse_identity()?;
        self.transact(cmd::IDLE_TIME, cmd::IDLE_TIME_SIZE, &seconds.to_be_bytes())?;
        if self.device_sleep_seconds()? != seconds {
            return Err(Error::Unsupported(
                "the device did not retain the requested sleep timer".into(),
            ));
        }
        Ok(())
    }

    /// Read the exact byte before a temporary change. No percent normalization.
    pub fn critical_threshold_raw(&self) -> Result<u8> {
        self.require_power_model()?;
        let report = self.get(cmd::CRITICAL_LEVEL, cmd::CRITICAL_LEVEL_SIZE, &[])?;
        if report.0[5] != cmd::CRITICAL_LEVEL_SIZE {
            return Err(Error::Unsupported(
                "unexpected critical-threshold response length".into(),
            ));
        }
        Ok(report.args()[0])
    }

    /// Write and verify an exact byte, suitable for restoring a captured register.
    pub fn set_critical_threshold_raw(&self, raw: u8) -> Result<()> {
        self.require_power_model()?;
        self.require_mouse_identity()?;
        self.transact(cmd::CRITICAL_LEVEL, cmd::CRITICAL_LEVEL_SIZE, &[raw])?;
        if self.critical_threshold_raw()? != raw {
            return Err(Error::Unsupported(
                "the device did not retain the raw low-power threshold".into(),
            ));
        }
        Ok(())
    }

    /// Present the source UI's floor-converted value. Never use it as a raw backup.
    pub fn critical_threshold_percent(&self) -> Result<u8> {
        self.critical_threshold_raw().map(critical_percent)
    }

    /// Set one of the twenty source UI values and verify the exact encoded byte.
    /// This does not persist a profile: Synapse keeps this in device configuration.
    pub fn set_critical_threshold_percent(&self, percent: u8) -> Result<u8> {
        self.require_power_model()?;
        let raw = critical_raw_for_percent(percent)?;
        self.set_critical_threshold_raw(raw)?;
        Ok(critical_percent(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Sim;

    fn mouse(pid: u16) -> (Sim, Device) {
        let sim = Sim::wireless_mouse();
        sim.state().critical_threshold = Some(78);
        let dev = sim.open(pid, None).unwrap();
        (sim, dev)
    }

    #[test]
    fn all_twenty_source_ui_values_round_trip_through_real_codec() {
        for pid in [196, 197] {
            let (sim, dev) = mouse(pid);
            for percent in (5..=100).step_by(5) {
                let sent = critical_raw_for_percent(percent).unwrap();
                assert_eq!(
                    dev.set_critical_threshold_percent(percent).unwrap(),
                    percent
                );
                assert_eq!(sim.state().critical_threshold, Some(sent));
                assert_eq!(dev.critical_threshold_raw().unwrap(), sent);
                assert_eq!(dev.critical_threshold_percent().unwrap(), percent);
            }
            assert!(sim.state().log.contains(&cmd::CRITICAL_LEVEL));
            assert!(sim.state().log.contains(&cmd::CRITICAL_LEVEL.get()));
        }
    }

    #[test]
    fn every_raw_value_can_be_restored_without_a_percent_round_trip() {
        let (_, dev) = mouse(196);
        for raw in 0..=255 {
            dev.set_critical_threshold_raw(raw).unwrap();
            let backup = dev.critical_threshold_raw().unwrap();
            dev.set_critical_threshold_percent(100).unwrap();
            dev.set_critical_threshold_raw(backup).unwrap();
            assert_eq!(dev.critical_threshold_raw().unwrap(), raw);
        }
        // Example proving why a percentage is insufficient as a backup.
        assert_eq!(critical_percent(78), 30);
        assert_eq!(critical_raw_for_percent(30).unwrap(), 77);
    }

    #[test]
    fn unsupported_models_and_invalid_ui_values_send_nothing() {
        let (sim, dev) = mouse(196);
        let before = sim.state().log.len();
        for percent in [0, 1, 4, 6, 99, 101, 255] {
            assert!(dev.set_critical_threshold_percent(percent).is_err());
        }
        assert_eq!(sim.state().log.len(), before);
        let (sim, dev) = mouse(605);
        let before = sim.state().log.len();
        assert!(dev.critical_threshold_raw().is_err());
        assert!(dev.set_critical_threshold_raw(77).is_err());
        assert!(dev.critical_threshold_percent().is_err());
        assert!(dev.set_critical_threshold_percent(30).is_err());
        assert_eq!(sim.state().log.len(), before);
    }

    #[test]
    fn unsupported_hardware_register_is_an_error() {
        let sim = Sim::mouse();
        let dev = sim.open(196, None).unwrap();
        assert!(dev.critical_threshold_raw().is_err());
        assert!(dev.set_critical_threshold_raw(77).is_err());
    }

    #[test]
    fn wireless_lifecycle_never_writes_to_sleeping_or_changed_identity() {
        for pid in [196, 197] {
            let (sim, dev) = mouse(pid);
            let original = sim.state().idle_seconds;
            sim.state().available = false; // sleep/unplug/failure all mean unknown
            assert!(dev.set_device_sleep_minutes(4).is_err());
            assert!(dev.set_critical_threshold_raw(64).is_err());
            assert_eq!(sim.state().idle_seconds, original);
            assert_eq!(sim.state().critical_threshold, Some(78));
            assert!(sim.open(pid, None).is_none());
            sim.state().available = true;
            sim.state().serial = "UNRELATED-MOUSE".into();
            sim.state().log.clear();
            assert!(dev.set_device_sleep_minutes(4).is_err());
            assert!(dev.set_critical_threshold_raw(64).is_err());
            assert!(sim.state().log.iter().all(|c| c.id & 0x80 != 0));
            sim.state().serial = dev.serial.clone();
            let reopened = sim.open(pid, None).unwrap();
            assert_eq!(reopened.key(), dev.key());
            assert_ne!(reopened.connection_id(), dev.connection_id());
            reopened.set_device_sleep_minutes(4).unwrap();
            reopened.set_critical_threshold_raw(64).unwrap();
            assert_eq!(sim.state().idle_seconds, Some(240));
            assert_eq!(sim.state().critical_threshold, Some(64));
        }
    }

    #[test]
    fn ambiguous_receiver_does_not_become_a_mouse() {
        let sim = Sim::wireless_mouse();
        for serial in ["", "000000", "FFFFFF"] {
            sim.state().serial = serial.into();
            assert!(sim.open(197, None).is_none());
        }
        // A shared dongle retains its own PID even when it reports a paired mouse.
        sim.state().serial = "SHARED-RECEIVER".into();
        sim.state().paired = vec![(true, 196)];
        let shared = sim.open(0x00b3, None).unwrap();
        sim.state().log.clear();
        assert!(shared.set_device_sleep_minutes(4).is_err());
        assert!(shared.set_critical_threshold_raw(64).is_err());
        assert!(sim.state().log.is_empty());
    }
}
