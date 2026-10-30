//! Explicit, one-action physical probe. Never selected by the ordinary test suite.
//! The external wrapper isolates the stock user owner and restarts it in finally.
use super::*;

fn registers(device: &Device) -> Result<Value, Error> {
    let mode = device.ornata_mode_readback()?;
    let mut effects = Vec::new();
    let mut brightness = Vec::new();
    for profile in [0, 1] {
        let effect = device.ornata_effect_readback(profile, 0)?;
        let level = device.ornata_brightness_readback(profile, 0)?;
        if !effect.selectors_match() || !level.selectors_match() {
            return Err(Error::InvalidArg(
                "Physical baseline selectors differ".into(),
            ));
        }
        effects.push(
            json!({"profile":profile,"declared_size":effect.declared_size(),
            "declared_args":&effect.raw_args()[..usize::from(effect.declared_size())]}),
        );
        brightness.push(json!({"profile":profile,"raw":level.raw_brightness()}));
    }
    Ok(json!({"mode":mode.mode(),"mode_parameter":mode.param(),
        "mode_declared_size":mode.declared_size(),"effects":effects,"brightness":brightness}))
}

fn save_new(path: &std::path::Path, value: &Value) {
    let bytes = serde_json::to_vec_pretty(value).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
#[ignore = "requires reviewed external stock isolation, fresh config and explicit physical opt-in"]
fn one_explicit_static_action_preserves_readable_registers() {
    assert_eq!(
        std::env::var("SYNAPSE_LITE_PHYSICAL_STATIC_PROBE").as_deref(),
        Ok("one-frame-v1")
    );
    assert!(
        std::env::var_os("SECBIRD_SIM").is_none(),
        "Physical probe refuses simulator configuration"
    );
    let directory = std::path::PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_CONFIG_DIR").expect("fresh config required"),
    );
    assert!(directory.is_absolute());
    assert!(
        !directory.join("profiles.json").exists(),
        "Physical probe requires an unsaved native store"
    );
    let report = std::path::PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_PHYSICAL_STATIC_REPORT").expect("report required"),
    );
    assert!(report.is_absolute() && !report.exists());
    let baseline = report.with_extension("baseline.json");
    assert!(!baseline.exists());

    let mut engine = Engine::new().unwrap();
    engine.rescan();
    let targets: Vec<_> = engine
        .devices()
        .iter()
        .filter(|d| d.pid == 605 && d.interface == 2)
        .collect();
    assert_eq!(
        targets.len(),
        1,
        "Exactly one physical Ornata interface2 required"
    );
    let key = targets[0].key();
    assert!(
        targets[0]
            .path
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("\\\\?\\hid#"),
        "Physical Windows HID path required"
    );
    crate::host_static::preflight(targets[0]).unwrap();
    let before = registers(targets[0]).unwrap();
    assert!(before["brightness"][0]["raw"]
        .as_u64()
        .is_some_and(|v| v > 0));
    save_new(
        &baseline,
        &json!({"scope":"readable controls, not RGB buffer or animation phase", "before":before}),
    );
    drop(targets);

    engine.start_lighting_runtime();
    let body = serde_json::to_vec(&json!({"id":key,"expected_device":key,
        "expected_profile":"Default","source_effect_id":1,
        "source_effect_setting":{"color1":"#102030"}}))
    .unwrap();
    let response = post(&mut engine, "/api/source-effect", &body).unwrap();
    assert_eq!(response["execution"], "host-static");
    assert_eq!(response["accepted_packets"], 7);
    assert_eq!(response["attempted_packets"], 7);
    assert_eq!(response["usb_out_complete"], true);
    assert_eq!(response["control_state_verified"], true);
    let store = Store::load(&directory.join("profiles.json")).unwrap();
    assert_eq!(store.devices[&key].active, "Default");
    assert_eq!(
        store.devices[&key].active().source_static_color(605),
        Some(Rgb(16, 32, 48))
    );
    let provenance =
        secbird_core::profile_compat::source_brightness_baseline(store.devices[&key].active())
            .unwrap();
    assert_eq!(provenance["retained_intent_known"], false);
    engine.request_lighting_refresh();
    let after_refresh =
        registers(engine.devices().iter().find(|d| d.key() == key).unwrap()).unwrap();
    let shutdown_errors = engine.shutdown_lighting_runtime();
    let after_shutdown =
        registers(engine.devices().iter().find(|d| d.key() == key).unwrap()).unwrap();
    save_new(
        &report,
        &json!({"scope":"one physical source Static API action, fresh native profile, one policy refresh and shutdown",
        "requested_color":[16,32,48],"response":response,"before":before,
        "after_refresh":after_refresh,"after_shutdown":after_shutdown,
        "refresh_registers_unchanged":before==after_refresh,"shutdown_registers_unchanged":before==after_shutdown,
        "shutdown_errors":shutdown_errors,"saved_native_color":true,"baseline_retained_intent_known":false,
        "optically_verified":false,"input_recovery_verified":false,"stock_restore":"external wrapper required"}),
    );
    assert!(shutdown_errors.is_empty());
    assert_eq!(before, after_refresh);
    assert_eq!(before, after_shutdown);
}
