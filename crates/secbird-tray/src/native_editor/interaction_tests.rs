//! Opt-in real HWND/message-loop checks using only the simulated transport.
//! Run this test alone with --ignored --test-threads=1; it temporarily changes
//! process-local config/smoke environment and opens one bounded native window.
#[path="calibration_interaction_tests.rs"]
mod calibration;
#[path="onboard_interaction_tests.rs"]
mod onboard;
#[path="advanced_interaction_tests.rs"]
mod advanced;
#[path="wheel_interaction_tests.rs"]
mod wheel;
#[path="linked_games_interaction_tests.rs"]
mod linked_games;
#[path="macro_interaction_tests.rs"]
mod macros;
#[path="shortcut_interaction_tests.rs"]
mod shortcuts;
#[path="macro_xml_interaction_tests.rs"]
mod macro_xml;
#[path="macro_playback_interaction_tests.rs"]
mod macro_playback;
#[path="macro_random_delay_interaction_tests.rs"]
mod macro_random_delay;
#[path="studio_interaction_tests.rs"]
mod studio;
#[path="chroma_interaction_tests.rs"]
mod chroma;
#[path="settings_interaction_tests.rs"]
mod settings;
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
            // Actual message dispatch with the same readonly State policy set
            // by --runtime-smoke, before reverting to normal interaction checks.
            let before_log = controller_sim.state().log.len();
            let before_store = std::fs::read(controller_directory.join("profiles.json")).unwrap();
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
            let shape = geometry.threshold.slider;
            let start = position(shape.x + 8.0, shape.y + 36.0, scale);
            let end = position(shape.x + shape.width - 8.0, shape.y + 36.0, scale);
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
            SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
            SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
            std::thread::sleep(Duration::from_millis(80));
            if controller_sim.state().log.len() != before_log
                || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                    != before_store
            {
                return Err("Readonly native slider changed device or profile data".into());
            }
            let thread = GetWindowThreadProcessId(hwnd, null_mut());
            let mut gui: GUITHREADINFO = std::mem::zeroed();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            GetGUIThreadInfo(thread, &mut gui);
            if gui.hwndCapture == hwnd {
                return Err("Readonly source control started pointer capture".into());
            }
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
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
                results.push(json!({"readonly_native_messages_no_io_or_save":true,"control":kind,"value":target,"no_write_before_release":true,"verified_simulated_write":true}));
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

unsafe fn redraw(hwnd: HWND) {
    InvalidateRect(hwnd, null(), 0);
    UpdateWindow(hwnd);
}
unsafe fn lighting_click(hwnd: HWND, x: f64, y: f64, scale: f64) {
    let point = position(x, y, scale);
    SendMessageW(hwnd, WM_LBUTTONDOWN, 1, point);
    SendMessageW(hwnd, WM_LBUTTONUP, 0, point);
    redraw(hwnd);
}

unsafe fn native_click(hwnd: HWND, x: f64, y: f64, scale: f64) {
    let point = position(x, y, scale);
    wait_for(
        || SendMessageW(hwnd, RUNTIME_TEST_QUERY, 9, point) == 1,
        "UI-thread click batch accepted",
    ).expect("Native click failed");
}

unsafe fn native_send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) {
    let request = (message, wparam, lparam);
    wait_for(
        || SendMessageW(hwnd, RUNTIME_TEST_QUERY, 10,
            &request as *const (u32, usize, isize) as isize) == 1,
        "UI-thread test message accepted",
    ).expect("Native message failed");
}

// A synchronous sent message may reenter paint/rebuild while State is borrowed.
// The test port explicitly acknowledges a snapshot; an unavailable snapshot is
// never interpreted as absent geometry or an idle worker.
unsafe fn native_query(hwnd: HWND, message: u32, wparam: usize) -> Result<Value, String> {
    let mut value = Value::Null;
    let mut attempts = 0;
    wait_for(|| {
        attempts += 1;
        value = Value::Null;
        SendMessageW(hwnd, message, wparam, &mut value as *mut Value as isize);
        value.is_object()
    }, "Native state snapshot available")?;
    if attempts > 1 {
        eprintln!("Native query {message} retried {} busy snapshots", attempts - 1);
    }
    Ok(value)
}

unsafe fn acknowledge_and_drain(hwnd: HWND) -> bool {
    acknowledge_owned_failure(hwnd);
    native_query(hwnd, RUNTIME_TEST_QUERY, 0).is_ok_and(|v|
        v["pending"] == 0 && v["queued"] == 0 && v["failures"] == false)
}
unsafe fn edit_text(hwnd: HWND, id: u16) -> Option<String> {
    let edit = GetDlgItem(hwnd, i32::from(id));
    if edit.is_null() || IsWindowVisible(edit) == 0 {
        return None;
    }
    let mut text = vec![0u16; GetWindowTextLengthW(edit).max(0) as usize + 1];
    let len = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32);
    Some(String::from_utf16_lossy(&text[..len.max(0) as usize]))
}
/// Acknowledge only the failure dialog owned by this test's editor HWND.
unsafe fn acknowledge_owned_failure(hwnd: HWND) -> bool {
    let dialog = FindWindowW(wide("#32770").as_ptr(), wide(FAILURE_TITLE).as_ptr());
    if dialog.is_null() || GetWindow(dialog, GW_OWNER) != hwnd {
        return false;
    }
    let button = FindWindowExW(dialog, null_mut(), wide("Button").as_ptr(), null());
    if button.is_null() {
        return false;
    }
    PostMessageW(
        dialog,
        WM_COMMAND,
        GetDlgCtrlID(button) as usize,
        button as isize,
    );
    true
}
fn brightness_write_count(sim: &Sim) -> usize {
    sim.state()
        .log
        .iter()
        .filter(|command| **command == cmd::MATRIX_BRIGHTNESS || **command == cmd::LED_BRIGHTNESS)
        .count()
}

