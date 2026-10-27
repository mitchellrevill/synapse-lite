//! User-facing changes with shared validation and persistence. Profile settings are
//! recorded in the active profile; device-wide Power intent stays outside profiles.

use crate::device::Device;
use crate::error::Result;
use crate::features::{DpiStages, Rgb};
use crate::profile::{Store, Zone};

fn zone<'a>(store: &'a mut Store, dev: &Device, name: &str) -> &'a mut Zone {
    store.for_device(dev).active_mut().lighting.entry(name.to_string()).or_default()
}

/// Only change ownership; neither enabling nor disabling sends a lighting command.
pub fn set_lighting_control(dev: &Device, store: &mut Store, enabled: bool) -> Result<()> {
    store.for_device(dev).lighting_off = !enabled;
    store.save()
}

fn check_lighting(dev: &Device, store: &mut Store) -> Result<()> {
    if store.for_device(dev).lighting_off {
        return Err(crate::error::Error::InvalidArg(format!(
            "lighting control is disabled for {}; enable it before changing lighting", dev.name()
        )));
    }
    Ok(())
}

/// Returns the DPI actually set, after rounding to the device's step.
pub fn set_dpi(dev: &Device, store: &mut Store, x: u16, y: u16) -> Result<(u16, u16)> {
    if let Some(mut intent) = store.for_device(dev).active().performance.clone() {
        intent.validate()?;
        let (x, y) = dev.fitted_dpi(x, y)?;
        let row = &mut intent.stages[usize::from(intent.active - 1)];
        row.x = x;
        row.y = y;
        if x != y { row.independent = true; }
        let sent = set_performance(dev, store, &intent)?;
        let packed = sent.pack()?;
        return Ok(packed.stages[usize::from(packed.active - 1)]);
    }
    let applied = dev.set_dpi(x, y)?;
    store.for_device(dev).active_mut().dpi = Some(applied);
    store.save()?;
    Ok(applied)
}

/// Replace the DPI stages. The mouse moves to the active stage, so the profile's DPI
/// follows it. Returns the stages as sent, after rounding.
pub fn set_dpi_stages(dev: &Device, store: &mut Store, stages: &DpiStages) -> Result<DpiStages> {
    let sent = dev.set_dpi_stages(stages)?;
    let profile = store.for_device(dev).active_mut();
    profile.dpi = Some(sent.stages[sent.active as usize - 1]);
    profile.dpi_stages = Some(sent.clone());
    // This legacy operation explicitly replaces the physical stage list.
    // It carries no hidden-row or independent-axis intent.
    profile.performance = None;
    store.save()?;
    Ok(sent)
}

/// Apply the source stage intent while retaining rows not sent to the device.
/// Validate the snapped complete candidate before any hardware write.
pub fn set_performance(dev: &Device, store: &mut Store, want: &crate::performance::Performance) -> Result<crate::performance::Performance> {
    want.validate()?;
    let mut normalized = want.clone();
    let selected = usize::from(want.active - 1);
    for (index, row) in normalized.stages.iter_mut().enumerate() {
        if (want.enabled && row.visible) || (!want.enabled && index == selected) {
            (row.x, row.y) = dev.fitted_dpi(row.x, row.y)?;
        }
    }
    let before = store.for_device(dev).active().clone();
    normalized.store_in_profile(store.for_device(dev).active_mut())?;
    let validation = crate::profile_compat::validate_store_budget(store);
    *store.for_device(dev).active_mut() = before;
    validation?;
    let sent = normalized.apply(dev)?;
    sent.store_in_profile(store.for_device(dev).active_mut())?;
    store.save()?;
    Ok(sent)
}

