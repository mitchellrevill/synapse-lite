//! Explicit development-only active Lighting probe policy and sanitized readiness.
//! No profile creation, opt-in, persistence or hardware access occurs here.
use secbird_core::{Error, Result, Store};
use serde_json::{json, Value};

pub(crate) const FLAG: &str = "--runtime-lighting-probe";

pub(crate) fn duration(args: &[String]) -> std::result::Result<Option<u32>, String> {
    let positions: Vec<_> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == FLAG)
        .map(|(i, _)| i)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() != 1
        || args.len() != 2
        || positions[0] != 0
        || args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--runtime-smoke" | "--native-smoke" | "--serve" | "--tray"
            )
        })
    {
        return Err(format!("{FLAG} must be used alone with one duration"));
    }
    let seconds = args
        .get(positions[0] + 1)
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| (1..=600).contains(s));
    seconds
        .map(Some)
        .ok_or_else(|| format!("{FLAG} requires 1..600 seconds"))
}

/// Probe presentation stays on the admitted Lighting owner. Spectrum has no
/// source color editor; only a pre-existing fixed Breathing profile can use it.
pub(crate) fn presentation(requested: &str) -> std::result::Result<&str, String> {
    match requested {
        "keyboard-lighting" => Ok("keyboard-lighting"),
        "keyboard-lighting-color-editor" | "keyboard-color-editor" => Ok("keyboard-lighting-color-editor"),
        _ => Err("Active Lighting probe supports only keyboard-lighting or keyboard-lighting-color-editor".into()),
    }
}

fn reject(reason: &str) -> Error {
    Error::InvalidArg(format!("Lighting probe rejected: {reason}"))
}

/// Conservative probe-only admission. Normal stores and all import/export paths
/// retain their existing, more general semantics.
pub(crate) fn validate_store(store: &Store) -> Result<(String, String)> {
    if !store.extra.is_empty() || store.devices.len() != 1 {
        return Err(reject(
            "exactly one configured device and no root extensions are required",
        ));
    }
    let (key, device) = store.devices.iter().next().unwrap();
    if !key.starts_with("025D:") || key.len() <= 5 || key[5..].trim().is_empty() {
        return Err(reject(
            "an exact Ornata serial-bound device key is required",
        ));
    }
    if device.profiles.len() != 1
        || !device.profiles.contains_key(&device.active)
        || device.lighting_off
        || device.onboard.is_some()
        || device.power.is_some()
        || !device.extra.is_empty()
    {
        return Err(reject("one active profile, enabled lighting, and no device action/power extensions are required"));
    }
    let profile = &device.profiles[&device.active];
    if profile.dpi.is_some()
        || profile.polling.is_some()
        || profile.dpi_stages.is_some()
        || profile.performance.is_some()
        || profile.sleep_minutes.is_some()
        || profile.auto_sleep.is_some()
        || profile.dimming.is_some()
        || profile.game_mode.is_some()
        || !profile.apps.is_empty()
        || !profile.buttons.is_empty()
        || !profile.keys.is_empty()
    {
        return Err(reject("nonlighting settings, input actions, linked applications or per-key programs are present"));
    }
    if profile.extra.len() != 1 || !profile.extra.contains_key("_synapseLiteSynapse4") {
        return Err(reject(
            "only retained source Lighting metadata is permitted",
        ));
    }
    let metadata = profile.extra["_synapseLiteSynapse4"]
        .as_object()
        .ok_or_else(|| reject("source metadata must be an object"))?;
    if metadata.keys().any(|k| {
        !matches!(
            k.as_str(),
            "entry" | "decoded" | "uniformHostIntent" | "brightnessBaseline"
        )
    }) {
        return Err(reject("unrecognized source metadata is present"));
    }
    let decoded = metadata
        .get("decoded")
        .and_then(Value::as_object)
        .ok_or_else(|| reject("decoded source Lighting intent is required"))?;
    if decoded.keys().any(|k| {
        !matches!(
            k.as_str(),
            "guid"
                | "name"
                | "brightness"
                | "switchOffLighting"
                | "quickEffects"
                | "gamingMode"
                | "macro"
                | "mappings"
        )
    }) {
        return Err(reject("retained nonlighting source settings are present"));
    }
    if decoded
        .get("macro")
        .is_some_and(|v| !v.as_object().is_some_and(|v| v.is_empty()))
        || decoded
            .get("mappings")
            .is_some_and(|v| !v.as_array().is_some_and(|v| v.is_empty()))
    {
        return Err(reject("source macros or input mappings are present"));
    }
    if let Some(game) = decoded.get("gamingMode") {
        let game = game
            .as_object()
            .ok_or_else(|| reject("source gamingMode must be inert"))?;
        if game.keys().any(|k| {
            !matches!(
                k.as_str(),
                "state" | "isWindowsKeyDisabled" | "isAltTabDisabled" | "isAltF4Disabled"
            )
        }) || game.get("state").is_some_and(|v| *v != json!(0))
            || game.iter().any(|(k, v)| k != "state" && *v != json!(false))
        {
            return Err(reject("retained gaming-mode actions are present"));
        }
    }
    if profile.lighting.len() != 1
        || !profile.lighting.contains_key("backlight")
        || !profile.lighting["backlight"].extra.is_empty()
        || profile.lighting["backlight"].color.is_some()
    {
        return Err(reject(
            "one unambiguous uniform backlight program is required",
        ));
    }
    let program = crate::host_uniform::Program::from_profile(profile, 605)?
        .ok_or_else(|| reject("source Spectrum or fixed Breathing intent is required"))?;
    if !program.opted_in(profile) {
        return Err(reject(
            "existing explicit native uniform opt-in is required",
        ));
    }
    let quick = secbird_core::profile_compat::source_quick_effects(profile)
        .ok_or_else(|| reject("source quick effect is missing"))?;
    if quick["selectedEffectId"] != json!(program.source_id()) {
        return Err(reject("source effect and opted native projection disagree"));
    }
    let settings = secbird_core::lighting::LightingSettings::from_profile(profile)?
        .ok_or_else(|| reject("retained brightness intent is required"))?;
    settings.validate()?;
    if !settings
        .brightness
        .as_ref()
        .is_some_and(|b| b.enabled && b.value > 0)
    {
        return Err(reject("positive enabled retained brightness is required"));
    }
    secbird_core::profile_compat::validate_store_budget(store)?;
    Ok((key.clone(), device.active.clone()))
}

