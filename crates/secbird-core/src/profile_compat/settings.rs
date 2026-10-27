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
    if let Some(value) = source.get("powerSavingValue") {
        profile.sleep_minutes = Some(integer(Some(value), 1, 15, "powerSavingValue")? as u16);
    }
    if let Some(stages) = source.get("dpiStages") {
        let raw = stages
            .get("stages")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("dpiStages.stages must be an array"))?;
        if raw.is_empty() || raw.len() > 5 {
            return Err(invalid(
                "DeathAdder DPI stages require between 1 and 5 entries",
            ));
        }
        let active = integer(
            stages.get("active"),
            1,
            raw.len() as u64,
            "dpiStages.active",
        )? as u8;
        let enabled = stages
            .get("enable")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("dpiStages.enable must be boolean"))?;
        let count = integer(stages.get("count"), 1, raw.len() as u64, "dpiStages.count")? as usize;
        let mut values = Vec::new();
        let mut visible_count = 0;
        for stage in raw {
            let x = integer(stage.get("x"), 100, 26_000, "DPI stage x")? as u16;
            let y = integer(stage.get("y"), 100, 26_000, "DPI stage y")? as u16;
            let independent = stage
                .get("independent")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("DPI stage independent must be boolean"))?;
            let visible = stage
                .get("visible")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("DPI stage visible must be boolean"))?;
            visible_count += usize::from(visible);
            values.push((x, if independent { y } else { x }));
        }
        if count != visible_count {
            return Err(invalid("dpiStages.count does not match visible stages"));
        }
        if !enabled || visible_count != raw.len() {
            warnings.push(format!("{name}: disabled or hidden DPI stages are preserved; native stage cycling cannot express their enable/visibility state"));
        } else {
            profile.dpi_stages = Some(DpiStages {
                active,
                stages: values,
                ..Default::default()
            });
        }
    }
    Ok(())
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
    if let Some(minutes) = profile.sleep_minutes {
        if !(1..=15).contains(&minutes) {
            return Err(invalid(
                "DeathAdder sleep timer must be between 1 and 15 minutes",
            ));
        }
        decoded.insert("powerSavingValue".into(), json!(minutes));
    } else if !preserve_defaults {
        decoded.remove("powerSavingValue");
    }
    let Some(stages) = &profile.dpi_stages else {
        if preserve_defaults {
            return Ok(());
        }
        // An unsupported original visibility/enable configuration was not translated.
        // Preserve it; absent native stages must not silently erase that configuration.
        if let Some(original) = decoded.get("dpiStages") {
            let mut scratch = Profile::default();
            let mut warnings = Vec::new();
            import_mouse(
                &json!({"dpiStages": original}),
                &mut scratch,
                "profile",
                &mut warnings,
            )?;
            if scratch.dpi_stages.is_some() {
                decoded.remove("dpiStages");
            }
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