/// Change device-wide low-power intent, independent of the active profile.
/// The expected physical identity is required even when a caller selected by PID.
/// Validate the complete prospective native backup before any hardware request.
pub fn set_critical_threshold(
    dev: &Device,
    store: &mut Store,
    expected_device: &str,
    percent: u8,
) -> Result<u8> {
    if expected_device != dev.key() {
        return Err(crate::Error::InvalidArg(
            "The selected device changed; refresh before changing Power settings".into(),
        ));
    }
    if !matches!(dev.pid, 196 | 197) {
        return Err(crate::Error::Unsupported(
            "low-power threshold is verified only for DeathAdder V3 HyperSpeed".into(),
        ));
    }
    crate::power::critical_raw_for_percent(percent)?;
    let device_key = dev.key();
    let previous_keys: Vec<String> = store.devices.keys().cloned().collect();
    let previous = store.for_device(dev).power.clone();
    let mut desired = previous.clone().unwrap_or_default();
    desired.critical_threshold_percent = Some(percent);
    store.for_device(dev).power = Some(desired.clone());
    let validation = crate::profile_compat::validate_store_budget(store);
    store.for_device(dev).power = previous;
    // for_device can create a default entry or migrate its connection-mode key.
    // Undo that temporary resolution too, so a refusal leaves the native state intact.
    if !previous_keys.contains(&device_key) {
        let candidate = store
            .devices
            .remove(&device_key)
            .expect("candidate device exists");
        if let Some(original_key) = previous_keys
            .iter()
            .find(|key| !store.devices.contains_key(*key))
        {
            store.devices.insert(original_key.clone(), candidate);
        }
    }
    validation?;
    if dev.polling_rate()? > 1000 {
        return Err(crate::Error::InvalidArg(
            "Low Power Mode is not supported at polling rates of 2000 Hz and above".into(),
        ));
    }
    let sent = dev.set_critical_threshold_percent(percent)?;
    desired.critical_threshold_percent = Some(sent);
    store.for_device(dev).power = Some(desired);
    store.save()?;
    Ok(sent)
}

/// Installed-model Power page: hardware is the source of truth for sleep.
/// Keep legacy profile-local sleep APIs available for existing native profiles.
pub fn set_device_sleep_minutes(dev: &Device, expected_device: &str, minutes: u16) -> Result<()> {
    if expected_device != dev.key() {
        return Err(crate::Error::InvalidArg(
            "The selected device changed; refresh before changing Power settings".into(),
        ));
    }
    dev.set_device_sleep_minutes(minutes)
}

pub fn set_sleep_minutes(dev: &Device, store: &mut Store, minutes: u16) -> Result<()> {
    dev.set_sleep_minutes(minutes)?;
    store.for_device(dev).active_mut().sleep_minutes = Some(minutes);
    store.save()
}

pub fn set_dimming(dev: &Device, store: &mut Store, dimming: crate::Dimming) -> Result<()> {
    dev.set_dimming(dimming)?;
    store.for_device(dev).active_mut().dimming = Some(dimming.to_string());
    store.save()
}

pub fn set_game_mode(dev: &Device, store: &mut Store, mode: crate::GameMode) -> Result<()> {
    dev.set_game_mode(mode)?;
    store.for_device(dev).active_mut().game_mode = Some(mode.to_string());
    store.save()
}

pub fn set_auto_sleep(dev: &Device, store: &mut Store, enabled: bool) -> Result<()> {
    dev.set_auto_sleep(enabled)?;
    store.for_device(dev).active_mut().auto_sleep = Some(enabled);
    store.save()
}

pub fn set_polling(dev: &Device, store: &mut Store, hz: u32) -> Result<()> {
    dev.set_polling_rate(hz)?;
    store.for_device(dev).active_mut().polling = Some(hz);
    store.save()
}

pub fn set_color(dev: &Device, store: &mut Store, zone_name: &str, color: Rgb) -> Result<()> {
    check_lighting(dev, store)?;
    dev.set_static_color(dev.led_id(zone_name)?, color)?;
    store.for_device(dev).active_mut().keys.clear();
    let z = zone(store, dev, zone_name);
    z.color = Some(color);
    z.effect = None;
    z.off = false;
    store.save()
}

