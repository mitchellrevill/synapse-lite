use super::*;

#[test]
#[ignore = "opens an isolated native window with simulated keyboard; run alone"]
fn native_keyboard_assignment_save_cancel_layers_roundtrip_and_conflict() {
    let folder = std::env::temp_dir().join(format!(
        "native-keyboard-assignment-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let frame = folder.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-customize")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, folder.join("profiles.json")).unwrap();
    assert!(engine.event_paths().is_empty());
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    let profiles = store.for_device(&engine.devices()[0]);
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
        {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"futureGroup","futureGroup":{"keep":[1,2]}}
    ]);
    profiles.profiles.clear();
    profiles.profiles.insert("Fixture".into(), profile);
    profiles.active = "Fixture".into();
    store.save().unwrap();
    let controller_folder = folder.clone();
    let controller_sim = sim.clone();
    let controller = std::thread::spawn(move || -> Result<(), String> {
        let result = (|| -> Result<(), String> {
            let started = Instant::now();
            while !std::fs::read(&frame).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|f|f["presented"]==true) {
                if started.elapsed()>Duration::from_secs(30) {return Err("Timed out waiting for initial asset/font presentation".into());}
                std::thread::sleep(Duration::from_millis(20));
            }
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            unsafe {
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
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    2048,
                    1104,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                RedrawWindow(hwnd, null(), null_mut(), RDW_INVALIDATE | RDW_UPDATENOW);
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let width = client.right as f64 / scale;
                let height = client.bottom as f64 / scale;
                std::fs::write(controller_folder.join("geometry.json"),serde_json::to_vec_pretty(&json!({"client_physical":[client.right,client.bottom],"scale":scale,"client_css":[width,height],"input":"KEY_A","profile":"Fixture","transport":"simulated only"})).unwrap()).unwrap();
                let layout =
                    source_layout::SourceLayout::load(&assets_dir().join("keyboard-layout.json"))?;
                let key = layout
                    .bounds(
                        "KEY_A",
                        source_layout::Viewport {
                            left: assignment_geometry::keyboard_center(width, false) - 365.0,
                            top: 154.0,
                            width: 730.0,
                            height: 387.0,
                        },
                    )
                    .ok_or("KEY_A missing")?;
                let open = || {
                    lighting_click(
                        hwnd,
                        (key.left + key.right) / 2.0,
                        (key.top + key.bottom) / 2.0,
                        scale,
                    );
                };
                let source_bytes = std::fs::read(controller_folder.join("profiles.json")).unwrap();
                open();
                let x = query()["popup"]["x"].as_f64().unwrap();
                let y = query()["popup"]["y"].as_f64().unwrap();
                lighting_click(hwnd, x + 20.0, y + 100.0, scale);
                if query()["category"] != "KEYBOARD_FUNCTION" {
                    return Err(format!("Keyboard category failed: {}", query()));
                }
                let g = assignment_layout::shell(height - 42.0, 42.0, x, 36.0, false).unwrap();
                let f = assignment_geometry::form(
                    g,
                    605,
                    Some(assignment_model::Category::Keyboard),
                    false,
                    true,
                    0.0,
                );
                // All-keys chooser: C is the third literal source descriptor.
                lighting_click(hwnd, f.dropdown.x + 40.0, f.dropdown.y + 12.0, scale);
                let panel = assignment_geometry::options(f.dropdown, height);
                lighting_click(
                    hwnd,
                    panel.x + 50.0,
                    panel.y + 1.0 + 2.0 * 25.0 + 12.0,
                    scale,
                );
                // Right Ctrl is modifier row 1, right column.
                lighting_click(
                    hwnd,
                    f.display.x + 155.0,
                    f.display.y + 80.0 + 27.0 + 11.0,
                    scale,
                );
                if query()["view"]["keyboard"]["setting"]["key"] != "KEY_C"
                    || query()["view"]["keyboard"]["setting"]["modifiers"]
                        != json!(["KEY_RIGHT_CTRL"])
                {
                    return Err(format!("Chooser/modifier failed: {}", query()));
                }
                lighting_click(hwnd, f.cancel.x + 45.0, f.cancel.y + 13.0, scale);
                if std::fs::read(controller_folder.join("profiles.json")).unwrap() != source_bytes {
                    return Err("Cancel wrote profile".into());
                }
                open();
                lighting_click(hwnd, x + 20.0, y + 100.0, scale);
                if query()["view"]["keyboard"]["can_save"] == true {
                    return Err("Empty key enabled Save".into());
                }
                lighting_click(hwnd, f.dropdown.x + 40.0, f.dropdown.y + 12.0, scale);
                lighting_click(hwnd, panel.x + 50.0, panel.y + 63.0, scale);
                lighting_click(hwnd, f.display.x + 155.0, f.display.y + 118.0, scale);
                if std::env::var_os("SYNAPSE_LITE_KEYBOARD_RENDER_PATH").is_some() {
                    let mut render=Value::Null;
                    SendMessageW(hwnd,ASSIGNMENT_TEST_QUERY,6,&mut render as *mut Value as isize);
                    if render["ok"]!=true {return Err(format!("Render artifact failed: {render}"));}
                }
                // Optional bounded screenshot hold, only for this owned fixture.
                if let Some(ready) = std::env::var_os("SYNAPSE_LITE_KEYBOARD_CAPTURE_READY") {
                    let ready = PathBuf::from(ready);
                    std::fs::write(&ready, b"ready").unwrap();
                    let start = Instant::now();
                    while start.elapsed() < Duration::from_secs(55)
                        && !ready.with_extension("continue").exists()
                    {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                let before = controller_sim
                    .state()
                    .log
                    .iter()
                    .filter(|c| c.id & 0x80 == 0)
                    .count();
                lighting_click(hwnd,g.close.x+18.0,g.close.y+18.0,scale);
                if query()["prompt"] != true {return Err("Dirty close did not prompt".into());}
                let alert=assignment_geometry::alert(width,height);
                lighting_click(hwnd,alert.save.x+50.0,alert.save.y+13.0,scale);
                wait_for(|| query()["open"] == false, "save acknowledged")?;
                let saved = Store::load(&controller_folder.join("profiles.json")).unwrap();
                let p = saved.devices.values().next().unwrap().active();
                let records = &p.extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
                if records[0]["futureGroup"] != json!({"keep":[1,2]})
                    || records[1]["keyboardGroup"]["key"] != "KEY_C"
                {
                    return Err("Layer/opaque mapping changed".into());
                }
                open();
                if query()["view"]["keyboard"]["setting"]["key"] != "KEY_C" {
                    return Err(format!("Reopen failed: {}", query()));
                }
                // Stale external profile edits must refuse, without overwriting.
                lighting_click(hwnd, f.display.x + 45.0, f.display.y + 91.0, scale);
                let mut external = saved.clone();
                external
                    .devices
                    .values_mut()
                    .next()
                    .unwrap()
                    .active_mut()
                    .extra
                    .insert("external".into(), json!({"keep":true}));
                external.save().unwrap();
                let external_bytes =
                    std::fs::read(controller_folder.join("profiles.json")).unwrap();
                lighting_click(hwnd, f.save.x + 45.0, f.save.y + 13.0, scale);
                wait_for(|| acknowledge_owned_failure(hwnd),"Expected stale-profile refusal acknowledged")?;
                wait_for(|| query()["pending"] == 0, "conflict completed")?;
                if std::fs::read(controller_folder.join("profiles.json")).unwrap() != external_bytes
                {
                    return Err("Conflict overwrote external profile".into());
                }
                if controller_sim
                    .state()
                    .log
                    .iter()
                    .filter(|c| c.id & 0x80 == 0)
                    .count()
                    != before
                {
                    return Err("Assignment sent device SET".into());
                }
                let mut ignored = Value::Null;
                SendMessageW(
                    hwnd,
                    ASSIGNMENT_TEST_QUERY,
                    1,
                    &mut ignored as *mut Value as isize,
                );
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            Ok(())
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
                let start=Instant::now();while start.elapsed()<Duration::from_secs(3) && WINDOW.load(Ordering::Acquire)!=0 {acknowledge_owned_failure(hwnd);std::thread::sleep(Duration::from_millis(20));}
            }
        }
        result
    });
    let engine = Arc::new(Mutex::new(engine));
    run_window(engine.clone(),Some(Duration::from_secs(65)),None).unwrap();
    controller.join().unwrap().unwrap();
    let before = std::fs::read(folder.join("profiles.json")).unwrap();
    let mut engine = engine.lock().unwrap();
    engine.enforce_measurement_read_only().unwrap();
    assert!(server::dispatch(&mut engine,"POST","/api/keyboard-assignment-local",b"{}").is_err());
    assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),before);
    eprintln!("Keyboard assignment evidence: {}", folder.display());
}