#[test]
#[ignore = "opens a bounded native Lighting window; simulated transport only, run alone"]
fn native_lighting_messages_commit_release_cancel_and_isolate_color_drafts() {
    use secbird_core::lighting::{Brightness, LightingSettings, SwitchOffLighting};
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-lighting-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-lighting")),
    ]);
    let sim = Sim::keyboard();
    let mut engine =
        Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    let device = &engine.devices()[0];
    assert_eq!(device.pid, 605);
    assert!(device.path.as_bytes().starts_with(b"sim:"));
    let key = device.key();
    device.set_brightness(5, 60).unwrap();
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    LightingSettings {
        brightness: Some(Brightness {
            value: 60,
            ..Default::default()
        }),
        switch_off: Some(SwitchOffLighting::default()),
        ..Default::default()
    }
    .store_in_profile(&mut profile)
    .unwrap();
    secbird_core::profile_compat::set_source_quick_effect(
        &mut profile,
        605,
        "Fixture",
        1,
        &json!({"color1":"#00ff00"}),
    )
    .unwrap();
    let opaque = json!({"color":"rgb(7, 8, 9)","future":{"keep":[1,2]},"picked":true});
    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["quickEffects"]
        ["customColors"] = json!([
        opaque.clone(),{"color":"#112233","picked":true},{"color":"#445566","picked":true}
    ]);
    let untouched = secbird_core::profile_compat::new_native_profile(605, "Other").unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(device);
    profiles.profiles.clear();
    profiles.profiles.insert("Fixture".into(), profile);
    profiles.profiles.insert("Other".into(), untouched.clone());
    profiles.active = "Fixture".into();
    store.save().unwrap();
    assert!(engine.rescan().is_empty());
    assert_eq!(engine.devices().len(), 1);
    assert!(engine.event_paths().is_empty());
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let controller_directory = directory.clone();
    let controller_key = key.clone();
    let controller_opaque = opaque.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| {
                            // The generic keyboard simulator has LED5, not the physical
                            // Ornata region0 alias. This fixture supplies profile intent;
                            // physical smoke matched/readback checks remain stricter.
                            f["presented"] == true
                                && f["source_lighting_view"] == true
                                && f["device_kind"] == "keyboard"
                                && f["tab"] == "LIGHTING"
                                && f["lighting_intent_origin"]["source"] == "profile-intent"
                                && f["lighting_intent_origin"]["retained_value_known"] == true
                        })
                },
                "Lighting frame",
            )?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            if hwnd.is_null() {
                return Err("No Lighting HWND".into());
            }
            unsafe {
                // Real WM_SIZE/layout/paint; wide enough for the source two-column mode.
                let requested_scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    (1320.0 * requested_scale).round() as i32,
                    (900.0 * requested_scale).round() as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                redraw(hwnd);
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let width = f64::from(client.right) / scale;
                if width < 1280.0 {
                    return Err("Interactive desktop constrained source two-column fixture".into());
                }
                let geometry = lighting_layout::geometry(width, 128.0, 0.0, 1)
                    .ok_or("No Lighting geometry")?;
                let shape = geometry.brightness_slider;
                let point = |percent: f64| {
                    position(
                        shape.x + 8.0 + percent / 100.0 * (shape.width - 16.0),
                        shape.y + 36.0,
                        scale,
                    )
                };
                let before = brightness_write_count(&controller_sim);
                native_send(hwnd, WM_LBUTTONDOWN, 1, point(60.0));
                let thread = GetWindowThreadProcessId(hwnd, null_mut());
                let mut gui: GUITHREADINFO = std::mem::zeroed();
                gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
                GetGUIThreadInfo(thread, &mut gui);
                if gui.hwndCapture != hwnd {
                    return Err(format!("Brightness press did not capture HWND; client={}x{}, scale={}, source track={:?}, capture={:?}",client.right,client.bottom,scale,lighting_layout::track(shape),gui.hwndCapture));
                }
                native_send(hwnd, WM_MOUSEMOVE, 1, point(45.0));
                // Inject through the same cached collector, then force the queued
                // dialog message to run while a real HWND capture is still held.
                let runtime = json!({"profile":"Fixture","uniform_host":{"profile":"Fixture","state":"faulted","error":"simulated background fault"}});
                let cached_log = controller_sim.state().log.len();
                let cached_store =
                    std::fs::read(controller_directory.join("profiles.json")).unwrap();
                let mut before_feedback = Value::Null;
                native_send(
                    hwnd,
                    RUNTIME_TEST_QUERY,
                    0,
                    &mut before_feedback as *mut Value as isize,
                );
                native_send(
                    hwnd,
                    RUNTIME_TEST_INJECT,
                    0,
                    &runtime as *const Value as isize,
                );
                PostMessageW(hwnd, FAILURE_DIALOG, 1, 0);
                std::thread::sleep(Duration::from_millis(80));
                if brightness_write_count(&controller_sim) != before {
                    return Err("Brightness wrote before release".into());
                }
                let mut observed = Value::Null;
                native_send(
                    hwnd,
                    RUNTIME_TEST_QUERY,
                    0,
                    &mut observed as *mut Value as isize,
                );
                GetGUIThreadInfo(thread, &mut gui);
                if observed["status"] != before_feedback["status"]
                    || observed["failed"] != before_feedback["failed"]
                    || observed["pending"] != before_feedback["pending"]
                {
                    return Err(format!(
                        "Cached reply changed foreground feedback: {before_feedback} -> {observed}"
                    ));
                }
                if gui.hwndCapture != hwnd
                    || observed["brightness"] != 45
                    || observed["deferred"] != true
                {
                    return Err(format!(
                        "Background failure interrupted draft/capture: {observed}"
                    ));
                }
                if controller_sim.state().log.len() != cached_log
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                        != cached_store
                {
                    return Err("Cached status performed device IO or profile persistence".into());
                }
                native_send(hwnd, WM_LBUTTONUP, 0, point(45.0));
                native_send(
                    hwnd,
                    RUNTIME_TEST_QUERY,
                    0,
                    &mut observed as *mut Value as isize,
                );
                PostMessageW(hwnd, WM_TIMER, CACHED_LIGHTING_TIMER, 0);
                wait_for(
                    || acknowledge_and_drain(hwnd),
                    "Deferred runtime failure acknowledged after release",
                )?;
                wait_for(
                || {
                    Store::load(&controller_directory.join("profiles.json"))
                        .ok()
                        .is_some_and(|s| {
                            s.devices
                                .get(&controller_key)
                                .and_then(|d| {
                                    LightingSettings::from_profile(d.active()).ok().flatten()
                                })
                                .and_then(|s| s.brightness)
                                .is_some_and(|b| b.value == 45)
                        })
                },
                "Released brightness persisted",
            ).map_err(|error| {
                let diagnostic=json!({"failure":error,"sim_commands":format!("{:?}",controller_sim.state().log),"sim_brightness":format!("{:?}",controller_sim.state().brightness),"store":std::fs::read_to_string(controller_directory.join("profiles.json")).unwrap_or_default()});
                let _=std::fs::write(controller_directory.join("failure.json"),serde_json::to_vec_pretty(&diagnostic).unwrap());
                format!("{error}; diagnostics: {}",controller_directory.join("failure.json").display())
            })?;
                if brightness_write_count(&controller_sim) <= before {
                    return Err("Release did not reach simulated transport".into());
                }
                if controller_sim.state().brightness.get(&5) != Some(&114) {
                    return Err("Unexpected simulated brightness raw value".into());
                }
                // User operations reset foreground dedup; an identical background
                // fault still must stay acknowledged independently of that reset.
                native_send(
                    hwnd,
                    RUNTIME_TEST_INJECT,
                    1,
                    &runtime as *const Value as isize,
                );
                native_send(
                    hwnd,
                    RUNTIME_TEST_QUERY,
                    0,
                    &mut observed as *mut Value as isize,
                );
                if observed["failures"] != false || observed["deferred"] != false {
                    return Err(format!(
                        "Acknowledged background fault repeated: {observed}"
                    ));
                }
                let after_release = brightness_write_count(&controller_sim);
                native_send(hwnd, WM_LBUTTONDOWN, 1, point(80.0));
                native_send(hwnd, WM_CANCELMODE, 0, 0);
                native_send(hwnd, WM_LBUTTONUP, 0, point(80.0));
                std::thread::sleep(Duration::from_millis(80));
                if brightness_write_count(&controller_sim) != after_release {
                    return Err("Cancelled brightness gesture wrote".into());
                }
                redraw(hwnd);
                let anchor = geometry.colors[0].dropdown;
                let palette = color_editor_layout::geometry(
                    color_editor_layout::Rect {
                        x: anchor.x,
                        y: anchor.y,
                        width: anchor.width,
                        height: anchor.height,
                    },
                    f64::from(client.bottom) / scale,
                )
                .ok_or("No palette geometry")?;
                native_click(hwnd, anchor.x + 15.0, anchor.y + 12.0, scale);
                let open_edit = |slot: usize| {
                    let swatch = palette.custom_slots[slot];
                    native_send(
                        hwnd,
                        WM_RBUTTONUP,
                        0,
                        position(swatch.x + 10.0, swatch.y + 10.0, scale),
                    );
                    redraw(hwnd);
                    let menu = color_editor_layout::context_menu(swatch);
                    native_click(hwnd, menu.x + 20.0, menu.y + 13.0, scale);
                };
                open_edit(1);
                wait_for(
                    || edit_text(hwnd, lighting_view::HEX).as_deref() == Some("112233"),
                    "First slot native EDIT",
                )?;
                let edit = GetDlgItem(hwnd, i32::from(lighting_view::HEX));
                SetWindowTextW(edit, wide("abcdef").as_ptr());
                if edit_text(hwnd, lighting_view::HEX).as_deref() != Some("abcdef") {
                    return Err("HEX message draft not accepted".into());
                }
                native_click(
                    hwnd,
                    palette.cancel.x + 40.0,
                    palette.cancel.y + 13.0,
                    scale,
                );
                open_edit(2);
                wait_for(
                    || edit_text(hwnd, lighting_view::HEX).as_deref() == Some("445566"),
                    "Second slot must not inherit first draft",
                )?;
                // Exercise acknowledged failure feedback through a real source control.
                // The native eyedropper is explicitly unavailable; no capture is started.
                native_click(
                    hwnd,
                    palette.eyedropper.x + 12.0,
                    palette.eyedropper.y + 12.0,
                    scale,
                );
                wait_for(
                    || acknowledge_and_drain(hwnd),
                    "Acknowledged native failure dialog",
                )?;
                let edit = GetDlgItem(hwnd, i32::from(lighting_view::HEX));
                SetWindowTextW(edit, wide("a1b2c3").as_ptr());
                native_click(hwnd, palette.save.x + 40.0, palette.save.y + 13.0, scale);
                wait_for(
                    || {
                        Store::load(&controller_directory.join("profiles.json"))
                            .ok()
                            .is_some_and(|s| {
                                s.devices
                                    .get(&controller_key)
                                    .and_then(|d| {
                                        secbird_core::profile_compat::source_quick_effects(
                                            d.active(),
                                        )
                                    })
                                    .and_then(|q| q["customColors"].as_array())
                                    .is_some_and(|slots| {
                                        slots.len() == 3
                                            && slots[0] == controller_opaque
                                            && slots[1]["color"] == "#112233"
                                            && slots[2]["color"] == "#a1b2c3"
                                    })
                            })
                    },
                    "Saved slot and unchanged opaque slot persisted",
                )?;
                // Save first persists the custom slot, then applies its effect.
                // Disk visibility of the first operation is not worker completion.
                // Finish/acknowledge that accepted tail before measuring close.
                wait_for(|| {
                    acknowledge_and_drain(hwnd)
                }, "Color Save worker and feedback drained")?;
                if brightness_write_count(&controller_sim) != after_release {
                    return Err("Color editor unexpectedly changed brightness".into());
                }
                // An acknowledged background failure must not prevent closing
                // while a real color EDIT still owns focus and an unsaved draft.
                native_click(hwnd, anchor.x + 15.0, anchor.y + 12.0, scale);
                open_edit(2);
                wait_for(
                    || edit_text(hwnd, lighting_view::HEX).as_deref() == Some("a1b2c3"),
                    "Color editor reopened before focused close",
                )?;
                let focused_fault = json!({"profile":"Fixture","uniform_host":{"profile":"Fixture","error":"simulated focused close fault"}});
                let unsaved_edit = GetDlgItem(hwnd, i32::from(lighting_view::HEX));
                SetWindowTextW(unsaved_edit, wide("fedcba").as_ptr());
                if edit_text(hwnd, lighting_view::HEX).as_deref() != Some("fedcba") {
                    return Err("Unsaved close draft was not accepted".into());
                }
                let before_close_log = controller_sim.state().log.len();
                let before_close_store =
                    std::fs::read(controller_directory.join("profiles.json")).unwrap();
                native_send(
                    hwnd,
                    RUNTIME_TEST_INJECT,
                    2,
                    &focused_fault as *const Value as isize,
                );
                native_send(
                    hwnd,
                    RUNTIME_TEST_QUERY,
                    0,
                    &mut observed as *mut Value as isize,
                );
                if observed["busy"] != true || observed["deferred"] != true {
                    return Err(format!(
                        "Focused editor did not defer runtime failure: {observed}"
                    ));
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                wait_for(
                    || {
                        acknowledge_owned_failure(hwnd);
                        IsWindow(hwnd) == 0
                    },
                    "Editor closed after failure acknowledgement",
                )?;
                if IsWindow(unsaved_edit) != 0 || acknowledge_owned_failure(hwnd) {
                    return Err("Focused editor or its failure dialog survived close".into());
                }
                if controller_sim.state().log.len() != before_close_log
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                        != before_close_store
                {
                    let after = std::fs::read(controller_directory.join("profiles.json")).unwrap();
                    let diagnostic = json!({
                        "before_store":serde_json::from_slice::<Value>(&before_close_store).unwrap(),
                        "after_store":serde_json::from_slice::<Value>(&after).unwrap(),
                        "commands_after_close":format!("{:?}", &controller_sim.state().log[before_close_log..]),
                        "before_close_feedback":observed,
                    });
                    let path = controller_directory.join("focused-close-diagnostic.json");
                    std::fs::write(&path, serde_json::to_vec_pretty(&diagnostic).unwrap()).unwrap();
                    return Err(format!("Focused close persisted draft or dispatched hardware: {}",path.display()));
                }
                Ok(
                    json!({"native_failure_dialog_acknowledged":true,"transport":"simulated only","physical_discovery_disabled":true,"source_region0_readback_not_modeled":true,
                "focused_background_failure_close_acknowledged":true,"unsaved_close_draft":"fedcba","saved_close_slot":"#a1b2c3","focused_edit_destroyed":true,"close_command_delta":0,"close_store_bytes_unchanged":true,"cached_status_no_hid_or_store_write":true,"background_fault_preserves_capture_and_draft":true,"background_fault_once_across_user_operation":true,"brightness_capture_verified":true,"source_layout_client_css_width":width,"scale":scale,"brightness_no_write_before_release":true,"brightness_release_raw":114,"brightness_cancel_no_write":true,
                "native_edit_draft_switch_isolated":true,"cancelled_color_retained":true,"opaque_slot_preserved":true,
                "edited_slot_saved":"#a1b2c3","color_actions_did_not_write_brightness":true,
                "evidence":"actual HWND mouse/EDIT notifications and worker persistence; no OS pointer injection or hardware proof"}),
                )
            }
        })();
        if let Err(error) = &result {
            eprintln!("Lighting controller failed: {error}");
            // A failed assertion must not strand its owned acknowledged dialog.
            // Cleanup is limited to this simulated test's HWND.
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                PostMessageW(hwnd, WM_CANCELMODE, 0, 0);
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                let until = Instant::now() + Duration::from_secs(2);
                while !hwnd.is_null() && IsWindow(hwnd) != 0 && Instant::now() < until {
                    acknowledge_owned_failure(hwnd);
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        result
    });
    let window_result = run_window(engine, Some(Duration::from_secs(20)), None);
    let result = controller
        .join()
        .expect("Lighting controller panicked")
        .expect("Lighting interaction failed");
    window_result.unwrap();
    let store = Store::load(&directory.join("profiles.json")).unwrap();
    let device = &store.devices[&key];
    assert_eq!(
        serde_json::to_value(&device.profiles["Other"]).unwrap(),
        serde_json::to_value(&untouched).unwrap()
    );
    assert_eq!(
        LightingSettings::from_profile(device.active())
            .unwrap()
            .unwrap()
            .brightness
            .unwrap()
            .value,
        45
    );
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Native simulated Lighting interaction evidence: {}",
        directory.display()
    );
}

