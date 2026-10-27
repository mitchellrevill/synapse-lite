//! Device and profile handling for the tray, kept free of Win32 code.

use secbird_core::{config_dir, discover, hidapi::HidApi, Db, Device, Dimming, Result, Store};
use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use crate::lighting_runtime::{Config as LightingConfig, DeviceIo, Runtime as LightingRuntime};
use std::time::{Duration, Instant, SystemTime};
mod source_static;
mod source_uniform;

pub struct Engine {
    #[cfg(test)]
    simulated_only: bool,
    api: HidApi,
    devices: Vec<Device>,
    unsupported: Vec<Unsupported>,
    /// Synapse processes seen at the last rescan.
    synapse: Vec<String>,
    /// The program in front, when some profile is tied to it.
    foreground: Option<String>,
    /// Programs tied to any profile, and the profiles.json timestamp they were read at.
    apps: HashSet<String>,
    apps_read: Option<SystemTime>,
    /// Latest DPI each device reported through its event reports.
    live_dpi: HashMap<String, (u16, u16)>,
    live_seq: u64,
    /// No per-key refresh until then, while the editor's light finder is open.
    frames_paused_until: Option<Instant>,
    /// Brightness percent while the PC is locked or its screen is off ([`Engine::lights_away`]).
    away: Option<u8>,
    /// Devices that didn't answer [`Engine::lights_back`], and until when to keep trying.
    relight: HashMap<String, Instant>,
    lighting_runtime: RefCell<LightingRuntime>,
    uniform: source_uniform::UniformState,
    host_static_results: RefCell<HashMap<String, (String, secbird_core::Rgb, serde_json::Value)>>,
    #[cfg(test)]
    runtime_store_path: Option<std::path::PathBuf>,
}

const FINDER_PAUSE: Duration = Duration::from_secs(10 * 60);
/// A wireless keyboard can miss the "back" write and answer ~20 s later (#52).
const RELIGHT_FOR: Duration = Duration::from_secs(2 * 60);

/// Battery state of one wireless device.
pub struct Battery {
    pub key: String,
    pub name: String,
    pub percent: u8,
    pub charging: bool,
}

/// A connected Razer product that can't be controlled (yet), shown so it isn't silently missing.
/// Why a wireless device we know is quiet, most often: it sleeps until a keypress (#37).
const ASLEEP: &str = "Not answering. Asleep? Press a key or move it";
const BLUETOOTH: &str = "Connected over Bluetooth, which can't be controlled yet. Use the cable or the USB receiver";

pub struct Unsupported {
    pub pid: u16,
    pub name: String,
    pub reason: &'static str,
}

/// What happened when a device's active profile was sent to it.
pub struct ApplyReport {
    pub device: String,
    pub profile: String,
    pub failures: Vec<String>,
}

impl Engine {
    /// The text of `secbird report`, for the tray's "Make a device report" item. Only reads.
    pub fn report_text(&mut self) -> String {
        #[cfg(test)]
        if self.simulated_only { return "Simulated native interaction test; physical discovery disabled".into(); }
        let _ = self.api.refresh_devices();
        let db = Db::load(Some(&config_dir().join("devices.json"))).unwrap_or_default();
        secbird_core::report::build(&self.api, &db).to_string()
    }

    pub fn new() -> Result<Self> {
        Ok(Self::from_api(HidApi::new()?))
    }

    fn from_api(api: HidApi) -> Self {
        Self {
            #[cfg(test)]
            simulated_only: false,
            api,
            devices: Vec::new(),
            unsupported: Vec::new(),
            synapse: Vec::new(),
            foreground: None,
            apps: HashSet::new(),
            apps_read: None,
            live_dpi: HashMap::new(),
            live_seq: 0,
            frames_paused_until: None,
            away: None,
            relight: HashMap::new(),
            lighting_runtime: RefCell::new(LightingRuntime::default()),
            uniform: source_uniform::UniformState::default(),
            host_static_results: RefCell::new(HashMap::new()),
            #[cfg(test)]
            runtime_store_path: None,
        }
    }

    /// No enumeration, physical transports or later rescan in a native UI test.
    #[cfg(test)]
    pub(crate) fn with_simulated_mouse(sim: &secbird_core::sim::Sim) -> Result<Self> {
        HidApi::disable_device_discovery();
        let mut engine = Self::from_api(HidApi::new()?);
        let db = Db::builtin()?;
        let device = sim.open(196, db.get(196).cloned())
            .ok_or_else(|| secbird_core::Error::InvalidArg("Simulated mouse did not answer".into()))?;
        assert!(device.path.as_bytes().starts_with(b"sim:"));
        engine.devices.push(device);
        engine.simulated_only = true;
        Ok(engine)
    }

    #[cfg(test)]
    pub(crate) fn with_simulated_keyboard(
        sim: &secbird_core::sim::Sim,
        path: std::path::PathBuf,
    ) -> Result<Self> {
        HidApi::disable_device_discovery();
        let mut engine = Self::from_api(HidApi::new()?);
        let mut dev = sim
            .open(
                605,
                secbird_core::sim::Sim::spec_for(secbird_core::sim::KEYBOARD_PID),
            )
            .ok_or_else(|| {
                secbird_core::Error::InvalidArg("simulated keyboard did not answer".into())
            })?;
        assert!(dev.path.as_bytes().starts_with(b"sim:"));
        dev.interface = 2;
        engine.devices.push(dev);
        engine.simulated_only = true;
        engine.runtime_store_path = Some(path);
        Ok(engine)
    }

