//! Per-device source Rotation controller; register this module only on Windows.
//!
//! Keep one controller per physical Device for its whole engine lifetime, across
//! profile switches. Call restore(device) before removal/shutdown and surface
//! errors. Hardware restore is explicit because this controller does not own the
//! HID Device. The filter additionally has its existing best-effort Drop recovery.
//! Coordinate stock ownership before SET: neither transport provides atomic CAS.


pub use crate::dynamic_sensitivity::Rotation;
use crate::sensor_rotation::{SensorRotation, SensorRotationReader};
use crate::{commands as cmd, Device, Error, Result};
use serde::Serialize;

/// Source profile object. Disabled intent retains its original value and opaque
/// fields; the filter's disabled wire projection is zero degrees.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationPath {
    Hardware,
    VendorFilter,
}

/// Observed register state, separate from source profile intent. In the filter
/// a zero angle cannot distinguish disabled from enabled-at-zero, so enabled is
/// None in that case. Unrepresentable raw angles have value=None.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RotationObservation {
    pub path: RotationPath,
    pub enabled: Option<bool>,
    pub value: Option<i32>,
    pub raw_angle: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Raw {
    Hardware([u8; 3]),
    Filter(u32),
}
impl Raw {
    fn desired(path: RotationPath, rotation: &Rotation) -> Result<Self> {
        rotation.validate()?;
        Ok(match path {
            RotationPath::Hardware => {
                Self::Hardware([1, u8::from(rotation.is_enabled), rotation.value as i8 as u8])
            }
            RotationPath::VendorFilter => Self::Filter(if rotation.is_enabled {
                rotation.value.rem_euclid(360) as u32
            } else {
                0
            }),
        })
    }
    fn observation(self) -> RotationObservation {
        match self {
            Self::Hardware([_, enabled, angle]) => {
                let signed = i32::from(angle as i8);
                RotationObservation {
                    path: RotationPath::Hardware,
                    enabled: Some(enabled != 0),
                    value: (-44..=44).contains(&signed).then_some(signed),
                    raw_angle: u32::from(angle),
                }
            }
            Self::Filter(angle) => RotationObservation {
                path: RotationPath::VendorFilter,
                enabled: (angle != 0).then_some(true),
                value: filter_degrees(angle),
                raw_angle: angle,
            },
        }
    }
}

fn filter_degrees(raw: u32) -> Option<i32> {
    match raw {
        0..=44 => Some(raw as i32),
        316..=359 => Some(raw as i32 - 360),
        _ => None,
    }
}

/// Four numeric firmware components are required; no lexical or padded compare.
/// Old wired PID00C4 remains unsupported until its filter handler is inspected.
pub fn path_for_firmware(pid: u16, firmware: &str) -> Result<RotationPath> {
    if !matches!(pid, 196 | 197) {
        return Err(unsupported("Rotation is verified only for196/197"));
    }
    let parts: Vec<_> = firmware.split('.').map(str::parse::<u8>).collect();
    if parts.len() != 4 || parts.iter().any(|part| part.is_err()) {
        return Err(unsupported(
            "Rotation requires a verified four-component numeric firmware version",
        ));
    }
    let mut version = [0_u8; 4];
    for (out, part) in version.iter_mut().zip(parts) {
        *out = part.unwrap();
    }
    if version >= [1, 10, 1, 0] {
        Ok(RotationPath::Hardware)
    } else if pid == 197 {
        Ok(RotationPath::VendorFilter)
    } else {
        Err(unsupported(
            "Old-firmware wired00C4 software Rotation is not verified",
        ))
    }
}

fn unsupported(message: &str) -> Error {
    Error::Unsupported(message.to_owned())
}
fn io_error(error: std::io::Error) -> Error {
    Error::System(error.to_string())
}
fn firmware_path(device: &Device) -> Result<RotationPath> {
    if !matches!(device.pid, 196 | 197) {
        return Err(unsupported("Unsupported Rotation product"));
    }
    // Do not use firmware_version()'s two-component fallback to enable writes.
    let reply = device.get(cmd::FIRMWARE_EXT, 4, &[])?;
    if reply.0[5] != 4 {
        return Err(unsupported("Extended firmware response is not four bytes"));
    }
    let a = reply.args();
    path_for_firmware(device.pid, &format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3]))
}