unsafe fn modal_snapshot(hwnd: HWND) -> Value {
    let mut value = Value::Null;
    native_send(
        hwnd,
        PROFILE_MODAL_TEST_QUERY,
        0,
        &mut value as *mut Value as isize,
    );
    value
}

#[test]
#[ignore = "opens a bounded native profile modal; simulated transport only, run alone"]
fn native_profile_modal_preview_selection_commit_and_full_backup() {
    use secbird_core::profile_compat as compat;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-profiles-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-lighting")),
    ]);
    let sim = Sim::keyboard();
    let mut engine =
        Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    let key = engine.devices()[0].key();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let d = store.for_device(&engine.devices()[0]);
    d.profiles.clear();
    d.active = "Work".into();
    for name in ["Work", "Other"] {
        let mut profile = compat::new_native_profile(605, name).unwrap();
        profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([{"inputID":"DKM_F1","outputType":"future-macro","macroGroup":{"guid":"macro-fixture","name":"Macro"}}]);
        d.profiles.insert(name.into(), profile);
    }
    let macro_value = json!({"guid":"macro-fixture","macroEvents":[],"macroPlaybackOption":1});
    let mut envelope: Value =
        serde_json::from_slice(&compat::export_synapse4(&store, &key).unwrap().bytes).unwrap();
    envelope["macros"] = json!([{"name":"Macro","hash":compat::synapse_hash(&macro_value).unwrap(),"payload":"eyJndWlkIjoibWFjcm8tZml4dHVyZSIsIm1hY3JvRXZlbnRzIjpbXSwibWFjcm9QbGF5YmFja09wdGlvbiI6MX0="}]);
    store
        .devices
        .get_mut(&key)
        .unwrap()
        .extra
        .insert("_synapseLiteSynapse4".into(), envelope);
    let incoming = directory.join("incoming.synapse4");
    std::fs::write(
        &incoming,
        compat::export_synapse4(&store, &key).unwrap().bytes,
    )
    .unwrap();
    let mut damaged: Value = serde_json::from_slice(&std::fs::read(&incoming).unwrap()).unwrap();
    // Lexical export order is Other,Work; preserve the good first record.
    damaged["profiles"][1]["hash"] = json!("bad-hash");
    damaged["macros"][0]["hash"] = json!("bad-hash");
    let partial_incoming = directory.join("partial.synapse4");
    std::fs::write(&partial_incoming, serde_json::to_vec(&damaged).unwrap()).unwrap();
    damaged["profiles"][0]["hash"] = json!("bad-hash");
    let invalid_incoming = directory.join("all-invalid.synapse4");
    std::fs::write(&invalid_incoming, serde_json::to_vec(&damaged).unwrap()).unwrap();
    let long_incoming = directory.join("long-incoming.synapse4");
    let mut long: Store = serde_json::from_value(serde_json::to_value(&store).unwrap()).unwrap();
    for i in 0..20 {
        let name = format!("Long {i:02}");
        long.devices.get_mut(&key).unwrap().profiles.insert(
            name.clone(),
            compat::new_native_profile(605, &name).unwrap(),
        );
    }
    std::fs::write(
        &long_incoming,
        compat::export_synapse4(&long, &key).unwrap().bytes,
    )
    .unwrap();
    // The existing profile stays selected; imported copies must remain inactive.
    store
        .devices
        .get_mut(&key)
        .unwrap()
        .profiles
        .remove("Other");
    store
        .extra
        .insert("opaque-global".into(), json!({"retain":[1,2]}));
    let foreign:secbird_core::profile::DeviceProfiles=serde_json::from_value(json!({"active":"Foreign","profiles":{"Foreign":{"future":{"retain":true}}},"device-future":17})).unwrap();
    store.devices.insert("FFFF:foreign".into(), foreign);
    store.save().unwrap();
    assert!(engine.rescan().is_empty());
    assert!(engine.event_paths().is_empty());
    let engine = Arc::new(Mutex::new(engine));
    let controller_directory = directory.clone();
    let controller_key = key.clone();
    let controller_sim = sim.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        wait_for(
            || {
                std::fs::read(&frame)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|f| f["presented"] == true && f["device_kind"] == "keyboard")
            },
            "Profile fixture frame",
        )?;
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                (1320.0 * scale) as i32,
                (900.0 * scale) as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            redraw(hwnd);
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            let width = f64::from(client.right) / scale;
            let initial = std::fs::read(controller_directory.join("profiles.json"))
                .map_err(|e| e.to_string())?;
            let before = brightness_write_count(&controller_sim);
            let writes_before = controller_sim
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count();
            let browse = || {
                SendMessageW(hwnd, PROFILE_MODAL_TEST_OPEN, 0, 0);
                SendMessageW(
                    hwnd,
                    PROFILE_MODAL_TEST_BROWSE,
                    0,
                    &incoming as *const PathBuf as isize,
                );
            };
            SendMessageW(hwnd, PROFILE_MODAL_TEST_OPEN, 0, 0);
            SendMessageW(
                hwnd,
                PROFILE_MODAL_TEST_BROWSE,
                0,
                &long_incoming as *const PathBuf as isize,
            );
            wait_for(
                || modal_snapshot(hwnd)["enabled"] == true,
                "Long import preview",
            )?;
            redraw(hwnd);
            let g = profile_modal_layout::geometry(width, true, false, true);
            let snap = modal_snapshot(hwnd);
            let expanded: Vec<_> = snap["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["selected"] == true, r["macros"].as_array().unwrap().len()))
                .collect();
            let bar = profile_modal_layout::scrollbar(&g, &expanded, 0.0)
                .ok_or("Missing long-list source scrollbar")?;
            let x = bar.thumb.x + 4.0;
            let y = bar.thumb.y + bar.thumb.height / 2.0;
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, position(x, y, scale));
            SendMessageW(
                hwnd,
                WM_MOUSEMOVE,
                1,
                position(x, bar.track.y + bar.track.height - 4.0, scale),
            );
            SendMessageW(
                hwnd,
                WM_LBUTTONUP,
                0,
                position(x, bar.track.y + bar.track.height - 4.0, scale),
            );
            if modal_snapshot(hwnd)["scroll"].as_f64().unwrap_or(0.0) < bar.maximum * 0.9 {
                return Err(format!(
                    "Source thumb drag did not scroll: {}",
                    modal_snapshot(hwnd)
                ));
            }

            if std::fs::read(controller_directory.join("profiles.json")).unwrap() != initial {
                return Err("Browse changed saved profiles".into());
            }
            let g = profile_modal_layout::geometry(width, true, false, true);
            lighting_click(hwnd, g.cancel.x + 20.0, g.cancel.y + 13.0, scale);
            if modal_snapshot(hwnd)["open"] != false {
                return Err("Cancel did not close modal".into());
            }
            if std::fs::read(controller_directory.join("profiles.json")).unwrap() != initial {
                return Err("Cancel changed saved profiles".into());
            }
            for (path, count) in [(&partial_incoming, 1usize), (&invalid_incoming, 0usize)] {
                SendMessageW(hwnd, PROFILE_MODAL_TEST_OPEN, 0, 0);
                SendMessageW(
                    hwnd,
                    PROFILE_MODAL_TEST_BROWSE,
                    0,
                    path as *const PathBuf as isize,
                );
                wait_for(
                    || {
                        let s = modal_snapshot(hwnd);
                        s["open"] == true && s["loading"] == false
                    },
                    "Damaged-record preview",
                )?;
                let snap = modal_snapshot(hwnd);
                if snap["warning"] != true
                    || snap["rows"].as_array().is_none_or(|a| a.len() != count)
                    || snap["enabled"] != json!(count > 0)
                {
                    return Err(format!(
                        "Partial/all-invalid source preview mismatch: {snap}"
                    ));
                }
                if std::fs::read(controller_directory.join("profiles.json")).unwrap() != initial {
                    return Err("Damaged-record preview saved profiles".into());
                }
                redraw(hwnd);
                lighting_click(hwnd, g.cancel.x + 20.0, g.cancel.y + 13.0, scale);
            }
            browse();
            wait_for(
                || modal_snapshot(hwnd)["enabled"] == true,
                "Second import preview",
            )?;
            redraw(hwnd);
            let snapshot = modal_snapshot(hwnd);
            let rows = snapshot["rows"].as_array().ok_or("Missing preview rows")?;
            let other = rows
                .iter()
                .position(|r| r["name"] == "Other")
                .ok_or("Missing Other row")?;
            let work = rows
                .iter()
                .position(|r| r["name"] == "Work")
                .ok_or("Missing Work row")?;
            let expanded: Vec<_> = rows
                .iter()
                .map(|r| (r["selected"] == true, r["macros"].as_array().unwrap().len()))
                .collect();
            let (layout, _) = profile_modal_layout::rows(&g, &expanded, 0.0);
            let r = layout[other].check;
            lighting_click(hwnd, r.x + 10.0, r.y + 10.0, scale);
            redraw(hwnd);
            let snapshot = modal_snapshot(hwnd);
            let expanded: Vec<_> = snapshot["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["selected"] == true, r["macros"].as_array().unwrap().len()))
                .collect();
            let (layout, _) = profile_modal_layout::rows(&g, &expanded, 0.0);
            let r = layout[work].macros[0];
            lighting_click(hwnd, r.x + 10.0, r.y + 10.0, scale);
            // Preserve the visible source transition while correcting its stale
            // macro action flags: checked available macros must be included.
            redraw(hwnd);
            lighting_click(hwnd, g.all.x + 30.0, g.all.y + 20.0, scale);
            redraw(hwnd);
            lighting_click(hwnd, g.all.x + 30.0, g.all.y + 20.0, scale);
            let snap = modal_snapshot(hwnd);
            if snap["rows"][work]["macros"][0]["mark"] != true
                || snap["rows"][work]["macros"][0]["selected"] != true
            {
                return Err(format!("Select-all macro mark/action mismatch: {snap}"));
            }
            let expanded: Vec<_> = snap["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["selected"] == true, r["macros"].as_array().unwrap().len()))
                .collect();
            let (layout, _) = profile_modal_layout::rows(&g, &expanded, 0.0);
            let r = layout[other].check;
            redraw(hwnd);
            lighting_click(hwnd, r.x + 10.0, r.y + 10.0, scale);
            let snap = modal_snapshot(hwnd);
            if snap["rows"][other]["selected"] != false
                || snap["rows"][work]["macros"][0]["selected"] != true
            {
                return Err(format!("Checkbox events missed: {snap}"));
            }
            let expanded: Vec<_> = snap["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (r["selected"] == true, r["macros"].as_array().unwrap().len()))
                .collect();
            let (layout, _) = profile_modal_layout::rows(&g, &expanded, 0.0);
            let r = layout[work].macros[0];
            redraw(hwnd);
            lighting_click(hwnd, r.x + 10.0, r.y + 10.0, scale);
            let snap = modal_snapshot(hwnd);
            if snap["rows"][work]["macros"][0]["selected"] != false
                || snap["rows"][work]["macros"][0]["mark"] != false
            {
                return Err(format!("Explicit macro exclusion missed: {snap}"));
            }
            redraw(hwnd);
            lighting_click(hwnd, g.commit.x + 20.0, g.commit.y + 13.0, scale);
            wait_for(
                || {
                    Store::load(&controller_directory.join("profiles.json"))
                        .ok()
                        .is_some_and(|s| {
                            s.devices[&controller_key].profiles.contains_key("Work (1)")
                        })
                },
                "Selected inactive import persisted",
            )?;
            let saved = Store::load(&controller_directory.join("profiles.json")).unwrap();
            let d = &saved.devices[&controller_key];
            if d.active != "Work"
                || d.profiles.contains_key("Other")
                || d.profiles["Work (1)"].extra["_synapseLiteSynapse4"]["decoded"]["guid"]
                    == d.profiles["Work"].extra["_synapseLiteSynapse4"]["decoded"]["guid"]
            {
                return Err("Import identity/selection/activation contract failed".into());
            }
            if d.profiles["Work (1)"].extra["_synapseLiteSynapse4"]["decoded"]["mappings"]
                .as_array()
                .is_none_or(|a| !a.is_empty())
            {
                return Err("Deselected macro binding was retained".into());
            }
            let export = |path: &PathBuf, format: usize| -> Result<(), String> {
                SendMessageW(hwnd, PROFILE_MODAL_TEST_OPEN, 1, 0);
                wait_for(|| modal_snapshot(hwnd)["enabled"] == true, "Export preview")?;
                let s = modal_snapshot(hwnd);
                let selected = s["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["selected"] == true)
                    .count();
                if selected != 1 {
                    return Err(format!("Export must initially select active only: {s}"));
                }
                redraw(hwnd);
                let g = profile_modal_layout::geometry(width, false, false, false);
                lighting_click(hwnd, g.commit.x + 20.0, g.commit.y + 13.0, scale);
                wait_for(
                    || modal_snapshot(hwnd)["export_plan"] == true,
                    "Save picker request",
                )?;
                SendMessageW(
                    hwnd,
                    PROFILE_MODAL_TEST_SAVE,
                    format,
                    path as *const PathBuf as isize,
                );
                wait_for(|| path.exists(), "Export output")
            };
            let selected = controller_directory.join("selected.synapse4");
            export(&selected, 1)?;
            let output: Value = serde_json::from_slice(&std::fs::read(&selected).unwrap()).unwrap();
            if output["profiles"].as_array().unwrap().len() != 1
                || output["profiles"][0]["name"] != "Work"
            {
                return Err("Synapse export did not honor selection".into());
            }
            let full = controller_directory.join("backup.json");
            export(&full, 2)?;
            let full: Value = serde_json::from_slice(&std::fs::read(full).unwrap()).unwrap();
            if full["devices"]["FFFF:foreign"]["device-future"] != 17
                || full["opaque-global"]["retain"] != json!([1, 2])
                || full["devices"][&controller_key]["profiles"]["Work (1)"].is_null()
            {
                return Err("Full backup dropped unselected or opaque data".into());
            }
            // A fresh-store owner change after preview must fail rather than import
            // into whichever profile happens to be active by commit time.
            browse();
            wait_for(
                || modal_snapshot(hwnd)["enabled"] == true,
                "Stale-owner preview",
            )?;
            let mut changed = Store::load(&controller_directory.join("profiles.json")).unwrap();
            changed.devices.get_mut(&controller_key).unwrap().active = "Work (1)".into();
            changed.save().unwrap();
            let changed_bytes = std::fs::read(controller_directory.join("profiles.json")).unwrap();
            redraw(hwnd);
            let g = profile_modal_layout::geometry(width, true, false, true);
            lighting_click(hwnd, g.commit.x + 20.0, g.commit.y + 13.0, scale);
            wait_for(
                || acknowledge_owned_failure(hwnd),
                "Stale owner acknowledged failure",
            )?;
            if std::fs::read(controller_directory.join("profiles.json")).unwrap() != changed_bytes {
                return Err("Stale preview changed saved profiles".into());
            }
            if controller_sim
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count()
                != writes_before
            {
                return Err("Profile flow sent a simulated SET command".into());
            }
            if brightness_write_count(&controller_sim) != before {
                return Err("Profile flow wrote simulated hardware brightness".into());
            }
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            wait_for(
                || {
                    acknowledge_owned_failure(hwnd);
                    IsWindow(hwnd) == 0
                },
                "Profile editor closed",
            )?;
            Ok(
                json!({"browse_cancel_read_only":true,"partial_record_warning_and_empty_disabled_preview":true,"source_checkbox_events":true,"available_macro_marks_match_action_selection":true,"source_scrollbar_drag":true,"fresh_guid_inactive_import":true,"macro_binding_removed":true,"selected_export":true,"full_backup_retains_foreign_and_opaque":true,"stale_owner_refused":true,"no_simulated_set_commands":true,"sim_only":true}),
            )
        }
    });
    let window = run_window(engine, Some(Duration::from_secs(25)), None);
    let result = controller
        .join()
        .expect("Profile controller panic")
        .expect("Profile modal interaction failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Native simulated profile modal evidence: {}",
        directory.display()
    );
}

