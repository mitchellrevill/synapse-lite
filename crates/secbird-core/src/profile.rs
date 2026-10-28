//! Named settings profiles, kept per device and re-applied on demand.
//!
//! Profile settings are recorded in the active profile. Device-wide Power preferences
//! live beside the profiles and are not replayed when the active profile changes.

use crate::device::Device;
use crate::error::{Error, Result};
use crate::features::{DpiStages, Rgb};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod storage;

pub const DEFAULT_PROFILE: &str = "Default";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    /// Fields introduced by a newer compatible editor are retained on every save.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
    /// A built-in effect instead of the static color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<crate::effects::Effect>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_sensitivity: Option<crate::dynamic_sensitivity::DynamicSensitivity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<crate::dynamic_sensitivity::Rotation>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<(u16, u16)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polling: Option<u32>,
    /// What the DPI button cycles through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi_stages: Option<DpiStages>,
    /// Synapse stage intent, including hidden rows and the stage-cycling switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<crate::performance::Performance>,
    /// Minutes idle before a wireless device sleeps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep_minutes: Option<u16>,
    /// Sleeping when idle on or off, for devices with that switch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_sleep: Option<bool>,
    /// Laptops: what the keyboard backlight does when idle (`stays-lit`, `half`, `quarter`,
    /// `dark`), see [`crate::Dimming`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimming: Option<String>,
    /// Keyboards: the keys gaming mode blocks (`off`, or like `win,alt-tab`), see
    /// [`crate::GameMode`]. Per profile, so a profile tied to a game can turn it on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    /// Lighting zone name (or numeric id) -> state.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lighting: BTreeMap<String, Zone>,
    /// Source brightness enable/value and retained host switch-off policy flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting_settings: Option<crate::lighting::LightingSettings>,
    /// Per-key colors, row-major over the device's matrix. Empty when unused.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Rgb>,
    /// Programs (lowercase exe names, e.g. `cs2.exe`) that switch the device to this
    /// profile while their window is in front.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
    /// Button remaps: setting key (`side-forward`, `sniper/hypershift`, see
    /// [`crate::buttons::setting_key`]) -> action text (`key:ctrl+c`, `turbo:left@15`).
    /// Buttons not listed keep their saved on-board setting.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub buttons: BTreeMap<String, String>,
}

/// `C:\Games\CS2.exe`, `cs2` or ` CS2.EXE ` -> `cs2.exe`. Empty when nothing is left.
pub fn normalize_app(name: &str) -> String {
    let name = name.trim().rsplit(['\\', '/']).next().unwrap_or("").trim().to_lowercase();
    if name.is_empty() || name.ends_with(".exe") {
        name
    } else {
        format!("{name}.exe")
    }
}

/// Device-wide Power intent. It is not re-applied when the active profile changes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DevicePower {
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_threshold_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfiles {
    /// Smart Tracking is device-wide, not part of named profile exports/replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_calibration: Option<crate::mouse_sensor::CalibrationPreferences>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    pub active: String,
    pub profiles: BTreeMap<String, Profile>,
    /// Leave this device's lighting to another app, across every profile.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lighting_off: bool,
    /// Button remaps written into the mouse's own memory with "Save to mouse".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboard: Option<OnboardSave>,
    /// Power settings shared by every profile for this physical device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<DevicePower>,
}

/// A profile's button remaps saved in one of the mouse's on-board slots, so they work
/// without Secretarybird (on another PC, on Linux, before the tray starts, after sleep).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OnboardSave {
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    /// The on-board slot written: the one active when saving (1-5).
    pub slot: u8,
    /// The profile saved, and its remaps as saved, to tell when it changed since.
    pub profile: String,
    #[serde(default)]
    pub buttons: BTreeMap<String, String>,
    /// What each written button held before Secretarybird first wrote it (Synapse's or the
    /// factory setting), to put back.
    #[serde(default)]
    pub before: BTreeMap<String, String>,
}

impl Default for DeviceProfiles {
    fn default() -> Self {
        Self {
            extra: BTreeMap::new(),
            active: DEFAULT_PROFILE.into(),
            sensor_calibration: None,
            profiles: BTreeMap::from([(DEFAULT_PROFILE.into(), Profile::default())]),
            lighting_off: false,
            onboard: None,
            power: None,
        }
    }
}

impl DeviceProfiles {
    pub fn active(&self) -> &Profile {
        &self.profiles[&self.active]
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        self.profiles.entry(self.active.clone()).or_default()
    }

    /// Create `name` as a copy of the active profile.
    pub fn create(&mut self, name: &str) -> Result<()> {
        if name.trim().is_empty() { return Err(Error::InvalidArg("profile name cannot be empty".into())); }
        if self.profiles.contains_key(name) {
            return Err(Error::InvalidArg(format!("profile '{name}' already exists")));
        }
        // A program belongs to one profile, so the copy starts without any.
        let mut copy = Profile { apps: Vec::new(), ..self.active().clone() };
        crate::profile_compat::renew_profile_identity(&mut copy)?;
        self.profiles.insert(name.into(), copy);
        Ok(())
    }

