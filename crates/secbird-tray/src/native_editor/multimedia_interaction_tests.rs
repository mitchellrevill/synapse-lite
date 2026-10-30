use super::*;
#[test]
#[ignore = "opens one owned native window with simulated transport and temporary store"]
fn native_multimedia_save_cancel_and_readonly() {
    let directory =
        std::env::temp_dir().join(format!("synapse-multimedia-native-{}", std::process::id()));
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
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(&engine.devices()[0]);
    profiles.profiles.clear();
    profiles.active = "Fixture".into();
    profiles.profiles.insert(
        "Fixture".into(),
        secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap(),
    );
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let evidence = directory.clone();
    let observed = sim.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| f["presented"] == true)
                },
                "multimedia frame",
            )?;
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                SetWindowPos(hwnd,null_mut(),0,0,2048,1104,SWP_NOMOVE|SWP_NOZORDER);
                redraw(hwnd);
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let w = client.right as f64 / scale;
                let h = client.bottom as f64 / scale;
                let layout =
                    source_layout::SourceLayout::load(&assets_dir().join("keyboard-layout.json"))?;
                let key = layout
                    .bounds(
                        "KEY_A",
                        source_layout::Viewport {
                            left: assignment_geometry::keyboard_center(w, false) - 365.0,
                            top: 154.0,
                            width: 730.0,
                            height: 387.0,
                        },
                    )
                    .ok_or("KEY_A geometry missing")?;
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
                let open = || {
                    lighting_click(
                        hwnd,
                        (key.left + key.right) / 2.0,
                        (key.top + key.bottom) / 2.0,
                        scale,
                    )
                };
                let capture = |name: &str| -> Result<(), String> {
                    let value = json!(evidence.join(name).to_str().unwrap());
                    if SendMessageW(hwnd, STUDIO_TEST, 6, &value as *const Value as isize) == 0 {
                        return Err("Frame capture failed".into());
                    }
                    Ok(())
                };
                let choose_media = || -> Result<(f64, f64), String> {
                    let q = query();
                    let x = q["popup"]["x"].as_f64().ok_or("Popup missing")?;
                    let y = q["popup"]["y"].as_f64().unwrap();
                    let index = q["categories"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|c| c == "MULTIMEDIA")
                        .ok_or("Media category missing")?;
                    lighting_click(hwnd, x + 20.0, y + index as f64 * 40.0 + 20.0, scale);
                    if query()["category"] != "MULTIMEDIA" {
                        return Err(format!("Media rail hit failed: {}", query()));
                    }
                    Ok((x, y))
                };
                let commands = observed.state().log.len();
                let before = std::fs::read(evidence.join("profiles.json")).unwrap();
                open();
                let (x, y) = choose_media()?;
                capture("media-form.png")?;
                lighting_click(hwnd, x + 85.0, y + 73.0, scale);
                capture("media-choices.png")?;
                // Third row: source Mute Volume. Menu's source row height is25px.
                lighting_click(hwnd, x + 85.0, y + 101.0 + 2.0 * 25.0 + 12.0, scale);
                if query()["media_option"] != 2 {
                    return Err(format!("Dropdown failed: {}", query()));
                }
                let shell = assignment_layout::shell(h - 42.0, 42.0, x, 36.0, false).unwrap();
                let form = assignment_geometry::form(
                    shell,
                    605,
                    Some(assignment_model::Category::Media),
                    false,
                    true,
                    0.0,
                );
                lighting_click(hwnd, form.cancel.x + 50.0, form.cancel.y + 13.0, scale);
                if query()["open"] == true
                    || std::fs::read(evidence.join("profiles.json")).unwrap() != before
                {
                    return Err("Cancel mutated profile".into());
                }
                open();
                choose_media()?;
                // Read-only must block even a valid Save hit.
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                redraw(hwnd);
                lighting_click(hwnd, form.save.x + 50.0, form.save.y + 13.0, scale);
                if query()["pending"] != 0
                    || std::fs::read(evidence.join("profiles.json")).unwrap() != before
                {
                    return Err("Read-only Save submitted".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                redraw(hwnd);
                lighting_click(hwnd, form.save.x + 50.0, form.save.y + 13.0, scale);
                wait_for(
                    || query()["open"] != true && query()["pending"] == 0,
                    "saved multimedia reply",
                )?;
                let saved =
                    Store::load(&evidence.join("profiles.json")).map_err(|e| e.to_string())?;
                let rows = &saved.devices.values().next().unwrap().profiles["Fixture"].extra
                    ["_synapseLiteSynapse4"]["decoded"]["mappings"];
                if rows[0]["multimediaGroup"]
                    != json!({"multimediaAssignment":"VolumeDown","useTurbo":true})
                {
                    return Err(format!("Wrong source record: {rows}"));
                }
                open();
                if query()["category"] != "MULTIMEDIA" || query()["media_option"] != 0 {
                    return Err("Saved selection did not reopen".into());
                }
                capture("media-reopened.png")?;
                let status = query()["status"].clone();
                if !status.as_str().is_some_and(|s| s.contains("unavailable")) {
                    return Err(format!("Dishonest status: {status}"));
                }
                lighting_click(hwnd, form.cancel.x + 50.0, form.cancel.y + 13.0, scale);
                let delta=observed.state().log[commands..].to_vec();
                let sets=delta.iter().filter(|command|command.id & 0x80 == 0).count();
                if sets != 0 {
                    return Err(format!("Multimedia sent simulated SET commands: {delta:?}"));
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"save_reopen":true,"cancel_bytes_unchanged":true,"readonly_no_submission":true,"device_set_commands_added":sets,"snapshot_queries_added":delta.len(),"status":status,"client_width":client.right,"client_height":client.bottom,"dpi":scale*96.0}),
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
    let window = run_window(engine, Some(Duration::from_secs(20)), None);
    let result = controller
        .join()
        .unwrap()
        .expect("Multimedia HWND checks failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Multimedia evidence: {}", directory.display());
}
