//! Lossless profile interchange. Foreign data is retained before translating settings.
//!
//! Synapse 4 is a JSON envelope containing base64 JSON records and a stable-JSON MD5.
//! This codec does not execute exports, query accounts, or modify Synapse's databases.

use crate::effects::Effect;
use crate::error::{Error, Result};
use crate::features::Rgb;
use crate::profile::{DeviceProfiles, Profile, Store};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub mod gui;
pub mod macro_library;
mod dependencies;
pub use dependencies::{recover_macro_dependencies,MacroRecoveryReport};
mod mappings;
pub mod mouse_assignment;
mod settings;

pub(crate) fn source_key_usage(id: &str) -> Option<u8> {
    mappings::key_usage(id)
}

/// Retained source intent also exists in native backups saved before typed
/// Snap Tap support. Reading it must not rewrite the user's profile store.
pub fn source_snap_tap(profile: &Profile) -> Option<&Value> {
    profile.extra.get(SOURCE)?.get("decoded")?.get("snapTap")
}

/// Retained host Gaming Mode settings, including conditional settings that have
/// no legacy game_mode projection. This reads without migrating saved bytes.
pub fn source_gaming_mode(profile: &Profile) -> Option<&Value> {
    profile.extra.get(SOURCE)?.get("decoded")?.get("gamingMode")
}

#[cfg(test)]
mod expanded_tests;

const SOURCE: &str = "_synapseLiteSynapse4";

/// Retained source settings for an editor, not a live firmware/engine read.
pub fn source_quick_effects(profile: &Profile) -> Option<&Value> {
    profile
        .extra
        .get(SOURCE)?
        .get("decoded")?
        .get("quickEffects")
        .filter(|value| value.is_object())
}

/// Native provenance for a first editor baseline; this is outside the foreign
/// decoded payload and must never claim the stock profile's retained intent.
pub fn source_brightness_baseline(profile: &Profile) -> Option<&Value> {
    profile.extra.get(SOURCE)?.get("brightnessBaseline")
}
pub fn record_ornata_brightness_baseline(
    profile: &mut Profile,
    saved_raw: u8,
    live_raw: u8,
) -> Result<()> {
    let metadata = profile
        .extra
        .get_mut(SOURCE)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("source lighting metadata is missing"))?;
    metadata.insert("brightnessBaseline".into(),json!({"origin":"hardware-register-baseline","saved_raw":saved_raw,"live_raw":live_raw,"retained_intent_known":false}));
    Ok(())
}
pub fn clear_source_brightness_baseline(profile: &mut Profile) {
    if let Some(metadata) = profile.extra.get_mut(SOURCE).and_then(Value::as_object_mut) {
        metadata.remove("brightnessBaseline");
    }
}

fn source_quick_effects_mut<'a>(
    profile: &'a mut Profile,
    pid: u16,
    name: &str,
) -> Result<&'a mut Map<String, Value>> {
    if pid != 605 {
        return Err(invalid(
            "source quick-effect editing is verified only for Ornata V2",
        ));
    }
    if !profile.extra.contains_key(SOURCE) {
        let mut source = new_native_profile(pid, name)?;
        profile.extra.insert(
            SOURCE.into(),
            source
                .extra
                .remove(SOURCE)
                .expect("source default has metadata"),
        );
    }
    let decoded = profile
        .extra
        .get_mut(SOURCE)
        .and_then(|value| value.get_mut("decoded"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("source profile metadata cannot be edited"))?;
    decoded
        .entry("quickEffects")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| invalid("source quick-effects metadata cannot be edited"))
}

/// Update the profile's selected source effect while preserving palette/opaque
/// members. Callers must validate/implement the effect before committing a store.
pub fn set_source_quick_effect(
    profile: &mut Profile,
    pid: u16,
    name: &str,
    source_id: u8,
    setting: &Value,
) -> Result<()> {
    if !matches!(source_id, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 11 | 12 | 13 | 19)
        || !setting.is_object()
    {
        return Err(invalid("source quick-effect choice or setting is invalid"));
    }
    let effects = source_quick_effects_mut(profile, pid, name)?;
    let mut merged = if effects.get("selectedEffectId") == Some(&json!(source_id)) {
        effects.get("selectedEffectSetting").filter(|v| v.is_object()).cloned()
            .unwrap_or_else(||json!({}))
    } else {
        json!({})
    };
    merge_object(&mut merged, setting.clone())?;
    effects.insert("selectedEffectId".into(), json!(source_id));
    effects.insert("selectedEffectSetting".into(), merged);
    Ok(())
}

/// Source palette persistence replaces the complete16-slot cache, filtering its
/// '#111' empty sentinel. Unknown members of filled slots remain intact.
pub fn set_source_custom_colors(
    profile: &mut Profile,
    pid: u16,
    name: &str,
    slots: &[Value],
) -> Result<()> {
    if pid != 605 || slots.len() != 16 {
        return Err(invalid(
            "Ornata custom colors require the complete 16-slot palette",
        ));
    }
    let retained = source_quick_effects(profile)
        .and_then(|effects| effects.get("customColors"))
        .and_then(Value::as_array);
    let mut filled = Vec::new();
    for slot in slots {
        // Imported future formats are opaque. Permit retaining an exact value,
        // while still validating every newly authored slot before mutation.
        if retained.is_some_and(|values| values.contains(slot)) {
            filled.push(slot.clone());
            continue;
        }
        let object = slot
            .as_object()
            .ok_or_else(|| invalid("custom color slot must be an object"))?;
        let color = object
            .get("color")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("custom color needs a color string"))?;
        if !object.get("picked").is_some_and(Value::is_boolean) {
            return Err(invalid("custom color needs a picked flag"));
        }
        if color == "#111" {
            continue;
        }
        if color.len() != 7 || !color.starts_with('#') || color[1..].parse::<Rgb>().is_err() {
            return Err(invalid("custom color must be a six-digit RGB hex value; malformed source drafts cannot be saved"));
        }
        filled.push(slot.clone());
    }
    let effects = source_quick_effects_mut(profile, pid, name)?;
    effects.insert("customColors".into(), Value::Array(filled));
    Ok(())
}
pub(crate) const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 1024 * 1024;
const MAX_JSON_TOKENS: usize = 32_768;