fn hardware_read(device: &Device) -> Result<Raw> {
    let reply = device.get(cmd::SENSOR_ANGLE, 3, &[1, 0, 0])?;
    if reply.0[5] != 3 || reply.args()[0] != 1 || reply.args()[1] > 1 {
        return Err(unsupported(
            "Hardware Rotation returned an invalid profile/state selector",
        ));
    }
    Ok(Raw::Hardware([
        reply.args()[0],
        reply.args()[1],
        reply.args()[2],
    ]))
}

/// A separate GET-only path. It never constructs a restoring lease, even on
/// old firmware. The caller must explicitly mark its Device query-only first.
pub fn read_rotation(device: &Device) -> Result<RotationObservation> {
    if !device.is_query_only() {
        return Err(unsupported(
            "Rotation read-only inspection requires a query-only Device",
        ));
    }
    Ok(match firmware_path(device)? {
        RotationPath::Hardware => hardware_read(device)?,
        RotationPath::VendorFilter => {
            let mut reader = SensorRotationReader::open_for_device(device).map_err(io_error)?;
            Raw::Filter(reader.angle().map_err(io_error)?)
        }
    }
    .observation())
}

enum Backend {
    Hardware,
    Filter(SensorRotation),
}
impl Backend {
    fn path(&self) -> RotationPath {
        match self {
            Self::Hardware => RotationPath::Hardware,
            Self::Filter(_) => RotationPath::VendorFilter,
        }
    }
}

trait Port {
    fn read(&mut self) -> Result<Raw>;
    fn write(&mut self, raw: Raw) -> Result<()>;
}
struct DevicePort<'a> {
    backend: &'a mut Backend,
    device: &'a Device,
}
impl Port for DevicePort<'_> {
    fn read(&mut self) -> Result<Raw> {
        match self.backend {
            Backend::Hardware => hardware_read(self.device),
            Backend::Filter(filter) => Ok(Raw::Filter(filter.angle().map_err(io_error)?)),
        }
    }
    fn write(&mut self, raw: Raw) -> Result<()> {
        if self.device.is_query_only() {
            return Err(unsupported("Read-only Device refuses Rotation SET"));
        }
        match (&mut *self.backend, raw) {
            (Backend::Hardware, Raw::Hardware(bytes)) if bytes[0] == 1 && bytes[1] <= 1 => {
                self.device.transact(cmd::SENSOR_ANGLE, 3, &bytes)?;
                Ok(())
            }
            (Backend::Filter(filter), Raw::Filter(angle)) => {
                if angle == filter.baseline_angle() {
                    filter.restore().map_err(io_error)
                } else {
                    let degrees = filter_degrees(angle).ok_or_else(|| unsupported("Filter rollback angle is neither its exact baseline nor a supported UI angle"))?;
                    filter.set_degrees(degrees).map_err(io_error)
                }
            }
            _ => Err(unsupported("Rotation route/register mismatch")),
        }
    }
}

