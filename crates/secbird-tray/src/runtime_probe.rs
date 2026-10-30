//! Explicit development-only active Lighting probe policy and sanitized readiness.
//! No profile creation, opt-in, persistence or hardware access occurs here.
use secbird_core::{Error, Result, Store};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Policy {
    Normal,
    ReadOnlySmoke,
    ActiveLightingProbe,
    ActiveWaveProbe,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeKind {
    Uniform,
    Wave,
}
impl Policy {
    pub(crate) fn from_args(args: &[String]) -> Self {
        if args.iter().any(|arg| arg == "--runtime-smoke") {
            Self::ReadOnlySmoke
        } else if args.iter().any(|arg| arg == WAVE_FLAG) {
            Self::ActiveWaveProbe
        } else if args.iter().any(|arg| arg == FLAG) {
            Self::ActiveLightingProbe
        } else {
            Self::Normal
        }
    }
    pub(crate) fn read_only(self) -> bool {
        self == Self::ReadOnlySmoke
    }
    pub(crate) fn active_probe(self) -> Option<ProbeKind> {
        match self {
            Self::ActiveLightingProbe => Some(ProbeKind::Uniform),
            Self::ActiveWaveProbe => Some(ProbeKind::Wave),
            _ => None,
        }
    }
    pub(crate) fn restricted_actions(self) -> bool {
        self != Self::Normal
    }
    pub(crate) fn starts_worker(self) -> bool {
        !self.read_only()
    }
    pub(crate) fn starts_policies(self) -> bool {
        !self.read_only()
    }
    pub(crate) fn generic_apply(self) -> bool {
        self == Self::Normal
    }
}
pub(crate) fn read_only_duration(args: &[String]) -> std::result::Result<Option<u32>, String> {
    if !args.iter().any(|arg| arg == "--runtime-smoke") {
        return Ok(None);
    }
    if args.len() != 2 || args[0] != "--runtime-smoke" {
        return Err("--runtime-smoke must be used alone with one duration".into());
    }
    args[1]
        .parse::<u32>()
        .ok()
        .filter(|seconds| (1..=600).contains(seconds))
        .map(Some)
        .ok_or_else(|| "--runtime-smoke requires1..600 seconds".into())
}

pub(crate) const FLAG: &str = "--runtime-lighting-probe";
pub(crate) const WAVE_FLAG: &str = "--runtime-wave-probe";

pub(crate) fn duration(args: &[String]) -> std::result::Result<Option<u32>, String> {
    duration_for(args, FLAG)
}
pub(crate) fn wave_duration(args: &[String]) -> std::result::Result<Option<u32>, String> {
    duration_for(args, WAVE_FLAG)
}
fn duration_for(args: &[String], flag: &str) -> std::result::Result<Option<u32>, String> {
    let positions: Vec<_> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == flag)
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
        return Err(format!("{flag} must be used alone with one duration"));
    }
    let seconds = args
        .get(positions[0] + 1)
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|s| (1..=600).contains(s));
    seconds
        .map(Some)
        .ok_or_else(|| format!("{flag} requires 1..600 seconds"))
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
pub(crate) fn presentation_for(
    kind: ProbeKind,
    requested: &str,
) -> std::result::Result<&str, String> {
    if kind == ProbeKind::Wave {
        if requested == "keyboard-lighting" {
            Ok("keyboard-lighting")
        } else {
            Err("Active Wave probe supports only keyboard-lighting".into())
        }
    } else {
        presentation(requested)
    }
}

fn reject(reason: &str) -> Error {
    Error::InvalidArg(format!("Lighting probe rejected: {reason}"))
}

