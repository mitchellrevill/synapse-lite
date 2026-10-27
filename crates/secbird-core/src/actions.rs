//! User-facing changes with shared validation and persistence. Profile settings are
//! recorded in the active profile; device-wide Power intent stays outside profiles.

use crate::device::Device;
use crate::error::Result;
use crate::features::{DpiStages, Rgb};
use crate::profile::{Store, Zone};

/// Deterministic resource refusal precedes all SETs. Filesystem errors after
/// device mutation remain errors, not a hardware rollback claim.
fn write_and_commit<T>(
    store: &mut Store,
    candidate: Store,
    write: impl FnOnce() -> Result<T>,
) -> Result<T> {
    candidate.validate_save()?;
    let result = write()?;
    candidate.save()?;
    *store = candidate;
    Ok(result)
}

fn zone<'a>(store: &'a mut Store, dev: &Device, name: &str) -> &'a mut Zone {
    store
        .for_device(dev)
        .active_mut()
        .lighting
        .entry(name.to_string())
        .or_default()
}

/// Only change ownership; neither enabling nor disabling sends a lighting command.
pub fn set_lighting_control(dev: &Device, store: &mut Store, enabled: bool) -> Result<()> {
    let mut candidate = store.clone();
    candidate.for_device(dev).lighting_off = !enabled;
    write_and_commit(store, candidate, || Ok(()))
}

fn check_lighting(dev: &Device, store: &mut Store) -> Result<()> {
    if store.for_device(dev).lighting_off {
        return Err(crate::error::Error::InvalidArg(format!(
            "lighting control is disabled for {}; enable it before changing lighting",
            dev.name()
        )));
    }
    Ok(())
}

// Legacy color/effect callers have no mutable host-frame runtime. A firmware
// acknowledgement in Driver Mode does not demonstrate that color was rendered.
fn check_firmware_lighting(dev: &Device) -> Result<()> {
    if dev.pid == 605 && dev.interface == 2 {
        let mode = dev.ornata_mode_readback()?;
        if mode.mode() != 0 {
            return Err(crate::Error::Unsupported(format!(
                "Ornata firmware color/effect requires Normal Mode0; observed mode{}. Use the tray's source Lighting controls for supported host effects; no mode change was attempted",
                mode.mode()
            )));
        }
    }
    Ok(())
}

/// Returns the DPI actually set, after rounding to the device's step.
pub fn set_dpi(dev: &Device, store: &mut Store, x: u16, y: u16) -> Result<(u16, u16)> {
    let mut candidate = store.clone();
    if let Some(mut intent) = candidate.for_device(dev).active().performance.clone() {
        intent.validate()?;
        let (x, y) = dev.fitted_dpi(x, y)?;
        let row = &mut intent.stages[usize::from(intent.active - 1)];
        row.x = x;
        row.y = y;
        if x != y {
            row.independent = true;
        }
        let sent = set_performance(dev, store, &intent)?;
        let packed = sent.pack()?;
        return Ok(packed.stages[usize::from(packed.active - 1)]);
    }
    let fitted = dev.fitted_dpi(x, y)?;
    candidate.for_device(dev).active_mut().dpi = Some(fitted);
    write_and_commit(store, candidate, || dev.set_dpi(fitted.0, fitted.1))
}

/// Replace the DPI stages. The mouse moves to the active stage, so the profile's DPI
/// follows it. Returns the stages as sent, after rounding.
pub fn set_dpi_stages(dev: &Device, store: &mut Store, stages: &DpiStages) -> Result<DpiStages> {
    let fitted = dev.fitted_dpi_stages(stages)?;
    let mut candidate = store.clone();
    let profile = candidate.for_device(dev).active_mut();
    profile.dpi = Some(fitted.stages[usize::from(fitted.active - 1)]);
    profile.dpi_stages = Some(fitted.clone());
    profile.performance = None;
    write_and_commit(store, candidate, || dev.set_dpi_stages(&fitted))
}