#[derive(Debug)]
pub struct ImportReport {
    pub store: Store,
    /// Settings retained in the document but not translated into device commands.
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ExportReport {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

/// Native Secretarybird JSON or a verified Synapse 4 export. ZIP/Synapse 3 fails explicitly.
pub fn import_document(bytes: &[u8]) -> Result<ImportReport> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid("Profile document exceeds 2 MiB"));
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if bytes.starts_with(b"PK") {
        return Err(Error::Unsupported(
            "Synapse 3 ZIP migration is not implemented; original archive was not modified".into(),
        ));
    }
    check_json_complexity(bytes)?;
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| invalid(format!("Invalid profile JSON: {e}")))?;
    if value.get("devices").is_some() {
        let mut store: Store = serde_json::from_value(value)
            .map_err(|e| invalid(format!("Invalid Secretarybird profiles: {e}")))?;
        store.rename_old_buttons();
        validate_store_budget(&store)?;
        return Ok(ImportReport {
            store,
            warnings: Vec::new(),
        });
    }
    import_synapse4(value)
}

fn import_synapse4(envelope: Value) -> Result<ImportReport> {
    let pid = envelope
        .get("productId")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("Synapse export has no numeric productId"))?;
    let category = match pid {
        196 | 197 => "MOUSE",
        605 => "KEYBOARD",
        _ => return Err(Error::Unsupported(format!("Synapse translation has only been verified for product IDs 196, 197 and 605, received {pid}"))),
    };
    if envelope.get("category").and_then(Value::as_str) != Some(category) {
        return Err(invalid("Synapse productId and category do not match"));
    }
    let records = envelope
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Synapse export has no profiles array"))?;
    if records.is_empty() {
        return Err(invalid("Synapse export has no profiles"));
    }
    let encoded_bytes: usize = records
        .iter()
        .chain(
            envelope
                .get("macros")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .filter_map(|entry| entry.get("payload").and_then(Value::as_str))
        .map(str::len)
        .sum();
    if encoded_bytes > MAX_DECODED_BYTES * 4 / 3 {
        return Err(invalid(
            "Combined decoded Synapse payloads exceed the 1 MiB memory budget",
        ));
    }
    let mut warnings = Vec::new();
    if let Some(macros) = envelope.get("macros") {
        for entry in macros
            .as_array()
            .ok_or_else(|| invalid("Synapse macros must be an array"))?
        {
            decode_record(entry)?;
        }
        if macros.as_array().is_some_and(|items| !items.is_empty()) {
            warnings.push(
                "Macros are preserved for Synapse re-export; their execution is not translated"
                    .into(),
            );
        }
    }
    let mut device = DeviceProfiles::default();
    device.profiles.clear();
    device.extra.insert(SOURCE.into(), envelope.clone());
    for record in records {
        let decoded = decode_record(record)?;
        let name = record
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Synapse profile entry has no name"))?;
        if name.trim().is_empty() {
            return Err(invalid("Synapse profile name is empty"));
        }
        if device.profiles.contains_key(name) {
            return Err(invalid(format!(
                "Duplicate Synapse profile name '{name}'; import would be ambiguous"
            )));
        }
        let mut profile = translate_profile(&decoded, pid as u16, name, &mut warnings)?;
        profile.extra.insert(
            SOURCE.into(),
            preserved_source(pid as u16, record.clone(), decoded),
        );
        device.profiles.insert(name.to_string(), profile);
    }
    device.active = device
        .profiles
        .keys()
        .next()
        .expect("nonempty checked")
        .clone();
    let mut store = Store::default();
    store.devices.insert(format!("{pid:04X}"), device);
    validate_store_budget(&store)?;
    Ok(ImportReport { store, warnings })
}

/// Check the complete post-merge store before committing an import or profile edit.
pub fn validate_store_budget(store: &Store) -> Result<()> {
    store.validate_save().map_err(|error| invalid(format!(
        "Lossless native profile backup refused without discarding fields: {error}"
    )))
}

/// A copied native profile is a new Synapse identity even when it retains the
/// original foreign mappings and metadata. Renaming alone should not call this.
pub fn renew_profile_identity(profile: &mut Profile) -> Result<()> {
    if let Some(source) = profile.extra.get_mut(SOURCE) {
        let decoded = source
            .get_mut("decoded")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("Preserved Synapse profile has no decoded object"))?;
        decoded.insert("guid".into(), json!(settings::new_guid()?));
    }
    Ok(())
}

/// Create an installed model's observed Synapse default profile. The caller owns
/// selecting a unique visible name, inserting it, and making it active.
pub fn new_native_profile(product_id: u16, name: &str) -> Result<Profile> {
    if name.trim().is_empty() {
        return Err(invalid("Profile name is empty"));
    }
    let decoded = settings::default_profile(product_id, name)?;
    let mut warnings = Vec::new();
    let mut profile = translate_profile(&decoded, product_id, name, &mut warnings)?;
    let entry = json!({
        "name":name, "hash":synapse_hash(&decoded)?,
        "payload":STANDARD.encode(serde_json::to_vec(&decoded).map_err(|e|invalid(e.to_string()))?)
    });
    profile
        .extra
        .insert(SOURCE.into(), preserved_source(product_id, entry, decoded));
    Ok(profile)
}