/// Uses cached state only. No identity, profile name or source payload is emitted.
pub(crate) fn readiness(runtime: &Value, expected_profile: &str) -> Value {
    let uniform = &runtime["uniform_host"];
    let owner_matches = runtime["profile"].as_str() == Some(expected_profile)
        && uniform["profile"].as_str() == Some(expected_profile);
    let worker_alive = uniform["worker_enabled"] == true && uniform["worker_stopped"] != true;
    let accepted_frames = uniform["accepted_frames"].as_u64().unwrap_or(0);
    let no_error = [
        runtime.get("error"),
        uniform.get("error"),
        uniform.get("configuration_error"),
    ]
    .into_iter()
    .flatten()
    .all(|v| v.is_null());
    let observation = &uniform["control_observation"];
    let checkpoint_age = observation["last_control_check_age_us"].as_u64();
    let controls = observation["completed_control_checkpoints"]
        .as_u64()
        .is_some_and(|n| n >= 1)
        && checkpoint_age.is_some_and(|n| n <= 500_000)
        && uniform["mode_before"] == 3;
    let streaming = uniform["streaming"] == true && uniform["state"] == "playing";
    let supported = matches!(uniform["source_id"].as_u64(), Some(2 | 3));
    let ready = owner_matches
        && worker_alive
        && accepted_frames >= 3
        && no_error
        && controls
        && streaming
        && supported;
    json!({"ready":ready,"owner_matches":owner_matches,"worker_alive":worker_alive,"streaming":streaming,
        "accepted_frames":accepted_frames,"controls_checkpoint_ready":controls,
        "control_checkpoint_age_us":checkpoint_age,"control_parameter_known":uniform["parameter_before"].is_number(),
        "per_frame_controls_verified":uniform["custom_frame_control_verified"]==true,"source_id":uniform.get("source_id"),
        "native_id":uniform.get("native_id"),"no_cached_error":no_error,"active_lighting_probe":true,
        "read_only_smoke":false,"final_ram_acceptance":false})
}

/// A previously valid cached frame is insufficient after observation stalls.
pub(crate) fn fresh_readiness(runtime: &Value, profile: &str, age_ms: Option<u64>) -> Value {
    let mut status = readiness(runtime, profile);
    status["status_snapshot_age_ms"] = json!(age_ms);
    status["ready"] = json!(status["ready"] == true && age_ms.is_some_and(|age| age <= 2500));
    status
}