    /// Renaming changes identity labels, never device settings or recovery data.
    pub fn rename(&mut self, previous: &str, name: &str) -> Result<()> {
        if !self.profiles.contains_key(previous) {
            return Err(Error::NotFound(format!("profile '{previous}'")));
        }
        if name.trim().is_empty() { return Err(Error::InvalidArg("profile name cannot be empty".into())); }
        if previous == name { return Ok(()); }
        if self.profiles.contains_key(name) {
            return Err(Error::InvalidArg(format!("profile '{name}' already exists")));
        }
        let profile = self.profiles.remove(previous).expect("profile checked above");
        self.profiles.insert(name.to_string(), profile);
        if self.active == previous { self.active = name.to_string(); }
        if let Some(onboard) = self.onboard.as_mut().filter(|saved| saved.profile == previous) {
            onboard.profile = name.to_string();
        }
        Ok(())
    }

    /// Set the programs that bring up profile `name`. A program listed here is taken
    /// off any other profile of this device.
    pub fn set_apps(&mut self, name: &str, apps: &[String]) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(Error::NotFound(format!("profile '{name}'")));
        }
        let mut list: Vec<String> = apps.iter().map(|a| normalize_app(a)).filter(|a| !a.is_empty()).collect();
        list.sort();
        list.dedup();
        for (other, p) in self.profiles.iter_mut() {
            if other != name {
                p.apps.retain(|a| !list.contains(a));
            }
        }
        self.profiles.get_mut(name).expect("checked above").apps = list;
        Ok(())
    }

    /// The profile tied to a program (a normalized exe name), if any.
    pub fn for_app(&self, exe: &str) -> Option<&str> {
        self.profiles.iter().find(|(_, p)| p.apps.iter().any(|a| a == exe)).map(|(n, _)| n.as_str())
    }

    /// The profile to show right now: the one tied to the program in front, else the active one.
    pub fn effective(&self, foreground: Option<&str>) -> &str {
        foreground.and_then(|exe| self.for_app(exe)).unwrap_or(&self.active)
    }

    /// Send profile `name` to the device. Buttons remapped by another profile of this
    /// device (or saved on the mouse) but not by this one go back to their original
    /// setting, so switching profiles never leaves a stray remap behind.
    pub fn apply(&self, name: &str, dev: &Device) -> Vec<Applied> {
        let Some(profile) = self.profiles.get(name) else {
            return vec![Applied { setting: format!("profile '{name}'"), result: Err(Error::NotFound(name.into())) }];
        };
        let mut out = profile.apply_settings(dev, !self.lighting_off);
        let saved = self.onboard.iter().flat_map(|o| o.before.keys());
        let others: std::collections::BTreeSet<&String> = self
            .profiles
            .values()
            .flat_map(|p| p.buttons.keys())
            .chain(saved)
            .filter(|k| !profile.buttons.contains_key(*k))
            .collect();
        for key in others {
            let result = self.reset_button(key, dev);
            out.push(Applied { setting: format!("{key} back to saved"), result: result.map(|_| ()) });
        }
        out
    }

    /// Put a button or key (a setting key like `side-forward/hypershift`) back to its original
    /// setting: what it held before "Save to mouse" wrote the slot, else the slot's own.
    pub fn reset_button(&self, key: &str, dev: &Device) -> Result<bool> {
        let (input, layer) = crate::buttons::parse_setting_key(key)?;
        match self.onboard.as_ref().and_then(|o| o.before.get(key)) {
            Some(original) => dev.set_input(input, layer, original.parse()?),
            None => dev.reset_input(input, layer),
        }
    }

    /// Whether the device has lost profile `name`'s button remaps, as a wireless mouse
    /// does when it sleeps and wakes (#45). Reads back one remapped button; `false` when
    /// the profile remaps nothing or the device doesn't answer.
    pub fn buttons_lost(&self, name: &str, dev: &Device) -> bool {
        let Some((key, action)) = self.profiles.get(name).and_then(|p| p.buttons.iter().next()) else {
            return false;
        };
        let Ok((input, layer)) = crate::buttons::parse_setting_key(key) else { return false };
        let Ok(want) = action.parse::<crate::buttons::Action>() else { return false };
        dev.input(input, layer).is_ok_and(|have| have != want)
    }

    /// Send only profile `name`'s button remaps (see [`DeviceProfiles::buttons_lost`]).
    pub fn apply_buttons(&self, name: &str, dev: &Device) -> Vec<Applied> {
        self.profiles.get(name).map(|p| p.apply_buttons(dev)).unwrap_or_default()
    }

    /// Send the battery colour again to each zone of profile `name` that shows the battery
    /// ([`crate::effects::Effect::Battery`]), so it follows the level. Nothing when this
    /// device's lighting is left to another app.
    pub fn refresh_battery_lights(&self, name: &str, dev: &Device) -> Vec<Applied> {
        let Some(profile) = self.profiles.get(name).filter(|_| !self.lighting_off) else { return Vec::new() };
        profile
            .lighting
            .iter()
            .filter(|(_, z)| !z.off && z.effect == Some(crate::effects::Effect::Battery))
            .map(|(zone, _)| Applied {
                setting: format!("{zone} battery"),
                result: dev.led_id(zone).and_then(|led| dev.set_effect(led, &crate::effects::Effect::Battery)),
            })
            .collect()
    }

    /// Send profile `name`'s per-key colors again: a keyboard drops them in its quick
    /// adjustment mode (#45) and never says when it leaves it. `None` when the profile has
    /// none or this device's lighting is left to another app.
    pub fn refresh_frame(&self, name: &str, dev: &Device) -> Option<Result<()>> {
        let profile = self.profiles.get(name).filter(|p| !self.lighting_off && !p.keys.is_empty())?;
        Some(dev.show_frame(&profile.keys))
    }

    pub fn switch(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(Error::NotFound(format!("profile '{name}'")));
        }
        self.active = name.into();
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<()> {
        if name == self.active {
            return Err(Error::InvalidArg("cannot delete the active profile; switch first".into()));
        }
        self.profiles
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| Error::NotFound(format!("profile '{name}'")))
    }
}