    pub fn foreground(&self) -> Option<&str> {
        self.foreground.as_deref()
    }

    /// Collections that carry a device's own event reports (DPI and profile changes made
    /// with its buttons), as (device key, path). On the Basilisk V3 that is interface 1,
    /// collection 5 (usage 0001:0000, 16-byte reports); its sibling with the same usage
    /// stays silent, so listening to both costs nothing.
    pub fn event_paths(&self) -> Vec<(String, std::ffi::CString)> {
        #[cfg(test)]
        if self.simulated_only { return Vec::new(); }
        self.devices
            .iter()
            .flat_map(|dev| {
                self.api
                    .device_list()
                    .filter(move |i| {
                        i.vendor_id() == secbird_core::db::RAZER_VID
                            && i.product_id() == dev.pid
                            && i.usage_page() == 0x0001
                            && i.usage() == 0x0000
                    })
                    .map(move |i| (dev.key(), i.path().to_owned()))
            })
            .collect()
    }

    pub fn open_events(&self, path: &std::ffi::CStr) -> Option<secbird_core::hidapi::HidDevice> {
        #[cfg(test)]
        if self.simulated_only { return None; }
        self.api.open_path(path).ok()
    }

    /// The device changed DPI by itself (its DPI button). Record it in the profile in
    /// use, so a replug or wake keeps it instead of going back.
    pub fn dpi_changed(&mut self, key: &str, x: u16, y: u16) {
        self.live_seq += 1;
        self.live_dpi.insert(key.to_string(), (x, y));
        let Some(dev) = self.devices.iter().find(|d| d.key() == key) else { return };
        if let Ok(mut store) = self.load_store() {
            let p = store.for_device(dev);
            let name = p.effective(self.foreground.as_deref()).to_string();
            if let Some(profile) = p.profiles.get_mut(&name) {
                profile.dpi = Some((x, y));
                let _ = store.save();
            }
        }
    }

    /// Latest DPI reported by each device, and a counter that grows with every report,
    /// so the editor follows new reports without undoing a DPI just set in the editor.
    pub fn live_dpi(&self) -> (u64, &HashMap<String, (u16, u16)>) {
        (self.live_seq, &self.live_dpi)
    }

    /// Called when another window comes to the front, with its exe name. Devices whose
    /// profile depends on the program in front get that profile; nothing is sent when
    /// neither the old nor the new program is tied to a profile.
    pub fn set_foreground(&mut self, exe: Option<String>) -> Vec<ApplyReport> {
        self.refresh_apps();
        let exe = exe.map(|e| secbird_core::profile::normalize_app(&e)).filter(|e| self.apps.contains(e));
        if exe == self.foreground {
            return Vec::new();
        }
        let before = std::mem::replace(&mut self.foreground, exe);
        let Ok(mut store) = self.load_store() else { return Vec::new() };
        let changed: Vec<String> = self
            .devices
            .iter()
            .filter(|d| {
                let p = store.for_device(d);
                p.effective(before.as_deref()) != p.effective(self.foreground.as_deref())
            })
            .map(|d| d.key())
            .collect();
        self.apply(&changed)
    }

    /// Re-read the tied programs only when profiles.json has changed: a foreground switch
    /// then costs one file-time lookup.
    fn refresh_apps(&mut self) {
        let stamp = std::fs::metadata(config_dir().join("profiles.json")).and_then(|m| m.modified()).ok();
        if stamp != self.apps_read || stamp.is_none() {
            self.apps = self.load_store().map(|s| s.apps()).unwrap_or_default();
            self.apps_read = stamp;
        }
    }

    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub fn unsupported(&self) -> &[Unsupported] {
        &self.unsupported
    }

    pub fn synapse(&self) -> &[String] {
        &self.synapse
    }

    /// Battery of every wireless device that answers.
    pub fn batteries(&self) -> Vec<Battery> {
        self.devices
            .iter()
            .filter_map(|d| {
                let percent = d.battery().ok().flatten()?;
                Some(Battery { key: d.key(), name: d.name(), percent, charging: d.charging().unwrap_or(false) })
            })
            .collect()
    }

    /// Zones showing a wireless mouse's battery (a dock's light, #39) follow its level.
    /// Failures are left out: a mouse off its dock or asleep doesn't answer, and the
    /// colour stays until the next check.
    pub fn refresh_battery_lights(&self) {
        let Ok(mut store) = self.load_store() else { return };
        for dev in self.devices.iter().filter(|d| d.spec.as_ref().is_some_and(|s| s.wireless)) {
            let p = store.for_device(dev);
            let name = p.effective(self.foreground()).to_string();
            p.refresh_battery_lights(&name, dev);
        }
    }