/// Keep pre-stop activity evidence distinct from errors observed after joining
/// the worker. A late fault must not be replaced by an older successful sample.
pub(crate) fn stop_report(
    observation: Option<(&Value, u64)>,
    stopped: bool,
    restoration_failures: usize,
    post_stop: Option<&Value>,
) -> Value {
    let mut report = observation
        .map(|(v, _)| v.clone())
        .unwrap_or_else(|| json!({"ready":false,"worker_alive":false,"accepted_frames":0}));
    let age = observation.map(|(_, age)| age);
    report["ready_before_stop"] =
        json!(report["ready"] == true && age.is_some_and(|age| age <= 2500));
    report["worker_was_running"] = report["worker_alive"].clone();
    report["status_snapshot_age_ms"] = json!(age);
    report["worker_stopped"] = json!(stopped);
    report["restoration_failure_count"] = json!(restoration_failures);
    report["clean_stop"] = json!(stopped && restoration_failures == 0);
    let available = post_stop.is_some_and(|v| v["owner_matches"] == true);
    let no_error = available && post_stop.is_some_and(|v| v["no_cached_error"] == true);
    report["post_stop_status_available"] = json!(available);
    report["post_stop_no_cached_error"] = json!(no_error);
    report["last_error"] = if restoration_failures > 0 {
        json!("shutdown-restoration-failed")
    } else if no_error {
        Value::Null
    } else {
        json!("post-stop-cached-error-or-unavailable")
    };
    report["generic_profile_apply_enabled"] = json!(false);
    report["nonlighting_actions_enabled"] = json!(false);
    report["measurement_only_config_pin_overhead"] = json!(true);
    report
}

