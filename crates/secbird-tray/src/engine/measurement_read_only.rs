//! Whole-lifecycle regression for an existing populated native Store.
use super::*;
use secbird_core::{effects::Effect, sim::Sim, Profile, Rgb, Zone};
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn populated_store_callbacks_dispatch_and_direct_device_paths_issue_no_set() {
    let keyboard = Sim::keyboard();
    let mouse = Sim::mouse();
    let folder = std::env::temp_dir().join(format!(
        "readonly-lifecycle-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = folder.join("profiles.json");
    let mut engine = Engine::with_simulated_keyboard(&keyboard, path.clone()).unwrap();
    let db = Db::builtin().unwrap();
    engine
        .devices
        .push(mouse.open(196, db.get(196).cloned()).unwrap());
    let performance = secbird_core::profile_compat::new_native_profile(196, "Synthetic")
        .unwrap()
        .performance;
    let mut store = Store::load(&path).unwrap();
    for device in &engine.devices {
        let profile = Profile {
            dpi: Some((1600, 1800)),
            polling: Some(1000),
            dpi_stages: Some(Default::default()),
            performance: performance.clone(),
            sleep_minutes: Some(3),
            auto_sleep: Some(true),
            dimming: Some("half".into()),
            game_mode: Some("win,alt-tab".into()),
            keys: vec![Rgb(10, 20, 30); 132],
            apps: vec!["synthetic-game.exe".into()],
            buttons: std::collections::BTreeMap::from([("side-forward".into(), "key:a".into())]),
            lighting: std::collections::BTreeMap::from([(
                "backlight".into(),
                Zone {
                    color: Some(Rgb(1, 2, 3)),
                    brightness: Some(73),
                    effect: Some(Effect::Spectrum),
                    ..Default::default()
                },
            )]),
            lighting_settings: Some(secbird_core::lighting::LightingSettings {
                brightness: Some(secbird_core::lighting::Brightness {
                    enabled: true,
                    value: 73,
                    ..Default::default()
                }),
                switch_off: Some(secbird_core::lighting::SwitchOffLighting {
                    idle_enabled: true,
                    display_off: true,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let profiles = store.for_device(device);
        profiles.profiles.insert("Default".into(), profile);
        profiles.profiles.insert("Other".into(), Profile::default());
        profiles.onboard = Some(secbird_core::profile::OnboardSave {
            slot: 1,
            profile: "Default".into(),
            ..Default::default()
        });
        profiles.power = Some(secbird_core::profile::DevicePower {
            critical_threshold_percent: Some(20),
            ..Default::default()
        });
    }
    store.save().unwrap();
    let before = std::fs::read(&path).unwrap();
    engine.enforce_measurement_read_only().unwrap();
    assert!(engine.devices.iter().all(Device::is_query_only));
    keyboard.state().log.clear();
    mouse.state().log.clear();
    let keys = engine.all_keys();
    engine.rescan();
    engine.start_lighting_runtime();
    assert!(!engine.lighting_runtime.borrow().running);
    assert!(engine.apply(&keys).is_empty());
    assert!(engine
        .set_foreground(Some("synthetic-game.exe".into()))
        .is_empty());
    engine.lights_away(50);
    engine.set_lighting_display_off(true);
    engine.refresh_lighting_runtime();
    engine.request_lighting_refresh();
    engine.refresh_frames();
    engine.refresh_battery_lights();
    engine.keep_laptops_lit();
    assert!(engine.lights_back().is_empty());
    engine.retry_lights_back();
    assert!(engine.restore_lost_buttons(&HashSet::new()).is_empty());
    assert!(engine.restore_buttons().is_empty());
    assert!(engine.switch_profile(&keys[0], "Other").is_err());
    engine.dpi_changed(&keys[0], 800, 800);
    for route in [
        "/api/dpi",
        "/api/polling",
        "/api/button",
        "/api/macro",
        "/api/device-sleep",
        "/api/critical-threshold",
        "/api/profile",
        "/api/import",
        "/api/sync",
        "/api/settings",
        "/api/update-install",
        "/api/rescan",
        "/api/source-effect",
    ] {
        assert!(
            crate::server::dispatch(&mut engine, "POST", route, b"{}").is_err(),
            "{route}"
        );
    }
    assert!(crate::server::import_profile_document(&mut engine, b"{}", None).is_err());
    assert!(
        crate::server::import_profile_document_bound(&mut engine, b"{}", &keys[0], "Default")
            .is_err()
    );
    assert!(keyboard.state().log.is_empty());
    assert!(mouse.state().log.is_empty());
    // GET snapshots/listeners remain useful. Direct setters are independently
    // denied even if a future caller accidentally bypasses a callback guard.
    for device in engine.devices() {
        assert!(device.firmware_version().is_ok());
        assert!(device
            .transact(secbird_core::proto::Command::new(4, 5), 0, &[])
            .is_err());
    }
    assert!(crate::server::dispatch(&mut engine, "GET", "/api/live", b"").is_ok());
    assert!(engine.shutdown_lighting_runtime().is_empty());
    assert!(keyboard
        .state()
        .log
        .iter()
        .all(|command| command.id & 0x80 != 0));
    assert!(mouse
        .state()
        .log
        .iter()
        .all(|command| command.id & 0x80 != 0));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let state = crate::server::dispatch(&mut engine, "GET", "/api/state", b"").unwrap();
    assert_eq!(state["measurement_read_only"], true);
    assert_eq!(state["query_only_device_handles"], true);
    let shared = std::sync::Arc::new(std::sync::Mutex::new(engine));
    assert!(crate::uniform_worker::Worker::start(shared.clone()).is_err());
    assert!(!shared.lock().unwrap().uniform_is_enabled());
    std::fs::remove_dir_all(folder).unwrap();
}
