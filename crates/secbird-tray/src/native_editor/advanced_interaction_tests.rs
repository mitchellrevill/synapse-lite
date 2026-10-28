//! End-to-end native Advanced controls; simulator only, no physical input injection.
use super::*;
#[test]
#[ignore = "opens native Advanced with isolated simulated registers; run alone"]
fn native_advanced_modes_curve_drag_rotation_and_recovery() {
    let folder = std::env::temp_dir().join(format!(
        "native-advanced-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-advanced")),
    ]);
    let sim = Sim::wireless_mouse();
    {
        let mut state = sim.state();
        state.sensor_calibration = Some([0, 0, 0]);
        state.sensor_dynamic = Some((0, 0, vec![0; 1000]));
        state.firmware_extended = Some([1, 10, 1, 0]);
        state.sensor_rotation = Some([1, 0, 0]);
    }
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let key = engine.devices()[0].key();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    let d = store.for_device(&engine.devices()[0]);
    d.active_mut()
        .extra
        .insert("opaque".into(), json!("preserve"));
    d.profiles.insert("Other".into(), Default::default());
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_engine = engine.clone();
    let device = sim.clone();
    let directory = folder.clone();
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
                    "Advanced presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, RUNTIME_TEST_QUERY, 0, &mut v as *mut Value as isize);
                    v
                };
                wait_for(
                    || {
                        query()["advanced"]["available"] == true
                            && query()["advanced"]["rotation_available"] == true
                    },
                    "Advanced reads",
                )?;
                let ready =
                    || query()["pending"] == 0 && query()["calibration"]["loading"] == false;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let x = ((width - 1240.) / 2.).max(0.) + 10.;
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                redraw(hwnd);
                let before = std::fs::read(directory.join("profiles.json")).unwrap();
                let sets = || {
                    device
                        .state()
                        .log
                        .iter()
                        .filter(|c| c.id & 0x80 == 0)
                        .count()
                };
                let baseline_sets = sets();
                lighting_click(hwnd, x + 265., 200., scale);
                if sets() != baseline_sets
                    || std::fs::read(directory.join("profiles.json")).unwrap() != before
                {
                    return Err("Read-only Advanced wrote data".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                redraw(hwnd);
                lighting_click(hwnd, x + 265., 200., scale);
                wait_for(
                    || ready() && query()["advanced"]["intent"]["state"] == 1,
                    "Sensitivity enabled",
                )?;
                for mode in [1, 2, 3] {
                    lighting_click(hwnd, x + 40. + f64::from(mode) * 278. + 100., 280., scale);
                    wait_for(
                        || ready() && query()["advanced"]["intent"]["mode"] == mode,
                        "Sensitivity mode",
                    )?;
                }
                redraw(hwnd);
                let zoom_store = std::fs::read(directory.join("profiles.json")).unwrap();
                let zoom_sets = sets();
                let scroll_before = query()["advanced"]["scroll"].clone();
                SendMessageW(
                    hwnd,
                    WM_MOUSEWHEEL,
                    ((-120i16 as u16 as usize) << 16) | 8,
                    0,
                );
                if query()["advanced"]["zoom"] != json!([20., 0.5]) {
                    return Err("Ctrl+wheel did not zoom both axes out".into());
                }
                SendMessageW(hwnd, WM_MOUSEWHEEL, (120usize << 16) | 8, 0);
                if query()["advanced"]["zoom"] != json!([15., 0.25])
                    || query()["advanced"]["scroll"] != scroll_before
                    || sets() != zoom_sets
                    || std::fs::read(directory.join("profiles.json")).unwrap() != zoom_store
                {
                    return Err("Ctrl+wheel changed hardware/profile data or page scroll".into());
                }
                if query()["mouse_feedback"]["registered"] != false {
                    return Err("Simulator unexpectedly registered physical input".into());
                }
                let before = query()["advanced"]["intent"]["points"].clone();
                let px = x + 40. + 70. + 20. / 105. * 1010.;
                let py = 343. + 280. - 1.1 / 1.5 * 260.;
                let start = position(px, py, scale);
                let end = position(px + 30., py - 15., scale);
                let count = sets();
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
                if query()["advanced"]["drag"] != true || sets() != count {
                    return Err("Curve did not capture or wrote before release".into());
                }
                SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
                wait_for(|| ready(), "Curve verified after release")?;
                if query()["advanced"]["intent"]["points"] == before {
                    return Err("Curve point did not change".into());
                }
                if query()["advanced"]["intent"]["templateId"] != 3 {
                    return Err("Dragged curve retained a preset template and could be reset by stock Synapse".into());
                }
                let saved = Store::load(&directory.join("profiles.json")).unwrap();
                if saved.devices[&key].profiles["Other"] != Default::default()
                    || saved.devices[&key].profiles["Default"].extra["opaque"] != "preserve"
                {
                    return Err("Advanced overwrote another profile or opaque data".into());
                }
                redraw(hwnd);
                lighting_click(hwnd, x + 40. + 952. + 75., 319., scale);
                if query()["advanced"]["template_open"] != true {
                    return Err("Template dropdown did not open".into());
                }
                SendMessageW(hwnd, WM_KEYDOWN, 0x26, 0);
                SendMessageW(hwnd, WM_KEYDOWN, 0x0d, 0);
                wait_for(
                    || ready() && query()["advanced"]["intent"]["templateId"] == 2,
                    "Jump template dropdown selection",
                )?;
                let seed_points = query()["advanced"]["intent"]["points"].clone();
                if seed_points[1]["x"] != 9.0 {
                    return Err("Dropdown did not load the source Jump template".into());
                }
                redraw(hwnd);
                lighting_click(hwnd, x + 40. + 952. + 75., 319., scale);
                SendMessageW(hwnd, WM_KEYDOWN, 0x28, 0);
                SendMessageW(hwnd, WM_KEYDOWN, 0x0d, 0);
                wait_for(
                    || ready() && query()["advanced"]["intent"]["templateId"] == 3,
                    "None template selection",
                )?;
                if query()["advanced"]["intent"]["points"] != seed_points {
                    return Err("None template replaced the current curve".into());
                }
                redraw(hwnd);
                let before_menu = std::fs::read(directory.join("profiles.json")).unwrap();
                let count = sets();
                lighting_click(hwnd, x + 40. + 952. + 75., 319., scale);
                SendMessageW(hwnd, WM_KEYDOWN, 0x1b, 0);
                if query()["advanced"]["template_open"] != false
                    || sets() != count
                    || std::fs::read(directory.join("profiles.json")).unwrap() != before_menu
                {
                    return Err("Closing template dropdown changed saved/device data".into());
                }
                // Scroll the source Rotation card into view and use its real slider hit.
                for _ in 0..7 {
                    SendMessageW(hwnd, WM_MOUSEWHEEL, (-120i16 as u16 as usize) << 16, 0);
                }
                redraw(hwnd);
                let scroll = query()["advanced"]["scroll"].as_f64().unwrap();
                let rx = (width - 600.) / 2.;
                lighting_click(hwnd, rx + 175., 723. - scroll + 40., scale);
                wait_for(
                    || ready() && query()["advanced"]["rotation"]["isEnabled"] == true,
                    "Rotation enabled",
                )?;
                redraw(hwnd);
                lighting_click(hwnd, rx + 86., 723. - scroll + 97., scale);
                if query()["advanced"]["tool_requested"] != true {
                    return Err("Rotation tool link did not route its action".into());
                }
                let sy = 723. - scroll + 329.;
                let start = position(rx + 300., sy, scale);
                let end = position(rx + 48. + (20. + 44.) / 88. * 504., sy, scale);
                let count = sets();
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, start);
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, end);
                if query()["advanced"]["drag"] != true || sets() != count {
                    return Err("Rotation wrote before release".into());
                }
                SendMessageW(hwnd, WM_LBUTTONUP, 0, end);
                wait_for(
                    || ready() && query()["advanced"]["rotation"]["value"] == 20,
                    "Rotation readback",
                )?;
                if device.state().sensor_rotation != Some([1, 1, 20]) {
                    return Err("Wrong Rotation hardware bytes".into());
                }
                let failures = controller_engine
                    .lock()
                    .unwrap()
                    .shutdown_lighting_runtime();
                if !failures.is_empty() || device.state().sensor_rotation != Some([1, 0, 0]) {
                    return Err(format!("Rotation baseline not restored: {failures:?}"));
                }
                Ok(
                    json!({"actual_hwnd":true,"simulator_only":true,"modes":[1,2,3],"curve_commit_on_release":true,"rotation_commit_on_release":true,"rotation_baseline_restored":true,"profile_scope_preserved":true,"readonly_no_write":true}),
                )
            }
        })();
        unsafe {
            PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0);
        }
        result
    });
    run_window(engine, Some(Duration::from_secs(25)), None).unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        folder.join("interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!("Advanced native evidence: {}", folder.display());
}