#[test]
#[ignore = "opens four bounded native smoke overlays; isolated simulator and local assets only"]
fn native_read_only_overlay_smokes_present_ready_without_saving_or_set_commands() {
    use secbird_core::profile_compat as compat;
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-assets");
    assert!(
        assets.join("lighting-value.lut").is_file(),
        "Provision local source assets before this opt-in test"
    );
    for scenario in [
        "keyboard-lighting-color-editor",
        "keyboard-profile-import",
        "keyboard-profile-export",
        "keyboard-profile-cloud",
    ] {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "synapse-native-overlay-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let frame = directory.join("frame.json");
        let _environment = Environment::set(&[
            ("SYNAPSE_LITE_CONFIG_DIR", &directory),
            ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
            ("SYNAPSE_LITE_SMOKE_VIEW", Path::new(scenario)),
            ("SYNAPSE_LITE_ASSETS", &assets),
        ]);
        let sim = Sim::keyboard();
        let mut engine =
            Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
        let key = engine.devices()[0].key();
        let mut profile = compat::new_native_profile(605, "OverlayFixture").unwrap();
        // The simulator deliberately starts with a source effect without a color
        // control, proving that the editor override is presentation-only.
        compat::set_source_quick_effect(&mut profile, 605, "OverlayFixture", 3, &json!({}))
            .unwrap();
        let mut store = Store::load(&directory.join("profiles.json")).unwrap();
        let d = store.for_device(&engine.devices()[0]);
        d.profiles.clear();
        d.profiles.insert("OverlayFixture".into(), profile);
        d.active = "OverlayFixture".into();
        store.save().unwrap();
        let before = std::fs::read(directory.join("profiles.json")).unwrap();
        assert!(engine.rescan().is_empty());
        assert!(engine.event_paths().is_empty());
        let sets = sim
            .state()
            .log
            .iter()
            .filter(|command| command.id & 0x80 == 0)
            .count();
        let controller_frame = frame.clone();
        let controller_key = key.clone();
        let controller = std::thread::spawn(move || -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&controller_frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| {
                            f["requested"] == scenario
                                && f["matched"] == true
                                && f["presented"] == true
                        })
                },
                "Read-only overlay readiness",
            )?;
            let result: Value =
                serde_json::from_slice(&std::fs::read(&controller_frame).unwrap()).unwrap();
            let text = result.to_string();
            if text.contains("OverlayFixture") || text.contains(&controller_key) {
                return Err("Smoke report leaked profile/device identity".into());
            }
            if scenario == "keyboard-lighting-color-editor" {
                if result["lighting_overlay"]["color_editor_inputs"] != 4
                    || result["lighting_overlay"]["color_editor_exact_canvas_assets"] != true
                    || result["presentation_only_override"] != true
                {
                    return Err(format!("Incomplete color editor smoke: {result}"));
                }
            } else {
                let modal = &result["profile_overlay"];
                if modal["loading"] != false
                    || modal["owner_matches_current_snapshot"] != true
                    || modal["has_browsed_file"] != false
                    || result["profile_source_icons"] != 5
                {
                    return Err(format!("Incomplete modal smoke: {result}"));
                }
                if scenario == "keyboard-profile-export"
                    && (modal["profile_rows"] != 1 || modal["selected_profiles"] != 1)
                {
                    return Err("Export did not use actual fixture snapshot".into());
                }
                if scenario != "keyboard-profile-export"
                    && (modal["profile_rows"] != 0 || modal["commit_enabled"] != false)
                {
                    return Err("Empty import/cloud unexpectedly became committable".into());
                }
            }
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                wait_for(|| IsWindow(hwnd) == 0, "Read-only overlay close")?;
            }
            Ok(result)
        });
        let window = run_window(
            Arc::new(Mutex::new(engine)),
            Some(Duration::from_secs(8)),
            None,
        );
        let result = controller
            .join()
            .expect("Overlay controller panic")
            .unwrap_or_else(|error| panic!("{scenario}: {error}; frame {}", frame.display()));
        window.unwrap();
        assert_eq!(
            std::fs::read(directory.join("profiles.json")).unwrap(),
            before,
            "{scenario} changed saved profiles"
        );
        assert_eq!(
            sim.state()
                .log
                .iter()
                .filter(|command| command.id & 0x80 == 0)
                .count(),
            sets,
            "{scenario} sent a SET command"
        );
        std::fs::write(
            directory.join("interaction.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        eprintln!("Read-only overlay {scenario}: {}", directory.display());
    }
}
#[test]
#[ignore = "opens a bounded native window with simulated transport only"]
fn native_assignment_source_draft_has_no_device_or_store_writes() {
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-assignment-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-customize")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    assert!(engine.event_paths().is_empty());
    let device = &engine.devices()[0];
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    let opaque = json!({"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":false,"outputType":"futureGroup","futureGroup":{"opaque":[7,8],"unknownFlag":true}});
    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([opaque]);
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(device);
    profiles.profiles.clear();
    profiles.profiles.insert("Fixture".into(), profile);
    profiles.active = "Fixture".into();
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let controller_directory = directory.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| f["matched"] == true && f["presented"] == true)
                },
                "Customize native frame",
            )?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            unsafe {
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let width = client.right as f64 / scale;
                let height = client.bottom as f64 / scale;
                let layout =
                    source_layout::SourceLayout::load(&assets_dir().join("keyboard-layout.json"))?;
                let viewport = source_layout::Viewport {
                    left: assignment_geometry::keyboard_center(width, false) - 365.0,
                    top: 154.0,
                    width: 730.0,
                    height: 387.0,
                };
                let key = layout
                    .bounds("KEY_A", viewport)
                    .ok_or("Missing source KEY_A path")?;
                let before = controller_sim.state().log.len();
                let before_store =
                    std::fs::read(controller_directory.join("profiles.json")).unwrap();
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(
                        hwnd,
                        ASSIGNMENT_TEST_QUERY,
                        0,
                        &mut value as *mut Value as isize,
                    );
                    value
                };
                lighting_click(
                    hwnd,
                    (key.left + key.right) / 2.0,
                    (key.top + key.bottom) / 2.0,
                    scale,
                );
                let mut fonts = Value::Null;
                SendMessageW(
                    hwnd,
                    ASSIGNMENT_TEST_QUERY,
                    2,
                    &mut fonts as *mut Value as isize,
                );
                if fonts["private_font_files"] != 3
                    || !fonts["rows"].as_array().is_some_and(|rows| {
                        rows.len() == 10 && rows.iter().all(|row| row["matches"] == true)
                    })
                {
                    return Err(format!(
                        "Native source baseline vector check failed: {fonts}"
                    ));
                }
                std::fs::write(
                    controller_directory.join("font-window-geometry.json"),
                    serde_json::to_vec_pretty(&fonts).unwrap(),
                )
                .unwrap();
                let initial = query();
                if initial["open"] != true
                    || initial["input"] != "KEY_A"
                    || !initial["category"].is_null()
                {
                    return Err(format!("Source key/opaque mapping open failed: {initial}"));
                }
                let x = initial["popup"]["x"].as_f64().unwrap();
                let y = initial["popup"]["y"].as_f64().unwrap();
                let original = initial["source_original"].clone();
                // The fourth source category is Mouse Function. This is the real
                // popup hitmap, not a test action injected into its model.
                lighting_click(hwnd, x + 20.0, y + 3.0 * 40.0 + 20.0, scale);
                if query()["category"] != "MOUSE_FUNCTION" {
                    return Err(format!("Category hit failed: {}", query()));
                }
                lighting_click(hwnd, x + 65.0, y + 73.0, scale);
                lighting_click(hwnd, x + 75.0, y + 28.0 + 73.0 + 3.0 * 25.0, scale);
                if query()["dirty"] != true {
                    return Err(format!("Mouse dropdown did not edit draft: {}", query()));
                }
                lighting_click(hwnd, x + 70.0, y + 107.0, scale);
                if edit_text(hwnd, assignment_view::SPEED).as_deref() != Some("7") {
                    return Err("Source Turbo did not create speed input7".into());
                }
                let edit = GetDlgItem(hwnd, i32::from(assignment_view::SPEED));
                SetWindowTextW(edit, wide("1e").as_ptr());
                if edit_text(hwnd, assignment_view::SPEED).as_deref() != Some("7") {
                    return Err("Controlled speed input accepted invalid exponent text".into());
                }
                let up = position(x + 109.0, y + 154.0, scale);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, up);
                SendMessageW(hwnd, WM_TIMER, ASSIGNMENT_REPEAT, 0);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, up);
                redraw(hwnd);
                if query()["speed"] != 9 {
                    return Err(format!(
                        "Source speed press/repeat/release failed: {}",
                        query()
                    ));
                }
                let start = position(x + 75.0, y + 191.0, scale);
                let end = position(x + 265.0, y + 191.0, scale);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
                redraw(hwnd);
                if query()["speed"] != 20 {
                    return Err(format!("Source speed drag did not reach20: {}", query()));
                }
                let source_shell =
                    assignment_layout::shell(height - 42.0, 42.0, x, 36.0, false).unwrap();
                let source_form = assignment_geometry::form(
                    source_shell,
                    605,
                    Some(assignment_model::Category::Mouse),
                    true,
                    true,
                    0.0,
                );
                lighting_click(
                    hwnd,
                    source_form.save.x + 50.0,
                    source_form.save.y + 13.0,
                    scale,
                );
                if query()["open"] != true || query()["pending"] != 0 {
                    return Err("Disabled Save submitted or dismissed draft".into());
                }

                lighting_click(hwnd, x + 265.0, y - 16.0, scale);
                if query()["prompt"] != true {
                    return Err("Dirty close did not show recovered save alert".into());
                }
                let alert = assignment_geometry::alert(width, height);
                lighting_click(hwnd, alert.close.x + 10.0, alert.close.y + 10.0, scale);
                if query()["prompt"] == true || query()["dirty"] != true {
                    return Err("Alert X discarded draft".into());
                }
                let mut request = json!({"key":87,"shift":false});
                SendMessageW(hwnd,RUNTIME_TEST_QUERY,7,&mut request as *mut Value as isize);
                redraw(hwnd);
                if query()["prompt"]!=true || request["host_tabs"]["selected"].is_null() {
                    return Err("Dirty host close bypassed draft gate".into());
                }
                lighting_click(hwnd,alert.close.x+10.0,alert.close.y+10.0,scale);
                if query()["dirty"]!=true { return Err("Host-close dismissal lost draft".into()); }
                let mut request=json!({"key":87,"shift":false});
                SendMessageW(hwnd,RUNTIME_TEST_QUERY,7,&mut request as *mut Value as isize);
                lighting_click(hwnd,alert.discard.x+50.0,alert.discard.y+13.0,scale);
                redraw(hwnd);
                if query()["open"]==true { return Err("Host discard left assignment open".into()); }
                let mut host_state=Value::Null;
                SendMessageW(hwnd,RUNTIME_TEST_QUERY,0,&mut host_state as *mut Value as isize);
                if !host_state["host_tabs"]["selected"].is_null() || host_state["host_tabs"]["device_count"]!=1 {
                    return Err("Host discard did not close only the view".into());
                }
                let mut reopen=json!({"key":84,"shift":true});
                SendMessageW(hwnd,RUNTIME_TEST_QUERY,7,&mut reopen as *mut Value as isize);
                redraw(hwnd);
                lighting_click(
                    hwnd,
                    (key.left + key.right) / 2.0,
                    (key.top + key.bottom) / 2.0,
                    scale,
                );
                if query()["source_original"] != original {
                    return Err("Source opaque profile changed during discard".into());
                }
                // Cancel is source direct discard; it sends no API request.
                lighting_click(hwnd, x + 20.0, y + 3.0 * 40.0 + 20.0, scale);
                let shell = assignment_layout::shell(height - 42.0, 42.0, x, 36.0, false).unwrap();
                let form = assignment_geometry::form(
                    shell,
                    605,
                    Some(assignment_model::Category::Mouse),
                    false,
                    true,
                    0.0,
                );
                lighting_click(hwnd, form.cancel.x + 50.0, form.cancel.y + 13.0, scale);
                if query()["open"] == true {
                    return Err("Cancel did not close popup".into());
                }
                let center = assignment_geometry::keyboard_center(width, false);
                lighting_click(hwnd, center - 105.8125, 572.0, scale);
                let drawer = query();
                if drawer["view"]["drawer"] != true || drawer["view"]["source_icons"] != 58 {
                    return Err(format!("Source drawer/assets incomplete: {drawer}"));
                }
                SendMessageW(hwnd, WM_MOUSEMOVE, 0, position(150.0, 260.0, scale));
                SendMessageW(hwnd, WM_MOUSEWHEEL, (-120i16 as u16 as usize) << 16, 0);
                redraw(hwnd);
                let scrolled = query();
                let offset = scrolled["view"]["drawer_scroll"].as_f64().unwrap();
                if offset <= 0.0 {
                    return Err("Drawer wheel did not scroll source rows".into());
                }
                let bar = assignment_geometry::scrollbar(
                    assignment_layout::Rect {
                        x: 0.0,
                        y: 195.0,
                        width: 230.0,
                        height: height - 207.0,
                    },
                    scrolled["view"]["drawer_count"].as_u64().unwrap() as f64 * 50.0,
                    offset,
                    8.0,
                )
                .ok_or("No source drawer scrollbar")?;
                let start = position(
                    bar.thumb.x + 4.0,
                    bar.thumb.y + bar.thumb.height / 2.0,
                    scale,
                );
                SendMessageW(hwnd, WM_MOUSEMOVE, 0, start);
                redraw(hwnd);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
                let end = position(bar.thumb.x + 4.0, height - 20.0, scale);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
                redraw(hwnd);
                if query()["view"]["drawer_scroll"].as_f64().unwrap() <= offset {
                    return Err("Drawer thumb did not drag source rows".into());
                }
                if controller_sim.state().log.len() != before
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                        != before_store
                {
                    return Err("Assignment presentation changed device/store".into());
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"dirty_host_close_deferred":true,"host_dismiss_keeps_view":true,"host_discard_closes_only_view":true,"source_key_path_hit":true,"opaque_mapping_preserved":true,"mouse_category_and_dropdown":true,"turbo_controlled_numeric_repeat_and_drag":true,"unsupported_save_disabled":true,"source_drawer_wheel_and_thumb_drag":true,"source_icons_loaded":58,"dirty_close_prompt":true,"dismiss_keeps_draft":true,"discard_and_cancel_no_writes":true,"sim_commands_added":0,"profile_bytes_unchanged":true,"physical_discovery_disabled":true}),
                )
            }
        })();
        if result.is_err() {
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let mut ignored = Value::Null;
                SendMessageW(
                    hwnd,
                    ASSIGNMENT_TEST_QUERY,
                    1,
                    &mut ignored as *mut Value as isize,
                );
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    let window = run_window(engine, Some(Duration::from_secs(12)), None);
    let result = controller
        .join()
        .unwrap()
        .expect("Assignment HWND interaction failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Native assignment evidence: {}", directory.display());
}