/// Apply the source stage intent while retaining rows not sent to the device.
/// Validate the snapped complete candidate before any hardware write.
pub fn set_performance(
    dev: &Device,
    store: &mut Store,
    want: &crate::performance::Performance,
) -> Result<crate::performance::Performance> {
    want.validate()?;
    let mut normalized = want.clone();
    let selected = usize::from(want.active - 1);
    for (index, row) in normalized.stages.iter_mut().enumerate() {
        if (want.enabled && row.visible) || (!want.enabled && index == selected) {
            (row.x, row.y) = dev.fitted_dpi(row.x, row.y)?;
        }
    }
    let mut candidate = store.clone();
    normalized.store_in_profile(candidate.for_device(dev).active_mut())?;
    write_and_commit(store, candidate, || normalized.apply(dev))
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
    let mut candidate = store.clone();
    let mut desired = candidate.for_device(dev).power.clone().unwrap_or_default();
    desired.critical_threshold_percent = Some(percent);
    candidate.for_device(dev).power = Some(desired);
    candidate.validate_save()?;
    if dev.polling_rate()? > 1000 {
        return Err(crate::Error::InvalidArg(
            "Low Power Mode is not supported at polling rates of 2000 Hz and above".into(),
        ));
    }
    write_and_commit(store, candidate, || {
        dev.set_critical_threshold_percent(percent)
    })
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
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().sleep_minutes = Some(minutes);
    write_and_commit(store, candidate, || dev.set_sleep_minutes(minutes))
}

pub fn set_dimming(dev: &Device, store: &mut Store, dimming: crate::Dimming) -> Result<()> {
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().dimming = Some(dimming.to_string());
    write_and_commit(store, candidate, || dev.set_dimming(dimming))
}

pub fn set_game_mode(dev: &Device, store: &mut Store, mode: crate::GameMode) -> Result<()> {
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().game_mode = Some(mode.to_string());
    write_and_commit(store, candidate, || dev.set_game_mode(mode))
}

pub fn set_auto_sleep(dev: &Device, store: &mut Store, enabled: bool) -> Result<()> {
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().auto_sleep = Some(enabled);
    write_and_commit(store, candidate, || dev.set_auto_sleep(enabled))
}

pub fn set_polling(dev: &Device, store: &mut Store, hz: u32) -> Result<()> {
    let mut candidate = store.clone();
    candidate.for_device(dev).active_mut().polling = Some(hz);
    write_and_commit(store, candidate, || dev.set_polling_rate(hz))
}

pub fn set_color(dev: &Device, store: &mut Store, zone_name: &str, color: Rgb) -> Result<()> {
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    let led = dev.led_id(zone_name)?;
    candidate.for_device(dev).active_mut().keys.clear();
    let z = zone(&mut candidate, dev, zone_name);
    z.color = Some(color);
    z.effect = None;
    z.off = false;
    candidate.validate_save()?;
    check_firmware_lighting(dev)?;
    write_and_commit(store, candidate, || dev.set_static_color(led, color))
}

/// A built-in effect on one zone. Like a color, it replaces per-key colors.
pub fn set_effect(
    dev: &Device,
    store: &mut Store,
    zone_name: &str,
    effect: crate::effects::Effect,
) -> Result<()> {
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    let led = dev.led_id(zone_name)?;
    candidate.for_device(dev).active_mut().keys.clear();
    let z = zone(&mut candidate, dev, zone_name);
    z.effect = Some(effect.clone());
    z.off = false;
    candidate.validate_save()?;
    check_firmware_lighting(dev)?;
    write_and_commit(store, candidate, || dev.set_effect(led, &effect))
}

/// Persist source-profile lighting intent and explicitly reapply its brightness.
/// Even unchanged intent is written/read back, so callers can repair hardware drift.
/// Use set_lighting_policy for policy-only edits that must issue no HID request.
pub fn set_lighting_settings(
    dev: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
    want: &crate::lighting::LightingSettings,
) -> Result<crate::lighting::LightingSettings> {
    change_lighting_settings(
        dev,
        store,
        expected_device,
        expected_profile,
        want,
        LightingChange::Brightness,
    )
}