/// All profiles on disk, keyed by device (`"0099:SERIAL"`, or `"0099"` without a serial).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceProfiles>,
    #[serde(skip)]
    path: PathBuf,
}

impl Store {
    /// Bounded native JSON for saving or lossless backup. Near the byte limit,
    /// compact formatting preserves data that pretty whitespace would displace.
    pub fn native_bytes(&self) -> Result<Vec<u8>> {
        storage::encode(self)
    }

    /// Validate the exact persistence budget before a caller sends hardware SETs.
    /// This performs no file operation or device access.
    pub fn validate_save(&self) -> Result<()> {
        self.native_bytes().map(drop)
    }

    pub fn load(path: &Path) -> Result<Self> {
        storage::load(path)
    }

    /// Button 128 was saved as `top-extra` before 0.5; it is the extra side button (#54).
    pub(crate) fn rename_old_buttons(&mut self) {
        let rename = |map: &mut BTreeMap<String, String>| {
            let old: Vec<String> = map.keys().filter(|k| k.split('/').next() == Some("top-extra")).cloned().collect();
            for key in old {
                let value = map.remove(&key).unwrap();
                map.insert(key.replacen("top-extra", "side-extra", 1), value);
            }
        };
        for device in self.devices.values_mut() {
            device.profiles.values_mut().for_each(|p| rename(&mut p.buttons));
            if let Some(onboard) = device.onboard.as_mut() {
                rename(&mut onboard.buttons);
                rename(&mut onboard.before);
            }
        }
    }

    /// Write atomically: a crash mid-save never leaves a half-written file.
    pub fn save(&self) -> Result<()> {
        storage::save(self)
    }

    /// Add every profile from `other` (an export). A name that already exists for that
    /// device is imported as "name (imported)", "name (imported 2)", ...; active
    /// profiles are left alone. Returns how many profiles were added.
    pub fn import(&mut self, other: Store) -> usize {
        let mut added = 0;
        crate::profile_compat::merge_extensions(&mut self.extra, other.extra);
        for (key, incoming) in other.devices {
            let target = self.devices.entry(key).or_insert_with(|| DeviceProfiles {
                extra: incoming.extra.clone(),
                active: incoming.active.clone(),
                profiles: BTreeMap::new(),
                lighting_off: incoming.lighting_off,
                // The only copy of what the mouse's slot held before a "Save to mouse".
                onboard: incoming.onboard.clone(),
                power: incoming.power.clone(),
                sensor_calibration: incoming.sensor_calibration.clone(),
            });
            crate::profile_compat::merge_extensions(&mut target.extra, incoming.extra);
            if let Some(incoming_power) = incoming.power {
                match target.power.as_mut() {
                    None => target.power = Some(incoming_power),
                    Some(current) => {
                        if current.critical_threshold_percent.is_some()
                            && current.critical_threshold_percent != incoming_power.critical_threshold_percent
                            && incoming_power.critical_threshold_percent.is_some()
                        {
                            // Keep current device intent; retain conflicting imported
                            // configuration in the native backup without applying it.
                            crate::profile_compat::merge_extensions(&mut target.extra,
                                BTreeMap::from([("_synapseLiteImported:power".into(), serde_json::to_value(&incoming_power).expect("DevicePower serializes"))]));
                        }
                        crate::profile_compat::merge_extensions(&mut current.extra, incoming_power.extra);
                        if current.critical_threshold_percent.is_none() {
                            current.critical_threshold_percent = incoming_power.critical_threshold_percent;
                        }
                    }
                }
            }

            for (name, profile) in incoming.profiles {
                let mut unique = name.clone();
                let mut n = 1;
                while target.profiles.contains_key(&unique) {
                    unique = if n == 1 { format!("{name} (imported)") } else { format!("{name} (imported {n})") };
                    n += 1;
                }
                target.profiles.insert(unique, profile);
                added += 1;
            }
            if !target.profiles.contains_key(&target.active) {
                target.active = target.profiles.keys().next().cloned().unwrap_or_default();
            }
        }
        added
    }