/// Provenance for newly translated source profiles only. Native JSON loading must
/// never infer this policy from PID or retroactively reinterpret old sleep values.
fn preserved_source(pid: u16, entry: Value, decoded: Value) -> Value {
    let mut metadata = json!({"entry":entry,"decoded":decoded});
    if matches!(pid, 196 | 197) {
        metadata["deviceSleepScope"] = json!(true);
    }
    metadata
}

fn translate_profile(
    source: &Value,
    pid: u16,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<Profile> {
    let object = source
        .as_object()
        .ok_or_else(|| invalid("Decoded Synapse profile must be an object"))?;
    let mut profile = Profile::default();
    if let Some(dpi) = source.get("dpi") {
        let x = integer(dpi.get("x"), 100, 26_000, "DPI x")? as u16;
        let y = integer(dpi.get("y"), 100, 26_000, "DPI y")? as u16;
        profile.dpi = Some((x, y));
    }
    if let Some(polling) = source.get("pollingRate") {
        let hz = integer(Some(polling), 125, 8000, "pollingRate")? as u32;
        if ![125, 500, 1000, 2000, 4000, 8000].contains(&hz) {
            return Err(invalid("Unsupported Synapse pollingRate"));
        }
        profile.polling = Some(hz);
    }
    if matches!(pid, 196 | 197) {
        settings::import_mouse(source, &mut profile, name, warnings)?;
    }
    mappings::import(source, &mut profile, pid, name, warnings)?;
    if pid == 605 {
        settings::import_snap_tap(source, &mut profile, name, warnings);
        settings::import_game_mode(source, &mut profile, name, warnings)?;
        settings::import_keyboard_lighting(source, &mut profile, name, warnings)?;
        if let Some(quick) = source.get("quickEffects") {
            let zone = profile.lighting.entry("backlight".into()).or_default();
            let id = integer(quick.get("selectedEffectId"), 0, 255, "selectedEffectId")?;
            let setting = quick
                .get("selectedEffectSetting")
                .cloned()
                .unwrap_or(json!({}));
            match id {
                0 => zone.off = true,
                1 => zone.color = Some(color(&setting, "color1")?),
                2 => zone.effect = Some(Effect::Breathing(colors(&setting)?)),
                3 => zone.effect = Some(Effect::Spectrum),
                4 => {
                    let direction = integer(setting.get("direction"), 1, 2, "wave direction")?;
                    zone.effect = Some(Effect::Wave {
                        reverse: direction == 2,
                        speed: 3,
                    });
                }
                5 => {
                    zone.effect = Some(Effect::Reactive {
                        color: color(&setting, "color1")?,
                        duration: integer(setting.get("duration"), 1, 3, "reactive duration")?
                            as u8,
                    })
                }
                7 => {
                    zone.effect = Some(Effect::Starlight {
                        colors: colors(&setting)?,
                        duration: integer(setting.get("duration"), 1, 3, "starlight duration")?
                            as u8,
                    })
                }
                _ => warnings.push(format!(
                    "{name}: lighting effect {id} is preserved but cannot be applied"
                )),
            }
        }
    }
    for (key, value) in object {
        if [
            "guid",
            "name",
            "hash",
            "gamemode",
            "dpi",
            "pollingRate",
            "mappings",
        ]
        .contains(&key.as_str())
            || (matches!(pid, 196 | 197)
                && ["dpiStages", "powerSavingValue"].contains(&key.as_str()))
            || (matches!(pid, 196 | 197)
                && ((key == "rotation" && profile.rotation.is_some())
                    || (key == "dynamicSensitivity" && profile.dynamic_sensitivity.is_some())))
            || (pid == 605
                && key == "snapTap")
            || (pid == 605
                && [
                    "brightness",
                    "quickEffects",
                    "gamingMode",
                    "switchOffLighting",
                ]
                .contains(&key.as_str()))
        {
            continue;
        }
        if !is_empty(value) {
            warnings.push(format!(
                "{name}: '{key}' is preserved but is not translated into device settings"
            ));
        }
    }
    Ok(profile)
}

fn is_empty(value: &Value) -> bool {
    value.is_null()
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_object().is_some_and(Map::is_empty)
}

fn colors(setting: &Value) -> Result<Vec<Rgb>> {
    if setting.get("isRandom").and_then(Value::as_bool) == Some(true) {
        return Ok(Vec::new());
    }
    let mut colors = vec![color(setting, "color1")?];
    if setting.get("color2").and_then(Value::as_str) != Some("no-color") {
        colors.push(color(setting, "color2")?);
    }
    Ok(colors)
}

fn color(setting: &Value, key: &str) -> Result<Rgb> {
    setting
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("Missing lighting {key}")))?
        .parse()
}

fn integer(value: Option<&Value>, min: u64, max: u64, label: &str) -> Result<u64> {
    let number = value
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{label} must be an integer")))?;
    if !(min..=max).contains(&number) {
        return Err(invalid(format!("{label} must be between {min} and {max}")));
    }
    Ok(number)
}

