//! Explicit bounded physical backend test, not part of ordinary validation.
use super::*;
use std::path::Path;

fn save(path: &Path, value: &Value) {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(&serde_json::to_vec_pretty(value).unwrap())
        .unwrap();
    file.sync_all().unwrap();
}
fn registers(device: &Device) -> Value {
    let mode = device.ornata_mode_readback().unwrap();
    let mut effects = Vec::new();
    let mut brightness = Vec::new();
    for profile in [0, 1] {
        let effect = device.ornata_effect_readback(profile, 0).unwrap();
        let level = device.ornata_brightness_readback(profile, 0).unwrap();
        assert!(effect.selectors_match() && level.selectors_match());
        effects.push(
            json!({"profile":profile,"declared_size":effect.declared_size(),
            "declared_args":&effect.raw_args()[..usize::from(effect.declared_size())]}),
        );
        brightness.push(json!({"profile":profile,"raw":level.raw_brightness()}));
    }
    json!({"mode":mode.mode(),"mode_parameter":mode.param(),"mode_declared_size":mode.declared_size(),
        "effects":effects,"brightness":brightness})
}
fn phase(engine: &Arc<Mutex<Engine>>, key: &str, id: u8, setting: Value, seconds: u64) -> Value {
    let body = serde_json::to_vec(
        &json!({"id":key,"expected_device":key,"expected_profile":"Default",
        "source_effect_id":id,"source_effect_setting":setting}),
    )
    .unwrap();
    let mut locked = engine.lock().unwrap();
    let action_started = std::time::Instant::now();
    let response = post(&mut locked, "/api/source-effect", &body).unwrap();
    let action_ms = action_started.elapsed().as_millis();
    assert_eq!(response["execution"], "host-uniform");
    assert_eq!(response["accepted_packets"], 7);
    assert_eq!(response["attempted_packets"], 7);
    locked.request_lighting_refresh();
    let window = std::time::Instant::now();
    drop(locked);
    std::thread::sleep(Duration::from_secs(seconds));
    let locked = engine.lock().unwrap();
    let elapsed = window.elapsed().as_secs_f64();
    let status = locked.lighting_runtime_status()[key]["uniform_host"].clone();
    let accepted = status["accepted_frames"]
        .as_u64()
        .expect("accepted frame count");
    assert!(
        accepted >= 3,
        "Continuous worker did not progress beyond the initial frame"
    );
    assert!(status["error"].is_null(), "Uniform worker faulted");
    let after = registers(locked.devices().iter().find(|d| d.key() == key).unwrap());
    json!({"source_effect_id":id,"initial_action_ms":action_ms,"initial_response":response,
        "observation_window_seconds":elapsed,"accepted_frames_including_initial":accepted,
        "subsequent_frames_per_second":(accepted-1) as f64/elapsed,"status":status,
        "registers":after,"optically_verified":false})
}

#[test]
#[ignore = "requires explicit physical opt-in and reviewed stock isolation/restart wrapper"]
fn bounded_spectrum_and_breathing_worker_preserve_controls() {
    assert_eq!(
        std::env::var("SYNAPSE_LITE_PHYSICAL_UNIFORM_PROBE").as_deref(),
        Ok("two-programs-v1")
    );
    assert!(std::env::var_os("SECBIRD_SIM").is_none());
    let directory = std::path::PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_CONFIG_DIR").expect("fresh config"),
    );
    assert!(directory.is_absolute() && !directory.join("profiles.json").exists());
    let report = std::path::PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_PHYSICAL_UNIFORM_REPORT").expect("fresh report"),
    );
    assert!(report.is_absolute() && !report.exists());
    let mut engine = Engine::new().unwrap();
    engine.rescan();
    let devices: Vec<_> = engine
        .devices()
        .iter()
        .filter(|d| d.pid == 605 && d.interface == 2)
        .collect();
    assert_eq!(devices.len(), 1);
    assert!(devices[0]
        .path
        .to_string_lossy()
        .to_ascii_lowercase()
        .starts_with("\\\\?\\hid#"));
    crate::host_static::preflight(devices[0]).unwrap();
    let key = devices[0].key();
    let before = registers(devices[0]);
    assert!(before["brightness"][0]["raw"]
        .as_u64()
        .is_some_and(|raw| raw > 0));
    save(
        &report.with_extension("baseline.json"),
        &json!({"before":before,"scope":"readable controls; no RGB or input snapshot"}),
    );
    drop(devices);
    engine.start_lighting_runtime();
    let engine = Arc::new(Mutex::new(engine));
    let mut worker = crate::uniform_worker::Worker::start(engine.clone()).unwrap();
    let ready_deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !engine.lock().unwrap().uniform_is_enabled() {
        assert!(
            std::time::Instant::now() < ready_deadline,
            "Worker readiness timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let spectrum = phase(&engine, &key, 3, json!({}), 2);
    let breathing = phase(
        &engine,
        &key,
        2,
        json!({"isRandom":false,"color1":"#204080","color2":"no-color"}),
        5,
    );
    // Disable while owning Engine after the completed checkpoint. No new batch
    // can begin between this point and the join, and no join holds the mutex.
    engine.lock().unwrap().set_uniform_worker(false, None, None);
    let stopped = worker.stop(Duration::from_secs(2));
    if !stopped {
        save(
            &report,
            &json!({"before":before,"spectrum":spectrum,"breathing":breathing,
            "worker_stopped":false,"cleanup_claimed":false}),
        );
        panic!("Worker did not stop within its bounded join interval");
    }
    let mut locked = engine.lock().unwrap();
    let shutdown_errors = locked.shutdown_lighting_runtime();
    let after = registers(locked.devices().iter().find(|d| d.key() == key).unwrap());
    save(
        &report,
        &json!({"scope":"bounded physical source API and actual off-UI worker; no tray/window",
        "before":before,"spectrum":spectrum,"breathing":breathing,"after_shutdown":after,
        "worker_stopped":stopped,"shutdown_errors":shutdown_errors,
        "controls_unchanged":before==after,"optically_verified":false,"input_recovery_verified":false,
        "cadence_certified":false,"stock_restore":"external wrapper required"}),
    );
    assert!(shutdown_errors.is_empty());
    assert_eq!(before, after);
}