    /// [`Store::import`] for a backup made on another PC. Devices are keyed by serial, so
    /// profiles saved for a unit that isn't connected here go to a connected device of the
    /// same model that the backup has nothing for (#40). `connected` are device keys.
    pub fn validate_import_targets(&self, connected: &[String]) -> Result<()> {
        for key in self.devices.keys().filter(|key| !connected.contains(key)) {
            let targets = connected.iter().filter(|target| profile_family(target) == profile_family(key) && !self.devices.contains_key(*target)).count();
            if targets > 1 {
                return Err(Error::InvalidArg(format!("profile device '{key}' matches several connected devices; choose the target device explicitly")));
            }
        }
        Ok(())
    }

    pub fn import_for(&mut self, mut other: Store, connected: &[String]) -> usize {
        let keys: Vec<String> = other.devices.keys().cloned().collect();
        for key in keys.into_iter().filter(|k| !connected.contains(k)) {
            let targets: Vec<_> = connected.iter().filter(|c| profile_family(c) == profile_family(&key) && !other.devices.contains_key(*c)).collect();
            let [target] = targets.as_slice() else {
                continue;
            };
            if let Some(profiles) = other.devices.remove(&key) {
                other.devices.insert((*target).clone(), profiles);
            }
        }
        self.import(other)
    }

    /// Profiles of one physical device (see [`Device::key`](crate::Device::key)).
    /// Profiles saved before devices were told apart by serial live under the bare
    /// product id; the first device of that model to ask takes them over.
    pub fn profiles_for(&mut self, key: &str, pid: u16) -> &mut DeviceProfiles {
        let legacy = format!("{pid:04X}");
        // A nonempty matching serial identifies the same physical DeathAdder
        // across its two connection modes. Never merge unrelated units by PID.
        if !self.devices.contains_key(key) {
            if let Some((_, serial)) = key.split_once(':').filter(|(_, serial)| !serial.is_empty()) {
                let peer = match pid { 0x00c4 => Some(0x00c5), 0x00c5 => Some(0x00c4), _ => None };
                if let Some(peer) = peer {
                    if let Some(profile) = self.devices.remove(&format!("{peer:04X}:{serial}")) {
                        self.devices.insert(key.to_string(), profile);
                    }
                }
            }
        }
        if key != legacy && !self.devices.contains_key(key) {
            if let Some(old) = self.devices.remove(&legacy) {
                self.devices.insert(key.to_string(), old);
            }
        }
        self.devices.entry(key.to_string()).or_default()
    }

    pub fn for_device(&mut self, dev: &crate::Device) -> &mut DeviceProfiles {
        self.profiles_for(&dev.key(), dev.pid)
    }

    pub fn device(&mut self, pid: u16) -> &mut DeviceProfiles {
        self.devices.entry(format!("{pid:04X}")).or_default()
    }

    /// Every program tied to a profile of any device.
    pub fn apps(&self) -> std::collections::HashSet<String> {
        self.devices.values().flat_map(|d| d.profiles.values()).flat_map(|p| p.apps.iter().cloned()).collect()
    }
}

fn profile_family(key: &str) -> String {
    let pid = key.split(':').next().unwrap_or(key).to_ascii_uppercase();
    if matches!(pid.as_str(), "00C4" | "00C5") { "00C4".into() } else { pid }
}

/// Outcome of applying one setting from a profile.
#[derive(Debug)]
pub struct Applied {
    pub setting: String,
    pub result: Result<()>,
}

impl Profile {
    /// Source Static semantics use the current native color projection. Native
    /// per-key/off/effect overrides remain authoritative and are not re-enabled.
    pub fn source_static_color(&self, pid: u16) -> Option<Rgb> {
        if pid != 605
            || !self.keys.is_empty()
            || crate::profile_compat::source_quick_effects(self)?
                .get("selectedEffectId")?
                .as_u64()?
                != 1
        {
            return None;
        }
        let zone = self.lighting.get("backlight")?;
        if zone.off || zone.effect.is_some() {
            None
        } else {
            zone.color
        }
    }

    /// Send every recorded setting to the device. Keeps going past individual failures.
    /// For stored profiles, use [`DeviceProfiles::apply`] to respect lighting ownership.
    pub fn apply(&self, dev: &Device) -> Vec<Applied> {
        self.apply_settings(dev, true)
    }