/// Re-export imported Synapse profiles, updating only verified settings and retaining
/// mappings, macros, GUIDs and foreign fields. New native profiles use the observed
/// default schema for the model, and receive an operating-system-generated UUID.
pub fn export_synapse4(store: &Store, device_key: &str) -> Result<ExportReport> {
    let device = store
        .devices
        .get(device_key)
        .ok_or_else(|| invalid("Device has no stored profiles"))?;
    if device
        .extra
        .keys()
        .any(|key| key.starts_with(&format!("_synapseLiteImported:{SOURCE}")))
    {
        return Err(Error::Unsupported("This device combines multiple Synapse source exports; merging foreign macro identities is not verified. Export a native JSON backup to retain every source document".into()));
    }
    if device.extra.keys().any(|key| key != SOURCE) {
        return Err(Error::Unsupported("Unknown native device extension fields cannot be translated to Synapse; retain a native JSON backup".into()));
    }
    let mut envelope = match device.extra.get(SOURCE) {
        Some(source) => source.clone(),
        None => {
            let pid =
                u16::from_str_radix(device_key.split(':').next().unwrap_or(device_key), 16)
                    .map_err(|_| invalid("Device key must start with a hexadecimal product ID"))?;
            settings::default_envelope(pid)?
        }
    };
    let pid = integer(envelope.get("productId"), 0, u16::MAX as u64, "productId")? as u16;
    settings::default_envelope(pid)?;
    let mut records = Vec::new();
    let mut warnings = Vec::new();
    if device.power.is_some() {
        warnings.push("Device-wide Power settings are retained only in the native JSON backup; the Synapse profile format has no verified device-global Power field".into());
    }
    if device.sensor_calibration.is_some() {
        warnings.push("Device-wide Smart Tracking settings are retained in the native JSON backup; Synapse stores Calibration outside named profiles".into());
    }
    let mut exported_guids = std::collections::BTreeSet::new();
    let mut guid_owners: BTreeMap<String, String> = BTreeMap::new();
    for (name, profile) in &device.profiles {
        if let Some(decoded) = profile
            .extra
            .get(SOURCE)
            .and_then(|source| source.get("decoded"))
        {
            if let Some(guid) = decoded.get("guid").and_then(Value::as_str) {
                if !guid_owners.contains_key(guid)
                    || decoded.get("name").and_then(Value::as_str) == Some(name.as_str())
                {
                    guid_owners.insert(guid.to_string(), name.clone());
                }
            }
        }
    }
    for (name, profile) in &device.profiles {
        if profile.extra.keys().any(|key| key != SOURCE)
            || profile
                .dpi_stages
                .as_ref()
                .is_some_and(|stages| !stages.extra.is_empty())
            || profile.lighting.values().any(|zone| !zone.extra.is_empty())
        {
            return Err(Error::Unsupported(format!("{name}: unknown native extension fields cannot be translated to Synapse; export a native JSON backup to retain them")));
        }
        let source = profile.extra.get(SOURCE);
        let mut decoded = match source {
            Some(source) => source
                .get("decoded")
                .cloned()
                .ok_or_else(|| invalid("Missing preserved Synapse payload"))?,
            None => settings::default_profile(pid, name)?,
        };
        let decoded_object = decoded
            .as_object_mut()
            .ok_or_else(|| invalid("Preserved Synapse payload is not an object"))?;
        decoded_object.insert("name".into(), json!(name));
        if source.is_some() || profile.dpi.is_some() {
            sync_optional(
                decoded_object,
                "dpi",
                profile.dpi.map(|(x, y)| json!({"x": x, "y": y})),
            );
        }
        if source.is_some() || profile.polling.is_some() {
            sync_optional(
                decoded_object,
                "pollingRate",
                profile.polling.map(|hz| json!(hz)),
            );
        }
        if matches!(pid, 196 | 197) {
            settings::export_mouse(profile, decoded_object, source.is_none())?;
            if !profile.lighting.is_empty()
                || profile.lighting_settings.is_some()
                || !profile.keys.is_empty()
                || profile.game_mode.is_some()
                || profile.snap_tap.is_some()
            {
                return Err(Error::Unsupported(format!("{name}: mouse lighting, keyboard colors or gaming mode have no matching Synapse mouse fields")));
            }
        }
        mappings::export(profile, decoded_object, pid)?;
        mouse_assignment::export_pending(profile, decoded_object, pid)?;
        if pid == 605 {
            settings::export_snap_tap(profile, decoded_object)?;
            settings::export_game_mode(profile, decoded_object)?;
            let source_lighting =
                settings::export_keyboard_lighting(profile, decoded_object, source.is_none())?;
            if profile.dpi.is_some()
                || profile.polling.is_some()
                || profile.dpi_stages.is_some()
                || profile.performance.is_some()
                || profile.sleep_minutes.is_some()
                || profile.dynamic_sensitivity.is_some()
                || profile.rotation.is_some()
            {
                return Err(Error::Unsupported(format!(
                    "{name}: mouse performance settings cannot be exported for Ornata V2"
                )));
            }
            if let Some(zone) = profile.lighting.get("backlight") {
                if !source_lighting
                    && (zone.brightness.is_some()
                        || zone.off
                        || decoded_object.get("brightness").is_some())
                {
                    let brightness = decoded_object.entry("brightness").or_insert(json!({}));
                    let object = brightness
                        .as_object_mut()
                        .ok_or_else(|| invalid("brightness must be an object"))?;
                    if let Some(percent) = zone.brightness {
                        if percent > 100 {
                            return Err(invalid("Brightness exceeds 100 percent"));
                        }
                        object.insert("value".into(), json!(percent));
                    } else if !object.contains_key("value") {
                        object.insert("value".into(), json!(100));
                    }
                    object.insert("isEnabled".into(), json!(!zone.off));
                }
                let quick = decoded_object.entry("quickEffects").or_insert(json!({}));
                let quick_object = quick
                    .as_object_mut()
                    .ok_or_else(|| invalid("quickEffects must be an object"))?;
                if source_lighting && zone.off {
                    // Source brightness-off is a zero brightness projection;
                    // Zone.off is reserved for an explicitly disabled effect.
                    quick_object.insert("selectedEffectId".into(), json!(0));
                } else if let Some((id, settings)) =
                    export_effect(zone.color, zone.effect.as_ref())?
                {
                    quick_object.insert("selectedEffectId".into(), json!(id));
                    if let Some(settings) = settings {
                        let original = quick_object
                            .entry("selectedEffectSetting")
                            .or_insert(json!({}));
                        merge_object(original, settings)?;
                    }
                }
            } else if source.is_some() {
                if !source_lighting {
                    decoded_object.remove("brightness");
                }
                decoded_object.remove("quickEffects");
            }
            if profile.lighting.keys().any(|key| key != "backlight") || !profile.keys.is_empty() {
                return Err(Error::Unsupported(format!(
                    "{name}: per-key or additional-zone Synapse export is not verified"
                )));
            }
        }
        for (label, present) in [
            ("applications", !profile.apps.is_empty() || !profile.app_paths.is_empty()),
            ("auto-sleep enable switch", profile.auto_sleep.is_some()),
            ("dimming", profile.dimming.is_some()),
        ] {
            if present {
                return Err(Error::Unsupported(format!(
                    "{name}: translating native {label} to Synapse is not verified; export refused"
                )));
            }
        }
        let guid = decoded_object
            .get("guid")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Synapse profile has no GUID"))?
            .to_string();
        let belongs_to_another_profile = guid_owners.get(&guid).is_some_and(|owner| owner != name);
        if belongs_to_another_profile || !exported_guids.insert(guid) {
            let replacement = settings::new_guid()?;
            exported_guids.insert(replacement.clone());
            decoded_object.insert("guid".into(), json!(replacement));
        }
        let mut entry = match source {
            Some(source) => source
                .get("entry")
                .cloned()
                .ok_or_else(|| invalid("Missing preserved Synapse envelope entry"))?,
            None => json!({}),
        };
        let object = entry
            .as_object_mut()
            .ok_or_else(|| invalid("Synapse entry is not an object"))?;
        object.insert("name".into(), json!(name));
        object.insert("hash".into(), json!(synapse_hash(&decoded)?));
        object.insert(
            "payload".into(),
            json!(
                STANDARD.encode(serde_json::to_vec(&decoded).map_err(|e| invalid(e.to_string()))?)
            ),
        );
        records.push(entry);
        for key in ["macro", "switchOffLighting"] {
            if decoded.get(key).is_some_and(|v| !is_empty(v))
                && (key != "switchOffLighting" || profile.lighting_settings.is_none())
            {
                warnings.push(format!(
                    "{name}: original Synapse '{key}' retained without translation"
                ));
            }
        }
    }
    envelope
        .as_object_mut()
        .ok_or_else(|| invalid("Synapse envelope is not an object"))?
        .insert("profiles".into(), Value::Array(records));
    let bytes = serde_json::to_vec_pretty(&envelope).map_err(|e| invalid(e.to_string()))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid(
            "Synapse export exceeds the 2 MiB interchange budget",
        ));
    }
    Ok(ExportReport { bytes, warnings })
}