/// A built-in effect on one zone. Like a color, it replaces per-key colors.
pub fn set_effect(dev: &Device, store: &mut Store, zone_name: &str, effect: crate::effects::Effect) -> Result<()> {
    check_lighting(dev, store)?;
    dev.set_effect(dev.led_id(zone_name)?, &effect)?;
    store.for_device(dev).active_mut().keys.clear();
    let z = zone(store, dev, zone_name);
    z.effect = Some(effect);
    z.off = false;
    store.save()
}

/// Brightness in percent, clamped to 0-100.
pub fn set_brightness(dev: &Device, store: &mut Store, zone_name: &str, percent: u8) -> Result<()> {
    check_lighting(dev, store)?;
    let percent = percent.min(100);
    dev.set_brightness(dev.led_id(zone_name)?, percent)?;
    zone(store, dev, zone_name).brightness = Some(percent);
    store.save()
}

pub fn lights_off(dev: &Device, store: &mut Store, zone_name: &str) -> Result<()> {
    check_lighting(dev, store)?;
    dev.lights_off(dev.led_id(zone_name)?)?;
    store.for_device(dev).active_mut().keys.clear();
    zone(store, dev, zone_name).off = true;
    store.save()
}

/// Per-key colors (row-major). They replace any whole-zone color or off state;
/// brightness is kept.
pub fn set_keys(dev: &Device, store: &mut Store, keys: Vec<Rgb>) -> Result<()> {
    check_lighting(dev, store)?;
    dev.show_frame(&keys)?;
    let profile = store.for_device(dev).active_mut();
    for z in profile.lighting.values_mut() {
        z.color = None;
        z.effect = None;
        z.off = false;
    }
    profile.keys = keys;
    store.save()
}

/// Light one matrix cell white and the rest off, so a user can say which key it is
/// (finding a board's layout, #45). Nothing is saved; apply the profile again afterwards.
pub fn show_light(dev: &Device, store: &mut Store, row: u8, col: u8) -> Result<()> {
    check_lighting(dev, store)?;
    let m = dev.matrix().ok_or_else(|| crate::error::Error::Unsupported("no per-key lighting".into()))?;
    if row >= m.rows || col >= m.cols {
        return Err(crate::error::Error::InvalidArg(format!("light {row},{col} outside {}x{}", m.rows, m.cols)));
    }
    let mut frame = vec![Rgb(0, 0, 0); m.rows as usize * m.cols as usize];
    frame[row as usize * m.cols as usize + col as usize] = Rgb(255, 255, 255);
    dev.show_frame(&frame)
}

/// Remap a button or key (`None` puts back its saved setting). `button` is a name from
/// [`crate::buttons::BUTTONS`], or `key-` and a name from [`crate::buttons::KEYS`].
pub fn set_button(
    dev: &Device,
    store: &mut Store,
    button: &str,
    layer: crate::buttons::Layer,
    action: Option<crate::buttons::Action>,
) -> Result<()> {
    let key = crate::buttons::setting_key(button, layer);
    let (input, _) = crate::buttons::parse_setting_key(&key)?;
    if let (Some(action), crate::buttons::Input::Button(id)) = (action, input) {
        keeps_a_left_click(dev, id, layer, action)?;
    }
    let profiles = store.for_device(dev);
    match action {
        Some(action) => {
            dev.set_input(input, layer, action)?;
            profiles.active_mut().buttons.insert(key, action.to_string());
        }
        None => {
            profiles.reset_button(&key, dev)?;
            profiles.active_mut().buttons.remove(&key);
        }
    }
    store.save()
}

