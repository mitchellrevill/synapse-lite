//! Verified scalar settings and default profile schemas for the installed models.

use super::{integer, invalid, merge_object, Result};
use crate::features::{DpiStages, GameMode};
use crate::profile::Profile;
use serde_json::{json, Map, Value};

pub(super) fn import_mouse(
    source: &Value,
    profile: &mut Profile,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    if let Some(raw)=source.get("rotation") {
        match serde_json::from_value::<crate::dynamic_sensitivity::Rotation>(raw.clone()) {
            Ok(intent) if intent.validate().is_ok()=>profile.rotation=Some(intent),
            _=>warnings.push(format!("{name}: unsupported Rotation retained without hardware replay")),
        }
    }
    if let Some(raw)=source.get("dynamicSensitivity") {
        match serde_json::from_value::<crate::dynamic_sensitivity::DynamicSensitivity>(raw.clone()) {
            Ok(intent) if intent.validate().is_ok()=>profile.dynamic_sensitivity=Some(intent),
            _=>warnings.push(format!("{name}: unsupported Dynamic Sensitivity data retained without hardware replay")),
        }
    }
    // Installed196 reads sleep from hardware, independently of named profiles.
    // Keep an exported powerSavingValue opaque in the original decoded payload;
    // translating its template default would reset the device on profile activation.
    if let Some(stages) = source.get("dpiStages") {
        let mut object = stages
            .as_object()
            .cloned()
            .ok_or_else(|| invalid("dpiStages must be an object"))?;
        let count = integer(object.get("count"), 1, 5, "dpiStages.count")? as usize;
        object.remove("count");
        let intent: crate::performance::Performance = serde_json::from_value(Value::Object(object))
            .map_err(|e| invalid(format!("invalid dpiStages: {e}")))?;
        intent.validate()?;
        if count != intent.visible_count() {
            return Err(invalid("dpiStages.count does not match visible stages"));
        }
        profile.dpi_stages = Some(intent.pack()?);
        profile.performance = Some(intent);
    }
    Ok(())
}

pub(super) fn import_keyboard_lighting(
    source: &Value,
    profile: &mut Profile,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let mut settings = crate::lighting::LightingSettings::default();
    if let Some(value) = source.get("brightness") {
        settings.brightness = Some(
            serde_json::from_value(value.clone())
                .map_err(|e| invalid(format!("invalid brightness: {e}")))?,
        );
    }
    if let Some(value) = source.get("switchOffLighting") {
        settings.switch_off = Some(
            serde_json::from_value(value.clone())
                .map_err(|e| invalid(format!("invalid switchOffLighting: {e}")))?,
        );
    }
    if settings.brightness.is_some() || settings.switch_off.is_some() {
        settings.validate()?;
        if settings.needs_runtime() {
            warnings.push(format!("{name}: idle/display switch-off settings are retained and require a running host runtime"));
        }
        settings.store_in_profile(profile)?;
    }
    Ok(())
}

/// Return true when typed source intent owns brightness and policy export.
/// Old native profiles without the typed field retain their previous overlay rules.
pub(super) fn export_keyboard_lighting(
    profile: &Profile,
    decoded: &mut Map<String, Value>,
    preserve_defaults: bool,
) -> Result<bool> {
    let Some(settings) = &profile.lighting_settings else {
        return Ok(false);
    };
    settings.validate()?;
    if !settings.extra.is_empty() {
        return Err(crate::Error::Unsupported(
            "unknown native lighting-settings extensions require a native JSON backup".into(),
        ));
    }
    let mut settings = settings.clone();
    if let Some(zone) = profile.lighting.get("backlight") {
        let projected = settings
            .brightness
            .as_ref()
            .map(crate::lighting::Brightness::projected);
        if zone.brightness != projected {
            // Preserve legacy direct zone edits without dropping retained source
            // metadata; supported editors should use store_in_profile to sync.
            if let Some(value) = zone.brightness {
                let brightness = settings.brightness.get_or_insert_with(Default::default);
                brightness.set_value(value)?;
            } else {
                settings.brightness = None;
            }
        }
    }
    match settings.brightness {
        Some(value) => merge_object(
            decoded.entry("brightness").or_insert(json!({})),
            serde_json::to_value(value).map_err(|e| invalid(e.to_string()))?,
        )?,
        None if !preserve_defaults => {
            decoded.remove("brightness");
        }
        None => {}
    }
    match settings.switch_off {
        Some(value) => merge_object(
            decoded.entry("switchOffLighting").or_insert(json!({})),
            serde_json::to_value(value).map_err(|e| invalid(e.to_string()))?,
        )?,
        None if !preserve_defaults => {
            decoded.remove("switchOffLighting");
        }
        None => {}
    }
    Ok(true)
}