fn sync_optional(object: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        if let Some(existing) = object.get_mut(key).filter(|v| v.is_object()) {
            if value.is_object() {
                merge_object(existing, value).expect("both objects checked");
                return;
            }
        }
        object.insert(key.into(), value);
    } else {
        object.remove(key);
    }
}

fn merge_object(target: &mut Value, fields: Value) -> Result<()> {
    let target = target
        .as_object_mut()
        .ok_or_else(|| invalid("Expected preserved object"))?;
    for (key, value) in fields
        .as_object()
        .ok_or_else(|| invalid("Expected translated object"))?
    {
        target.insert(key.clone(), value.clone());
    }
    Ok(())
}

fn export_effect(
    color: Option<Rgb>,
    effect: Option<&Effect>,
) -> Result<Option<(u8, Option<Value>)>> {
    let rgb = |value: Rgb| format!("#{:02x}{:02x}{:02x}", value.0, value.1, value.2);
    let colors = |items: &[Rgb]| {
        json!({
            "isRandom": items.is_empty(),
            "color1": items.first().copied().map(&rgb).unwrap_or_else(|| "#00ff00".into()),
            "color2": items.get(1).copied().map(&rgb).unwrap_or_else(|| "no-color".into())
        })
    };
    Ok(Some(match effect {
        Some(Effect::Spectrum) => (3, None),
        Some(Effect::Breathing(items)) if items.len() <= 2 => (2, Some(colors(items))),
        Some(Effect::Wave { reverse, speed: 3 }) => {
            (4, Some(json!({"direction": if *reverse {2} else {1}})))
        }
        Some(Effect::Reactive { color, duration }) => (
            5,
            Some(json!({"color1": rgb(*color), "duration": duration})),
        ),
        Some(Effect::Starlight {
            colors: items,
            duration,
        }) if items.len() <= 2 => {
            let mut settings = colors(items);
            settings["duration"] = json!(duration);
            (7, Some(settings))
        }
        Some(_) => return Err(Error::Unsupported(
            "Lighting effect or speed cannot be represented by the verified Ornata Synapse schema"
                .into(),
        )),
        None => match color {
            Some(value) => (1, Some(json!({"color1": rgb(value)}))),
            None => return Ok(None),
        },
    }))
}

