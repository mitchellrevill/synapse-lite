//! Product196 Dynamic Sensitivity. Profile intent and verified sensor transactions.
use crate::{
    proto::Command,
    sensor_curve::{encode_samples, Point, SensorCurve},
    Device, Error, Result, Store,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
const STATE: Command = Command::new(0x0b, 0x10);
const MODE: Command = Command::new(0x0b, 0x11);
const CURVE: Command = Command::new(0x0b, 0x13);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Rotation {
    pub is_enabled: bool,
    pub value: i32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Rotation {
    fn default() -> Self {
        Self {
            is_enabled: true,
            value: 0,
            extra: BTreeMap::new(),
        }
    }
}
impl Rotation {
    pub fn validate(&self) -> Result<()> {
        if !(-44..=44).contains(&self.value) {
            return Err(invalid("Rotation must be between -44 and 44 degrees"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicSensitivity {
    pub state: u8,
    pub mode: u8,
    #[serde(default)]
    pub points: Vec<Value>,
    #[serde(default)]
    pub template_id: u8,
    #[serde(default)]
    pub custom_sensor_accelerations: Vec<f64>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl DynamicSensitivity {
    pub fn control_points(&self) -> Result<Vec<Point>> {
        self.points
            .iter()
            .map(|v| {
                Ok(Point {
                    x: v["x"]
                        .as_f64()
                        .ok_or_else(|| invalid("Invalid curve X coordinate"))?,
                    y: v["y"]
                        .as_f64()
                        .ok_or_else(|| invalid("Invalid curve Y coordinate"))?,
                })
            })
            .collect()
    }
    pub fn validate(&self) -> Result<()> {
        if self.state > 1 || self.mode > 3 || self.template_id > 3 {
            return Err(invalid(
                "Unsupported Dynamic Sensitivity state, mode or template",
            ));
        }
        if self.points.len() > 64 || self.custom_sensor_accelerations.len() > 250 {
            return Err(invalid(
                "Dynamic Sensitivity curve exceeds supported limits",
            ));
        }
        if self.state == 1 && self.mode == 3 {
            self.curve_bytes()?;
        }
        Ok(())
    }
    pub fn curve_bytes(&self) -> Result<[u8; 1000]> {
        // Recompute from the retained control points exactly like the stock UI.
        let samples = SensorCurve::new(&self.control_points()?)?.firmware_samples()?;
        encode_samples(&samples)
    }
    pub fn seed(&mut self, template: u8) -> Result<()> {
        let rows: &[(f64, f64)] = match template {
            0 => &[(0., 1.), (20., 1.1), (40., 1.2), (70., 1.35), (100., 1.5)],
            1 => &[(0., 1.), (11., 1.45), (20., 1.5), (35., 1.5), (100., 1.5)],
            2 => &[(0., 1.), (9., 1.), (21., 1.5), (24., 1.5), (100., 1.5)],
            3 => {
                self.template_id = 3;
                return Ok(());
            }
            _ => return Err(invalid("Unknown sensitivity template")),
        };
        self.template_id = template;
        self.points = rows
            .iter()
            .map(|(x, y)| serde_json::json!({"x":x,"y":y}))
            .collect();
        self.custom_sensor_accelerations = SensorCurve::new(&self.control_points()?)?
            .firmware_samples()?
            .to_vec();
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidArg(message.into())
}
fn supported(dev: &Device) -> Result<()> {
    if !matches!(dev.pid, 196 | 197) {
        return Err(Error::Unsupported(
            "Dynamic Sensitivity is specific to DeathAdder V3 HyperSpeed".into(),
        ));
    }
    let firmware = dev.firmware_version()?;
    let parts: Vec<u16> = firmware
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| invalid("Unrecognized firmware version"))?;
    if parts.len() != 4 || parts.as_slice() < [1, 0, 1, 0].as_slice() {
        return Err(Error::Unsupported(
            "Advanced requires firmware 1.0.1.0 or newer".into(),
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DynamicRaw {
    pub state: u8,
    pub mode: u8,
    #[serde(skip)]
    pub curve: Option<Vec<u8>>,
}
impl Device {
    pub fn dynamic_raw(&self, with_curve: bool) -> Result<DynamicRaw> {
        supported(self)?;
        let read = |cmd| -> Result<u8> {
            let r = self.get(cmd, 2, &[1, 0])?;
            if r.0[5] != 2 || r.args()[0] != 1 {
                return Err(invalid("Unexpected Dynamic Sensitivity register response"));
            }
            Ok(r.args()[1])
        };
        let state = read(STATE)?;
        let mode = read(MODE)?;
        if state > 1 {
            return Err(invalid("Unknown Dynamic Sensitivity enable state"));
        }
        let curve = if with_curve {
            Some(self.read_dynamic_curve()?)
        } else {
            None
        };
        Ok(DynamicRaw { state, mode, curve })
    }
    fn read_dynamic_curve(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(1000);
        for offset in (0..250usize).step_by(8) {
            let count = (250 - offset).min(8);
            let mut args = [0u8; 37];
            args[..5].copy_from_slice(&[1, 3, 250, offset as u8, count as u8]);
            let r = self.get(CURVE, 37, &args)?;
            if r.0[5] != 37 || r.args()[..5] != args[..5] {
                return Err(invalid("Unexpected sensitivity curve read response"));
            }
            bytes.extend_from_slice(&r.args()[5..5 + count * 4]);
            if count == 8 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
        Ok(bytes)
    }
    fn write_dynamic_curve(&self, bytes: &[u8]) -> Result<()> {
        if bytes.len() != 1000 {
            return Err(invalid("Expected 250 sensitivity samples"));
        }
        for offset in (0..250usize).step_by(8) {
            let count = (250 - offset).min(8);
            let mut args = [0u8; 37];
            args[..5].copy_from_slice(&[1, 3, 250, offset as u8, count as u8]);
            args[5..5 + count * 4].copy_from_slice(&bytes[offset * 4..(offset + count) * 4]);
            self.transact(CURVE, 37, &args)?;
            if count == 8 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
        Ok(())
    }
    fn write_dynamic(&self, raw: &DynamicRaw, restore: bool) -> Result<()> {
        if restore {
            // A recovery restores the exact hidden curve/mode even when disabled.
            if let Some(bytes) = &raw.curve {
                self.write_dynamic_curve(bytes)?;
            }
            self.transact(MODE, 2, &[1, raw.mode])?;
            self.transact(STATE, 2, &[1, raw.state])?;
        } else {
            // Selected middleware writes enable state first, then mode/curve only if on.
            self.transact(STATE, 2, &[1, raw.state])?;
            if raw.state == 1 {
                self.transact(MODE, 2, &[1, raw.mode])?;
                if let Some(bytes) = &raw.curve {
                    self.write_dynamic_curve(bytes)?;
                }
            }
        }
        let observed = self.dynamic_raw(raw.curve.is_some())?;
        if &observed != raw {
            return Err(Error::Unsupported(
                "Dynamic Sensitivity read-back differs".into(),
            ));
        }
        Ok(())
    }
    /// Exact checkpoint recovery, including disabled mode and hidden curve bytes.
    pub fn restore_dynamic_raw(&self, raw: &DynamicRaw) -> Result<()> {
        if self.is_query_only() {
            return Err(Error::Unsupported("Read-only sensor handle".into()));
        }
        supported(self)?;
        if raw.state > 1 || raw.curve.as_ref().is_some_and(|bytes| bytes.len() != 1000) {
            return Err(invalid("Invalid sensitivity recovery checkpoint"));
        }
        self.write_dynamic(raw, true)
    }
    pub fn apply_dynamic_sensitivity(&self, intent: &DynamicSensitivity) -> Result<()> {
        if self.is_query_only() {
            return Err(Error::Unsupported("Read-only sensor handle".into()));
        }
        intent.validate()?;
        let curve = if intent.state == 1 && intent.mode == 3 {
            Some(intent.curve_bytes()?.to_vec())
        } else {
            None
        };
        let original = self.dynamic_raw(curve.is_some())?;
        let target = DynamicRaw {
            state: intent.state,
            mode: if intent.state == 0 {
                original.mode
            } else {
                intent.mode
            },
            curve,
        };
        if let Err(error) = self.write_dynamic(&target, false) {
            return match self.write_dynamic(&original, true) {
                Ok(()) => Err(Error::Unsupported(format!(
                    "{error}; original sensitivity restored"
                ))),
                Err(recovery) => Err(Error::Unsupported(format!(
                    "{error}; recovery failed: {recovery}"
                ))),
            };
        }
        Ok(())
    }
}
/// Caller pins both physical mouse and active profile before any sensor write.
pub fn set_dynamic(
    dev: &Device,
    store: &mut Store,
    device: &str,
    profile: &str,
    intent: DynamicSensitivity,
) -> Result<()> {
    if dev.is_query_only() {
        return Err(Error::Unsupported("Read-only sensor handle".into()));
    }
    if device != dev.key() || store.for_device(dev).active != profile {
        return Err(invalid(
            "Selected mouse or profile changed; refresh Advanced",
        ));
    }
    intent.validate()?;
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().dynamic_sensitivity = Some(intent.clone());
    candidate.validate_save()?;
    let original = dev.dynamic_raw(intent.state == 1 && intent.mode == 3)?;
    dev.apply_dynamic_sensitivity(&intent)?;
    if let Err(error) = candidate.save() {
        return match dev.write_dynamic(&original, true) {
            Ok(()) => Err(Error::Unsupported(format!(
                "{error}; original sensitivity restored"
            ))),
            Err(recovery) => Err(Error::Unsupported(format!(
                "{error}; recovery failed: {recovery}"
            ))),
        };
    }
    *store = candidate;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sim::Sim, Db};
    use serde_json::json;
    #[test]
    fn presets_custom_chunk_readback_rollback_and_profile_scope() {
        let sim = Sim::wireless_mouse();
        sim.state().sensor_dynamic = Some((0, 2, vec![0x31; 1000]));
        let db = Db::builtin().unwrap();
        let dev = sim.open(196, db.get(196).cloned()).unwrap();
        let folder = std::env::temp_dir().join(format!(
            "dynamic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("profiles.json");
        let mut store = Store::load(&path).unwrap();
        store
            .for_device(&dev)
            .active_mut()
            .extra
            .insert("opaque".into(), json!({"keep":3}));
        store.save().unwrap();
        let mut intent = DynamicSensitivity {
            state: 1,
            mode: 1,
            ..Default::default()
        };
        let log = sim.state().log.len();
        assert!(set_dynamic(&dev, &mut store, "stale", "Default", intent.clone()).is_err());
        assert_eq!(sim.state().log.len(), log);
        set_dynamic(&dev, &mut store, &dev.key(), "Default", intent.clone()).unwrap();
        assert_eq!(dev.dynamic_raw(false).unwrap().mode, 1);
        assert_eq!(
            store.for_device(&dev).active().extra["opaque"],
            json!({"keep":3})
        );
        intent.mode = 3;
        intent.seed(2).unwrap();
        let before = std::fs::read(&path).unwrap();
        sim.state().fail_next_dynamic_curve = true;
        assert!(set_dynamic(&dev, &mut store, &dev.key(), "Default", intent.clone()).is_err());
        assert_eq!(sim.state().sensor_dynamic, Some((1, 1, vec![0x31; 1000])));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        set_dynamic(&dev, &mut store, &dev.key(), "Default", intent.clone()).unwrap();
        assert_eq!(
            sim.state().sensor_dynamic,
            Some((1, 3, intent.curve_bytes().unwrap().to_vec()))
        );
        let before = std::fs::read(&path).unwrap();
        let hardware = sim.state().sensor_dynamic.clone();
        std::fs::create_dir(path.with_extension("json.tmp")).unwrap();
        intent.mode = 0;
        assert!(set_dynamic(&dev, &mut store, &dev.key(), "Default", intent.clone()).is_err());
        assert_eq!(sim.state().sensor_dynamic, hardware);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_dir(path.with_extension("json.tmp")).unwrap();
        intent.state = 0;
        set_dynamic(&dev, &mut store, &dev.key(), "Default", intent).unwrap();
        assert_eq!(dev.dynamic_raw(false).unwrap().state, 0);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn source_export_round_trip_retains_custom_and_opaque_fields() {
        let mut profile = crate::profile_compat::new_native_profile(196, "Fixture").unwrap();
        let mut intent = DynamicSensitivity {
            state: 1,
            mode: 3,
            ..Default::default()
        };
        intent.seed(1).unwrap();
        intent.points[1]["futurePoint"] = json!(true);
        intent.extra.insert("futureMode".into(), json!([7]));
        profile.dynamic_sensitivity = Some(intent.clone());
        let rotation=Rotation{is_enabled:false,value:-14,extra:BTreeMap::from([("futureRotation".into(),json!({"keep":true}))])};
        profile.rotation=Some(rotation.clone());
        let mut store =
            Store::load(&std::env::temp_dir().join("dynamic-export-not-written.json")).unwrap();
        let device = store.profiles_for("00C4", 196);
        device.profiles = BTreeMap::from([("Fixture".into(), profile)]);
        device.active = "Fixture".into();
        let exported = crate::profile_compat::export_synapse4(&store, "00C4").unwrap();
        let imported = crate::profile_compat::import_document(&exported.bytes).unwrap();
        assert_eq!(
            imported.store.devices.values().next().unwrap().profiles["Fixture"].dynamic_sensitivity,
            Some(intent)
        );
        assert_eq!(imported.store.devices.values().next().unwrap().profiles["Fixture"].rotation,Some(rotation));
    }
}