#[test]
#[ignore = "opens a bounded native window with simulated mouse transport only"]
fn native_assignment_mouse_labels_and_drawer_have_source_hit_targets() {
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-mouse-assignment-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-performance")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let device = &engine.devices()[0];
    assert!(matches!(device.pid, 196 | 197));
    assert!(engine.event_paths().is_empty());
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let p = store.for_device(device);
    p.profiles.clear();
    p.profiles.insert(
        "Fixture".into(),
        secbird_core::profile_compat::new_native_profile(device.pid, "Fixture").unwrap(),
    );
    p.active = "Fixture".into();
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let controller_directory = directory.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| f["matched"] == true && f["presented"] == true)
                },
                "Mouse native frame",
            )?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            unsafe {
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = client.right as f64 / scale;
                // Click the actual source Customize tab in its native hitmap.
                let total_width = Tab::Customize.width()
                    + 20
                    + Tab::Performance.width()
                    + 20
                    + Tab::Power.width()
                    + 20
                    + Tab::Calibration.width()
                    + 20
                    + Tab::Advanced.width()
                    + 20;
                let tabs_x = (width.round() as i32 / 2).max(420) - total_width / 2;
                lighting_click(
                    hwnd,
                    (tabs_x + Tab::Customize.width() / 2) as f64,
                    102.0,
                    scale,
                );
                let before = controller_sim.state().log.len();
                let before_store =
                    std::fs::read(controller_directory.join("profiles.json")).unwrap();
                // Exercise the shared host strip through its actual hit map,
                // not the old fixed block coordinates. Presentation stays read-only.
                let host = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, RUNTIME_TEST_QUERY, 0, &mut v as *mut Value as isize);
                    v["host_tabs"].clone()
                };
                let initial = host();
                if initial["natural_text_available"] != true {
                    return Err("Shared host text fell back from authenticated natural layout".into());
                }
                if initial["icons_loaded"] != 5 {
                    return Err("Shared host source icons were not provisioned/loaded".into());
                }
                let selected = initial["selected"].clone();
                let home = &initial["tabs"][0];
                let expected_home_width = ((8.0 + 107.732421875) * scale).round() as i64
                    - (8.0 * scale).round() as i64;
                if home["width"] != expected_home_width {
                    return Err(format!("Host tab used rounded GDI advances: {initial}"));
                }
                let point = |tab: &Value| {
                    (tab["x"].as_i64().unwrap() + tab["width"].as_i64().unwrap() / 2,
                     tab["y"].as_i64().unwrap() + tab["height"].as_i64().unwrap() / 2)
                };
                let (hx, hy) = point(home);
                let packed = (hx as u32 | (hy as u32) << 16) as isize;
                let mut batch = json!({"x":hx,"y":hy});
                SendMessageW(hwnd, RUNTIME_TEST_QUERY, 6, &mut batch as *mut Value as isize);
                let down = &batch["host_tabs"];
                if down["hover"] != "" || down["pressed"] != "" {
                    return Err(format!("Host hover/press did not reach shared component: {down}"));
                }
                SendMessageW(hwnd, WM_LBUTTONUP, 0, packed);
                redraw(hwnd);
                if !host()["selected"].is_null() || !host()["pressed"].is_null() {
                    return Err("Home did not clear selection/pressed state".into());
                }
                let dashboard = host();
                let target = dashboard["tabs"].as_array().unwrap().iter()
                    .find(|tab| tab["id"] == selected).unwrap();
                let (dx, dy) = point(target);
                lighting_click(hwnd, dx as f64 / scale, dy as f64 / scale, scale);
                redraw(hwnd);
                if host()["selected"] != selected {
                    return Err("Shared host device tab did not restore device view".into());
                }
                SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
                if !host()["hover"].is_null() {
                    return Err("Shared host hover survived mouse leave".into());
                }
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(
                        hwnd,
                        ASSIGNMENT_TEST_QUERY,
                        0,
                        &mut v as *mut Value as isize,
                    );
                    v
                };
                let g = assignment_geometry::mouse(
                    width,
                    false,
                    0.0,
                    0.0,
                    assignment_geometry::MOUSE_LABEL_WIDTHS,
                );
                let left = g.buttons[0];
                lighting_click(hwnd, left.x + left.width / 2.0, left.y + 15.0, scale);
                if query()["open"] == true {
                    return Err(
                        "Primary left button source-disabled target became actionable".into(),
                    );
                }
                let right = g.buttons[1];
                lighting_click(hwnd, right.x + right.width / 2.0, right.y + 15.0, scale);
                let opened = query();
                if opened["view"]["background_ready"] != true {
                    return Err("Mouse Customize did not render its source background".into());
                }
                if opened["input"] != "RightClick" || opened["category"] != "DEFAULT" {
                    return Err(format!(
                        "Source right label did not open correct input: {opened}"
                    ));
                }
                let x = opened["popup"]["x"].as_f64().unwrap();
                let y = opened["popup"]["y"].as_f64().unwrap();
                lighting_click(hwnd, x + 265.0, y - 16.0, scale);
                // Drawer opens from the shipped38x27 control and uses two explicit
                // filter options, rather than cycling state on a dropdown click.
                lighting_click(hwnd, g.drawer.x + 19.0, g.drawer.y + 13.0, scale);
                lighting_click(hwnd, 40.0, 160.0, scale);
                lighting_click(hwnd, 40.0, 214.0, scale);
                lighting_click(hwnd, 40.0, 160.0, scale);
                lighting_click(hwnd, 40.0, 189.0, scale);
                // Numeric source ordering makes RightClick row2; click only the
                // source drawer key binding region, not guessed mouse-image boxes.
                lighting_click(hwnd, 100.0, 195.0 + 50.0 + 25.0, scale);
                let drawer = query();
                if drawer["view"]["background_ready"] != true {
                    return Err("Opening the drawer lost the source background".into());
                }
                if drawer["input"] != "RightClick" || drawer["popup"]["x"] != 230.0 {
                    return Err(format!("Drawer source anchor failed: {drawer}"));
                }
                SendMessageW(hwnd, WM_KEYDOWN, 27, 0);
                redraw(hwnd);
                if query()["open"] == true {
                    return Err("Escape did not cancel drawer popup".into());
                }
                let shortcut = |key: u32, shift: bool| {
                    let mut value = json!({"key":key,"shift":shift});
                    SendMessageW(hwnd, RUNTIME_TEST_QUERY, 7, &mut value as *mut Value as isize);
                    redraw(hwnd);
                    host()
                };
                let performance_x = tabs_x + Tab::Customize.width() + 20 + Tab::Performance.width()/2;
                lighting_click(hwnd, performance_x as f64, 102.0, scale);
                if host()["page"] != "PERFORMANCE" { return Err("Performance tab did not open".into()); }
                shortcut(49,false); // Ctrl1 -> pinned Home
                let restored = shortcut(50,false); // Ctrl2 -> existing device view
                if restored["page"] != "PERFORMANCE" {
                    return Err(format!("Host selection forgot the device section: {restored}"));
                }
                let tab = restored["tabs"].as_array().unwrap().iter().find(|t|t["id"]==selected).unwrap();
                let (tx,ty) = point(tab);
                SendMessageW(hwnd, WM_MBUTTONDOWN, 0, (tx as u32 | (ty as u32)<<16) as isize);
                redraw(hwnd);redraw(hwnd);
                let closed = host();
                if !closed["selected"].is_null() || closed["order"].as_array().unwrap().len()!=0
                    || closed["device_count"]!=1 || closed["cards"].as_array().unwrap().len()!=1 {
                    return Err(format!("Closing view removed device or auto-reopened tab: {closed}"));
                }
                let saved_tabs:Value=serde_json::from_slice(&std::fs::read(controller_directory.join("tabs.json")).unwrap()).unwrap();
                if saved_tabs["closed"][0] != selected { return Err("User-closed tab was not persisted".into()); }
                let reopened = shortcut(84,true); // CtrlShiftT
                if reopened["selected"]!=selected || reopened["order"].as_array().unwrap().len()!=1 {
                    return Err(format!("Reopen shortcut failed: {reopened}"));
                }
                shortcut(87,false); // CtrlW
                let dashboard=host();let card=&dashboard["cards"][0];let (cx,cy)=point(card);
                lighting_click(hwnd,cx as f64/scale,cy as f64/scale,scale);redraw(hwnd);
                if host()["selected"]!=selected { return Err("Dashboard card did not reopen closed view".into()); }
                let mut close_button=Value::Null;
                SendMessageW(hwnd,RUNTIME_TEST_QUERY,8,&mut close_button as *mut Value as isize);
                redraw(hwnd);
                if !host()["selected"].is_null() || host()["order"].as_array().unwrap().len()!=0 {
                    return Err("Visible source close button did not close view".into());
                }
                shortcut(84,true);
                if controller_sim.state().log.len() != before
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                        != before_store
                {
                    return Err("Mouse presentation dispatched a command or saved a profile".into());
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"source_close_button_hit":true,"host_section_retained":true,"middle_close_persists_without_removing_device":true,"shortcut_mapper_close_reopen":true,"dashboard_card_reopens_closed_tab":true,"shared_host_home_device_navigation":true,"shared_host_hover_press_leave":true,"primary_left_disabled":true,"source_mouse_label_click":true,"drawer_filter_choices":true,"source_drawer_anchor_230":true,"escape_cancels":true,"sim_commands_added":0,"profile_bytes_unchanged":true,"physical_discovery_disabled":true}),
                )
            }
        })();
        if result.is_err() {
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let mut v = Value::Null;
                SendMessageW(
                    hwnd,
                    ASSIGNMENT_TEST_QUERY,
                    1,
                    &mut v as *mut Value as isize,
                );
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    let window = run_window(engine, Some(Duration::from_secs(12)), None);
    let result = controller
        .join()
        .unwrap()
        .expect("Mouse assignment HWND interaction failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Native mouse assignment evidence: {}", directory.display());
}
#[path="frame_interaction_tests.rs"]
mod frame;