    /// Per-key colors of wired keyboards and keypads, sent again. Leaving quick adjustment
    /// mode (#45) leaves them dark, and the keyboard doesn't say when; Synapse streams
    /// frames all the time, so this does the same at a slow pace. Wireless boards are left
    /// alone: the traffic would keep them awake. Failures are left out, as for batteries.
    /// Not while away: the lights are down then anyway.
    pub fn refresh_frames(&mut self) {
        if self.away.is_some() || self.frames_paused_until.is_some_and(|t| Instant::now() < t) {
            return;
        }
        let Ok(mut store) = self.load_store() else {
            return;
        };
        self.lighting_runtime.borrow_mut().sample_host();
        let foreground = self.foreground.clone();
        for index in 0..self.devices.len() {
            let dev = &self.devices[index];
            if !dev.matrix().is_some()
                || !dev.spec.as_ref().is_some_and(|s| {
                    !s.wireless
                        && matches!(
                            s.kind,
                            secbird_core::Kind::Keyboard | secbird_core::Kind::Keypad
                        )
                })
            {
                continue;
            }
            let key = dev.key();
            let profiles = store.for_device(dev);
            let name = profiles.effective(foreground.as_deref()).to_string();
            let color = profiles.profiles[&name].source_static_color(dev.pid);
            if let Some(color) = color {
                if self
                    .host_static_results
                    .borrow()
                    .get(&key)
                    .is_some_and(|(p, c, value)| {
                        p == &name && *c == color && value["deferred"] != true
                    })
                {
                    continue;
                }
                if !matches!(
                    self.host_static_gate(&key, &store, &name),
                    Ok(crate::host_static::Gate::Ready)
                ) {
                    continue;
                }
                if secbird_core::profile_compat::validate_store_budget(&store).is_err() {
                    continue;
                }
                let result = Self::apply_source_static(&mut self.devices[index], color);
                self.record_host_static(&key, &name, color, &result);
                continue;
            }
            if dev.pid == 605
                && (!self.lighting_runtime.borrow().owner_clear()
                    || self.lighting_runtime.borrow().blocks_frames(&key))
            {
                continue;
            }
            profiles.refresh_frame(&name, dev);
        }
    }

    /// Laptops set to "stays lit" still fade by themselves (#34), but any read restarts
    /// that idle fade, so read the setting back now and then. Only while the tray runs.
    pub fn keep_laptops_lit(&self) {
        if self.away.is_some() {
            return;
        }
        let Ok(mut store) = self.load_store() else { return };
        let laptops = self.devices.iter().filter(|d| d.spec.as_ref().is_some_and(|s| s.kind == secbird_core::Kind::Laptop));
        for dev in laptops {
            let p = store.for_device(dev);
            let name = p.effective(self.foreground()).to_string();
            let stays_lit = Dimming::StaysLit.to_string();
            if p.profiles.get(&name).is_some_and(|p| p.dimming.as_deref() == Some(stays_lit.as_str())) {
                let _ = dev.dimming();
            }
        }
    }

    /// The PC was locked or its screen went off (#48): every zone this app controls goes
    /// to `percent` of its saved brightness, with writes that aren't saved, so a replug or
    /// power cycle brings the lights back. Also call it after a rescan while away, for
    /// devices that just arrived. Devices with the older lighting commands are left lit.
    pub fn lights_away(&mut self, percent: u8) {
        self.away = Some(percent);
        self.relight.clear();
        self.refresh_lighting_runtime();
        self.each_zone(|dev, led| {
            let saved = dev.brightness(led)?;
            dev.set_brightness_live(led, (saved as u32 * percent as u32 / 100) as u8)
        });
    }

    /// Back from [`Engine::lights_away`]: the saved brightness again, read from the device
    /// (it answers the saved level, not the dimmed one), so it works after a tray restart too.
    /// Then the profiles, in case one changed while away.
    pub fn lights_back(&mut self) -> Vec<ApplyReport> {
        if self.away.take().is_none() {
            return Vec::new();
        }
        self.refresh_lighting_runtime();
        let missed = self.each_zone(|dev, led| dev.set_brightness_live(led, dev.brightness(led)?));
        let until = Instant::now() + RELIGHT_FOR;
        self.relight = missed.into_iter().map(|key| (key, until)).collect();
        let mut store=self.load_store().ok();
        let legacy_keys=self.devices.iter().filter(|dev| {
            !store.as_mut().is_some_and(|store|dev.pid==605 && self.lighting_runtime.borrow().manages(&dev.key(),&self.lighting_config(dev,store)))
        }).map(Device::key).collect::<Vec<_>>();
        self.apply(&legacy_keys)
    }

    /// [`Engine::lights_back`] again for the devices that missed it, until they answer or
    /// [`RELIGHT_FOR`] runs out.
    pub fn retry_lights_back(&mut self) {
        let now = Instant::now();
        self.relight.retain(|_, until| now < *until);
        if self.relight.is_empty() || self.away.is_some() {
            return;
        }
        let missed = self.each_zone(|dev, led| {
            if !self.relight.contains_key(&dev.key()) {
                return Ok(());
            }
            dev.set_brightness_live(led, dev.brightness(led)?)
        });
        let back: Vec<String> = self.relight.keys().filter(|k| !missed.contains(k)).cloned().collect();
        for key in &back {
            self.relight.remove(key);
        }
        if !back.is_empty() {
            self.apply(&back);
        }
    }

    pub fn is_away(&self) -> bool {
        self.away.is_some()
    }

    /// Run `f` on each lighting zone of each device whose lighting this app controls.
    /// Returns the keys of the devices where `f` failed: an asleep wireless device doesn't answer.
    fn each_zone(&self, f: impl Fn(&Device, u8) -> Result<()>) -> Vec<String> {
        let Ok(mut store) = self.load_store() else { return Vec::new() };
        let mut failed = Vec::new();
        for dev in &self.devices {
            if dev.pid==605 {
                let config=self.lighting_config(dev,&mut store);
                let runtime=self.lighting_runtime.borrow();
                if runtime.stopping || runtime.manages(&dev.key(),&config) || (runtime.running && !runtime.owner_clear()) {continue;}
            }
            if store.for_device(dev).lighting_off {
                continue;
            }
            let leds: Vec<u8> = dev.spec.iter().flat_map(|s| s.leds.iter().map(|l| l.id)).collect();
            if leds.into_iter().map(|led| f(dev, led)).filter(Result::is_err).count() > 0 {
                failed.push(dev.key());
            }
        }
        failed
    }