    fn apply_settings(&self, dev: &Device, lighting: bool) -> Vec<Applied> {
        let mut out = Vec::new();
        let mut push = |setting: String, result: Result<()>| out.push(Applied { setting, result });

        if let Some(hz) = self.polling {
            push(format!("polling {hz} Hz"), dev.set_polling_rate(hz));
        }
        if let Some(intent)=&self.dynamic_sensitivity {
            push("Dynamic Sensitivity".into(),dev.apply_dynamic_sensitivity(intent));
        }
        if let Some(performance) = &self.performance {
            push("DPI stage intent".into(), performance.apply(dev).map(|_| ()));
        } else {
            // Legacy stages first: their active stage changes the current DPI.
            if let Some(stages) = &self.dpi_stages {
                push(format!("dpi stages ({})", stages.stages.len()), dev.set_dpi_stages(stages).map(|_| ()));
            }
            if let Some((x, y)) = self.dpi {
                push(format!("dpi {x}x{y}"), dev.set_dpi(x, y).map(|_| ()));
            }
        }
        if let Some(on) = self.auto_sleep {
            push(format!("auto sleep {}", if on { "on" } else { "off" }), dev.set_auto_sleep(on));
        }
        if let Some(minutes) = self.sleep_minutes {
            push(format!("sleep after {minutes} min"), dev.set_sleep_minutes(minutes));
        }
        if let Some(d) = &self.dimming {
            push(format!("keyboard when idle: {d}"), d.parse().and_then(|d| dev.set_dimming(d)));
        }
        if let Some(g) = &self.game_mode {
            push(format!("gaming mode: {g}"), g.parse().and_then(|g| dev.set_game_mode(g)));
        }
        // No firmware color/effect projection may brighten an old driver-mode
        // frame before reporting the host-runtime limitation. Host shadows clear
        // their handled color/effect fields; brightness/per-key-only paths stay
        // separate. Preserve the prior source-Static wrong-interface refusal.
        let firmware_projection = self
            .lighting
            .values()
            .any(|zone| !zone.off && (zone.color.is_some() || zone.effect.is_some()));
        let needs_mode_guard = self.source_static_color(dev.pid).is_some()
            || (dev.pid == 605 && dev.interface == 2 && firmware_projection);
        let lighting = if lighting && needs_mode_guard {
            match dev.ornata_mode_readback() {
                Ok(mode) if mode.mode() == 0 => true,
                Ok(mode) => {
                    push("Ornata firmware color/effect".into(),Err(Error::Unsupported(format!("Ornata firmware color/effect in mode{} requires the running tray host-frame runtime and an explicit supported host choice; lighting projection was not applied",mode.mode()))));
                    false
                }
                Err(error) => {
                    push("Ornata firmware lighting mode preflight".into(), Err(error));
                    false
                }
            }
        } else {
            lighting
        };
        if lighting {
            if let Some(settings) = &self.lighting_settings {
                push("source brightness".into(), settings.apply_brightness(dev).map(|_| ()));
                if settings.needs_runtime() {
                    push("source idle/display lighting policy".into(), Err(Error::Unsupported("idle/display lighting policies require a running host runtime".into())));
                }
            }
        }
        for (name, zone) in self.lighting.iter().filter(|_| lighting) {
            let led = match dev.led_id(name) {
                Ok(led) => led,
                Err(e) => {
                    push(format!("{name} lighting"), Err(e));
                    continue;
                }
            };
            if name != "backlight" || self.lighting_settings.is_none() {
                if let Some(pct) = zone.brightness {
                    push(format!("{name} brightness {pct}%"), dev.set_brightness(led, pct));
                }
            }
            if zone.off {
                push(format!("{name} off"), dev.lights_off(led));
            } else if let Some(effect) = &zone.effect {
                push(format!("{name} {effect}"), dev.set_effect(led, effect));
            } else if let Some(color) = zone.color {
                push(format!("{name} color {color}"), dev.set_static_color(led, color));
            }
        }
        if lighting && !self.keys.is_empty() {
            push(format!("per-key colors ({} keys)", self.keys.len()), dev.show_frame(&self.keys));
        }
        out.extend(self.apply_buttons(dev));
        out
    }

