use super::*;
use std::time::SystemTime;

#[test]
#[ignore = "opens one owned native window; simulated transport/temp store only; run alone"]
fn native_mouse_function_save_cancel_reopen_and_dirty_transition() {
    let directory = std::env::temp_dir().join(format!(
        "mouse-function-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
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
    assert!(engine.event_paths().is_empty());
    let device = &engine.devices()[0];
    let key = device.key();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(device);
    profiles.profiles.clear();
    profiles.profiles.insert(
        "Fixture".into(),
        secbird_core::profile_compat::new_native_profile(device.pid, "Fixture").unwrap(),
    );
    profiles.active = "Fixture".into();
    store.save().unwrap();
    let controller_directory = directory.clone();
    let controller_sim = sim.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| f["presented"] == true)
                },
                "native frame",
            )?;
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    2048,
                    1104,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                redraw(hwnd);
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = client.right as f64 / scale;
                let height = client.bottom as f64 / scale;
                let total = Tab::Customize.width()
                    + Tab::Performance.width()
                    + Tab::Power.width()
                    + Tab::Calibration.width()
                    + Tab::Advanced.width()
                    + 100;
                lighting_click(
                    hwnd,
                    ((width as i32 - total) / 2 + Tab::Customize.width() / 2) as f64,
                    102.0,
                    scale,
                );
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
                let click = |r: assignment_layout::Rect| {
                    lighting_click(hwnd, r.x + r.width / 2.0, r.y + r.height / 2.0, scale)
                };
                let mouse = assignment_geometry::mouse(
                    width,
                    false,
                    0.0,
                    0.0,
                    assignment_geometry::MOUSE_LABEL_WIDTHS,
                );
                let path = controller_directory.join("profiles.json");
                let before = std::fs::read(&path).unwrap();
                let before_log = controller_sim.state().log.len();
                click(mouse.buttons[1]);
                let opened = query();
                if opened["input"] != "RightClick" {
                    return Err(format!("Wrong input {opened}"));
                }
                let x = opened["popup"]["x"].as_f64().unwrap();
                let y = opened["popup"]["y"].as_f64().unwrap();
                let shell = assignment_layout::shell(height - 42.0, 42.0, x, 36.0, false).unwrap();
                let form = assignment_geometry::form(
                    shell,
                    196,
                    Some(assignment_model::Category::Mouse),
                    false,
                    true,
                    0.0,
                );
                let choose = |index: usize| {
                    lighting_click(hwnd, x + 20.0, y + 140.0, scale);
                    click(form.dropdown);
                    let panel = assignment_geometry::options(
                        form.dropdown,
                        shell.body.y + shell.body.height,
                    );
                    click(assignment_geometry::option_rows(panel, 0.0)[index]);
                };
                choose(3);
                if query()["view"]["can_save"] != true {
                    return Err(format!("Save not enabled {}", query()));
                }
                let mut capture = json!(controller_directory
                    .join("mouse-function-draft.png")
                    .to_str()
                    .unwrap());
                if SendMessageW(hwnd, STUDIO_TEST, 6, &mut capture as *mut Value as isize) != 1 {
                    return Err("Draft capture failed".into());
                }
                click(form.cancel);
                if query()["open"] == true || std::fs::read(&path).unwrap() != before {
                    return Err("Cancel changed storage or left draft open".into());
                }
                click(mouse.buttons[1]);
                choose(3);
                click(form.save);
                wait_for(
                    || query()["open"] == false && query()["pending"] == 0,
                    "Save completion",
                )?;
                let saved = std::fs::read(&path).unwrap();
                if saved == before {
                    return Err("Save did not persist".into());
                }
                let loaded = Store::load(&path).unwrap();
                let p = loaded.devices[&key].active();
                if !p.buttons.is_empty() {
                    return Err("Local Save added firmware replay entries".into());
                }
                let maps =
                    secbird_core::profile_compat::mouse_assignment::effective_mappings(p, 196)
                        .map_err(|e| e.to_string())?;
                if maps[0]["mouseGroup"]["mouseAssignment"] != "DoubleClick" {
                    return Err("Wrong saved choice".into());
                }
                click(mouse.buttons[1]);
                if query()["category"] != "MOUSE_FUNCTION" || query()["view"]["mouse_option"] != 3 {
                    return Err(format!("Reopen lost assignment {}", query()));
                }
                choose(6);
                // Closing a dirty draft uses the actual alert Save and pending Close.
                lighting_click(hwnd, x + 265.0, y - 16.0, scale);
                if query()["prompt"] != true {
                    return Err("Missing dirty prompt".into());
                }
                click(assignment_geometry::alert(width, height).save);
                wait_for(
                    || query()["open"] == false && query()["pending"] == 0,
                    "Alert Save completion",
                )?;
                click(mouse.buttons[1]);
                if query()["view"]["mouse_option"] != 6 {
                    return Err("Dirty alert Save did not roundtrip".into());
                }
                // Stale full-profile snapshot must refuse, retain draft, preserve file.
                choose(4);
                let mut external = Store::load(&path).unwrap();
                external
                    .devices
                    .get_mut(&key)
                    .unwrap()
                    .active_mut()
                    .extra
                    .insert("future".into(), json!({"changed":true}));
                external.save().unwrap();
                let changed = std::fs::read(&path).unwrap();
                click(form.save);
                wait_for(
                    || query()["pending"] == 0 && query()["view"]["saving"] == false,
                    "stale refusal",
                )?;
                if query()["open"] != true
                    || query()["dirty"] != true
                    || std::fs::read(&path).unwrap() != changed
                {
                    return Err("Stale Save lost draft or overwrote storage".into());
                }
                wait_for(
                    || {
                        let dialog =
                            FindWindowW(wide("#32770").as_ptr(), wide(FAILURE_TITLE).as_ptr());
                        if dialog.is_null() || GetWindow(dialog, GW_OWNER) != hwnd {
                            return false;
                        }
                        PostMessageW(dialog, WM_CLOSE, 0, 0) != 0
                    },
                    "owned stale-error acknowledgment",
                )?;
                wait_for(
                    || GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_DISABLED == 0,
                    "owned stale-error modal release",
                )?;
                click(form.cancel);
                if controller_sim.state().log[before_log..]
                    .iter()
                    .any(|c| c.id & 0x80 == 0)
                {
                    return Err("Assignment editor issued a device SET".into());
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"cancel_unchanged":true,"save_reopen":true,"dirty_alert_save":true,"stale_refused_draft_retained":true,"firmware_replay_entries_added":0,"device_sets":0,"physical_discovery":false,"client_width":client.right,"client_height":client.bottom,"scale":scale}),
                )
            }
        })();
        if result.is_err() {
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let mut value = Value::Null;
                SendMessageW(
                    hwnd,
                    ASSIGNMENT_TEST_QUERY,
                    1,
                    &mut value as *mut Value as isize,
                );
                let dialog = FindWindowW(wide("#32770").as_ptr(), wide(FAILURE_TITLE).as_ptr());
                if !dialog.is_null() && GetWindow(dialog, GW_OWNER) == hwnd {
                    PostMessageW(dialog, WM_CLOSE, 0, 0);
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    let window = run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(20)),
        None,
    );
    let result = controller
        .join()
        .unwrap()
        .expect("Mouse Function native workflow failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Mouse Function evidence: {}", directory.display());
}