/// Persist only host-policy intent. A concurrent brightness edit is rejected,
/// rather than silently saving brightness that was never sent to the hardware.
pub fn set_lighting_policy(
    dev: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
    want: &crate::lighting::LightingSettings,
) -> Result<crate::lighting::LightingSettings> {
    change_lighting_settings(
        dev,
        store,
        expected_device,
        expected_profile,
        want,
        LightingChange::Policy,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum LightingChange {
    Brightness,
    Policy,
}

fn change_lighting_settings(
    dev: &Device,
    store: &mut Store,
    expected_device: &str,
    expected_profile: &str,
    want: &crate::lighting::LightingSettings,
    change: LightingChange,
) -> Result<crate::lighting::LightingSettings> {
    if expected_device != dev.key() {
        return Err(crate::Error::InvalidArg(
            "The selected device changed; refresh before changing Lighting settings".into(),
        ));
    }
    if dev.pid != 605 {
        return Err(crate::Error::Unsupported(
            "source lighting intent is verified only for Ornata V2".into(),
        ));
    }
    want.validate()?;
    let mut candidate = store.clone();
    let profiles = candidate.for_device(dev);
    if profiles.active != expected_profile {
        return Err(crate::Error::InvalidArg(
            "The active profile changed; refresh before changing Lighting settings".into(),
        ));
    }
    if profiles.lighting_off {
        return Err(crate::Error::InvalidArg(
            "lighting control is disabled for this device".into(),
        ));
    }
    if change == LightingChange::Policy
        && crate::lighting::LightingSettings::from_profile(profiles.active())?
            .and_then(|s| s.brightness)
            != want.brightness
    {
        return Err(crate::Error::InvalidArg(
            "A policy-only action cannot change brightness; use the brightness action".into(),
        ));
    }
    want.store_in_profile(profiles.active_mut())?;
    if change == LightingChange::Brightness {
        crate::profile_compat::clear_source_brightness_baseline(profiles.active_mut());
    }
    write_and_commit(store, candidate, || {
        if change == LightingChange::Brightness {
            want.apply_brightness(dev).map(|_| ())
        } else {
            Ok(())
        }
    })?;
    Ok(want.clone())
}

/// Brightness in percent, clamped to 0-100.
pub fn set_brightness(dev: &Device, store: &mut Store, zone_name: &str, percent: u8) -> Result<()> {
    let percent = percent.min(100);
    if dev.pid == 605 && dev.led_id(zone_name)? == 5 {
        // Read the source profile without migrating keys before the guarded action.
        // Ornata has one PID; its only upstream fallback is the bare legacy PID key.
        let profiles = store
            .devices
            .get(&dev.key())
            .or_else(|| store.devices.get("025D"));
        if let Some(profiles) = profiles {
            if let Some(mut settings) = profiles.active().lighting_settings.clone() {
                let active = profiles.active.clone();
                settings
                    .brightness
                    .get_or_insert_with(Default::default)
                    .set_value(percent)?;
                return set_lighting_settings(dev, store, &dev.key(), &active, &settings)
                    .map(|_| ());
            }
        }
    }
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    let led = dev.led_id(zone_name)?;
    zone(&mut candidate, dev, zone_name).brightness = Some(percent);
    write_and_commit(store, candidate, || dev.set_brightness(led, percent))
}

pub fn lights_off(dev: &Device, store: &mut Store, zone_name: &str) -> Result<()> {
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    let led = dev.led_id(zone_name)?;
    candidate.for_device(dev).active_mut().keys.clear();
    zone(&mut candidate, dev, zone_name).off = true;
    write_and_commit(store, candidate, || dev.lights_off(led))
}

/// Per-key colors (row-major). They replace any whole-zone color or off state;
/// brightness is kept.
pub fn set_keys(dev: &Device, store: &mut Store, keys: Vec<Rgb>) -> Result<()> {
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    let profile = candidate.for_device(dev).active_mut();
    for z in profile.lighting.values_mut() {
        z.color = None;
        z.effect = None;
        z.off = false;
    }
    profile.keys = keys.clone();
    write_and_commit(store, candidate, || dev.show_frame(&keys))
}

/// Light one matrix cell white and the rest off, so a user can say which key it is
/// (finding a board's layout, #45). Nothing is saved; apply the profile again afterwards.
pub fn show_light(dev: &Device, store: &mut Store, row: u8, col: u8) -> Result<()> {
    let mut candidate = store.clone();
    check_lighting(dev, &mut candidate)?;
    candidate.validate_save()?;
    let m = dev
        .matrix()
        .ok_or_else(|| crate::error::Error::Unsupported("no per-key lighting".into()))?;
    if row >= m.rows || col >= m.cols {
        return Err(crate::error::Error::InvalidArg(format!(
            "light {row},{col} outside {}x{}",
            m.rows, m.cols
        )));
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
    let mut candidate = store.clone();
    let profiles = candidate.for_device(dev);
    let target = match action {
        Some(action) => {
            profiles
                .active_mut()
                .buttons
                .insert(key.clone(), action.to_string());
            action
        }
        None => {
            let original = match profiles
                .onboard
                .as_ref()
                .and_then(|save| save.before.get(&key))
            {
                Some(original) => original.parse()?,
                None => dev.saved_input(input, layer)?,
            };
            profiles.active_mut().buttons.remove(&key);
            original
        }
    };
    write_and_commit(store, candidate, || {
        dev.set_input(input, layer, target).map(|_| ())
    })
}

/// Refuse a change that would leave the mouse with no left click on its normal layer: the
/// user could no longer click anything, including the button to undo it.
fn keeps_a_left_click(
    dev: &Device,
    id: u8,
    layer: crate::buttons::Layer,
    action: crate::buttons::Action,
) -> Result<()> {
    use crate::buttons::{Action, Layer};
    const LEFT: Action = Action::Mouse(1);
    if layer != Layer::Normal || action == LEFT || dev.button(id, Layer::Normal).ok() != Some(LEFT)
    {
        return Ok(());
    }
    if dev
        .buttons()
        .iter()
        .any(|&(other, a)| other != id && a == LEFT)
    {
        return Ok(());
    }
    Err(crate::error::Error::InvalidArg(
        "this is the only left click; give another button left click first, or you couldn't click anything".into(),
    ))
}

struct SlotOperation {
    slot: u8,
    input: crate::buttons::Input,
    layer: crate::buttons::Layer,
    action: crate::buttons::Action,
}
impl SlotOperation {
    fn apply(&self, dev: &Device) -> Result<()> {
        dev.set_slot_input(self.slot, self.input, self.layer, self.action)
    }
}
fn slot_operations(
    slot: u8,
    buttons: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<SlotOperation>> {
    buttons
        .iter()
        .map(|(key, action)| {
            let (input, layer) = crate::buttons::parse_setting_key(key)?;
            Ok(SlotOperation {
                slot,
                input,
                layer,
                action: action.parse()?,
            })
        })
        .collect()
}

/// Put every remapped button of the active profile back to what the mouse stores (Synapse's
/// or the factory setting), on both layers, and forget the remaps so they aren't applied
/// again. A "Save to mouse" is put back first, so this is a full way back. Returns how many
/// buttons were restored.
pub fn restore_all_buttons(dev: &Device, store: &mut Store) -> Result<usize> {
    let mut candidate = store.clone();
    let old = candidate.for_device(dev).onboard.clone();
    let old_operations = old
        .as_ref()
        .map(|save| slot_operations(save.slot, &save.before))
        .transpose()?
        .unwrap_or_default();
    let current_slot = dev.saved_slot();
    let keys: Vec<String> = candidate
        .for_device(dev)
        .active()
        .buttons
        .keys()
        .cloned()
        .collect();
    let mut reset = Vec::new();
    for key in keys {
        let (input, layer) = crate::buttons::parse_setting_key(&key)?;
        let expected_saved = old
            .as_ref()
            .filter(|save| save.slot == current_slot)
            .and_then(|save| save.before.get(&key));
        let target = match expected_saved {
            Some(value) => value.parse()?,
            None => dev.saved_input(input, layer)?,
        };
        reset.push((key, input, layer, target));
    }
    // First bound the resolved backup state, then the post-onboard phase. All
    // later partial states only remove button-map entries, without changing
    // optional metadata/active names/aliases; bytes and tokens cannot increase.
    candidate.validate_save()?;
    let recovery = candidate.clone();
    candidate.for_device(dev).onboard = None;
    candidate.validate_save()?;
    if old.is_some() {
        recovery.save()?;
        *store = recovery;
        for operation in &old_operations {
            operation.apply(dev)?;
        }
        candidate.save()?;
        *store = candidate.clone();
    }
    let mut restored = 0;
    let mut first_error = None;
    for (key, input, layer, target) in reset {
        match dev.set_input(input, layer, target) {
            Ok(_) => {
                candidate.for_device(dev).active_mut().buttons.remove(&key);
                restored += 1;
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    candidate.save()?;
    *store = candidate;
    match first_error {
        Some(error) if restored == 0 => Err(error),
        _ => Ok(restored),
    }
}

/// "Save to mouse": the active profile's button remaps go into the mouse's active on-board
/// slot, so they work without Secretarybird. What each button held there before is kept
/// first (once, so it stays Synapse's or the factory setting), and buttons an earlier save
/// wrote that this profile doesn't remap go back to it. Returns the slot written.
pub fn save_buttons_to_mouse(dev: &Device, store: &mut Store) -> Result<u8> {
    if dev.buttons().is_empty() {
        return Err(crate::Error::Unsupported(format!(
            "{} keeps no button settings on board",
            dev.name()
        )));
    }
    let slot = dev.saved_slot();
    let mut base = store.clone();
    let old = base.for_device(dev).onboard.clone();
    let old_recovery = base.clone();
    let old_restore = if let Some(old) = old.as_ref().filter(|old| old.slot != slot) {
        slot_operations(old.slot, &old.before)?
    } else {
        Vec::new()
    };
    if !old_restore.is_empty() || old.as_ref().is_some_and(|old| old.slot != slot) {
        base.for_device(dev).onboard = None;
    }
    let profiles = base.for_device(dev);
    let want = profiles.active().buttons.clone();
    let mut backup = profiles.onboard.clone().unwrap_or_default();
    for key in want.keys() {
        if !backup.before.contains_key(key) {
            let (input, layer) = crate::buttons::parse_setting_key(key)?;
            backup
                .before
                .insert(key.clone(), dev.slot_input(slot, input, layer)?.to_string());
        }
    }
    backup.slot = slot;
    let mut recovery = base.clone();
    recovery.for_device(dev).onboard = Some(backup.clone());
    let mut writes = std::collections::BTreeMap::new();
    for (key, original) in backup
        .before
        .iter()
        .filter(|(key, _)| !want.contains_key(*key))
    {
        writes.insert(key.clone(), original.clone());
    }
    writes.extend(want.clone());
    let writes = slot_operations(slot, &writes)?;
    let mut final_candidate = recovery.clone();
    backup.before.retain(|key, _| want.contains_key(key));
    backup.profile = final_candidate.for_device(dev).active.clone();
    backup.buttons = want;
    final_candidate.for_device(dev).onboard = Some(backup);
    // Every deterministic recovery/final state is bounded before old-slot SETs.
    old_recovery.validate_save()?;
    base.validate_save()?;
    recovery.validate_save()?;
    final_candidate.validate_save()?;
    if old.as_ref().is_some_and(|old| old.slot != slot) {
        old_recovery.save()?;
        *store = old_recovery;
        for operation in &old_restore {
            operation.apply(dev)?;
        }
        base.save()?;
        *store = base;
    }
    // Mandatory recovery phase: persist originals before any new-slot mutation.
    recovery.save()?;
    *store = recovery;
    for operation in &writes {
        operation.apply(dev)?;
    }
    final_candidate.save()?;
    *store = final_candidate;
    Ok(slot)
}

/// Undo "Save to mouse": the on-board slot gets back what it held before. The live remaps
/// stay as they are. Returns how many buttons were put back.
pub fn put_back_onboard(dev: &Device, store: &mut Store) -> Result<usize> {
    let mut candidate = store.clone();
    let Some(save) = candidate.for_device(dev).onboard.clone() else {
        return Ok(0);
    };
    let operations = slot_operations(save.slot, &save.before)?;
    let recovery = candidate.clone();
    candidate.for_device(dev).onboard = None;
    recovery.validate_save()?;
    candidate.validate_save()?;
    recovery.save()?;
    *store = recovery;
    for operation in &operations {
        operation.apply(dev)?;
    }
    candidate.save()?;
    *store = candidate;
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

mod sync_plan;

#[cfg(test)]
mod resource_tests;

/// The same static color on every lighting zone of every device, each recorded in
/// that device's active profile. Devices without lighting or with control disabled are skipped.
pub fn sync_color(devices: &[Device], store: &mut Store, color: Rgb) -> Vec<Synced> {
    sync_plan::run(devices, store, sync_plan::Change::Color(color))
}

/// The same built-in effect on every lighting zone of every device, started together so
/// they run in step (each device keeps its own timer, so they can drift apart over time;
/// syncing again lines them up). The zones are all sent first and the profiles saved
/// once after, to keep the start times close. Devices with lighting control disabled are skipped.
pub fn sync_effect(
    devices: &[Device],
    store: &mut Store,
    effect: &crate::effects::Effect,
) -> Vec<Synced> {
    sync_plan::run(devices, store, sync_plan::Change::Effect(effect.clone()))
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

#[cfg(test)]
mod ornata_firmware_guard_tests {
    use super::*;
    use crate::sim::Sim;
    use std::{ffi::CString, sync::atomic::{AtomicUsize, Ordering}};
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> (Sim, Device, Store, std::path::PathBuf) {
        let sim = Sim::keyboard();
        let dev = Device::open(
            Box::new(sim.clone()), 605, 2, CString::new("sim-mode-guard").unwrap(),
            &[0xff], Sim::spec_for(crate::sim::KEYBOARD_PID),
        ).unwrap();
        let path = std::env::temp_dir().join(format!(
            "synapse-firmware-guard-{}-{}", std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        )).join("profiles.json");
        let mut store = Store::load(&path).unwrap();
        store.for_device(&dev);
        sim.state().log.clear();
        (sim, dev, store, path)
    }

    #[test]
    fn driver_mode_and_unreadable_mode_refuse_legacy_single_and_sync_without_sets_or_save() {
        for mode in [Some(3), Some(2), None] {
            let (sim, dev, mut store, path) = fixture();
            sim.state().device_mode = mode;
            let before = serde_json::to_value(&store).unwrap();
            assert!(set_color(&dev, &mut store, "backlight", Rgb(1, 2, 3)).is_err());
            assert!(set_effect(&dev, &mut store, "backlight", "spectrum".parse().unwrap()).is_err());
            let devices = [dev];
            assert!(sync_color(&devices, &mut store, Rgb(3, 2, 1)).iter().all(|r| r.result.is_err()));
            assert!(sync_effect(&devices, &mut store, &"spectrum".parse().unwrap()).iter().all(|r| r.result.is_err()));
            assert_eq!(serde_json::to_value(&store).unwrap(), before);
            assert!(!path.exists());
            assert!(sim.state().log.iter().all(|c| *c == crate::proto::Command::new(0, 0x84)));
        }
    }

    #[test]
    fn normal_mode_keeps_firmware_projection_and_disabled_control_precedes_mode_query() {
        let (sim, dev, mut store, path) = fixture();
        store.for_device(&dev).lighting_off = true;
        assert!(set_color(&dev, &mut store, "backlight", Rgb(1, 2, 3)).is_err());
        assert!(sim.state().log.is_empty());
        store.for_device(&dev).lighting_off = false;
        set_color(&dev, &mut store, "backlight", Rgb(1, 2, 3)).unwrap();
        set_effect(&dev, &mut store, "backlight", "spectrum".parse().unwrap()).unwrap();
        assert!(sim.state().log.contains(&crate::commands::MATRIX_EFFECT));
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }
}

#[cfg(test)]
mod lighting_intent_tests {
    use super::*;
    use crate::{
        lighting::{Brightness, LightingSettings, SwitchOffLighting},
        sim::Sim,
    };
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    fn setup() -> (Sim, Device, Store, std::path::PathBuf) {
        let sim = Sim::keyboard();
        let dev = sim
            .open(605, Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        let folder = std::env::temp_dir().join(format!(
            "synapse-lighting-actions-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut store = Store::load(&folder.join("profiles.json")).unwrap();
        *store.for_device(&dev).active_mut() =
            crate::profile_compat::new_native_profile(605, "Default").unwrap();
        (sim, dev, store, folder)
    }
    #[test]
    fn off_on_retains_value_effect_and_policy_and_policy_only_edit_has_no_hid_write() {
        let (sim, dev, mut store, folder) = setup();
        let mut want = LightingSettings::from_profile(store.for_device(&dev).active())
            .unwrap()
            .unwrap();
        want.brightness.as_mut().unwrap().set_value(73).unwrap();
        want.brightness
            .as_mut()
            .unwrap()
            .extra
            .insert("future".into(), serde_json::json!(42));
        want.brightness.as_mut().unwrap().set_enabled(false);
        want.switch_off.as_mut().unwrap().idle_minutes = 7;
        set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        assert_eq!(dev.brightness(5).unwrap(), 0);
        assert!(!store.for_device(&dev).active().lighting["backlight"].off);
        assert_eq!(
            store.for_device(&dev).active().lighting["backlight"].effect,
            Some(crate::effects::Effect::Spectrum)
        );
        assert!(!sim.state().log.contains(&crate::commands::MATRIX_EFFECT));
        want.brightness.as_mut().unwrap().set_enabled(true);
        set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        assert_eq!(dev.brightness(5).unwrap(), 73);
        let before = sim.state().log.len();
        want.switch_off.as_mut().unwrap().display_off = true;
        set_lighting_policy(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        assert_eq!(
            sim.state().log.len(),
            before,
            "host policy editing must not send an unrelated HID timer/effect"
        );
        let mut disk = Store::load(&folder.join("profiles.json")).unwrap();
        assert_eq!(
            disk.for_device(&dev).active().lighting_settings,
            Some(want.clone())
        );
        set_brightness(&dev, &mut store, "backlight", 20).unwrap();
        let settings = store
            .for_device(&dev)
            .active()
            .lighting_settings
            .as_ref()
            .unwrap();
        assert_eq!(settings.brightness.as_ref().unwrap().value, 20);
        assert_eq!(settings.brightness.as_ref().unwrap().extra["future"], 42);
        assert!(settings.switch_off.as_ref().unwrap().display_off);
        let outcomes = store.for_device(&dev).active().apply(&dev);
        assert!(outcomes
            .iter()
            .any(|r| r.setting.contains("idle/display") && r.result.is_err()));
        assert!(!sim.state().log.contains(&crate::commands::IDLE_TIME));
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn invalid_identity_profile_ownership_values_and_budget_refuse_before_hardware() {
        let (sim, dev, mut store, folder) = setup();
        let mut want = LightingSettings {
            brightness: Some(Brightness::default()),
            switch_off: Some(SwitchOffLighting::default()),
            extra: BTreeMap::new(),
        };
        let before = sim.state().log.len();
        assert!(set_lighting_settings(&dev, &mut store, "stale-device", "Default", &want).is_err());
        assert!(
            set_lighting_settings(&dev, &mut store, &dev.key(), "stale-profile", &want).is_err()
        );
        store.for_device(&dev).lighting_off = true;
        assert!(set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).is_err());
        store.for_device(&dev).lighting_off = false;
        want.brightness.as_mut().unwrap().value = 101;
        assert!(set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).is_err());
        want.brightness.as_mut().unwrap().value = 50;
        want.brightness.as_mut().unwrap().extra.insert(
            "huge".into(),
            serde_json::json!("x".repeat(2 * 1024 * 1024)),
        );
        let native_before = serde_json::to_value(&store).unwrap();
        assert!(set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), native_before);
        assert_eq!(sim.state().log.len(), before);
        assert!(!folder.exists());
    }
    #[test]
    fn explicit_same_brightness_repairs_hardware_drift_but_policy_edit_sends_nothing() {
        let (sim, dev, mut store, folder) = setup();
        let mut want = LightingSettings::from_profile(store.for_device(&dev).active())
            .unwrap()
            .unwrap();
        want.brightness.as_mut().unwrap().set_value(73).unwrap();
        set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        dev.set_brightness(5, 30).unwrap();
        assert_eq!(dev.brightness(5).unwrap(), 30);
        let before = sim.state().log.len();
        set_brightness(&dev, &mut store, "backlight", 73).unwrap();
        assert!(sim.state().log[before..].contains(&crate::commands::MATRIX_BRIGHTNESS));
        assert_eq!(dev.brightness(5).unwrap(), 73);
        dev.set_brightness(5, 30).unwrap();
        want.switch_off.as_mut().unwrap().display_off = true;
        let before = sim.state().log.len();
        set_lighting_policy(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        assert_eq!(sim.state().log.len(), before);
        assert_eq!(
            dev.brightness(5).unwrap(),
            30,
            "policy persistence must not covertly resync brightness"
        );
        let mut changed_brightness = want.clone();
        changed_brightness
            .brightness
            .as_mut()
            .unwrap()
            .set_value(74)
            .unwrap();
        let native_before = serde_json::to_value(&store).unwrap();
        let before = sim.state().log.len();
        assert!(
            set_lighting_policy(&dev, &mut store, &dev.key(), "Default", &changed_brightness)
                .is_err()
        );
        assert_eq!(sim.state().log.len(), before);
        assert_eq!(serde_json::to_value(&store).unwrap(), native_before);
        set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).unwrap();
        assert_eq!(
            dev.brightness(5).unwrap(),
            73,
            "full explicit brightness action must also resync an unchanged desired value"
        );
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn all_preflight_refusals_restore_legacy_key_resolution_and_default_creation() {
        // Ornata has no alternate receiver PID. Its relevant connection identity
        // migration is the upstream bare-PID key becoming a serial-qualified key.
        for policy_only in [false, true] {
            for refusal in ["profile", "ownership", "budget"] {
                let (sim, dev, mut store, folder) = setup();
                let profiles = store.devices.remove(&dev.key()).unwrap();
                store.devices.insert("025D".into(), profiles);
                let mut want = LightingSettings::from_profile(store.devices["025D"].active())
                    .unwrap()
                    .unwrap();
                let expected = if refusal == "profile" {
                    "stale-profile"
                } else {
                    "Default"
                };
                if refusal == "ownership" {
                    store.devices.get_mut("025D").unwrap().lighting_off = true;
                }
                if refusal == "budget" {
                    want.extra.insert(
                        "huge".into(),
                        serde_json::json!("x".repeat(2 * 1024 * 1024)),
                    );
                }
                let native_before = serde_json::to_value(&store).unwrap();
                let before = sim.state().log.len();
                let result = if policy_only {
                    set_lighting_policy(&dev, &mut store, &dev.key(), expected, &want)
                } else {
                    set_lighting_settings(&dev, &mut store, &dev.key(), expected, &want)
                };
                assert!(result.is_err());
                assert_eq!(
                    serde_json::to_value(&store).unwrap(),
                    native_before,
                    "refusal {refusal} must not migrate device keys"
                );
                assert_eq!(sim.state().log.len(), before);
                assert!(!folder.exists());
                if refusal == "ownership" {
                    assert!(set_brightness(&dev, &mut store, "backlight", 73).is_err());
                    assert_eq!(serde_json::to_value(&store).unwrap(),native_before,"legacy explicit brightness wrapper must not migrate a refused typed profile");
                    assert_eq!(sim.state().log.len(), before);
                }
            }
        }
        let (sim, dev, mut store, folder) = setup();
        store.devices.clear();
        let want = LightingSettings {
            brightness: Some(Brightness::default()),
            ..Default::default()
        };
        let native_before = serde_json::to_value(&store).unwrap();
        let before = sim.state().log.len();
        assert!(
            set_lighting_settings(&dev, &mut store, &dev.key(), "stale-profile", &want).is_err()
        );
        assert_eq!(
            serde_json::to_value(&store).unwrap(),
            native_before,
            "stale request must not create a default device entry"
        );
        assert_eq!(sim.state().log.len(), before);
        assert!(!folder.exists());
    }

    #[test]
    fn unsupported_register_does_not_commit_profile_or_create_backup() {
        let (sim, dev, mut store, folder) = setup();
        sim.state().leds.clear();
        let mut want = LightingSettings::from_profile(store.for_device(&dev).active())
            .unwrap()
            .unwrap();
        want.brightness.as_mut().unwrap().set_value(37).unwrap();
        let before = serde_json::to_value(&store).unwrap();
        assert!(set_lighting_settings(&dev, &mut store, &dev.key(), "Default", &want).is_err());
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
        assert!(!folder.exists());
    }

    #[test]
    fn legacy_profile_application_and_serialization_remain_unchanged() {
        let sim = Sim::keyboard();
        let dev = sim
            .open(605, Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        let mut profile = crate::Profile::default();
        profile
            .lighting
            .entry("backlight".into())
            .or_default()
            .brightness = Some(37);
        assert!(serde_json::to_value(&profile)
            .unwrap()
            .get("lighting_settings")
            .is_none());
        assert!(profile.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert_eq!(dev.brightness(5).unwrap(), 37);
        profile.lighting.get_mut("backlight").unwrap().off = true;
        assert!(profile.apply(&dev).iter().all(|r| r.result.is_ok()));
        assert!(sim.state().log.contains(&crate::commands::MATRIX_EFFECT));
    }
}
