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
    PostMessageW(button, BM_CLICK, 0, 0);
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
            let geometry =
                lighting_layout::geometry(width, 128.0, 0.0, 1).ok_or("No Lighting geometry")?;
            let shape = geometry.brightness_slider;
            let point = |percent: f64| {
                position(
                    shape.x + 8.0 + percent / 100.0 * (shape.width - 16.0),
                    shape.y + 36.0,
                    scale,
                )
            };
            let before = brightness_write_count(&controller_sim);
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, point(60.0));
            let thread = GetWindowThreadProcessId(hwnd, null_mut());
            let mut gui: GUITHREADINFO = std::mem::zeroed();
            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            GetGUIThreadInfo(thread, &mut gui);
            if gui.hwndCapture != hwnd {
                return Err(format!("Brightness press did not capture HWND; client={}x{}, scale={}, source track={:?}, capture={:?}",client.right,client.bottom,scale,lighting_layout::track(shape),gui.hwndCapture));
            }
            SendMessageW(hwnd, WM_MOUSEMOVE, 1, point(45.0));
            std::thread::sleep(Duration::from_millis(80));
            if brightness_write_count(&controller_sim) != before {
                return Err("Brightness wrote before release".into());
            }
            SendMessageW(hwnd, WM_LBUTTONUP, 0, point(45.0));
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
            let after_release = brightness_write_count(&controller_sim);
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, point(80.0));
            SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
            SendMessageW(hwnd, WM_LBUTTONUP, 0, point(80.0));
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
            lighting_click(hwnd, anchor.x + 15.0, anchor.y + 12.0, scale);
            let open_edit = |slot: usize| {
                let swatch = palette.custom_slots[slot];
                SendMessageW(
                    hwnd,
                    WM_RBUTTONUP,
                    0,
                    position(swatch.x + 10.0, swatch.y + 10.0, scale),
                );
                redraw(hwnd);
                let menu = color_editor_layout::context_menu(swatch);
                lighting_click(hwnd, menu.x + 20.0, menu.y + 13.0, scale);
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
            lighting_click(
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
            lighting_click(
                hwnd,
                palette.eyedropper.x + 12.0,
                palette.eyedropper.y + 12.0,
                scale,
            );
            wait_for(
                || acknowledge_owned_failure(hwnd),
                "Acknowledged native failure dialog",
            )?;
            let edit = GetDlgItem(hwnd, i32::from(lighting_view::HEX));
            SetWindowTextW(edit, wide("a1b2c3").as_ptr());
            lighting_click(hwnd, palette.save.x + 40.0, palette.save.y + 13.0, scale);
            wait_for(
                || {
                    Store::load(&controller_directory.join("profiles.json"))
                        .ok()
                        .is_some_and(|s| {
                            s.devices
                                .get(&controller_key)
                                .and_then(|d| {
                                    secbird_core::profile_compat::source_quick_effects(d.active())
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
            if brightness_write_count(&controller_sim) != after_release {
                return Err("Color editor unexpectedly changed brightness".into());
            }
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            wait_for(
                || {
                    acknowledge_owned_failure(hwnd);
                    IsWindow(hwnd) == 0
                },
                "Editor closed after failure acknowledgement",
            )?;
            Ok(
                json!({"native_failure_dialog_acknowledged":true,"transport":"simulated only","physical_discovery_disabled":true,"source_region0_readback_not_modeled":true,
                "brightness_capture_verified":true,"source_layout_client_css_width":width,"scale":scale,"brightness_no_write_before_release":true,"brightness_release_raw":114,"brightness_cancel_no_write":true,
                "native_edit_draft_switch_isolated":true,"cancelled_color_retained":true,"opaque_slot_preserved":true,
                "edited_slot_saved":"#a1b2c3","color_actions_did_not_write_brightness":true,
                "evidence":"actual HWND mouse/EDIT notifications and worker persistence; no OS pointer injection or hardware proof"}),
            )
        }
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