fn decode_record(entry: &Value) -> Result<Value> {
    let payload = entry
        .get("payload")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Synapse entry has no base64 payload"))?;
    let decoded = STANDARD
        .decode(payload)
        .or_else(|_| STANDARD_NO_PAD.decode(payload))
        .or_else(|_| URL_SAFE.decode(payload))
        .or_else(|_| URL_SAFE_NO_PAD.decode(payload))
        .map_err(|e| invalid(format!("Invalid Synapse base64: {e}")))?;
    if decoded.len() > MAX_DECODED_BYTES {
        return Err(invalid("Decoded Synapse payload exceeds 1 MiB"));
    }
    check_json_complexity(&decoded)?;
    let value: Value = serde_json::from_slice(&decoded)
        .map_err(|e| invalid(format!("Invalid Synapse payload JSON: {e}")))?;
    if !value.is_object() {
        return Err(invalid("Synapse payload must be an object"));
    }
    let expected = entry
        .get("hash")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Synapse entry has no hash"))?;
    if expected != synapse_hash(&value)? {
        return Err(invalid("Synapse payload hash mismatch"));
    }
    Ok(value)
}

/// Matches Synapse's JSON-stable-stringify + MD5, excluding only top-level fields.
pub fn synapse_hash(value: &Value) -> Result<String> {
    let mut value = value.clone();
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("Only objects have Synapse hashes"))?;
    object.remove("hash");
    object.remove("gamemode");
    Ok(format!("{:x}", md5::compute(stable_json(&value)?)))
}

fn stable_json(value: &Value) -> Result<String> {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
            let mut entries = Vec::with_capacity(keys.len());
            for key in keys {
                entries.push(format!(
                    "{}:{}",
                    serde_json::to_string(key).map_err(|e| invalid(e.to_string()))?,
                    stable_json(&map[key])?
                ));
            }
            Ok(format!("{{{}}}", entries.join(",")))
        }
        Value::Array(items) => Ok(format!(
            "[{}]",
            items
                .iter()
                .map(stable_json)
                .collect::<Result<Vec<_>>>()?
                .join(",")
        )),
        Value::Number(number) => {
            let number = number
                .as_f64()
                .ok_or_else(|| invalid("Number cannot be represented as a JavaScript Number"))?;
            Ok(ryu_js::Buffer::new().format(number).into())
        }
        _ => serde_json::to_string(value).map_err(|e| invalid(e.to_string())),
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidArg(message.into())
}

// Bound allocation before serde_json creates a Value for a document full of tiny
// objects. Full syntax validation is still performed by serde_json afterwards.
pub(crate) fn check_json_complexity(bytes: &[u8]) -> Result<()> {
    let mut in_string = false;
    let mut escaped = false;
    let mut tokens = 0;
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                tokens += 1;
                in_string = true;
            }
            b'{' | b'[' => {
                tokens += 1;
                depth += 1;
                if depth > 64 {
                    return Err(invalid("Profile JSON nesting exceeds 64 levels"));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            b'-' | b'0'..=b'9' | b't' | b'f' | b'n'
                if index == 0
                    || matches!(
                        bytes[index - 1],
                        b' ' | b'\r' | b'\n' | b'\t' | b':' | b'[' | b','
                    ) =>
            {
                tokens += 1;
            }
            _ => {}
        }
        if tokens > MAX_JSON_TOKENS {
            return Err(invalid(
                "Profile JSON exceeds the 32768-token memory budget",
            ));
        }
    }
    Ok(())
}