/// Refuse a change that would leave the mouse with no left click on its normal layer: the
/// user could no longer click anything, including the button to undo it.
fn keeps_a_left_click(dev: &Device, id: u8, layer: crate::buttons::Layer, action: crate::buttons::Action) -> Result<()> {
    use crate::buttons::{Action, Layer};
    const LEFT: Action = Action::Mouse(1);
    if layer != Layer::Normal || action == LEFT || dev.button(id, Layer::Normal).ok() != Some(LEFT) {
        return Ok(());
    }
    if dev.buttons().iter().any(|&(other, a)| other != id && a == LEFT) {
        return Ok(());
    }
    Err(crate::error::Error::InvalidArg(
        "this is the only left click; give another button left click first, or you couldn't click anything".into(),
    ))
}

/// Put every remapped button of the active profile back to what the mouse stores (Synapse's
/// or the factory setting), on both layers, and forget the remaps so they aren't applied
/// again. A "Save to mouse" is put back first, so this is a full way back. Returns how many
/// buttons were restored.
pub fn restore_all_buttons(dev: &Device, store: &mut Store) -> Result<usize> {
    if store.for_device(dev).onboard.is_some() {
        put_back_onboard(dev, store)?;
    }
    let keys: Vec<String> = store.for_device(dev).active().buttons.keys().cloned().collect();
    let mut restored = 0;
    let mut first_error = None;
    for key in keys {
        match crate::buttons::parse_setting_key(&key).and_then(|(input, layer)| dev.reset_input(input, layer)) {
            Ok(_) => {
                store.for_device(dev).active_mut().buttons.remove(&key);
                restored += 1;
            }
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    store.save()?;
    match first_error {
        Some(e) if restored == 0 => Err(e),
        _ => Ok(restored),
    }
}

/// "Save to mouse": the active profile's button remaps go into the mouse's active on-board
/// slot, so they work without Secretarybird. What each button held there before is kept
/// first (once, so it stays Synapse's or the factory setting), and buttons an earlier save
/// wrote that this profile doesn't remap go back to it. Returns the slot written.
pub fn save_buttons_to_mouse(dev: &Device, store: &mut Store) -> Result<u8> {
    if dev.buttons().is_empty() {
        return Err(crate::error::Error::Unsupported(format!("{} keeps no button settings on board", dev.name())));
    }
    let slot = dev.saved_slot();
    if store.for_device(dev).onboard.as_ref().is_some_and(|o| o.slot != slot) {
        put_back_onboard(dev, store)?;
    }
    let profiles = store.for_device(dev);
    let want = profiles.active().buttons.clone();
    let mut save = profiles.onboard.clone().unwrap_or_default();
    // The backup goes to disk before anything is written to the mouse.
    for key in want.keys() {
        if !save.before.contains_key(key) {
            let (input, layer) = crate::buttons::parse_setting_key(key)?;
            save.before.insert(key.clone(), dev.slot_input(slot, input, layer)?.to_string());
        }
    }
    save.slot = slot;
    profiles.onboard = Some(save.clone());
    store.save()?;

    for (key, original) in save.before.iter().filter(|(k, _)| !want.contains_key(*k)) {
        let (input, layer) = crate::buttons::parse_setting_key(key)?;
        dev.set_slot_input(slot, input, layer, original.parse()?)?;
    }
    for (key, action) in &want {
        let (input, layer) = crate::buttons::parse_setting_key(key)?;
        dev.set_slot_input(slot, input, layer, action.parse()?)?;
    }
    let profiles = store.for_device(dev);
    save.before.retain(|k, _| want.contains_key(k));
    save.profile = profiles.active.clone();
    save.buttons = want;
    profiles.onboard = Some(save);
    store.save()?;
    Ok(slot)
}

/// Undo "Save to mouse": the on-board slot gets back what it held before. The live remaps
/// stay as they are. Returns how many buttons were put back.
pub fn put_back_onboard(dev: &Device, store: &mut Store) -> Result<usize> {
    let Some(save) = store.for_device(dev).onboard.clone() else { return Ok(0) };
    for (key, original) in &save.before {
        let (input, layer) = crate::buttons::parse_setting_key(key)?;
        dev.set_slot_input(save.slot, input, layer, original.parse()?)?;
    }
    store.for_device(dev).onboard = None;
    store.save()?;
    Ok(save.before.len())
}

/// One zone's outcome from [`sync_color`].
pub struct Synced {
    pub pid: u16,
    /// [`Device::key`]: tells identical models apart.
    pub key: String,
    pub device: String,
    pub zone: String,
    pub result: Result<()>,
}

/// The same static color on every lighting zone of every device, each recorded in
/// that device's active profile. Devices without lighting or with control disabled are skipped.
pub fn sync_color(devices: &[Device], store: &mut Store, color: Rgb) -> Vec<Synced> {
    let mut out = Vec::new();
    for dev in devices {
        if store.for_device(dev).lighting_off {
            continue;
        }
        let zones: Vec<String> = dev.spec.as_ref().map(|s| s.leds.iter().map(|l| l.name.clone()).collect()).unwrap_or_default();
        for zone in zones {
            let result = set_color(dev, store, &zone, color);
            out.push(Synced { pid: dev.pid, key: dev.key(), device: dev.name(), zone, result });
        }
    }
    out
}

/// The same built-in effect on every lighting zone of every device, started together so
/// they run in step (each device keeps its own timer, so they can drift apart over time;
/// syncing again lines them up). The zones are all sent first and the profiles saved
/// once after, to keep the start times close. Devices with lighting control disabled are skipped.
pub fn sync_effect(devices: &[Device], store: &mut Store, effect: &crate::effects::Effect) -> Vec<Synced> {
    let mut out = Vec::new();
    for dev in devices {
        if store.for_device(dev).lighting_off {
            continue;
        }
        let zones: Vec<String> = dev.spec.as_ref().map(|s| s.leds.iter().map(|l| l.name.clone()).collect()).unwrap_or_default();
        for zone in zones {
            let result = dev.led_id(&zone).and_then(|led| dev.set_effect(led, effect));
            out.push(Synced { pid: dev.pid, key: dev.key(), device: dev.name(), zone, result });
        }
    }
    for dev in devices {
        let done: Vec<&Synced> = out.iter().filter(|r| r.key == dev.key() && r.result.is_ok()).collect();
        if done.is_empty() {
            continue;
        }
        store.for_device(dev).active_mut().keys.clear();
        for r in done {
            let z = zone(store, dev, &r.zone);
            z.effect = Some(effect.clone());
            z.off = false;
        }
    }
    if let Err(e) = store.save() {
        out.iter_mut().filter(|r| r.result.is_ok()).for_each(|r| r.result = Err(crate::error::Error::System(format!("set, but not saved: {e}"))));
    }
    out
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use crate::performance::{DpiStage, Performance};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    fn store() -> (Store, std::path::PathBuf) {
        let folder = std::env::temp_dir().join(format!("synapse-performance-actions-{}-{}",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed)));
        (Store::load(&folder.join("profiles.json")).unwrap(),folder)
    }
    fn intent() -> Performance {
        Performance { enabled:true, active:3, stages:[(400,true),(800,false),(1600,true)].into_iter().map(|(value,visible)|DpiStage {
            x:value,y:value,independent:false,visible,extra:BTreeMap::from([("future".into(),serde_json::json!(value))])
        }).collect(), extra:BTreeMap::new() }
    }

    #[test]
    fn explicit_dpi_edits_keep_hidden_rows_and_persist_the_verified_projection() {
        let sim=crate::sim::Sim::mouse();
        let dev=sim.open(196,crate::sim::Sim::spec_for(crate::sim::MOUSE_PID)).unwrap();
        let (mut store,folder)=store();
        let want=intent();
        set_performance(&dev,&mut store,&want).unwrap();
        assert_eq!(dev.dpi_stages().unwrap().stages,vec![(400,400),(1600,1600)]);
        assert_eq!(set_dpi(&dev,&mut store,1801,1901).unwrap(),(1800,1900));
        let mut disk=Store::load(&folder.join("profiles.json")).unwrap();
        let saved=disk.for_device(&dev).active().performance.as_ref().unwrap();
        assert_eq!(saved.stages[1],want.stages[1]);
        assert!(saved.stages[2].independent);
        assert_eq!(dev.dpi().unwrap(),(1800,1900));
        assert_eq!(dev.dpi_stages().unwrap().stages,vec![(400,400),(1800,1900)]);
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn oversized_intent_is_refused_before_hardware_or_profile_changes() {
        let sim=crate::sim::Sim::mouse();
        let dev=sim.open(196,crate::sim::Sim::spec_for(crate::sim::MOUSE_PID)).unwrap();
        let before=dev.dpi_stages().unwrap();
        let (mut store,folder)=store();
        let mut want=intent();
        want.extra.insert("future-large".into(),serde_json::json!("x".repeat(2*1024*1024)));
        assert!(set_performance(&dev,&mut store,&want).is_err());
        assert_eq!(dev.dpi_stages().unwrap(),before);
        assert!(store.for_device(&dev).active().performance.is_none());
        assert!(!folder.exists());
    }
}

#[cfg(test)]
mod power_tests {
    use super::*;
    use crate::{profile::DeviceProfiles, sim::Sim, DevicePower};
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    fn setup(high: bool) -> (Sim, Device, Store, std::path::PathBuf) {
        let sim = Sim::wireless_mouse();
        sim.state().critical_threshold = Some(78);
        let mut spec = Sim::spec_for(crate::sim::WIRELESS_MOUSE_PID).unwrap();
        if high {
            spec.polling = Some(crate::db::Polling::High);
            sim.state().polling_hs = Some(crate::commands::polling_hs_code(2000).unwrap());
        }
        let dev = sim.open(196, Some(spec)).unwrap();
        let folder = std::env::temp_dir().join(format!(
            "synapse-power-actions-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        store.for_device(&dev);
        (sim, dev, store, folder)
    }
    fn cleanup(folder: &std::path::Path) {
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn threshold_is_device_scoped_preserves_unknowns_and_profile_activation_does_not_apply_it() {
        let (sim, dev, mut store, folder) = setup(false);
        store.for_device(&dev).create("Second").unwrap();
        store.for_device(&dev).power = Some(DevicePower {
            extra: BTreeMap::from([("future".into(), serde_json::json!({"keep":true}))]),
            ..Default::default()
        });
        let profiles = store.for_device(&dev).profiles.clone();
        assert_eq!(
            set_critical_threshold(&dev, &mut store, &dev.key(), 45).unwrap(),
            45
        );
        assert_eq!(store.for_device(&dev).profiles, profiles);
        let mut disk = Store::load(&folder.join("profiles.json")).unwrap();
        let power = disk.for_device(&dev).power.as_ref().unwrap();
        assert_eq!(power.critical_threshold_percent, Some(45));
        assert_eq!(power.extra["future"], serde_json::json!({"keep":true}));
        let count = sim
            .state()
            .log
            .iter()
            .filter(|c| **c == crate::commands::CRITICAL_LEVEL)
            .count();
        store.for_device(&dev).active = "Second".into();
        assert!(store
            .for_device(&dev)
            .active()
            .apply(&dev)
            .iter()
            .all(|r| r.result.is_ok()));
        assert_eq!(
            sim.state()
                .log
                .iter()
                .filter(|c| **c == crate::commands::CRITICAL_LEVEL)
                .count(),
            count
        );
        assert_eq!(
            store
                .for_device(&dev)
                .power
                .as_ref()
                .unwrap()
                .critical_threshold_percent,
            Some(45)
        );
        cleanup(&folder);
    }
    #[test]
    fn high_polling_and_identity_mismatch_refuse_before_threshold_write() {
        let (sim, dev, mut store, folder) = setup(true);
        let before = sim.state().log.len();
        assert!(set_critical_threshold(&dev, &mut store, "stale-device", 30).is_err());
        assert_eq!(sim.state().log.len(), before);
        assert!(set_critical_threshold(&dev, &mut store, &dev.key(), 30).is_err());
        assert_eq!(sim.state().critical_threshold, Some(78));
        assert!(!sim.state().log.contains(&crate::commands::CRITICAL_LEVEL));
        assert!(store.for_device(&dev).power.is_none());
        assert!(!folder.exists());
        // Recovery remains unrestricted by the normal UI polling policy.
        dev.set_critical_threshold_raw(78).unwrap();
    }
    #[test]
    fn budget_and_invalid_ui_values_refuse_before_any_hardware_request() {
        let (sim, dev, mut store, folder) = setup(false);
        store.extra.insert(
            "too-large".into(),
            serde_json::json!("x".repeat(2 * 1024 * 1024)),
        );
        let before = sim.state().log.len();
        assert!(set_critical_threshold(&dev, &mut store, &dev.key(), 30).is_err());
        assert!(set_critical_threshold(&dev, &mut store, &dev.key(), 31).is_err());
        assert_eq!(sim.state().log.len(), before);
        assert!(store.for_device(&dev).power.is_none());
        assert!(!folder.exists());
        store.devices.clear();
        let before_state = serde_json::to_value(&store).unwrap();
        assert!(set_critical_threshold(&dev, &mut store, &dev.key(), 30).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), before_state);
        assert_eq!(sim.state().log.len(), before);
    }
    #[test]
    fn device_sleep_changes_hardware_only_and_legacy_profile_sleep_remains_usable() {
        let (sim, dev, mut store, folder) = setup(false);
        store.for_device(&dev).active_mut().sleep_minutes = Some(7);
        store.for_device(&dev).create("Second").unwrap();
        store.for_device(&dev).active = "Second".into();
        let before = serde_json::to_value(&store).unwrap();
        set_device_sleep_minutes(&dev, &dev.key(), 3).unwrap();
        assert_eq!(sim.state().idle_seconds, Some(180));
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
        assert!(!folder.exists());
        assert!(store
            .for_device(&dev)
            .active()
            .apply(&dev)
            .iter()
            .all(|r| r.result.is_ok()));
        assert_eq!(sim.state().idle_seconds, Some(420));
        let count = sim.state().log.len();
        assert!(set_device_sleep_minutes(&dev, "other-device", 5).is_err());
        assert!(set_device_sleep_minutes(&dev, &dev.key(), 0).is_err());
        assert!(set_device_sleep_minutes(&dev, &dev.key(), 16).is_err());
        assert_eq!(sim.state().log.len(), count);
    }
    #[test]
    fn native_power_defaults_and_conflicting_imports_are_lossless_without_hardware() {
        assert!(serde_json::to_value(DeviceProfiles::default())
            .unwrap()
            .get("power")
            .is_none());
        let mut store = Store::default();
        store.device(196).power = Some(DevicePower {
            critical_threshold_percent: Some(30),
            ..Default::default()
        });
        let mut incoming = Store::default();
        incoming.device(196).power = Some(DevicePower {
            critical_threshold_percent: Some(45),
            extra: BTreeMap::from([("future".into(), serde_json::json!(42))]),
        });
        store.import(incoming);
        let device = store.device(196);
        assert_eq!(
            device.power.as_ref().unwrap().critical_threshold_percent,
            Some(30)
        );
        assert_eq!(device.power.as_ref().unwrap().extra["future"], 42);
        assert_eq!(
            device.extra["_synapseLiteImported:power"]["critical_threshold_percent"],
            45
        );
        let value = serde_json::to_value(&store).unwrap();
        let restored: Store = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), value);
    }
}