    /// The editor's light finder shows one light at a time; don't paint over it. A finder
    /// left open (a closed browser never says) stops pausing after a while.
    pub fn pause_frames(&mut self, pause: bool) {
        self.frames_paused_until = pause.then(|| Instant::now() + FINDER_PAUSE);
    }

    /// Re-enumerate devices. Returns the keys of devices that were not present before.
    pub fn rescan(&mut self) -> Vec<String> {
        #[cfg(test)]
        if self.simulated_only { return Vec::new(); }
        let before: HashSet<String> = self.devices.iter().map(|d| d.key()).collect();
        self.host_static_results.borrow_mut().clear();
        self.reset_uniform_handles();
        self.devices.clear(); // close stale handles before reopening
        let _ = self.api.refresh_devices();
        let db = Db::load(Some(&config_dir().join("devices.json")))
            .or_else(|_| Db::builtin())
            .unwrap_or_default();
        self.devices = discover(&self.api, &db);
        self.unsupported = secbird_core::device::unanswered(&self.api, &self.devices)
            .into_iter()
            .map(|pid| match db.get(pid) {
                Some(spec) if spec.lighting == secbird_core::Lighting::Kraken => {
                    Unsupported { pid, name: spec.name.clone(), reason: "Headset protocol not supported yet" }
                }
                Some(spec) if spec.wireless => Unsupported { pid, name: spec.name.clone(), reason: ASLEEP },
                Some(spec) => Unsupported { pid, name: spec.name.clone(), reason: "Not answering. Is Synapse running?" },
                None => match db.bluetooth_name(pid) {
                    Some(name) => Unsupported { pid, name, reason: BLUETOOTH },
                    None => Unsupported { pid, name: format!("Razer device {pid:04X}"), reason: "Unknown model. Run secbird report" },
                },
            })
            .collect();
        self.synapse = secbird_core::debloat::synapse_running();
        self.lighting_runtime.borrow_mut().invalidate_all();
        self.refresh_lighting_runtime();
        self.devices.iter().map(|d| d.key()).filter(|k| !before.contains(k)).collect()
    }

    /// A known wireless device is plugged in (its receiver is) but not answering yet.
    pub fn has_sleeping(&self) -> bool {
        self.unsupported.iter().any(|u| u.reason == ASLEEP)
    }

    /// Read profiles fresh from disk, so changes made with the CLI are picked up.
    fn store() -> Result<Store> {
        Store::load(&config_dir().join("profiles.json"))
    }