/// Merge unknown fields without overwriting either version when metadata conflicts.
pub(crate) fn merge_extensions(
    target: &mut BTreeMap<String, Value>,
    incoming: BTreeMap<String, Value>,
) {
    for (key, value) in incoming {
        match target.get(&key) {
            None => {
                target.insert(key, value);
            }
            Some(existing) if existing == &value => {}
            Some(_) => {
                let mut archive_key = format!("_synapseLiteImported:{key}");
                let mut index = 2;
                while target.contains_key(&archive_key) {
                    archive_key = format!("_synapseLiteImported:{key}:{index}");
                    index += 1;
                }
                target.insert(archive_key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUSE: &[u8] = include_bytes!("../tests/fixtures/synthetic.synapse4");
    const NATIVE: &[u8] = include_bytes!("../tests/fixtures/secretarybird-unknown-fields.json");

    fn envelope(profile: Value, pid: u16) -> Value {
        json!({
            "productId": pid,
            "category": if pid == 605 { "KEYBOARD" } else { "MOUSE" },
            "deviceName": "Synthetic device",
            "profiles": [{
                "name": "Fixture", "hash": synapse_hash(&profile).unwrap(),
                "payload": STANDARD.encode(serde_json::to_vec(&profile).unwrap())
            }],
            "macros": []
        })
    }

    #[test]
    fn fixed_fixture_hash_and_foreign_fields_survive_dpi_edit() {
        let mut imported = import_document(MOUSE).unwrap();
        assert!(imported.warnings.iter().any(|w| w.contains("mappings")));
        assert!(imported.warnings.iter().any(|w| w.contains("Macros")));
        let profile = imported
            .store
            .devices
            .get_mut("00C4")
            .unwrap()
            .profiles
            .get_mut("Fixture Mouse")
            .unwrap();
        assert_eq!(profile.dpi, Some((800, 1600)));
        assert_eq!(profile.polling, Some(1000));
        assert_eq!(
            synapse_hash(&profile.extra[SOURCE]["decoded"]).unwrap(),
            "b5824efd4c1bc79d2e98a20019eb914b"
        );
        profile.dpi = Some((3200, 6400));
        let exported = export_synapse4(&imported.store, "00C4").unwrap();
        let output: Value = serde_json::from_slice(&exported.bytes).unwrap();
        let input: Value = serde_json::from_slice(MOUSE).unwrap();
        assert_eq!(output["macros"], input["macros"]);
        assert_eq!(output["futureEnvelope"], input["futureEnvelope"]);
        assert_eq!(
            output["profiles"][0]["unknownEntry"],
            input["profiles"][0]["unknownEntry"]
        );
        let decoded = decode_record(&output["profiles"][0]).unwrap();
        let original = decode_record(&input["profiles"][0]).unwrap();
        assert_eq!(decoded["mappings"], original["mappings"]);
        assert_eq!(decoded["future"], original["future"]);
        assert_eq!(decoded["dpi"], json!({"x": 3200, "y": 6400}));
        assert_eq!(
            import_document(&exported.bytes).unwrap().store.devices["00C4"].profiles
                ["Fixture Mouse"]
                .dpi,
            Some((3200, 6400))
        );
    }

    #[test]
    fn unknown_native_fields_round_trip_through_typed_edit_and_disk() {
        let report = import_document(NATIVE).unwrap();
        let mut value = serde_json::to_value(&report.store).unwrap();
        assert_eq!(value["futureStore"]["version"], 99);
        assert_eq!(
            value["devices"]["00C4:SYNTHETIC"]["profiles"]["Fixture"]["dpi_stages"]["futureStages"],
            true
        );
        let path = std::env::temp_dir().join(format!(
            "synapse-lite-compat-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut store = Store::load(&path).unwrap();
        let profile = store
            .devices
            .get_mut("00C4:SYNTHETIC")
            .unwrap()
            .profiles
            .get_mut("Fixture")
            .unwrap();
        profile.dpi = Some((1600, 3200));
        profile.buttons.clear();
        profile.lighting.get_mut("backlight").unwrap().brightness = None;
        store.save().unwrap();
        value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        let device = &value["devices"]["00C4:SYNTHETIC"];
        let profile = &device["profiles"]["Fixture"];
        assert_eq!(device["futureDevice"]["calibration"], json!([1, 2, 3]));
        assert_eq!(device["onboard"]["futureOnboard"], "preserved");
        assert_eq!(
            profile["futureProfile"]["opaque"],
            json!([null, true, "synthetic"])
        );
        assert_eq!(
            profile["lighting"]["backlight"]["futureZone"]["effect"],
            123
        );
        assert!(
            profile.get("buttons").is_none(),
            "Deleted bindings must not be resurrected by preservation"
        );
        assert!(profile["lighting"]["backlight"].get("brightness").is_none());
        assert_eq!(profile["dpi"], json!([1600, 3200]));
    }

    #[test]
    fn lighting_brightness_and_spectrum_translate_in_both_directions() {
        let payload = json!({
            "guid": "00000000-0000-4000-8000-000000000003", "name": "Fixture",
            "brightness": {"value": 75, "isEnabled": true, "future": 42},
            "quickEffects": {"selectedEffectId": 1, "selectedEffectSetting": {"color1": "#123456", "unknown": "keep"}},
            "mappings": [{"future": true}]
        });
        let mut report =
            import_document(&serde_json::to_vec(&envelope(payload, 605)).unwrap()).unwrap();
        let zone = report
            .store
            .devices
            .get_mut("025D")
            .unwrap()
            .profiles
            .get_mut("Fixture")
            .unwrap()
            .lighting
            .get_mut("backlight")
            .unwrap();
        assert_eq!(zone.color, Some(Rgb(0x12, 0x34, 0x56)));
        assert_eq!(zone.brightness, Some(75));
        zone.color = None;
        zone.effect = Some(Effect::Spectrum);
        zone.brightness = Some(40);
        let bytes = export_synapse4(&report.store, "025D").unwrap().bytes;
        let exported: Value = serde_json::from_slice(&bytes).unwrap();
        let payload = decode_record(&exported["profiles"][0]).unwrap();
        assert_eq!(
            payload["brightness"],
            json!({"value": 40, "isEnabled": true, "future": 42})
        );
        assert_eq!(payload["quickEffects"]["selectedEffectId"], 3);
        assert_eq!(
            payload["quickEffects"]["selectedEffectSetting"]["unknown"],
            "keep"
        );
        assert_eq!(payload["mappings"], json!([{"future": true}]));
    }

    #[test]
    fn all_verified_lighting_variants_round_trip() {
        let variants = [
            (
                2,
                json!({"isRandom": false, "color1": "#ff0000", "color2": "#0000ff"}),
            ),
            (3, json!({})),
            (4, json!({"direction": 2})),
            (5, json!({"color1": "#00ff00", "duration": 2})),
            (7, json!({"isRandom": true, "duration": 3})),
        ];
        for (id, settings) in variants {
            let payload = json!({"guid": "synthetic-guid", "name": "Fixture", "quickEffects": {"selectedEffectId": id, "selectedEffectSetting": settings}});
            let report =
                import_document(&serde_json::to_vec(&envelope(payload, 605)).unwrap()).unwrap();
            assert!(report.warnings.is_empty());
            let exported = export_synapse4(&report.store, "025D").unwrap();
            let again = import_document(&exported.bytes).unwrap();
            assert_eq!(
                again.store.devices["025D"].profiles["Fixture"].lighting,
                report.store.devices["025D"].profiles["Fixture"].lighting
            );
        }
    }

    #[test]
    fn invalid_hash_and_non_json_archives_fail_without_partial_import() {
        let mut input: Value = serde_json::from_slice(MOUSE).unwrap();
        input["profiles"][0]["hash"] = json!("00000000000000000000000000000000");
        assert!(import_document(&serde_json::to_vec(&input).unwrap()).is_err());
        assert!(matches!(
            import_document(b"PK\x03\x04"),
            Err(Error::Unsupported(_))
        ));
        assert!(import_document(b"{not-json}").is_err());
        let original = input["profiles"][0].clone();
        input["profiles"].as_array_mut().unwrap().push(original);
        assert!(import_document(&serde_json::to_vec(&input).unwrap()).is_err());
    }

    #[test]
    fn javascript_canonicalization_handles_numbers_unicode_and_top_level_exclusions() {
        let input: Value = serde_json::from_str(r#"{"z":1.0,"tiny":1e-7,"large":1e21,"negativeZero":-0.0,"nested":{"hash":"keep"},"\ue000":2,"😀":1}"#).unwrap();
        assert_eq!(stable_json(&input).unwrap(), "{\"large\":1e+21,\"negativeZero\":0,\"nested\":{\"hash\":\"keep\"},\"tiny\":1e-7,\"z\":1,\"😀\":1,\"\u{e000}\":2}");
        let original = json!({"a": 1, "nested": {"hash": "keep"}});
        let mut excluded = original.clone();
        excluded["hash"] = json!("ignored");
        excluded["gamemode"] = json!({"ignored": true});
        assert_eq!(
            synapse_hash(&original).unwrap(),
            synapse_hash(&excluded).unwrap()
        );
    }

    #[test]
    fn numeric_object_keys_and_extreme_numbers_match_javascript_reference() {
        let reference = include_str!("../tests/fixtures/javascript-canonical-reference.jsonl");
        for line in reference.lines().take(2) {
            let fixture: Value = serde_json::from_str(line).unwrap();
            let input: Value = serde_json::from_str(fixture["input"].as_str().unwrap()).unwrap();
            assert_eq!(
                stable_json(&input).unwrap(),
                fixture["canonical"].as_str().unwrap()
            );
            assert_eq!(
                synapse_hash(&input).unwrap(),
                fixture["md5"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn unpaired_utf16_surrogates_are_rejected_instead_of_replaced() {
        let fixture: Value = serde_json::from_str(
            include_str!("../tests/fixtures/javascript-canonical-reference.jsonl")
                .lines()
                .nth(2)
                .unwrap(),
        )
        .unwrap();
        let raw_payload = fixture["input"].as_str().unwrap().as_bytes();
        let document = json!({
            "productId": 196, "category": "MOUSE", "profiles": [{
                "name": "Synthetic surrogate", "payload": STANDARD.encode(raw_payload), "hash": fixture["md5"]
            }], "macros": []
        });
        assert!(
            matches!(import_document(&serde_json::to_vec(&document).unwrap()), Err(Error::InvalidArg(message)) if message.contains("Invalid Synapse payload JSON"))
        );
    }

    #[test]
    fn native_import_keeps_conflicting_extension_metadata_without_overwriting() {
        let mut mine = import_document(NATIVE).unwrap().store;
        let mut incoming = import_document(NATIVE).unwrap().store;
        incoming
            .extra
            .insert("futureStore".into(), json!({"version": 100}));
        incoming
            .devices
            .get_mut("00C4:SYNTHETIC")
            .unwrap()
            .extra
            .insert("futureDevice".into(), json!({"other": true}));
        assert_eq!(mine.import(incoming), 1);
        assert_eq!(mine.extra["futureStore"]["version"], 99);
        assert_eq!(
            mine.extra["_synapseLiteImported:futureStore"]["version"],
            100
        );
        assert_eq!(
            mine.devices["00C4:SYNTHETIC"].extra["_synapseLiteImported:futureDevice"]["other"],
            true
        );
        assert!(mine.devices["00C4:SYNTHETIC"]
            .profiles
            .contains_key("Fixture (imported)"));
    }

    #[test]
    fn unrepresentable_native_settings_fail_export_instead_of_being_dropped() {
        let mut report = import_document(MOUSE).unwrap();
        report
            .store
            .devices
            .get_mut("00C4")
            .unwrap()
            .profiles
            .get_mut("Fixture Mouse")
            .unwrap()
            .buttons
            .insert("side-forward".into(), "macro:1".into());
        assert!(matches!(
            export_synapse4(&report.store, "00C4"),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn oversized_and_high_complexity_documents_fail_before_value_allocation() {
        assert!(import_document(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
        let many_values = format!("[{}]", vec!["0"; MAX_JSON_TOKENS + 1].join(","));
        assert!(check_json_complexity(many_values.as_bytes()).is_err());
        let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
        assert!(check_json_complexity(deep.as_bytes()).is_err());
        assert!(check_json_complexity(br#"{"string":"[not, structure] \\"}"#).is_ok());
        let expanded = envelope(
            json!({"guid": "synthetic", "name": "Fixture", "opaque": "x".repeat(800_000)}),
            196,
        );
        let bytes = serde_json::to_vec(&expanded).unwrap();
        assert!(bytes.len() < MAX_DOCUMENT_BYTES);
        assert!(
            matches!(import_document(&bytes), Err(Error::InvalidArg(message)) if message.contains("Lossless native profile backup"))
        );
    }
}