#[test]
#[ignore = "opens a bounded native Wave dropdown; simulated transport only, run alone"]
fn native_wave_dropdown_reopens_selected_scrolls_drags_and_toggles_source_direction() {
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-wave-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-lighting")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    secbird_core::lighting::LightingSettings::default()
        .store_in_profile(&mut profile)
        .unwrap();
    secbird_core::profile_compat::set_source_quick_effect(
        &mut profile,
        605,
        "Fixture",
        4,
        &json!({"direction":2,"future":{"keep":7}}),
    )
    .unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let d = store.for_device(&engine.devices()[0]);
    d.active = "Fixture".into();
    d.profiles.clear();
    d.profiles.insert("Fixture".into(), profile);
    store.save().unwrap();
    let before = std::fs::read(directory.join("profiles.json")).unwrap();
    let controller_sim = sim.clone();
    let controller_directory = directory.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "Wave presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let query = |enable: bool| {
                    native_query(hwnd, LIGHTING_DROPDOWN_TEST, usize::from(enable))
                };
                let click = |name: &str, r: &Value| -> Result<(), String> {
                    let coordinate = |field: &str| r[field].as_f64()
                        .ok_or_else(|| format!("Wave {name}.{field} missing: {r}"));
                    native_click(
                        hwnd,
                        coordinate("x")? + coordinate("width")? / 2.0,
                        coordinate("y")? + coordinate("height")? / 2.0,
                        scale,
                    );
                    Ok(())
                };
                let initial = query(true)?;
                let log = controller_sim.state().log.len();
                click("initial trigger", &initial["trigger"])?;
                let opened = query(false)?;
                if opened["open"] != true || opened["scroll"] != 122.0 {
                    return Err(format!("Wave not auto-visible: {opened}"));
                }
                let thumb = &opened["thumb"];
                let track = &opened["track"];
                let x = thumb["x"].as_f64().unwrap() + 4.0;
                let y = thumb["y"].as_f64().unwrap() + 10.0;
                native_send(hwnd, WM_LBUTTONDOWN, 1, position(x, y, scale));
                native_send(
                    hwnd,
                    WM_MOUSEMOVE,
                    1,
                    position(x, track["y"].as_f64().unwrap() + 10.0, scale),
                );
                native_send(
                    hwnd,
                    WM_LBUTTONUP,
                    0,
                    position(x, track["y"].as_f64().unwrap() + 10.0, scale),
                );
                redraw(hwnd);
                if query(false)?["scroll"] != 0.0 {
                    return Err("Thumb drag did not reach start".into());
                }
                click("close trigger", &initial["trigger"])?;
                click("reopen trigger", &initial["trigger"])?;
                let reopened = query(false)?;
                if reopened["scroll"] != 122.0 {
                    return Err("Reopen did not restore selected-row scroll".into());
                }
                // Choose the actual Wave row, which starts 250px into source content.
                native_click(
                    hwnd,
                    reopened["panel"]["x"].as_f64().unwrap() + 30.0,
                    reopened["panel"]["y"].as_f64().unwrap() + 1.0 + 250.0 - 122.0 + 12.0,
                    scale,
                );
                let selected = query(false)?;
                if selected["open"] != false || selected["selected"] != 4 {
                    return Err(format!("Wave row did not select: {selected}"));
                }
                // Clicking the already-active right tab must toggle left, per Z_.
                click("right direction", &selected["directions"][1])?;
                let left = query(false)?;
                click("left direction", &left["directions"][0])?;
                let right = query(false)?;
                let commands = right["commands"].as_array().unwrap();
                if commands.len() != 3
                    || commands
                        .iter()
                        .map(|c| c["setting"]["direction"].as_i64())
                        .collect::<Vec<_>>()
                        != vec![Some(2), Some(1), Some(2)]
                    || commands.iter().any(|c| {
                        c["source_id"] != 4
                            || c["profile"] != "Fixture"
                            || c["setting"]["future"]["keep"] != 7
                    })
                {
                    return Err(format!("Wrong source requests: {commands:?}"));
                }
                // The source single-column view places Effects below the two
                // left cards. Scroll it into view, then exercise upward opening.
                SetWindowPos(hwnd, null_mut(), 0, 0, (1000.0 * scale).round() as i32, (650.0 * scale).round() as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
                redraw(hwnd);
                for _ in 0..6 {
                    native_send(hwnd, WM_MOUSEWHEEL, (-120i16 as u16 as usize) << 16, 0);
                }
                redraw(hwnd);
                let narrow = query(false)?;
                click("narrow trigger", &narrow["trigger"])?;
                let upward = query(false)?;
                if upward["open"] != true || upward["panel"]["y"].as_f64().unwrap() >= upward["trigger"]["y"].as_f64().unwrap() {
                    return Err(format!("Narrow dropdown did not open upwards: {upward}"));
                }
                click("upward close trigger", &upward["trigger"])?;
                if controller_sim.state().log.len() != log
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap() != before
                {
                    return Err("Presentation test dispatched I/O or persisted".into());
                }
                Ok(
                    json!({"actual_hwnd":true,"source_wave_requests":[2,1,2],"selected_row_scroll":122,"thumb_drag_start":0,"narrow_upward_opening":true,"opaque_setting_preserved":true,"effect_command_sink":"cfg-test only","physical_io":false,"backend_or_pixel_parity_proof":false}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            PostMessageW(hwnd, WM_CANCELMODE, 0, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        result
    });
    run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(12)),
        None,
    )
    .unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!("Wave HWND evidence: {}", directory.display());
}