/// Conservative probe-only admission. Normal stores and all import/export paths
/// retain their existing, more general semantics.
pub(crate) fn validate_store(store: &Store) -> Result<(String, String)> {
    validate_store_for(store, ProbeKind::Uniform)
}
pub(crate) fn validate_wave_store(store: &Store) -> Result<(String, String)> {
    validate_store_for(store, ProbeKind::Wave)
}
fn validate_store_for(store: &Store, kind: ProbeKind) -> Result<(String, String)> {
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
        || !profile.app_paths.is_empty()
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
        !matches!(k.as_str(), "entry" | "decoded" | "brightnessBaseline")
            && k != if kind == ProbeKind::Wave {
                "waveHostIntent"
            } else {
                "uniformHostIntent"
            }
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
    if matches!(program, crate::host_uniform::Program::Wave(_)) != (kind == ProbeKind::Wave) {
        return Err(reject(if kind == ProbeKind::Wave {
            "only an exact source Wave program is admitted"
        } else {
            "the uniform probe does not admit Wave"
        }));
    }
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
#[cfg(test)]
pub(crate) fn readiness(runtime: &Value, expected_profile: &str) -> Value {
    readiness_for(ProbeKind::Uniform, runtime, expected_profile)
}
pub(crate) fn readiness_for(kind: ProbeKind, runtime: &Value, expected_profile: &str) -> Value {
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
    let preparation = &uniform["wave_preparation"];
    let authenticated = preparation["authenticated_manifest_sha256"]
        == secbird_core::ornata_spatial::SOURCE_MANIFEST_SHA256
        && preparation["source_byte_len"] == 2_895_966_u64
        && preparation["physical_rows"] == 6
        && preparation["physical_cols"] == 22
        && preparation["logical_rows"] == 7
        && preparation["logical_cols"] == 23
        && preparation["edition_id"] == 0
        && preparation["layout_id"].as_u64().is_some_and(|value| {
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18, 20].contains(&value)
        })
        && preparation["identity_declared_size"]
            .as_u64()
            .is_some_and(|value| (2..=80).contains(&value))
        && preparation["identity_query_completed_by_observation"] == true
        && if preparation["identity_declared_size"] == 2 {
            preparation["firmware_id"].is_null()
        } else {
            preparation["firmware_id"]
                .as_u64()
                .is_some_and(|value| value <= 255)
        }
        && preparation["identity_observation_age_us"]
            .as_u64()
            .zip(preparation["identity_query_span_us"].as_u64())
            .is_some_and(|(age, span)| age >= span)
        && matches!(preparation["source_direction"].as_u64(), Some(1 | 2));
    let supported = if kind == ProbeKind::Wave {
        uniform["source_id"] == 4 && uniform["native_id"] == 2 && authenticated
    } else {
        matches!(uniform["source_id"].as_u64(), Some(2 | 3))
    };
    let ready = owner_matches
        && worker_alive
        && accepted_frames >= 3
        && no_error
        && controls
        && streaming
        && supported;
    let mut result = json!({"ready":ready,"owner_matches":owner_matches,"worker_alive":worker_alive,"streaming":streaming,
        "worker_liveness_sampled_separately":runtime["worker_liveness_sampled_separately"]==true,
        "worker_liveness_scope":if runtime["worker_liveness_sampled_separately"]==true {"live-signal-at-cache-read"}else{"engine-status-snapshot"},
        "accepted_frames":accepted_frames,"controls_checkpoint_ready":controls,
        "controls_checkpoint_ready_at_observation":controls,
        "control_checkpoint_age_us":checkpoint_age,"control_age_at_producer_us":checkpoint_age,
        "current_controls_verified":false,"control_age_scope":"at-producer-observation",
        "control_parameter_known":uniform["parameter_before"].is_number(),
        "per_frame_controls_verified":uniform["custom_frame_control_verified"]==true,"source_id":uniform.get("source_id"),
        "native_id":uniform.get("native_id"),"no_cached_error":no_error,"active_lighting_probe":true,
        "read_only_smoke":false,"final_ram_acceptance":false});
    if kind == ProbeKind::Wave {
        result["active_wave_probe"] = json!(true);
        result["authenticated_layout_ready_at_observation"] = json!(authenticated);
        result["identity_age_at_producer_us"] =
            json!(preparation["identity_observation_age_us"].as_u64());
        result["current_identity_verified"] = json!(false);
        result["identity_age_scope"] = json!("at-producer-observation; not a lease");
        let mut safe = serde_json::Map::new();
        for key in [
            "layout_id",
            "edition_id",
            "firmware_id",
            "identity_declared_size",
            "identity_observation_age_us",
            "identity_query_span_us",
            "source_byte_len",
            "source_direction",
            "physical_rows",
            "physical_cols",
            "logical_rows",
            "logical_cols",
        ] {
            safe.insert(key.into(), json!(preparation[key].as_u64()));
        }
        safe.insert(
            "authenticated_manifest_sha256".into(),
            json!(preparation["authenticated_manifest_sha256"]
                .as_str()
                .filter(
                    |value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                )),
        );
        result["wave_preparation"] = Value::Object(safe);
    }
    result
}