/// Central startup rule: existing measurement/editor-only modes never schedule
/// uniform output. The explicit active probe is separately admitted first.
pub(crate) fn starts_worker(args: &[String]) -> bool {
    !args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--runtime-smoke" | "--native-smoke" | "--serve"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{Profile, Zone};
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }
    fn fixture() -> Store {
        let mut profile = Profile::default();
        profile.extra.insert(
            "_synapseLiteSynapse4".into(),
            json!({"decoded":{
                "quickEffects":{"selectedEffectId":3},"brightness":{"isEnabled":true,"value":60}
            }}),
        );
        profile.lighting.insert(
            "backlight".into(),
            Zone {
                effect: Some(secbird_core::effects::Effect::Spectrum),
                ..Default::default()
            },
        );
        profile.lighting_settings = Some(secbird_core::lighting::LightingSettings {
            brightness: Some(secbird_core::lighting::Brightness {
                value: 60,
                ..Default::default()
            }),
            switch_off: Some(Default::default()),
            ..Default::default()
        });
        crate::host_uniform::Program::Spectrum
            .opt_in(&mut profile)
            .unwrap();
        let mut store = Store::default();
        let mut device = secbird_core::profile::DeviceProfiles::default();
        device.profiles.insert("Default".into(), profile);
        store.devices.insert("025D:SIMULATED".into(), device);
        store
    }
    #[test]
    fn explicit_probe_rejects_invalid_duration_and_mixed_entrypoints() {
        assert_eq!(duration(&args(&[FLAG, "12"])).unwrap(), Some(12));
        for bad in [
            vec![FLAG],
            vec![FLAG, "0"],
            vec![FLAG, "601"],
            vec![FLAG, "oops"],
            vec![FLAG, "12", "--tray"],
            vec!["--runtime-smoke", "12", FLAG, "12"],
            vec![FLAG, "12", FLAG, "12"],
        ] {
            assert!(duration(&args(&bad)).is_err(), "{bad:?}");
        }
        assert_eq!(duration(&args(&["--runtime-smoke", "12"])).unwrap(), None);
        assert!(!starts_worker(&args(&["--runtime-smoke", "12"])));
        assert!(!starts_worker(&args(&["--native-smoke", "12"])));
        assert!(!starts_worker(&args(&["--serve"])));
        assert!(starts_worker(&args(&[FLAG, "12"])));
        assert!(starts_worker(&[]));
    }
    #[test]
    fn probe_admission_requires_existing_opt_in_and_forbids_actions_or_extra_scope() {
        let store = fixture();
        let before = serde_json::to_value(&store).unwrap();
        assert_eq!(
            validate_store(&store).unwrap(),
            ("025D:SIMULATED".into(), "Default".into())
        );
        assert_eq!(serde_json::to_value(&store).unwrap(), before);
        let mut no_opt = fixture();
        crate::host_uniform::Program::clear_opt_in(
            no_opt
                .devices
                .get_mut("025D:SIMULATED")
                .unwrap()
                .profiles
                .get_mut("Default")
                .unwrap(),
        );
        assert!(validate_store(&no_opt).is_err());
        for kind in 0..7 {
            let mut changed = fixture();
            let device = changed.devices.get_mut("025D:SIMULATED").unwrap();
            let profile = device.profiles.get_mut("Default").unwrap();
            match kind {
                0 => profile.game_mode = Some("off".into()),
                1 => profile
                    .buttons
                    .insert("side-forward".into(), "key:a".into())
                    .map(|_| ())
                    .unwrap_or(()),
                2 => profile.apps.push("game.exe".into()),
                3 => {
                    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["macro"] =
                        json!({"a":{"steps":[]}})
                }
                4 => profile
                    .extra
                    .insert("unknownAction".into(), json!(true))
                    .map(|_| ())
                    .unwrap_or(()),
                5 => device
                    .profiles
                    .insert("Other".into(), Profile::default())
                    .map(|_| ())
                    .unwrap_or(()),
                _ => device.onboard = Some(Default::default()),
            }
            assert!(validate_store(&changed).is_err(), "case{kind}");
        }
        let mut extra = fixture();
        extra
            .devices
            .insert("00C4:OTHER".into(), Default::default());
        assert!(validate_store(&extra).is_err());
    }
    fn playing() -> Value {
        json!({"profile":"Fixture","uniform_host":{"profile":"Fixture","worker_enabled":true,"streaming":true,"state":"playing","source_id":3,"native_id":4,"accepted_frames":3,"error":null,
            "mode_before":3,"parameter_before":null,"custom_frame_control_verified":false,"control_observation":{"completed_control_checkpoints":1,"last_control_check_age_us":100000}}})
    }
    #[test]
    fn readiness_requires_actual_bound_worker_frames_and_recent_control_checkpoint() {
        let status = playing();
        let ready = readiness(&status, "Fixture");
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["control_parameter_known"], false);
        assert_eq!(ready["per_frame_controls_verified"], false);
        assert_eq!(ready["final_ram_acceptance"], false);
        assert_eq!(readiness(&status, "Other")["ready"], false);
        assert_eq!(
            fresh_readiness(&status, "Fixture", Some(2500))["ready"],
            true
        );
        assert_eq!(
            fresh_readiness(&status, "Fixture", Some(2501))["ready"],
            false
        );
        assert_eq!(fresh_readiness(&status, "Fixture", None)["ready"], false);
        assert_eq!(
            presentation("keyboard-color-editor").unwrap(),
            "keyboard-lighting-color-editor"
        );
        assert!(presentation("dashboard").is_err());
        for path in ["worker_enabled", "streaming"] {
            let mut wrong = status.clone();
            wrong["uniform_host"][path] = json!(false);
            assert_eq!(readiness(&wrong, "Fixture")["ready"], false);
        }
        let mut wrong = status.clone();
        wrong["uniform_host"]["accepted_frames"] = json!(2);
        assert_eq!(readiness(&wrong, "Fixture")["ready"], false);
        wrong = status.clone();
        wrong["uniform_host"]["control_observation"]["last_control_check_age_us"] = json!(500001);
        assert_eq!(readiness(&wrong, "Fixture")["ready"], false);
        wrong = status.clone();
        wrong["uniform_host"]["error"] = json!("stopped");
        assert_eq!(readiness(&wrong, "Fixture")["ready"], false);
        wrong = status.clone();
        wrong["uniform_host"]["profile"] = json!("Other");
        assert_eq!(readiness(&wrong, "Fixture")["ready"], false);
    }
    #[test]
    fn late_shutdown_error_cannot_be_masked_by_good_pre_stop_observation() {
        let before = readiness(&playing(), "Fixture");
        let mut after = before.clone();
        after["worker_alive"] = json!(false);
        after["streaming"] = json!(false);
        let good = stop_report(Some((&before, 200)), true, 0, Some(&after));
        assert_eq!(good["worker_was_running"], true);
        assert_eq!(good["last_error"], Value::Null);
        assert_eq!(good["clean_stop"], true);
        after["no_cached_error"] = json!(false);
        let late = stop_report(Some((&before, 200)), true, 0, Some(&after));
        assert!(late["last_error"].is_string());
        assert_eq!(late["post_stop_no_cached_error"], false);
        assert_eq!(
            stop_report(Some((&before, 2501)), true, 0, Some(&after))["ready_before_stop"],
            false
        );
        assert!(stop_report(Some((&before, 200)), false, 0, None)["last_error"].is_string());
        assert_eq!(
            stop_report(Some((&before, 200)), true, 1, Some(&before))["clean_stop"],
            false
        );
    }
}
