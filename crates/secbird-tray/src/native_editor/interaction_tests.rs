//! Opt-in real HWND/message-loop checks using only the simulated transport.
//! Run this test alone with --ignored --test-threads=1; it temporarily changes
//! process-local config/smoke environment and opens one bounded native window.
use super::*;
use secbird_core::{commands as cmd, sim::Sim, Store};
use std::time::Instant;

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Environment {
    fn set(values: &[(&'static str, &Path)]) -> Self {
        let saved = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in values {
            std::env::set_var(key, value);
        }
        Self(saved)
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn wait_for(mut predicate: impl FnMut() -> bool, reason: &str) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(4) {
        if predicate() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("Timed out: {reason}"))
}

unsafe fn position(x: f64, y: f64, scale: f64) -> isize {
    let x = (x * scale).round() as i32;
    let y = (y * scale).round() as i32;
    assert!((0..=i16::MAX as i32).contains(&x));
    assert!((0..=i16::MAX as i32).contains(&y));
    (x as u32 | (y as u32) << 16) as isize
}

#[test]
#[ignore = "opens a bounded native window; run alone with an interactive desktop"]
fn native_power_pointer_routes_to_simulator_and_preserves_profile_scope() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-interaction-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-power")),
    ]);
    let sim = Sim::wireless_mouse();
    sim.state().critical_threshold = Some(77);
    let mut engine = Engine::with_simulated_mouse(&sim).unwrap();
    let device = &engine.devices()[0];
    let key = device.key();
    // Exercise the source-faithful native profile; its template5 must not become
    // a profile-local timer when the user changes the Power page's hardware.
    let profile = secbird_core::profile_compat::new_native_profile(196, "Fixture").unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(device);
    profiles.profiles.clear();
    profiles.profiles.insert("Fixture".into(), profile);
    profiles.active = "Fixture".into();
    store.save().unwrap();
    // A defensive rescan must keep this engine isolated from physical devices.
    assert!(engine.rescan().is_empty());
    assert_eq!(engine.devices().len(), 1);
    assert!(engine.event_paths().is_empty());
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let controller_frame = frame.clone();
    let controller_directory = directory.clone();
    let controller_engine = engine.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        wait_for(
            || {
                std::fs::read(&controller_frame)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .is_some_and(|frame| {
                        frame["matched"] == true
                            && frame["presented"] == true
                            && frame["source_power_view"] == true
                    })
            },
            "Power frame",
        )?;
        let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
        if hwnd.is_null() {
            return Err("No native HWND".into());
        }
        unsafe {
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
            let geometry =
                power_layout::geometry(f64::from(client.right) / scale, 128.0, 0.0, false)
                    .ok_or("No Power geometry")?;
            let mut results = Vec::new();
            for (kind, shape, fraction, target) in [
                ("sleep", geometry.sleep.slider, 3.0 / 14.0, 4u16),
                ("threshold", geometry.threshold.slider, 40.0 / 95.0, 45u16),
            ] {
                let setting = if kind == "sleep" {
                    cmd::IDLE_TIME
                } else {
                    cmd::CRITICAL_LEVEL
                };
                let before = controller_sim
                    .state()
                    .log
                    .iter()
                    .filter(|command| **command == setting)
                    .count();
                let start = position(shape.x + 8.0, shape.y + 36.0, scale);
                let end = position(
                    shape.x + 8.0 + fraction * (shape.width - 16.0),
                    shape.y + 36.0,
                    scale,
                );
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
                std::thread::sleep(Duration::from_millis(80));
                let during = controller_sim
                    .state()
                    .log
                    .iter()
                    .filter(|command| **command == setting)
                    .count();
                if during != before {
                    return Err(format!("{kind} wrote before release"));
                }
                SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
                wait_for(
                    || {
                        let state = controller_sim.state();
                        if kind == "sleep" {
                            state.idle_seconds == Some(target * 60)
                        } else {
                            state.critical_threshold
                                == Some(
                                    secbird_core::power::critical_raw_for_percent(target as u8)
                                        .unwrap(),
                                )
                        }
                    },
                    kind,
                )?;
                wait_for(
                    || {
                        Store::load(&controller_directory.join("profiles.json"))
                            .ok()
                            .is_some_and(|store| {
                                kind == "sleep"
                                    || store
                                        .devices
                                        .get(&key)
                                        .and_then(|d| d.power.as_ref())
                                        .and_then(|p| p.critical_threshold_percent)
                                        == Some(target as u8)
                            })
                    },
                    "device intent persisted",
                )?;
                results.push(json!({"control":kind,"value":target,"no_write_before_release":true,"verified_simulated_write":true}));
            }
            let shape = geometry.threshold.slider;
            let at_percent = |percent: u16| {
                position(
                    shape.x + 8.0 + f64::from(percent - 5) / 95.0 * (shape.width - 16.0),
                    shape.y + 36.0,
                    scale,
                )
            };
            // A cancelled native gesture must not turn a later mouseup into a write.
            let before = controller_sim
                .state()
                .log
                .iter()
                .filter(|command| **command == cmd::CRITICAL_LEVEL)
                .count();
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, at_percent(65));
            SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
            SendMessageW(hwnd, WM_LBUTTONUP, 0, at_percent(65));
            std::thread::sleep(Duration::from_millis(80));
            if controller_sim
                .state()
                .log
                .iter()
                .filter(|command| **command == cmd::CRITICAL_LEVEL)
                .count()
                != before
            {
                return Err("Cancelled gesture wrote a threshold".into());
            }
            // Hold the actual worker's engine lock: neither accepted request can
            // finish before close. Releasing it must drain the accepted tail.
            let held_engine = controller_engine
                .lock()
                .map_err(|_| "Worker lock poisoned")?;
            for percent in [50, 55] {
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, at_percent(percent));
                SendMessageW(hwnd, WM_LBUTTONUP, 0, at_percent(percent));
            }
            SendMessageW(hwnd, WM_CLOSE, 0, 0);
            if controller_sim
                .state()
                .log
                .iter()
                .filter(|command| **command == cmd::CRITICAL_LEVEL)
                .count()
                != before
            {
                return Err("Worker bypassed its engine lock".into());
            }
            drop(held_engine);
            Ok(
                json!({"checks":results,"cancelled_gesture_did_not_write":true,"closed_with_worker_blocked":true,"expected_final_threshold_percent":55,"transport":"simulated only","evidence":"Win32 messages through real native window and serialized worker; not OS pointer injection or hardware proof"}),
            )
        }
    });
    let window_result = run_window(engine, Some(Duration::from_secs(12)), None);
    let mut result = controller
        .join()
        .expect("Controller panicked")
        .expect("Interaction check failed");
    window_result.unwrap();
    let store = Store::load(&directory.join("profiles.json")).unwrap();
    let device = store.devices.values().next().unwrap();
    assert!(device.profiles["Fixture"].sleep_minutes.is_none());
    assert_eq!(
        device.power.as_ref().unwrap().critical_threshold_percent,
        Some(55)
    );
    assert_eq!(sim.state().idle_seconds, Some(240));
    assert_eq!(sim.state().critical_threshold, Some(141));
    result["final_threshold_percent_persisted"] =
        json!(device.power.as_ref().unwrap().critical_threshold_percent);
    result["final_threshold_raw"] = json!(sim.state().critical_threshold);
    result["final_sleep_seconds"] = json!(sim.state().idle_seconds);
    result["profile_sleep_override_absent"] =
        json!(device.profiles["Fixture"].sleep_minutes.is_none());
    result["physical_discovery_disabled"] = json!(true);
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Native simulated interaction evidence: {}",
        directory.display()
    );
}