pub(super) fn import_game_mode(
    source: &Value,
    profile: &mut Profile,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let Some(gaming) = source.get("gamingMode") else {
        return Ok(());
    };
    let state = integer(gaming.get("state"), 0, 2, "gamingMode.state")?;
    let flag = |key| {
        gaming
            .get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid(format!("gamingMode.{key} must be boolean")))
    };
    let win = flag("isWindowsKeyDisabled")?;
    let alt_tab = flag("isAltTabDisabled")?;
    let alt_f4 = flag("isAltF4Disabled")?;
    let conditional =
        state == 2 || gaming.get("enableInGame").and_then(Value::as_bool) == Some(true);
    let extra_shortcuts = ["isAltSpaceDisabled", "isAltEnterDisabled"]
        .iter()
        .any(|key| gaming.get(*key).and_then(Value::as_bool) == Some(true));
    if conditional || extra_shortcuts || (state == 1 && !win && !alt_tab && !alt_f4) {
        warnings.push(format!("{name}: conditional gaming mode or additional shortcut blocking is preserved but cannot be applied by the native gaming-mode contract"));
    } else {
        let mode = if state == 0 {
            GameMode::default()
        } else {
            GameMode {
                win,
                alt_tab,
                alt_f4,
            }
        };
        profile.game_mode = Some(mode.to_string());
    }
    Ok(())
}