    fn load_store(&self) -> Result<Store> {
        #[cfg(test)]
        if let Some(path) = &self.runtime_store_path {
            return Store::load(path);
        }
        Self::store()
    }
    fn lighting_config(&self, dev: &Device, store: &mut Store) -> LightingConfig {
        let profiles = store.for_device(dev);
        let name = profiles.effective(self.foreground()).to_string();
        let mut config = LightingConfig::from_profile(
            &name,
            &profiles.profiles[&name],
            !profiles.lighting_off,
            self.away,
        );
        if dev.pid == 605 && dev.interface != 2 {
            config.error = Some("host lighting bridge requires verified Ornata interface2".into());
        }
        config
    }
    /// Standalone editors do not claim a continuously running host policy service.
    pub fn start_lighting_runtime(&mut self) {
        self.lighting_runtime.borrow_mut().start();
        self.refresh_lighting_runtime();
    }
    pub fn set_lighting_display_off(&mut self, off: bool) {
        self.lighting_runtime.borrow_mut().environment.display_off = Some(off);
        self.refresh_lighting_runtime();
    }
    pub fn refresh_lighting_runtime(&mut self) {
        self.lighting_runtime.borrow_mut().sample_host();
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(error) => {
                self.lighting_runtime
                    .borrow_mut()
                    .store_error(&error.to_string());
                return;
            }
        };
        let keys = self
            .devices
            .iter()
            .filter(|d| d.pid == 605)
            .map(Device::key)
            .collect();
        self.lighting_runtime.borrow_mut().retain(&keys);
        for index in 0..self.devices.len() {
            if self.devices[index].pid != 605 {
                continue;
            }
            let dev = &self.devices[index];
            let key = dev.key();
            let mut config = self.lighting_config(dev, &mut store);
            let name = config.profile.clone();
            let color = store.for_device(dev).profiles[&name].source_static_color(dev.pid);
            if let Some(color) = color {
                let previous = self.host_static_results.borrow().get(&key).cloned();
                let pending = !previous.as_ref().is_some_and(|(p, c, value)| {
                    p == &name && *c == color && value["deferred"] != true
                });
                if pending {
                    match self.host_static_gate(&key, &store, &name) {
                        Ok(crate::host_static::Gate::Ready) => {
                            let result =
                                secbird_core::profile_compat::validate_store_budget(&store)
                                    .and_then(|_| {
                                        Self::apply_source_static(&mut self.devices[index], color)
                                    });
                            self.record_host_static(&key, &name, color, &result);
                        }
                        Ok(crate::host_static::Gate::Deferred(reason)) => {
                            if !previous.as_ref().is_some_and(|(p, c, v)| {
                                p == &name && *c == color && v["deferred"] == true
                            }) {
                                // The independent brightness policy can darken a
                                // deferred frame without claiming it was rendered.
                                let result = Ok(
                                    serde_json::json!({"execution":"host-static","deferred":true,"rendered":false,"control_state_verified":false,"reason":reason}),
                                );
                                self.record_host_static(&key, &name, color, &result);
                            }
                        }
                        Ok(crate::host_static::Gate::NeedsBaseline { .. }) | Err(_) => {}
                    }
                }
                if let Some((p, c, value)) = self.host_static_results.borrow().get(&key) {
                    if p == &name && *c == color && value["error"].is_string() {
                        // Never raise brightness onto a failed/partial pending frame.
                        config.error=Some("Host Static failed; explicit reapply/reconnect is required before brightness resume".into());
                    }
                }
            }
            if let Some((p, c, value)) = self.host_static_results.borrow_mut().get_mut(&key) {
                if p == &name
                    && color == Some(*c)
                    && value["live_policy_reconcile_pending"] == true
                    && config.error.is_none()
                {
                    self.lighting_runtime
                        .borrow_mut()
                        .request_configured_live(&key);
                    value["live_policy_reconcile_pending"] = serde_json::json!(false);
                }
            }
            let uniform_profile = store.for_device(&self.devices[index]).profiles[&name].clone();
            if crate::host_uniform::Program::from_profile(&uniform_profile, self.devices[index].pid)
                .ok()
                .flatten()
                .is_some()
            {
                if let Err(error) = secbird_core::profile_compat::validate_store_budget(&store) {
                    config.error = Some(error.to_string());
                }
            }
            if config.error.is_none() {
                self.uniform_before_brightness(index, &uniform_profile, &name, &mut config);
            }
            self.lighting_runtime.borrow_mut().reconcile(
                &key,
                &config,
                &mut DeviceIo(&self.devices[index]),
            );
        }
    }
    /// Re-sample input immediately after an action; do not leave a new saved value
    /// visible until the next timer when display/idle suppression is still active.
    pub fn request_lighting_refresh(&mut self) {
        self.invalidate_uniform();
        self.lighting_runtime.borrow_mut().invalidate_all();
        self.refresh_lighting_runtime();
    }
    pub fn lighting_runtime_status(&self) -> serde_json::Value {
        let mut status = self.lighting_runtime.borrow().status();
        for dev in self.devices.iter().filter(|d| d.pid == 605) {
            let key = dev.key();
            if status.get(&key).is_none() {
                status[&key] = serde_json::json!({"available":false,"can_execute_policies":false,"suppressed":false,"blocked_reason":"host-runtime-not-yet-sampled","error":null});
            }
        }
        for (key, (_, _, value)) in self.host_static_results.borrow().iter() {
            if let Some(state) = status.get_mut(key) {
                state["host_static"] = value.clone();
            }
        }
        let uniforms = self.uniform_status();
        if let Some(map) = uniforms.as_object() {
            for (key, value) in map {
                if let Some(target) = status.get_mut(key) {
                    target["uniform_host"] = value.clone();
                }
            }
        }
        status
    }
    /// Release only our live overrides, with bounded attempts and ownership gates.
    pub fn shutdown_lighting_runtime(&mut self) -> Vec<String> {
        if !self.lighting_runtime.borrow().running {
            return Vec::new();
        }
        self.lighting_runtime.borrow_mut().stopping = true;
        for _ in 0..3 {
            self.refresh_lighting_runtime();
            if !self.lighting_runtime.borrow().has_pending_restore() {
                break;
            }
        }
        let status = self.lighting_runtime.borrow().status();
        let failures = status
            .as_object()
            .into_iter()
            .flat_map(|items| items.iter())
            .filter(|(_, value)| value["restoration_pending"] == true)
            .map(|(key, value)| {
                format!(
                    "{key}: {}",
                    value["error"]
                        .as_str()
                        .or_else(|| value["blocked_reason"].as_str())
                        .unwrap_or("live brightness restoration did not complete")
                )
            })
            .collect();
        self.lighting_runtime.borrow_mut().finish_stop();
        failures
    }

    /// Profile names for a device and which one is active.
    pub fn profiles(dev: &Device) -> (Vec<String>, String) {
        match Self::store() {
            Ok(mut store) => {
                let p = store.for_device(dev);
                (p.profiles.keys().cloned().collect(), p.active.clone())
            }
            Err(_) => (Vec::new(), String::new()),
        }
    }

    /// The profile a device shows right now (see [`Engine::set_foreground`]).
    pub fn effective_profile(&self, dev: &Device) -> String {
        self.load_store().map(|mut s| s.for_device(dev).effective(self.foreground()).to_string()).unwrap_or_default()
    }

    /// Apply the current profile of each listed device (by [`Device::key`]): the one tied
    /// to the program in front, else the active one.
    pub fn apply(&mut self, keys: &[String]) -> Vec<ApplyReport> {
        self.lighting_runtime.borrow_mut().sample_host();
        for key in keys {
            self.host_static_results.borrow_mut().remove(key);
        }
        let mut store = self.load_store();
        let foreground = self.foreground.clone();
        let mut reports = Vec::new();
        for index in 0..self.devices.len() {
            if !keys.contains(&self.devices[index].key()) {
                continue;
            }
            let dev = &self.devices[index];
            let key = dev.key();
            let report = match &mut store {
                Ok(store) => {
                    let config = self.lighting_config(dev, store);
                    let runtime = self.lighting_runtime.borrow();
                    let handled = dev.pid == 605 && runtime.can_execute(&config);
                    let blocked = dev.pid == 605
                        && (runtime.stopping
                            || (runtime.running && runtime.should_suppress(&config)));
                    drop(runtime);
                    let mut profiles = store.for_device(dev).clone();
                    let name = profiles.effective(foreground.as_deref()).to_string();
                    let color = profiles.profiles[&name].source_static_color(dev.pid);
                    let host_gate = self.host_static_gate(&key, store, &name);
                    profiles.lighting_off |= self.away.is_some() || blocked;
                    let uniform_candidate = crate::host_uniform::Program::from_profile(
                        &profiles.profiles[&name],
                        dev.pid,
                    )
                    .ok()
                    .flatten();
                    if color.is_some() || uniform_candidate.is_some() {
                        if let Err(error) =
                            secbird_core::profile_compat::validate_store_budget(store)
                        {
                            let result = Err(error);
                            if let Some(color) = color {
                                self.record_host_static(&key, &name, color, &result);
                            }
                            reports.push(ApplyReport {
                                device: dev.name(),
                                profile: name,
                                failures: vec![result.unwrap_err().to_string()],
                            });
                            continue;
                        }
                    }
                    let static_mode = if color.is_some() && !profiles.lighting_off {
                        Some(dev.ornata_mode_readback())
                    } else {
                        None
                    };
                    // Normal0 retains its existing saved-brightness path. Driver3
                    // keeps brightness in the host policy bridge, after its frame.
                    if color.is_some() {
                        let p = profiles.profiles.get_mut(&name).unwrap();
                        let zone = p.lighting.get_mut("backlight").unwrap();
                        zone.color = None;
                        if !static_mode
                            .as_ref()
                            .is_some_and(|m| m.as_ref().is_ok_and(|m| m.mode() == 0))
                        {
                            zone.brightness = None;
                            if let Some(settings) = p.lighting_settings.as_mut() {
                                settings.brightness = None;
                            }
                        }
                    }
                    let uniform_failures =
                        self.uniform_shadow(&self.devices[index], &mut profiles, &name);
                    let dev = &mut self.devices[index];
                    let mut failures: Vec<String> = profiles
                        .apply(&name, dev)
                        .into_iter()
                        .filter_map(|outcome| {
                            if handled && outcome.setting == "source idle/display lighting policy" {
                                None
                            } else {
                                outcome
                                    .result
                                    .err()
                                    .map(|e| format!("{}: {e}", outcome.setting))
                            }
                        })
                        .collect();
                    failures.extend(uniform_failures);
                    if let Some(color) = color {
                        if !profiles.lighting_off {
                            let result=match static_mode.expect("unblocked source Static was queried") {
                                Ok(mode) if mode.mode()==0 => dev.set_static_color(5,color).map(|_|serde_json::json!({"execution":"firmware"})),
                                Ok(mode) if mode.mode()==3 => host_gate.and_then(|gate| match gate {
                                    crate::host_static::Gate::Ready=>crate::host_static::send_uniform(dev,color),
                                    crate::host_static::Gate::Deferred(reason)=>{crate::host_static::preflight(dev)?;Ok(serde_json::json!({"execution":"host-static","deferred":true,"rendered":false,"reason":reason}))},
                                    crate::host_static::Gate::NeedsBaseline{..}=>Err(secbird_core::Error::Unsupported("Static activation needs an initialized brightness baseline; open Lighting".into())),
                                }),
                                Ok(mode)=>Err(secbird_core::Error::Unsupported(format!("Source Static mode{} is unsupported",mode.mode()))),
                                Err(error)=>Err(error),
                            };
                            if let Err(error) = &result {
                                failures.push(format!("source Static: {error}"));
                            }
                            self.record_host_static(&key, &name, color, &result);
                        } else if let Ok(crate::host_static::Gate::Deferred(reason)) = host_gate {
                            // Profile activation has retained intent but does not
                            // paint over display/idle/global-away suppression.
                            let result = Ok(
                                serde_json::json!({"execution":"host-static","deferred":true,"rendered":false,"reason":reason,"control_state_verified":false}),
                            );
                            self.record_host_static(&key, &name, color, &result);
                        }
                    }
                    ApplyReport {
                        device: self.devices[index].name(),
                        profile: name,
                        failures,
                    }
                }
                Err(error) => ApplyReport {
                    device: dev.name(),
                    profile: String::new(),
                    failures: vec![format!("profiles.json: {error}")],
                },
            };
            reports.push(report);
        }
        self.lighting_runtime.borrow_mut().invalidate(keys);
        self.refresh_lighting_runtime();
        let status = self.lighting_runtime.borrow().status();
        for (dev, report) in self
            .devices
            .iter()
            .filter(|dev| keys.contains(&dev.key()))
            .zip(&mut reports)
        {
            if dev.pid != 605 {
                continue;
            }
            let state = &status[dev.key()];
            if let Some(error) = state["error"].as_str() {
                report
                    .failures
                    .push(format!("source lighting runtime: {error}"));
            } else if self.lighting_runtime.borrow().running {
                if let Some(reason) = state["blocked_reason"].as_str().filter(|r| {
                    *r != "no-retained-source-brightness" && *r != "lighting-ownership-disabled"
                }) {
                    report
                        .failures
                        .push(format!("source lighting runtime blocked: {reason}"));
                }
            }
        }
        reports
    }

    /// Wireless mice that dropped their button remaps while asleep (#45) get them back.
    /// `skip` holds keys re-sent moments ago, left alone so a mouse that keeps losing
    /// them isn't written over and over. Returns the keys re-sent, with their reports.
    pub fn restore_lost_buttons(&self, skip: &HashSet<String>) -> Vec<(String, ApplyReport)> {
        let Ok(mut store) = self.load_store() else { return Vec::new() };
        self.devices
            .iter()
            .filter(|d| d.spec.as_ref().is_some_and(|s| s.wireless && s.kind == secbird_core::Kind::Mouse))
            .filter(|d| !skip.contains(&d.key()))
            .filter_map(|dev| {
                let p = store.for_device(dev);
                let name = p.effective(self.foreground()).to_string();
                if !p.buttons_lost(&name, dev) {
                    return None;
                }
                let failures = p
                    .apply_buttons(&name, dev)
                    .into_iter()
                    .filter_map(|a| a.result.err().map(|e| format!("{}: {e}", a.setting)))
                    .collect();
                Some((dev.key(), ApplyReport { device: self.label(dev), profile: name, failures }))
            })
            .collect()
    }

    pub fn all_keys(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.key()).collect()
    }

    /// Make `name` the active profile for a device and apply it.
    /// Every mouse's remapped buttons back to what the mouse stores. Returns (device, count).
    pub fn restore_buttons(&self) -> Vec<(String, Result<usize>)> {
        let mut store = match self.load_store() {
            Ok(s) => s,
            Err(e) => return vec![("profiles".into(), Err(e))],
        };
        self.devices
            .iter()
            .filter(|d| d.spec.as_ref().is_some_and(|s| s.kind == secbird_core::Kind::Mouse))
            .map(|d| (self.label(d), secbird_core::actions::restore_all_buttons(d, &mut store)))
            .collect()
    }

    pub fn switch_profile(&mut self, key: &str, name: &str) -> Result<Vec<ApplyReport>> {
        let dev = self
            .devices
            .iter()
            .find(|d| d.key() == key)
            .ok_or_else(|| secbird_core::Error::NotFound(format!("device {key}")))?;
        let mut store = self.load_store()?;
        store.for_device(dev).switch(name)?;
        store.save()?;
        Ok(self.apply(&[key.to_string()]))
    }

    /// Menu/tooltip label: the name, plus a serial tail when several units share it.
    pub fn label(&self, dev: &Device) -> String {
        let twins = self.devices.iter().filter(|d| d.pid == dev.pid).count();
        match dev.serial.len() {
            n if twins > 1 && n > 0 => format!("{} ({})", dev.name(), &dev.serial[n.saturating_sub(4)..]),
            _ => dev.name(),
        }
    }
}