#[test]
#[ignore = "opens an isolated simulated dashboard; requires private device-pack assets"]
fn native_dashboard_uses_lazy_product_art_without_device_or_profile_writes() {
    let directory = std::env::temp_dir().join(format!("native-dashboard-pack-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("dashboard")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]); store.save().unwrap();
    let before = std::fs::read(directory.join("profiles.json")).unwrap();
    let folder = directory.clone();
    let observer = sim.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> { unsafe {
            wait_for(|| std::fs::read(&frame).ok().and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok())
                .is_some_and(|value|value["presented"] == true), "dashboard pack frame")?;
            let value: Value = serde_json::from_slice(&std::fs::read(&frame).map_err(|error|error.to_string())?).unwrap();
            let art = &value["device_dashboard_artwork"];
            if art["indexed_images"].as_u64().unwrap_or(0) < 2 || art["cached_entries"] != 1
                || art["cache"][0]["product_id"] != 196 || art["cache"][0]["loaded"] != true {
                return Err(format!("Dashboard did not demand-load the exact mouse art: {art}"));
            }
            if art["resident_pixel_bytes"].as_u64().unwrap_or(u64::MAX) > 8 * 1024 * 1024 {
                return Err("Dashboard pixel residency exceeded the budget".into());
            }
            observer.state().log.clear();
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            for _ in 0..10 { redraw(hwnd); }
            if !observer.state().log.is_empty() { return Err("Repainting queried/wrote the device".into()); }
            let hold = std::env::var("SYNAPSE_LITE_DASHBOARD_HOLD_SECONDS").ok()
                .and_then(|value|value.parse::<u64>().ok()).unwrap_or(0).min(120);
            for _ in 0..hold { std::thread::sleep(Duration::from_secs(1)); }
            Ok(json!({"frame":value,"sim_commands_added_by_repaint":0,
                "physical_discovery_disabled":true,"profile_bytes_unchanged":true}))
        }})();
        unsafe { PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0); }
        std::fs::write(folder.join("interaction.json"), serde_json::to_vec_pretty(&match &result {
            Ok(value)=>value.clone(),Err(error)=>json!({"error":error}),
        }).unwrap()).unwrap();
        result
    });
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(150)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}"); assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(directory.join("profiles.json")).unwrap(), before);
    assert!(WINDOW.load(Ordering::Acquire) == 0);
    eprintln!("Dashboard pack evidence: {}", directory.display());
}

#[path="studio_reactive_interaction_tests.rs"]
mod reactive;
#[path="studio_ambient_interaction_tests.rs"]
mod ambient;

#[path="studio_session_interaction_tests.rs"]
mod studio_session;

#[path="studio_presets_interaction_tests.rs"]
mod studio_presets;

#[path="studio_audio_interaction_tests.rs"]
mod audio;