    fn apply_buttons(&self, dev: &Device) -> Vec<Applied> {
        self.buttons
            .iter()
            .map(|(key, action)| {
                let result = crate::buttons::parse_setting_key(key).and_then(|(input, layer)| {
                    let action: crate::buttons::Action = action.parse()?;
                    dev.set_input(input, layer, action).map(|_| ())
                });
                Applied { setting: format!("{key} -> {action}"), result }
            })
            .collect()
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On the first real mouse: `cargo test -p secbird-core real_mouse -- --ignored`.
    /// Sends F13 to side-forward for a moment and puts it back to the saved setting.
    #[test]
    #[ignore]
    fn real_mouse_lost_buttons_are_noticed_and_sent_again() {
        use crate::buttons::{Action, Layer};
        let api = crate::hidapi::HidApi::new().unwrap();
        let devs = crate::discover(&api, &crate::Db::builtin().unwrap());
        let dev = devs.iter().find(|d| !d.buttons().is_empty()).expect("a mouse with remappable buttons");
        let mut p = DeviceProfiles::default();
        p.active_mut().buttons.insert("side-forward".into(), "key:f13".into());
        assert!(p.apply_buttons("Default", dev).iter().all(|a| a.result.is_ok()));
        assert!(!p.buttons_lost("Default", dev), "just sent");
        dev.reset_button(5, Layer::Normal).unwrap(); // what a wake does to the live slot
        assert!(p.buttons_lost("Default", dev), "noticed");
        assert!(p.apply_buttons("Default", dev).iter().all(|a| a.result.is_ok()));
        assert_eq!(dev.button(5, Layer::Normal).unwrap(), Action::Key { modifiers: 0, usage: 0x68 });
        assert!(!p.buttons_lost("Default", dev), "sent again");
        dev.reset_button(5, Layer::Normal).unwrap();
    }

    /// On the first real mouse: `cargo test -p secbird-core real_mouse_saves -- --ignored --nocapture`.
    /// Saves F13 on side-forward into the active on-board slot, then puts the slot back.
    #[test]
    #[ignore]
    fn real_mouse_saves_to_its_slot_and_puts_it_back() {
        use crate::buttons::{Action, Layer};
        let api = crate::hidapi::HidApi::new().unwrap();
        let devs = crate::discover(&api, &crate::Db::builtin().unwrap());
        let dev = devs.iter().find(|d| !d.buttons().is_empty()).expect("a mouse with remappable buttons");
        let slot = dev.saved_slot();
        let before = dev.slot_button(slot, 5, Layer::Normal).unwrap();
        println!("slot {slot}: side-forward is {before}");
        let path = std::env::temp_dir().join(format!("secbird-real-onboard-{}.json", std::process::id()));
        let mut store = Store::load(&path).unwrap();
        let f13 = Action::Key { modifiers: 0, usage: 0x68 };
        crate::actions::set_button(dev, &mut store, "side-forward", Layer::Normal, Some(f13)).unwrap();
        crate::actions::save_buttons_to_mouse(dev, &mut store).unwrap();
        let saved = dev.slot_button(slot, 5, Layer::Normal).unwrap();
        println!("after save: slot has {saved}, backup {:?}", store.for_device(dev).onboard.as_ref().map(|o| &o.before));
        crate::actions::restore_all_buttons(dev, &mut store).unwrap();
        let after = dev.slot_button(slot, 5, Layer::Normal).unwrap();
        println!("after put back: slot has {after}, live {}", dev.button(5, Layer::Normal).unwrap());
        let _ = std::fs::remove_file(&path);
        assert_eq!(saved, f13);
        assert_eq!(after, before);
        assert_eq!(dev.button(5, Layer::Normal).unwrap(), before);
    }

    /// On the first real mouse: `cargo test -p secbird-core real_mouse_dims -- --ignored --nocapture`.
    /// Its first zone goes dark for 3 seconds with a write that isn't saved, then back.
    #[test]
    #[ignore]
    fn real_mouse_dims_without_saving() {
        let api = crate::hidapi::HidApi::new().unwrap();
        let devs = crate::discover(&api, &crate::Db::builtin().unwrap());
        let dev = devs.iter().find(|d| !d.buttons().is_empty()).expect("a mouse");
        let led = dev.spec.as_ref().unwrap().leds[0].id;
        let saved = dev.brightness(led).unwrap();
        dev.set_brightness_live(led, 0).unwrap();
        let during = dev.brightness(led).unwrap();
        println!("led {led}: saved {saved}%, read while dark {during}%");
        std::thread::sleep(std::time::Duration::from_secs(3));
        dev.set_brightness_live(led, saved).unwrap();
        assert_eq!(dev.brightness(led).unwrap(), saved);
    }

    #[test]
    fn old_top_extra_button_is_renamed_on_load() {
        let path = std::env::temp_dir().join(format!("secbird-top-extra-{}.json", std::process::id()));
        let json = r#"{"devices":{"00E3":{"active":"Default","profiles":{"Default":{"buttons":{"top-extra":"key:f13","top-extra/hypershift":"disabled","wheel":"right"}}}}}}"#;
        std::fs::write(&path, json).unwrap();
        let store = Store::load(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let buttons = &store.devices["00E3"].profiles["Default"].buttons;
        let keys: Vec<&str> = buttons.keys().map(String::as_str).collect();
        assert_eq!(keys, ["side-extra", "side-extra/hypershift", "wheel"]);
        assert_eq!(buttons["side-extra"], "key:f13");
    }

    #[test]
    fn lighting_ownership_defaults_migrates_and_survives_import() {
        let mut old: DeviceProfiles = serde_json::from_str(r#"{"active":"Default","profiles":{"Default":{}}}"#).unwrap();
        assert!(!old.lighting_off);
        assert!(serde_json::to_value(&old).unwrap().get("lighting_off").is_none());
        old.lighting_off = true;
        let restored: DeviceProfiles = serde_json::from_value(serde_json::to_value(&old).unwrap()).unwrap();
        assert!(restored.lighting_off);

        let mut mine = Store::default();
        mine.devices.insert("0099".into(), restored);
        assert!(mine.profiles_for("0099:UNIT1", 0x0099).lighting_off);
        assert!(!mine.profiles_for("0099:UNIT2", 0x0099).lighting_off);
        let mut incoming = Store::default();
        incoming.devices.insert("0099:UNIT1".into(), DeviceProfiles::default());
        incoming.devices.insert("0084:NEW".into(), old.clone());
        incoming.devices.insert("0099:UNIT2".into(), old);
        mine.import(incoming);
        assert!(mine.devices["0099:UNIT1"].lighting_off, "import must not re-enable an existing device");
        assert!(!mine.devices["0099:UNIT2"].lighting_off, "existing ownership is kept in both directions");
        assert!(mine.devices["0084:NEW"].lighting_off, "new devices restore backed-up ownership");
    }

    #[test]
    fn create_copies_active_and_switch_changes_it() {
        let mut p = DeviceProfiles::default();
        p.active_mut().dpi = Some((800, 800));
        p.create("Gaming").unwrap();
        p.switch("Gaming").unwrap();
        assert_eq!(p.active().dpi, Some((800, 800)));
        p.active_mut().dpi = Some((1600, 1600));
        assert_eq!(p.profiles[DEFAULT_PROFILE].dpi, Some((800, 800)));
    }

    #[test]
    fn app_names_are_normalized() {
        assert_eq!(normalize_app(r"C:\Games\CS2.exe"), "cs2.exe");
        assert_eq!(normalize_app(" dota2 "), "dota2.exe");
        assert_eq!(normalize_app("  "), "");
    }

    #[test]
    fn an_app_belongs_to_one_profile() {
        let mut p = DeviceProfiles::default();
        p.create("Gaming").unwrap();
        p.create("Work").unwrap();
        p.set_apps("Gaming", &["CS2.exe".into(), "dota2".into(), "cs2.exe".into()]).unwrap();
        assert_eq!(p.profiles["Gaming"].apps, ["cs2.exe", "dota2.exe"]);
        p.set_apps("Work", &["dota2.exe".into()]).unwrap();
        assert_eq!(p.profiles["Gaming"].apps, ["cs2.exe"]);
        assert_eq!(p.for_app("dota2.exe"), Some("Work"));
        assert_eq!(p.effective(Some("cs2.exe")), "Gaming");
        assert_eq!(p.effective(Some("notepad.exe")), DEFAULT_PROFILE);
        assert_eq!(p.effective(None), DEFAULT_PROFILE);
        assert!(p.set_apps("Missing", &[]).is_err());
    }

    #[test]
    fn new_profile_does_not_copy_apps() {
        let mut p = DeviceProfiles::default();
        p.set_apps(DEFAULT_PROFILE, &["cs2.exe".into()]).unwrap();
        p.create("Copy").unwrap();
        assert!(p.profiles["Copy"].apps.is_empty());
        assert_eq!(p.for_app("cs2.exe"), Some(DEFAULT_PROFILE));
    }

    #[test]
    fn cannot_delete_active_profile() {
        let mut p = DeviceProfiles::default();
        assert!(p.delete(DEFAULT_PROFILE).is_err());
    }

    #[test]
    fn store_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("secbird-test-{}", std::process::id()));
        let path = dir.join("profiles.json");
        let mut store = Store::load(&path).unwrap();
        let dev = store.device(0x0099);
        dev.active_mut().polling = Some(500);
        dev.active_mut().lighting.insert(
            "logo".into(),
            Zone { color: Some(Rgb(255, 0, 0)), off: false, brightness: Some(40), effect: None, ..Default::default() },
        );
        store.save().unwrap();

        let mut again = Store::load(&path).unwrap();
        let p = again.device(0x0099).active().clone();
        assert_eq!(p.polling, Some(500));
        assert_eq!(p.lighting["logo"].color, Some(Rgb(255, 0, 0)));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn legacy_profiles_move_to_the_serial_key() {
        let mut store = Store::default();
        store.device(0x0099).active_mut().dpi = Some((1600, 1600));
        let p = store.profiles_for("0099:PM24AB", 0x0099);
        assert_eq!(p.active().dpi, Some((1600, 1600)));
        assert!(!store.devices.contains_key("0099"));
        // A second unit of the same model starts fresh.
        assert_eq!(store.profiles_for("0099:XYZ999", 0x0099).active().dpi, None);
        assert_eq!(store.profiles_for("0099:PM24AB", 0x0099).active().dpi, Some((1600, 1600)));
    }

    #[test]
    fn import_keeps_existing_profiles() {
        let mut mine = Store::default();
        mine.device(0x0099).active_mut().dpi = Some((800, 800));
        let mut theirs = Store::default();
        theirs.device(0x0099).active_mut().dpi = Some((3200, 3200));
        theirs.device(0x0099).create("Gaming").unwrap();
        theirs.device(0x0084).active_mut().polling = Some(500);

        assert_eq!(mine.import(theirs), 3);
        let d = mine.device(0x0099);
        assert_eq!(d.active, "Default");
        assert_eq!(d.active().dpi, Some((800, 800)));
        assert_eq!(d.profiles["Default (imported)"].dpi, Some((3200, 3200)));
        assert!(d.profiles.contains_key("Gaming"));
        assert_eq!(mine.device(0x0084).active().polling, Some(500));
    }

    #[test]
    fn import_from_another_pc_goes_to_the_connected_mouse() {
        let mut theirs = Store::default();
        theirs.profiles_for("0096:OTHERPC01", 0x0096).create("Gaming").unwrap();
        theirs.profiles_for("0084:UNPLUGGED", 0x0084).active_mut().polling = Some(500);
        let mut mine = Store::default();
        let connected = ["0096:THISPC001".to_string()];

        assert_eq!(mine.import_for(theirs, &connected), 3);
        assert!(mine.devices["0096:THISPC001"].profiles.contains_key("Gaming"));
        assert!(!mine.devices.contains_key("0096:OTHERPC01"));
        // Nothing connected of that model: kept as it was, for when it's plugged in.
        assert!(mine.devices.contains_key("0084:UNPLUGGED"));
    }

    #[test]
    fn rename_keeps_recovery_data_and_profile_identity() {
        let mut profiles = DeviceProfiles::default();
        profiles.active_mut().extra.insert("future-setting".into(), serde_json::json!({"opaque": 42}));
        profiles.onboard = Some(OnboardSave { profile: "Default".into(), before: BTreeMap::from([("left".into(), "default".into())]), ..Default::default() });
        let original = profiles.active().clone();
        let recovery = profiles.onboard.as_ref().unwrap().before.clone();
        profiles.rename("Default", "Work").unwrap();
        assert_eq!(profiles.active, "Work");
        assert_eq!(profiles.active(), &original);
        assert_eq!(profiles.onboard.as_ref().unwrap().profile, "Work");
        assert_eq!(profiles.onboard.as_ref().unwrap().before, recovery);
        profiles.create("Taken").unwrap();
        assert!(profiles.rename("Work", "Taken").is_err());
        assert!(profiles.rename("Work", "  ").is_err());
        assert!(profiles.rename("Missing", "New").is_err());
        assert_eq!(profiles.active, "Work");
    }

    #[test]
    fn connection_modes_share_profiles_only_for_the_same_serial() {
        let mut store = Store::default();
        store.profiles_for("00C4:UNIT1", 0x00c4).active_mut().dpi = Some((800, 800));
        assert_eq!(store.profiles_for("00C5:UNIT2", 0x00c5).active().dpi, None);
        assert_eq!(store.profiles_for("00C5:UNIT1", 0x00c5).active().dpi, Some((800, 800)));
        assert!(!store.devices.contains_key("00C4:UNIT1"));
        assert!(store.devices.contains_key("00C5:UNIT2"));
    }

    #[test]
    fn foreign_mouse_import_accepts_receiver_alias_and_rejects_ambiguous_units() {
        let mut incoming = Store::default();
        incoming.profiles_for("00C4:FOREIGN", 0x00c4).active_mut().dpi = Some((1200, 1200));
        assert!(incoming.validate_import_targets(&["00C5:ONE".into(), "00C5:TWO".into()]).is_err());
        let target = ["00C5:ONE".into()];
        incoming.validate_import_targets(&target).unwrap();
        let mut local = Store::default();
        local.import_for(incoming, &target);
        assert_eq!(local.devices["00C5:ONE"].active().dpi, Some((1200, 1200)));
        assert!(!local.devices.contains_key("00C4:FOREIGN"));
    }
}

#[cfg(test)]
mod ornata_projection_guard_tests {
    use super::*;
    use crate::{
        commands as cmd,
        effects::Effect,
        sim::{Sim, KEYBOARD_PID},
    };
    fn device(mode: Option<u8>) -> (Sim, Device) {
        let sim = Sim::keyboard();
        sim.state().device_mode = mode;
        let mut dev = sim.open(605, Sim::spec_for(KEYBOARD_PID)).unwrap();
        dev.interface = 2;
        sim.state().brightness.insert(5, 255);
        sim.state().live_brightness.insert(5, 255);
        sim.state().log.clear();
        (sim, dev)
    }
    fn profile(effect: bool) -> Profile {
        let mut profile = Profile::default();
        let zone = profile.lighting.entry("backlight".into()).or_default();
        zone.brightness = Some(25);
        if effect {
            zone.effect = Some(Effect::Spectrum);
        } else {
            zone.color = Some(Rgb(1, 2, 3));
        }
        profile
    }
    #[test]
    fn legacy_and_source_uniform_projection_refuse_driver_mode_before_brightness() {
        for mode in [Some(3), None] {
            for effect in [false, true] {
                let (sim, dev) = device(mode);
                let results = profile(effect).apply(&dev);
                assert!(results.iter().any(|r| r.result.is_err()));
                let state = sim.state();
                assert_eq!(state.brightness[&5], 255);
                assert!(!state.log.contains(&cmd::MATRIX_BRIGHTNESS));
                assert!(!state.log.contains(&cmd::MATRIX_EFFECT));
            }
        }
        let (sim, dev) = device(Some(3));
        let profile = crate::profile_compat::new_native_profile(605, "Imported").unwrap();
        let results = profile.apply(&dev);
        assert!(results.iter().any(|r| r
            .result
            .as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("explicit supported host choice"))));
        assert!(!sim.state().log.contains(&cmd::MATRIX_BRIGHTNESS));
        assert!(!sim.state().log.contains(&cmd::MATRIX_EFFECT));
    }
    #[test]
    fn normal_mode_and_brightness_only_keep_their_existing_projection() {
        let (sim, dev) = device(Some(0));
        assert!(profile(false).apply(&dev).iter().all(|r| r.result.is_ok()));
        assert!(sim.state().log.contains(&cmd::MATRIX_BRIGHTNESS));
        assert!(sim.state().log.contains(&cmd::MATRIX_EFFECT));
        let (sim, dev) = device(Some(3));
        let mut p = profile(false);
        p.lighting.get_mut("backlight").unwrap().color = None;
        assert!(p.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert!(!sim.state().log.contains(&cmd::DEVICE_MODE.get()));
        assert!(sim.state().log.contains(&cmd::MATRIX_BRIGHTNESS));
    }
}