/// A previously valid cached frame is insufficient after observation stalls.
#[cfg(test)]
pub(crate) fn fresh_readiness(runtime: &Value, profile: &str, age_ms: Option<u64>) -> Value {
    fresh_readiness_for(ProbeKind::Uniform, runtime, profile, age_ms)
}
pub(crate) fn fresh_readiness_for(
    kind: ProbeKind,
    runtime: &Value,
    profile: &str,
    age_ms: Option<u64>,
) -> Value {
    let mut status = readiness_for(kind, runtime, profile);
    annotate_age(&mut status, age_ms);
    status["ready"] = json!(status["ready"] == true && age_ms.is_some_and(|age| age <= 2500));
    status
}

fn annotate_age(status: &mut Value, age_ms: Option<u64>) {
    status["status_snapshot_age_ms"] = json!(age_ms);
    status["snapshot_age_ms"] = json!(age_ms);
    // Round upward because the public elapsed age is quantized to milliseconds.
    status["conservative_control_age_now_us"] = json!(status["control_age_at_producer_us"]
        .as_u64()
        .zip(age_ms)
        .map(|(control, elapsed)| control
            .saturating_add(elapsed.saturating_add(1).saturating_mul(1000))
            .saturating_add(1)));
    // This is a bounded cached observation, never a new control GET at draw time.
    status["current_controls_verified"] = json!(false);
    if status.get("identity_age_at_producer_us").is_some() {
        status["conservative_identity_age_now_us"] = json!(status["identity_age_at_producer_us"]
            .as_u64()
            .zip(age_ms)
            .map(|(identity, elapsed)| identity
                .saturating_add(elapsed.saturating_add(1).saturating_mul(1000))
                .saturating_add(1)));
    }
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
    annotate_age(&mut report, age);
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
    Policy::from_args(args).starts_worker()
        && !args.iter().any(|arg| {
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
    fn read_only_smoke_policy_is_distinct_and_refuses_ambiguous_entrypoints() {
        let readonly = args(&["--runtime-smoke", "5"]);
        assert_eq!(Policy::from_args(&readonly), Policy::ReadOnlySmoke);
        assert_eq!(read_only_duration(&readonly).unwrap(), Some(5));
        assert!(Policy::ReadOnlySmoke.read_only());
        assert!(!Policy::ReadOnlySmoke.starts_worker());
        assert!(!Policy::ReadOnlySmoke.starts_policies());
        assert!(!Policy::ReadOnlySmoke.generic_apply());
        assert!(Policy::ActiveLightingProbe.starts_worker());
        assert!(Policy::ActiveLightingProbe.starts_policies());
        assert!(!Policy::ActiveLightingProbe.generic_apply());
        assert!(!Policy::ActiveLightingProbe.read_only());
        assert!(Policy::Normal.generic_apply());
        for values in [
            vec!["--runtime-smoke", "5", "--serve"],
            vec!["--runtime-smoke", "5", "--native-smoke", "5"],
            vec!["--runtime-smoke", "0"],
            vec!["--runtime-smoke", "601"],
            vec!["--runtime-smoke", "5", "--runtime-smoke", "5"],
        ] {
            assert!(read_only_duration(&args(&values)).is_err());
        }
    }
    #[test]
    fn read_only_policy_is_applied_to_startup_callbacks_and_shutdown_source_paths() {
        let source = include_str!("win.rs");
        assert!(source.contains("engine.enforce_measurement_read_only()"));
        assert!(source.contains("if policy.starts_policies()"));
        assert!(source.contains("if !policy.generic_apply()"));
        assert!(source.contains("policy.restricted_actions()"));
        assert!(source.contains("self.policy.read_only()"));
        assert!(source.contains("worker.log_shutdown_diagnostics()"));
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
    fn wave_fixture() -> Store {
        let mut store = fixture();
        let profile = store.devices.values_mut().next().unwrap().active_mut();
        crate::host_uniform::Program::clear_opt_in(profile);
        secbird_core::profile_compat::set_source_quick_effect(
            profile,
            605,
            "Default",
            4,
            &json!({"direction":1}),
        )
        .unwrap();
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(crate::host_wave::Program::Left.native_projection());
        crate::host_uniform::Program::Wave(crate::host_wave::Program::Left)
            .opt_in(profile)
            .unwrap();
        store
    }
    fn wave_playing() -> Value {
        let mut runtime = playing();
        runtime["uniform_host"]["source_id"] = json!(4);
        runtime["uniform_host"]["native_id"] = json!(2);
        runtime["uniform_host"]["wave_preparation"] = json!({
            "authenticated_manifest_sha256":secbird_core::ornata_spatial::SOURCE_MANIFEST_SHA256,
            "source_byte_len":2_895_966,"layout_id":6,"edition_id":0,"firmware_id":null,
            "identity_declared_size":2,"physical_rows":6,"physical_cols":22,"logical_rows":7,"logical_cols":23,
            "identity_query_completed_by_observation":true,"identity_observation_age_us":10_000,"identity_query_span_us":8_000,"source_direction":1
        });
        runtime
    }
    #[test]
    fn wave_flag_is_separate_strict_and_has_only_a_real_wave_presentation() {
        let valid = args(&[WAVE_FLAG, "12"]);
        assert_eq!(wave_duration(&valid).unwrap(), Some(12));
        assert_eq!(duration(&valid).unwrap(), None);
        let policy = Policy::from_args(&valid);
        assert_eq!(policy, Policy::ActiveWaveProbe);
        assert!(policy.restricted_actions());
        assert!(!policy.generic_apply());
        assert!(!policy.read_only());
        assert!(policy.starts_worker() && policy.starts_policies());
        assert_eq!(policy.active_probe(), Some(ProbeKind::Wave));
        for bad in [
            vec![WAVE_FLAG],
            vec![WAVE_FLAG, "0"],
            vec![WAVE_FLAG, "601"],
            vec![WAVE_FLAG, "nan"],
            vec![WAVE_FLAG, "12", FLAG, "12"],
            vec![FLAG, "12", WAVE_FLAG, "12"],
            vec![WAVE_FLAG, "12", "--serve"],
            vec![WAVE_FLAG, "12", "--runtime-smoke", "12"],
            vec![WAVE_FLAG, "12", "--native-smoke", "12"],
            vec![WAVE_FLAG, "12", WAVE_FLAG, "12"],
        ] {
            assert!(wave_duration(&args(&bad)).is_err(), "{bad:?}");
        }
        assert_eq!(
            presentation_for(ProbeKind::Wave, "keyboard-lighting").unwrap(),
            "keyboard-lighting"
        );
        assert!(presentation_for(ProbeKind::Wave, "keyboard-lighting-color-editor").is_err());
        assert!(presentation_for(ProbeKind::Uniform, "keyboard-lighting-color-editor").is_ok());
    }
    #[test]
    fn wave_store_admission_is_exact_and_does_not_broaden_the_uniform_contract() {
        let wave = wave_fixture();
        let before = serde_json::to_value(&wave).unwrap();
        assert!(validate_wave_store(&wave).is_ok());
        assert!(validate_store(&wave).is_err());
        assert!(validate_wave_store(&fixture()).is_err());
        assert_eq!(serde_json::to_value(&wave).unwrap(), before);
        for reason in [
            "speed",
            "direction",
            "marker",
            "macro",
            "apps",
            "sleep",
            "brightness",
            "extra",
        ] {
            let mut changed = wave.clone();
            let profile = changed.devices.values_mut().next().unwrap().active_mut();
            match reason {
                "speed" => {
                    profile.lighting.get_mut("backlight").unwrap().effect =
                        Some(secbird_core::effects::Effect::Wave {
                            reverse: false,
                            speed: 2,
                        })
                }
                "direction" => {
                    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]
                        ["quickEffects"]["selectedEffectSetting"]["direction"] = json!(2)
                }
                "marker" => crate::host_uniform::Program::clear_opt_in(profile),
                "macro" => {
                    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["macro"] =
                        json!({"action":[1]})
                }
                "apps" => profile.apps.push("game.exe".into()),
                "sleep" => profile.sleep_minutes = Some(1),
                "brightness" => {
                    profile
                        .lighting_settings
                        .as_mut()
                        .unwrap()
                        .brightness
                        .as_mut()
                        .unwrap()
                        .enabled = false
                }
                _ => {
                    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["uniformHostIntent"] =
                        json!({"source_id":3})
                }
            }
            assert!(validate_wave_store(&changed).is_err(), "{reason}");
        }
    }
    #[test]
    fn wave_readiness_needs_authenticated_worker_preparation_not_only_an_old_ready_flag() {
        let wave = wave_playing();
        let ready = readiness_for(ProbeKind::Wave, &wave, "Fixture");
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["authenticated_layout_ready_at_observation"], true);
        assert_eq!(readiness(&wave, "Fixture")["ready"], false);
        assert_eq!(
            readiness_for(ProbeKind::Wave, &playing(), "Fixture")["ready"],
            false
        );
        for (field, value) in [
            ("authenticated_manifest_sha256", json!("0".repeat(64))),
            ("source_byte_len", json!(1)),
            ("layout_id", json!(13)),
            ("identity_declared_size", json!(1)),
            ("firmware_id", json!(9)),
            ("identity_query_completed_by_observation", json!(false)),
            ("identity_observation_age_us", Value::Null),
            ("identity_observation_age_us", json!(7_999)),
            ("identity_query_span_us", json!(10_001)),
            ("identity_query_span_us", json!("8000")),
            ("identity_observation_age_us", json!(-1)),
            ("source_direction", json!(5)),
        ] {
            let mut wrong = wave.clone();
            wrong["uniform_host"]["wave_preparation"][field] = value;
            assert_eq!(
                readiness_for(ProbeKind::Wave, &wrong, "Fixture")["ready"],
                false,
                "{field}"
            );
        }
        let fresh = fresh_readiness_for(ProbeKind::Wave, &wave, "Fixture", Some(1200));
        assert_eq!(fresh["conservative_identity_age_now_us"], 1_211_001);
        assert_eq!(fresh["current_identity_verified"], false);
        assert_eq!(
            fresh_readiness_for(ProbeKind::Wave, &wave, "Fixture", Some(2501))["ready"],
            false
        );
        assert_eq!(
            fresh_readiness_for(ProbeKind::Wave, &wave, "Fixture", None)["ready"],
            false
        );
        let mut three = wave.clone();
        three["uniform_host"]["wave_preparation"]["identity_declared_size"] = json!(3);
        assert_eq!(
            readiness_for(ProbeKind::Wave, &three, "Fixture")["ready"],
            false
        );
        three["uniform_host"]["wave_preparation"]["firmware_id"] = json!(255);
        assert_eq!(
            readiness_for(ProbeKind::Wave, &three, "Fixture")["ready"],
            true
        );
        let mut private = wave.clone();
        private["uniform_host"]["wave_preparation"]["private_path"] = json!("private fixture path");
        private["uniform_host"]["wave_preparation"]["raw_json"] = json!({"private":"data"});
        let public = readiness_for(ProbeKind::Wave, &private, "Fixture");
        assert_eq!(public["wave_preparation"].as_object().unwrap().len(), 13);
        assert!(public["wave_preparation"].get("private_path").is_none());
        assert!(public["wave_preparation"].get("raw_json").is_none());
        let mut stopped = ready.clone();
        stopped["worker_alive"] = json!(false);
        let stopped = stop_report(Some((&ready, 100)), true, 0, Some(&stopped));
        assert_eq!(stopped["clean_stop"], true);
        assert_eq!(stopped["ready_before_stop"], true);
        assert_eq!(stopped["source_id"], 4);
        assert_eq!(stopped["native_id"], 2);
        assert_eq!(stopped["authenticated_layout_ready_at_observation"], true);
    }
    #[test]
    #[ignore = "requires explicit SECBIRD_WAVE_PROBE_SEED_DIR with private synthetic derivative seeds; no hardware"]
    fn explicitly_selected_private_synthetic_wave_seeds_admit_without_printing_identity() {
        let folder = std::env::var_os("SECBIRD_WAVE_PROBE_SEED_DIR")
            .expect("explicit synthetic seed directory");
        for name in ["left", "right"] {
            let store = Store::load(&std::path::Path::new(&folder).join(format!("{name}.json")))
                .expect("synthetic seed loads");
            if let Err(error) = validate_wave_store(&store) {
                panic!("Synthetic derivative {name} failed: {error}");
            }
            assert!(validate_store(&store).is_err());
            println!(
                "Synthetic derivative {name}: Wave admission passed; uniform admission refused"
            );
        }
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
        let cached = fresh_readiness(&status, "Fixture", Some(1200));
        assert_eq!(
            cached["ready"], true,
            "producer checkpoint plus bounded snapshot age is admission evidence"
        );
        assert_eq!(cached["control_age_at_producer_us"], 100000);
        assert_eq!(cached["conservative_control_age_now_us"], 1301001);
        assert_eq!(cached["current_controls_verified"], false);
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