#[cfg(test)]
mod lighting_runtime_tests {
    use super::*;
    use crate::lighting_runtime::{Environment, Owner};
    use secbird_core::{
        lighting::{Brightness, LightingSettings, SwitchOffLighting},
        sim::Sim,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        engine: Engine,
        sim: Sim,
        folder: std::path::PathBuf,
        key: String,
    }
    impl Fixture {
        fn new() -> Self {
            let folder = std::env::temp_dir().join(format!(
                "synapse-host-lighting-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let path = folder.join("profiles.json");
            let sim = Sim::keyboard();
            let engine = Engine::with_simulated_keyboard(&sim, path.clone()).unwrap();
            let dev = &engine.devices[0];
            let key = dev.key();
            dev.set_brightness(5, 73).unwrap();
            let mut store = Store::load(&path).unwrap();
            let settings = LightingSettings {
                brightness: Some(Brightness {
                    value: 73,
                    ..Default::default()
                }),
                switch_off: Some(SwitchOffLighting {
                    idle_enabled: true,
                    display_off: true,
                    ..Default::default()
                }),
                ..Default::default()
            };
            settings
                .store_in_profile(store.for_device(dev).active_mut())
                .unwrap();
            store.for_device(dev).active_mut().keys = vec![secbird_core::Rgb(1, 2, 3); 6 * 22];
            store.save().unwrap();
            engine.lighting_runtime.borrow_mut().test_environment = Some(Environment {
                now_ms: 60_000,
                idle_ms: Ok(60_000),
                display_off: Some(false),
                owner: Owner::Clear,
            });
            sim.state().log.clear();
            Self {
                engine,
                sim,
                folder,
                key,
            }
        }
        fn environment(&self, idle: u64, display: bool, owner: Owner) {
            self.engine.lighting_runtime.borrow_mut().test_environment = Some(Environment {
                now_ms: 60_000 + idle,
                idle_ms: Ok(idle),
                display_off: Some(display),
                owner,
            });
        }
        fn edit(&self, change: impl FnOnce(&mut Store, &Device)) {
            let mut store = self.engine.load_store().unwrap();
            change(&mut store, &self.engine.devices[0]);
            store.save().unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.folder.join("profiles.json"));
            let _ = std::fs::remove_dir(&self.folder);
        }
    }
    #[test]
    fn engine_composes_global_dim_blocks_frames_and_releases_on_shutdown_without_effect_writes() {
        let mut f = Fixture::new();
        f.engine.start_lighting_runtime();
        assert_eq!(f.sim.state().live_brightness[&5], 0);
        let before = f.sim.state().log.len();
        f.engine.refresh_lighting_runtime();
        f.engine.refresh_frames();
        assert_eq!(f.sim.state().log.len(), before);
        f.engine.lights_away(50);
        assert_eq!(f.sim.state().live_brightness[&5], 0);
        f.environment(0, false, Owner::Clear);
        f.engine.request_lighting_refresh();
        assert_eq!(
            f.sim.state().live_brightness[&5],
            91,
            "73% * 50% = 36%, encoded to91"
        );
        f.engine.lights_back();
        assert_eq!(f.sim.state().live_brightness[&5], 186);
        f.environment(60_000, true, Owner::Clear);
        f.engine.request_lighting_refresh();
        assert_eq!(f.sim.state().live_brightness[&5], 0);
        assert!(f.engine.shutdown_lighting_runtime().is_empty());
        assert_eq!(f.sim.state().live_brightness[&5], 186);
        assert_eq!(f.engine.devices[0].brightness(5).unwrap(), 73);
        assert!(f
            .sim
            .state()
            .log
            .iter()
            .all(|c| *c == secbird_core::commands::MATRIX_BRIGHTNESS
                || *c == secbird_core::commands::MATRIX_BRIGHTNESS.get()
                || *c == secbird_core::commands::ACTIVE_SLOT.get()));
        assert_eq!(
            f.engine.lighting_runtime_status()[&f.key]["can_execute_policies"],
            false
        );
        let before = f.sim.state().log.len();
        f.engine.refresh_frames();
        f.engine.lights_away(0);
        f.engine.request_lighting_refresh();
        assert_eq!(
            f.sim.state().log.len(),
            before,
            "late display/global-away events must not undo shutdown restoration"
        );
    }
    #[test]
    fn engine_uses_latest_profile_value_and_respects_owner_revocation_and_competing_processes() {
        let mut f = Fixture::new();
        f.engine.start_lighting_runtime();
        f.edit(|store, dev| {
            let profiles = store.for_device(dev);
            profiles.create("Other").unwrap();
            profiles.active = "Other".into();
            let mut settings = profiles.active().lighting_settings.clone().unwrap();
            settings.brightness.as_mut().unwrap().set_value(41).unwrap();
            settings.store_in_profile(profiles.active_mut()).unwrap();
        });
        f.engine.request_lighting_refresh();
        assert_eq!(f.sim.state().live_brightness[&5], 0);
        f.environment(
            0,
            false,
            Owner::Competing(vec!["RazerAppEngine.exe".into()]),
        );
        let before = f.sim.state().log.len();
        f.engine.request_lighting_refresh();
        f.engine.refresh_frames();
        assert_eq!(f.sim.state().log.len(), before);
        assert!(f.engine.lighting_runtime_status()[&f.key]["blocked_reason"]
            .as_str()
            .unwrap()
            .contains("competing"));
        f.environment(0, false, Owner::Clear);
        f.edit(|store, dev| store.for_device(dev).lighting_off = true);
        f.engine.request_lighting_refresh();
        assert_eq!(f.sim.state().log.len(), before);
        f.edit(|store, dev| store.for_device(dev).lighting_off = false);
        f.engine.request_lighting_refresh();
        assert_eq!(
            f.sim.state().live_brightness[&5],
            104,
            "restore current41 rather than old73"
        );
        assert_eq!(
            f.engine.devices[0].brightness(5).unwrap(),
            73,
            "runtime never changes saved hardware brightness"
        );
    }
    #[test]
    fn shutdown_reports_unrestored_lease_without_fighting_a_competing_owner() {
        let mut f = Fixture::new();
        f.engine.start_lighting_runtime();
        f.environment(
            0,
            false,
            Owner::Competing(vec!["RazerAppEngine.exe".into()]),
        );
        let before = f.sim.state().log.len();
        let failures = f.engine.shutdown_lighting_runtime();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("competing-synapse-owner"));
        assert_eq!(f.sim.state().log.len(), before);
        assert_eq!(
            f.engine.lighting_runtime_status()[&f.key]["restoration_pending"],
            true
        );
    }

    #[test]
    fn engine_handles_core_policy_outcome_only_when_its_actual_gate_is_ready() {
        let mut f = Fixture::new();
        f.environment(0, false, Owner::Clear);
        f.edit(|store, dev| store.for_device(dev).active_mut().keys.clear());
        let mut store = f.engine.load_store().unwrap();
        let core = store
            .for_device(&f.engine.devices[0])
            .active()
            .apply(&f.engine.devices[0]);
        assert!(core
            .iter()
            .any(|r| r.setting == "source idle/display lighting policy" && r.result.is_err()));
        f.engine.start_lighting_runtime();
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0].failures.is_empty());
        f.environment(0, false, Owner::Unknown("unavailable".into()));
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0]
            .failures
            .iter()
            .any(|e| e.contains("owner-check-unavailable")));
        f.environment(60_000, false, Owner::Clear);
        f.sim.state().leds.clear();
        let reports = f.engine.apply(std::slice::from_ref(&f.key));
        assert!(reports[0]
            .failures
            .iter()
            .any(|e| e.contains("source lighting runtime")));
    }
}