pub(super) fn export_mouse(
    profile: &Profile,
    decoded: &mut Map<String, Value>,
    preserve_defaults: bool,
) -> Result<()> {
    if let Some(intent)=&profile.rotation {intent.validate()?;merge_object(decoded.entry("rotation").or_insert(json!({})),serde_json::to_value(intent).map_err(|e|invalid(e.to_string()))?)?;}
    if let Some(intent)=&profile.dynamic_sensitivity {
        intent.validate()?;
        merge_object(decoded.entry("dynamicSensitivity").or_insert(json!({})),serde_json::to_value(intent).map_err(|e|invalid(e.to_string()))?)?;
    }
    if let Some(minutes) = profile.sleep_minutes {
        if !(1..=15).contains(&minutes) {
            return Err(invalid(
                "DeathAdder sleep timer must be between 1 and 15 minutes",
            ));
        }
        decoded.insert("powerSavingValue".into(), json!(minutes));
    } else if !preserve_defaults
        && profile
            .extra
            .get(super::SOURCE)
            .and_then(|source| source.get("deviceSleepScope"))
            .and_then(Value::as_bool)
            != Some(true)
    {
        // No marker means an older native/source-backed profile: retain its
        // existing explicit known-field deletion semantics.
        decoded.remove("powerSavingValue");
    }
    if let Some(intent) = &profile.performance {
        let mut intent = intent.clone();
        // Legacy editors can still edit the packed projection. Merge those edits
        // back into retained rows instead of silently discarding hidden values.
        if let Some(legacy) = &profile.dpi_stages {
            if intent.pack()? != *legacy {
                merge_legacy_stages(&mut intent, legacy)?;
            }
        }
        intent.validate()?;
        let mut value = serde_json::to_value(&intent).map_err(|e| invalid(e.to_string()))?;
        value["count"] = json!(intent.visible_count());
        decoded.insert("dpiStages".into(), value);
        return Ok(());
    }
    let Some(stages) = &profile.dpi_stages else {
        if !preserve_defaults {
            decoded.remove("dpiStages");
        }
        return Ok(());
    };
    if stages.stages.is_empty()
        || stages.stages.len() > 5
        || stages.active == 0
        || stages.active as usize > stages.stages.len()
    {
        return Err(invalid(
            "DPI stages require 1–5 entries and a valid 1-based active stage",
        ));
    }
    let original = decoded.entry("dpiStages").or_insert(json!({}));
    let mut entries = original
        .get("stages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    entries.truncate(stages.stages.len());
    for (index, &(x, y)) in stages.stages.iter().enumerate() {
        if !(100..=26_000).contains(&x) || !(100..=26_000).contains(&y) {
            return Err(invalid("DPI stage value is outside 100–26000"));
        }
        if index == entries.len() {
            entries.push(json!({}));
        }
        let old_x = entries[index].get("x").and_then(Value::as_u64);
        let old_y = entries[index].get("y").and_then(Value::as_u64);
        let old_independent = entries[index].get("independent").and_then(Value::as_bool);
        let unchanged = old_x == Some(x as u64)
            && match old_independent {
                Some(true) => old_y == Some(y as u64),
                Some(false) => x == y,
                None => false,
            };
        if !unchanged {
            merge_object(&mut entries[index], json!({"x":x,"y":y,"independent":x!=y}))?;
        }
        merge_object(&mut entries[index], json!({"visible":true}))?;
    }
    merge_object(
        original,
        json!({"active": stages.active, "enable": true, "count": stages.stages.len(), "stages": entries}),
    )
}

/// Reconcile a legacy physical-stage edit while retaining rows invisible to it.
fn merge_legacy_stages(
    intent: &mut crate::performance::Performance,
    legacy: &DpiStages,
) -> Result<()> {
    use crate::performance::DpiStage;
    if legacy.stages.is_empty()
        || legacy.stages.len() > 5
        || legacy.active == 0
        || usize::from(legacy.active) > legacy.stages.len()
    {
        return Err(invalid("invalid legacy DPI stages"));
    }
    let old = intent.pack()?;
    let indices: Vec<usize> = if legacy.stages.len() == old.stages.len() {
        if intent.enabled {
            intent
                .stages
                .iter()
                .enumerate()
                .filter(|(_, r)| r.visible)
                .map(|(i, _)| i)
                .collect()
        } else {
            vec![usize::from(intent.active - 1)]
        }
    } else {
        // Legacy count changes have no visibility flags. Match the source UI's
        // prefix visibility selector and keep hidden trailing rows available.
        while intent.stages.len() < legacy.stages.len() {
            let &(x, y) = &legacy.stages[intent.stages.len()];
            intent.stages.push(DpiStage {
                x,
                y,
                independent: x != y,
                visible: true,
                extra: Default::default(),
            });
        }
        intent.enabled = true;
        intent.set_visible_count(legacy.stages.len() as u8)?;
        (0..legacy.stages.len()).collect()
    };
    intent.active = indices[usize::from(legacy.active - 1)] as u8 + 1;
    for (i, &(x, y)) in indices.into_iter().zip(&legacy.stages) {
        let row = &mut intent.stages[i];
        if (row.x, row.y) != (x, y) {
            row.x = x;
            row.y = y;
            row.independent = x != y;
        }
    }
    intent.validate()
}

pub(super) fn export_game_mode(profile: &Profile, decoded: &mut Map<String, Value>) -> Result<()> {
    let Some(text) = &profile.game_mode else {
        return Ok(());
    };
    let mode: GameMode = text.parse()?;
    let object = decoded.entry("gamingMode").or_insert(json!({}));
    let mut fields = json!({"state": if mode.is_off() { 0 } else { 1 }});
    if !mode.is_off() || object.as_object().is_some_and(Map::is_empty) {
        fields["isWindowsKeyDisabled"] = json!(mode.win);
        fields["isAltTabDisabled"] = json!(mode.alt_tab);
        fields["isAltF4Disabled"] = json!(mode.alt_f4);
    }
    for key in ["enableInGame", "isAltSpaceDisabled", "isAltEnterDisabled"] {
        if object.get(key).is_some() {
            fields[key] = json!(false);
        }
    }
    merge_object(object, fields)
}

pub(super) fn default_envelope(pid: u16) -> Result<Value> {
    let (name, category) = match pid {
        196 | 197 => ("Razer DeathAdder V3 HyperSpeed", "MOUSE"),
        605 => ("Razer Ornata V2", "KEYBOARD"),
        _ => return Err(invalid(
            "Native-to-Synapse export is only verified for DeathAdder V3 HyperSpeed and Ornata V2",
        )),
    };
    Ok(
        json!({"productId": pid, "deviceName": name, "category": category, "profiles": [], "macros": []}),
    )
}

pub(super) fn default_profile(pid: u16, name: &str) -> Result<Value> {
    let guid = new_guid()?;
    match pid {
        196 | 197 => Ok(json!({
            "guid": guid, "name": name, "dpi": {"x": 800, "y": 800}, "mappings": [],
            "dpiStages": {"stages": [
                {"x":400,"y":400,"independent":false,"visible":true},
                {"x":800,"y":800,"independent":false,"visible":true},
                {"x":1600,"y":1600,"independent":false,"visible":true},
                {"x":3200,"y":3200,"independent":false,"visible":true},
                {"x":6400,"y":6400,"independent":false,"visible":true}
            ], "active":3,"enable":true,"count":5},
            "pollingRate":1000,"powerSavingValue":5,"lowPowerMode":30
        })),
        605 => Ok(json!({
            "guid":guid,"name":name,"switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1},
            "brightness":{"isEnabled":true,"value":100},"quickEffects":{"selectedEffectId":3},
            "gamingMode":{"state":0,"isWindowsKeyDisabled":false,"isAltTabDisabled":false,"isAltF4Disabled":false},
            "macro":{},"mappings":[]
        })),
        _ => Err(invalid(
            "No verified Synapse default profile for this model",
        )),
    }
}

pub(super) fn new_guid() -> Result<String> {
    let mut bytes = [0u8; 16];
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        let result = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if result < 0 {
            return Err(invalid(format!(
                "Could not allocate a random profile GUID: {result:#x}"
            )));
        }
    }
    #[cfg(unix)]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut bytes))
            .map_err(|e| invalid(format!("Could not allocate a random profile GUID: {e}")))?;
    }
    #[cfg(not(any(unix, windows)))]
    return Err(invalid(
        "No operating-system random generator is implemented for profile GUIDs on this platform",
    ));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7],bytes[8],bytes[9],bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15]))
}