struct Ownership {
    baseline: Raw,
    owned: Option<Raw>,
}
impl Ownership {
    fn write_verified(port: &mut impl Port, desired: Raw) -> Result<()> {
        let written = port.write(desired);
        // A failed acknowledgement can still have changed the register; a fresh
        // exact GET establishes the state rather than assuming SET atomicity.
        match port.read() {
            Ok(actual) if actual == desired => Ok(()),
            observed => Err(unsupported(&format!(
                "Rotation write not verified: write={:?}, readback={observed:?}",
                written.err()
            ))),
        }
    }
    fn apply(
        &mut self,
        port: &mut impl Port,
        desired: Raw,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<Raw> {
        let before = port.read()?;
        if before != self.owned.unwrap_or(self.baseline) {
            self.owned = None;
            return Err(unsupported(
                "Rotation ownership changed; external state preserved",
            ));
        }
        let previous_ownership = self.owned;
        if before != desired {
            self.owned = Some(desired);
        }
        let applied = if before == desired {
            Ok(())
        } else {
            Self::write_verified(port, desired)
        };
        if let Err(error) = applied.and_then(|_| persist()) {
            let recovery = match port.read() {
                Ok(actual) if actual == before => Ok(()),
                Ok(actual) if actual == desired => Self::write_verified(port, before),
                Ok(_) => {
                    self.owned = None;
                    Err(unsupported(
                        "External Rotation change preserved during rollback",
                    ))
                }
                Err(read_error) => Err(read_error),
            };
            if recovery.is_ok() {
                self.owned = previous_ownership;
            }
            return Err(unsupported(&format!(
                "{error}; prewrite Rotation rollback: {}",
                match recovery {
                    Ok(()) => "verified".to_owned(),
                    Err(recovery) => recovery.to_string(),
                }
            )));
        }
        Ok(desired)
    }
    fn restore(&mut self, port: &mut impl Port) -> Result<()> {
        let Some(owned) = self.owned else {
            return Ok(());
        };
        match port.read()? {
            actual if actual == self.baseline => {
                self.owned = None;
                Ok(())
            }
            actual if actual == owned => {
                Self::write_verified(port, self.baseline)?;
                self.owned = None;
                Ok(())
            }
            _ => {
                self.owned = None;
                Err(unsupported(
                    "Rotation ownership lost; external state preserved during restore",
                ))
            }
        }
    }
}

/// Send, not Sync: Engine may move an owned controller to its worker, but must
/// serialize calls. No borrowed HID handle is stored and no request is pending
/// when a method returns. DriverIo documents its exclusive synchronous Send ABI.
pub struct RotationController {
    device_key: String,
    device_path: Vec<u8>,
    backend: Backend,
    ownership: Ownership,
}
impl RotationController {
    pub fn open(device: &Device) -> Result<Self> {
        if device.is_query_only() {
            return Err(unsupported(
                "A query-only Device cannot own a Rotation controller; use read_rotation",
            ));
        }
        let mut backend = match firmware_path(device)? {
            RotationPath::Hardware => Backend::Hardware,
            RotationPath::VendorFilter => {
                Backend::Filter(SensorRotation::open_for_device(device).map_err(io_error)?)
            }
        };
        let baseline = DevicePort {
            backend: &mut backend,
            device,
        }
        .read()?;
        Ok(Self {
            device_key: device.key(),
            device_path: device.path.as_bytes().to_vec(),
            backend,
            ownership: Ownership {
                baseline,
                owned: None,
            },
        })
    }
    fn matches(&self, device: &Device) -> Result<()> {
        if device.is_query_only()
            || device.key() != self.device_key
            || device.path.as_bytes() != self.device_path
        {
            return Err(unsupported(
                "Rotation controller does not own this writable physical Device",
            ));
        }
        Ok(())
    }
    pub fn path(&self) -> RotationPath {
        self.backend.path()
    }
    pub fn baseline(&self) -> RotationObservation {
        self.ownership.baseline.observation()
    }
    pub fn observe(&mut self, device: &Device) -> Result<RotationObservation> {
        self.matches(device)?;
        Ok(DevicePort {
            backend: &mut self.backend,
            device,
        }
        .read()?
        .observation())
    }
    /// Apply a profile without discarding the original device-lifetime baseline.
    pub fn apply(&mut self, device: &Device, rotation: &Rotation) -> Result<RotationObservation> {
        self.apply_with_commit(device, rotation, || Ok(()))
    }
    /// Caller validates/stages profile JSON before this method, then saves in
    /// persist. Only publish the staged Store after success. Save failure rolls
    /// back the exact prewrite register, not merely the lifetime baseline.
    pub fn apply_with_commit(
        &mut self,
        device: &Device,
        rotation: &Rotation,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<RotationObservation> {
        self.matches(device)?;
        let desired = Raw::desired(self.path(), rotation)?;
        let raw = self.ownership.apply(
            &mut DevicePort {
                backend: &mut self.backend,
                device,
            },
            desired,
            persist,
        )?;
        Ok(raw.observation())
    }
    pub fn restore(&mut self, device: &Device) -> Result<()> {
        self.matches(device)?;
        self.ownership.restore(&mut DevicePort {
            backend: &mut self.backend,
            device,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Mock {
        state: Raw,
        writes: Vec<Raw>,
        ignore: bool,
        fail_after_write: bool,
    }
    impl Port for Mock {
        fn read(&mut self) -> Result<Raw> {
            Ok(self.state)
        }
        fn write(&mut self, raw: Raw) -> Result<()> {
            self.writes.push(raw);
            if !self.ignore {
                self.state = raw;
            }
            if self.fail_after_write {
                return Err(unsupported("lost acknowledgement"));
            }
            Ok(())
        }
    }
    fn fixture(raw: Raw) -> (Ownership, Mock) {
        (
            Ownership {
                baseline: raw,
                owned: None,
            },
            Mock {
                state: raw,
                writes: vec![],
                ignore: false,
                fail_after_write: false,
            },
        )
    }
    #[test]
    fn source_profile_intent_roundtrips_opaque_and_disabled_angle() {
        let value = json!({"isEnabled":false,"value":-44,"opaque":{"future":true}});
        let typed: Rotation = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), value);
        assert_eq!(
            Raw::desired(RotationPath::Hardware, &typed).unwrap(),
            Raw::Hardware([1, 0, 212])
        );
        assert_eq!(
            Raw::desired(RotationPath::VendorFilter, &typed).unwrap(),
            Raw::Filter(0)
        );
    }
    #[test]
    fn exact_numeric_firmware_branch() {
        assert_eq!(
            path_for_firmware(197, "1.2.0.0").unwrap(),
            RotationPath::VendorFilter
        );
        assert_eq!(
            path_for_firmware(197, "1.10.0.255").unwrap(),
            RotationPath::VendorFilter
        );
        assert_eq!(
            path_for_firmware(196, "1.10.1.0").unwrap(),
            RotationPath::Hardware
        );
        assert_eq!(
            path_for_firmware(197, "2.0.0.0").unwrap(),
            RotationPath::Hardware
        );
        assert!(path_for_firmware(196, "1.2.0.0").is_err());
        for text in ["1.10.1", "1.10.1.0.1", "1.256.0.0", "x.10.1.0", ""] {
            assert!(path_for_firmware(197, text).is_err());
        }
    }
    #[test]
    fn source_bytes_and_unknown_observation() {
        let mut intent = Rotation { value: -44, ..Rotation::default() };
        assert_eq!(
            Raw::desired(RotationPath::Hardware, &intent).unwrap(),
            Raw::Hardware([1, 1, 212])
        );
        assert_eq!(
            Raw::desired(RotationPath::VendorFilter, &intent).unwrap(),
            Raw::Filter(316)
        );
        intent.value = 45;
        assert!(Raw::desired(RotationPath::Hardware, &intent).is_err());
        assert_eq!(Raw::Filter(0).observation().enabled, None);
        assert_eq!(Raw::Filter(123).observation().value, None);
        assert_eq!(Raw::Hardware([1, 0, 212]).observation().value, Some(-44));
    }
    #[test]
    fn profile_switch_keeps_baseline_and_save_failure_restores_prewrite() {
        let baseline = Raw::Filter(123);
        let first = Raw::Filter(348);
        let second = Raw::Filter(10);
        let (mut owner, mut port) = fixture(baseline);
        owner.apply(&mut port, first, || Ok(())).unwrap();
        assert!(owner
            .apply(&mut port, second, || Err(unsupported("save failed")))
            .is_err());
        assert_eq!(port.state, first);
        assert_eq!(owner.baseline, baseline);
        owner.restore(&mut port).unwrap();
        assert_eq!(port.state, baseline);
        assert_eq!(port.writes, [first, second, first, baseline]);
    }
    #[test]
    fn exact_hardware_baseline_is_restored_even_outside_ui_domain() {
        let baseline = Raw::Hardware([1, 0, 128]);
        let desired = Raw::Hardware([1, 1, 10]);
        let (mut owner, mut port) = fixture(baseline);
        owner.apply(&mut port, desired, || Ok(())).unwrap();
        owner.restore(&mut port).unwrap();
        assert_eq!(port.state, baseline);
    }
    #[test]
    fn external_changes_are_preserved_and_noop_save_does_not_write() {
        let (mut owner, mut port) = fixture(Raw::Filter(0));
        owner.apply(&mut port, Raw::Filter(0), || Ok(())).unwrap();
        assert!(port.writes.is_empty());
        port.state = Raw::Filter(7);
        assert!(owner.apply(&mut port, Raw::Filter(10), || Ok(())).is_err());
        assert!(port.writes.is_empty());
        let (mut owner, mut port) = fixture(Raw::Filter(0));
        owner.apply(&mut port, Raw::Filter(10), || Ok(())).unwrap();
        port.state = Raw::Filter(7);
        assert!(owner.restore(&mut port).is_err());
        assert_eq!(port.writes, [Raw::Filter(10)]);
    }
    #[test]
    fn failed_readback_does_not_commit_and_lost_ack_is_decided_by_get() {
        let (mut owner, mut port) = fixture(Raw::Filter(0));
        port.ignore = true;
        let mut saved = false;
        assert!(owner
            .apply(&mut port, Raw::Filter(10), || {
                saved = true;
                Ok(())
            })
            .is_err());
        assert!(!saved);
        assert_eq!(port.state, Raw::Filter(0));
        let (mut owner, mut port) = fixture(Raw::Filter(0));
        port.fail_after_write = true;
        owner.apply(&mut port, Raw::Filter(10), || Ok(())).unwrap();
        assert_eq!(port.state, Raw::Filter(10));
    }
    #[test]
    fn controller_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RotationController>();
    }
}
