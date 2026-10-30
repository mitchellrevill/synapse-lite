//! Device196 onboard performance settings. Exact wire contracts are kept separate
//! from the older generic button-slot path. See docs/architecture.md.
use crate::{commands as cmd, proto::Command, Device, DpiStages, Error, Result, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Capabilities {
    pub slots: u8,
    pub dpi: bool,
    pub polling: bool,
    pub buttons: bool,
    pub brightness: bool,
    pub macros: bool,
}
pub fn capabilities(device: &Device) -> Capabilities {
    let supported = matches!(device.pid, 196 | 197);
    Capabilities {
        slots: u8::from(supported),
        dpi: supported,
        polling: supported,
        buttons: false,
        brightness: false,
        macros: false,
    }
}
fn require(device: &Device) -> Result<()> {
    if capabilities(device).slots != 1 {
        return Err(Error::Unsupported(
            "Onboard performance settings are established only for Device196/197".into(),
        ));
    }
    if device.serial.is_empty() {
        return Err(Error::Unsupported(
            "Onboard recovery requires a stable nonempty physical-device serial".into(),
        ));
    }
    Ok(())
}

/// Raw GET payloads, including reserved bytes. Only explicit known selectors can
/// be replayed; imported metadata never supplies command IDs or arbitrary I/O.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub dpi: Vec<u8>,
    pub stages: Vec<u8>,
    pub polling: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recovery {
    pub schema: u8,
    pub serial: String,
    pub slot: u8,
    pub profile: String,
    pub phase: Phase,
    pub before: Settings,
    pub desired: Settings,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Saving,
    Applied,
    Restoring,
}

impl Recovery {
    fn validate(&self, device: &Device) -> Result<()> {
        require(device)?;
        if self.schema != 1 || self.slot != 1 || self.serial != device.serial {
            return Err(Error::InvalidArg("Onboard recovery belongs to a different physical device or unsupported format; record retained".into()));
        }
        self.before.validate()?;
        self.desired.validate()
    }
}

impl Settings {
    /// Compare applicable register bytes, excluding the unused stage buffer tail.
    pub fn matches_readback(&self, actual: &Self) -> bool {
        if self.validate().is_err() || actual.validate().is_err() {
            return false;
        }
        let n = 3 + usize::from(self.stages[2]) * 7;
        self.dpi == actual.dpi
            && self.polling == actual.polling
            && self.stages[..n] == actual.stages[..n]
    }
    pub fn dpi_pair(&self) -> (u16, u16) {
        (
            u16::from_be_bytes([self.dpi[1], self.dpi[2]]),
            u16::from_be_bytes([self.dpi[3], self.dpi[4]]),
        )
    }
    pub fn dpi_stages(&self) -> DpiStages {
        DpiStages {
            active: self.stages[1],
            stages: self.stages[3..3 + usize::from(self.stages[2]) * 7]
                .chunks_exact(7)
                .map(|s| {
                    (
                        u16::from_be_bytes([s[1], s[2]]),
                        u16::from_be_bytes([s[3], s[4]]),
                    )
                })
                .collect(),
            ..Default::default()
        }
    }
    fn validate(&self) -> Result<()> {
        wire::validate(self)
    }
    fn project(&self, profile: &crate::Profile) -> Result<Self> {
        let mut desired = self.clone();
        if let Some(performance) = crate::performance::Performance::from_profile(profile)? {
            let packed = performance.pack()?;
            wire::project_dpi(&mut desired, &packed)?;
        }
        if let Some(hz) = profile.polling {
            wire::project_polling(&mut desired, hz)?;
        }
        desired.validate()?;
        Ok(desired)
    }
}

/// Source period codes, including readable originals absent from this product's UI.
pub fn polling_hz(code: u8) -> Option<u32> {
    match code {
        1 => Some(1000),
        2 => Some(500),
        3 => Some(333),
        4 => Some(250),
        5 => Some(200),
        8 => Some(125),
        10 => Some(100),
        _ => None,
    }
}
mod wire;

/// Mandatory GETs have no fallback to another slot or another family's format.
pub fn read(device: &Device) -> Result<Settings> {
    require(device)?;
    let value = wire::read(device)?;
    value.validate()?;
    Ok(value)
}
fn identity(
    device: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
) -> Result<()> {
    require(device)?;
    if device.is_query_only() {
        return Err(Error::Unsupported(
            "Read-only device refuses onboard changes".into(),
        ));
    }
    if device.key() != expected_device || store.for_device(device).active != expected_profile {
        return Err(Error::InvalidArg(
            "Selected device or active profile changed; refresh onboard memory".into(),
        ));
    }
    Ok(())
}

/// Persist *all* originals before the first SET. Every error leaves a complete
/// durable journal; subsequent saves cannot replace an unfinished recovery.
pub fn save(
    device: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
) -> Result<Settings> {
    identity(device, store, expected_device, expected_profile)?;
    let mut candidate = store.clone();
    let profiles = candidate.for_device(device);
    let profile = profiles.active().clone();
    if profile.dpi.is_none()
        && profile.dpi_stages.is_none()
        && profile.performance.is_none()
        && profile.polling.is_none()
    {
        return Err(Error::InvalidArg(
            "The selected profile has no DPI or polling intent to save".into(),
        ));
    }
    let existing = profiles.onboard_settings.clone();
    if let Some(existing) = &existing {
        existing.validate(device)?;
        if existing.phase != Phase::Applied {
            return Err(Error::InvalidArg(
                "An incomplete onboard operation requires original-state recovery first".into(),
            ));
        }
    }
    let current = read(device)?;
    let desired = current.project(&profile)?;
    let mut recovery = Recovery {
        schema: 1,
        serial: device.serial.clone(),
        slot: 1,
        profile: expected_profile.into(),
        phase: Phase::Saving,
        before: existing
            .as_ref()
            .map_or_else(|| current.clone(), |r| r.before.clone()),
        desired: desired.clone(),
        extra: existing.map_or_else(BTreeMap::new, |r| r.extra),
    };
    recovery.validate(device)?;
    candidate.for_device(device).onboard_settings = Some(recovery.clone());
    candidate.validate_save()?;
    let mut complete = candidate.clone();
    recovery.phase = Phase::Applied;
    complete.for_device(device).onboard_settings = Some(recovery);
    complete.validate_save()?;
    candidate.save()?;
    *store = candidate;
    wire::write_verified(
        device,
        &desired,
        false,
        desired.dpi == current.dpi && desired.stages == current.stages,
    )?;
    complete.save()?;
    *store = complete;
    Ok(desired)
}

/// Idempotent recovery after disconnect, partial SET, readback or storage error.
/// The journal is removed only after all originals read back and a save succeeds.
pub fn restore(
    device: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
) -> Result<Settings> {
    identity(device, store, expected_device, expected_profile)?;
    let mut pending = store.clone();
    let mut recovery = pending
        .for_device(device)
        .onboard_settings
        .clone()
        .ok_or_else(|| Error::InvalidArg("No onboard settings recovery record exists".into()))?;
    recovery.validate(device)?;
    recovery.phase = Phase::Restoring;
    pending.for_device(device).onboard_settings = Some(recovery.clone());
    let mut complete = pending.clone();
    complete.for_device(device).onboard_settings = None;
    pending.validate_save()?;
    complete.validate_save()?;
    // Confirm the supported GET contract still exists before any restore SET.
    read(device)?;
    pending.save()?;
    *store = pending;
    wire::write_verified(device, &recovery.before, true, false)?;
    complete.save()?;
    *store = complete;
    Ok(recovery.before)
}

#[cfg(test)]
mod tests;
