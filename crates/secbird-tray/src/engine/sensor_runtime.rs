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
        let store = self.load_store()?;
        let intent = store
            .devices
            .get(id)
            .and_then(|p| p.profiles.get(profile))
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
}
