use super::*;

#[test]
#[ignore = "owned simulated native window, serial only"]
fn native_windows_assignment_saves_reopens_and_refuses_stale_and_readonly() {
    let directory =
        std::env::temp_dir().join(format!("synapse-windows-native-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-customize")),
    ]);
    let sim = Sim::keyboard();
    let mut engine =
        Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    assert!(engine.event_paths().is_empty());
    let id = engine.devices()[0].key();
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    profile.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([{"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"future","opaque":[7,8]}]);
    profile.extra.insert("opaque".into(), json!({"keep":true}));
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profiles = store.for_device(&engine.devices()[0]);
    profiles.profiles.clear();
    profiles.profiles.insert("Fixture".into(), profile.clone());
    profiles.active = "Fixture".into();
    store.save().unwrap();
    let before = std::fs::read(directory.join("profiles.json")).unwrap();
    let log = sim.state().log.len();
    let bad = json!({"id":id,"expected_profile":"stale","original_profile":profile,"input":"KEY_A","input_type":"KeyInput","hyper":false,"choice":"Copy"});
    assert!(server::dispatch(
        &mut engine,
        "POST",
        "/api/assignment-windows-local",
        &serde_json::to_vec(&bad).unwrap()
    )
    .is_err());
    let mut bad = bad;
    bad["expected_profile"] = json!("Fixture");
    bad["original_profile"]["opaque"] = json!(true);
    assert!(server::dispatch(
        &mut engine,
        "POST",
        "/api/assignment-windows-local",
        &serde_json::to_vec(&bad).unwrap()
    )
    .is_err());
    assert_eq!(sim.state().log.len(), log);
    assert_eq!(
        std::fs::read(directory.join("profiles.json")).unwrap(),
        before
    );
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let dir = directory.clone();
    let controller = std::thread::spawn(move || -> Result<(), String> {
        let result = (|| unsafe {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|v| v["presented"] == true)
                },
                "native frame",
            )?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
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
            let mut size = Value::Null;
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                3,
                &mut size as *mut Value as isize,
            );
            let scale = size["scale"].as_f64().unwrap();
            let width = size["width"].as_f64().unwrap() / scale;
            let wheel = |x:f64,y:f64| {
                let mut screen=POINT{x:(x*scale).round() as i32,y:(y*scale).round() as i32};
                ClientToScreen(hwnd,&mut screen);
                let at=(screen.x as u16 as usize | ((screen.y as u16 as usize)<<16)) as isize;
                native_send(hwnd,WM_MOUSEWHEEL,(-120i16 as u16 as usize)<<16,at);
            };
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
                .ok_or("KEY_A source path")?;
            let open = || {
                native_click(
                    hwnd,
                    (key.left + key.right) / 2.0,
                    (key.top + key.bottom) / 2.0,
                    scale,
                );
            };
            let click = |name: &str| -> Result<(), String> {
                redraw(hwnd);
                let v = query();
                let h = v["view"]["controls"]
                    .as_array()
                    .and_then(|a| a.iter().find(|h| h["key"] == name))
                    .ok_or_else(|| format!("Missing {name}: {v}"))?;
                native_click(
                    hwnd,
                    h["x"].as_f64().unwrap(),
                    h["y"].as_f64().unwrap(),
                    scale,
                );
                Ok(())
            };
            open();
            if !query()["view"]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["key"] == "windows")
            {
                let v = query();
                let x = v["popup"]["x"].as_f64().unwrap() + 20.0;
                let y = v["popup"]["y"].as_f64().unwrap() + 100.0;
                native_send(hwnd, WM_MOUSEMOVE, 0, position(x, y, scale));
                for _ in 0..7 {
                    wheel(x,y);
                }
                redraw(hwnd);
            }
            click("windows")?;
            let png=json!(dir.join("windows-calculator.png").to_string_lossy());
            SendMessageW(hwnd,STUDIO_TEST,6,&png as *const Value as isize);
            click("options")?;
            // Scroll the actual dropdown to its last source choice.
            let v = query();
            let h = v["view"]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|h| h["key"] == "option-0")
                .ok_or("First dropdown row")?;
            native_send(
                hwnd,
                WM_MOUSEMOVE,
                0,
                position(h["x"].as_f64().unwrap(), h["y"].as_f64().unwrap(), scale),
            );
            for _ in 0..22 {
                wheel(h["x"].as_f64().unwrap(),h["y"].as_f64().unwrap());
            }
            redraw(hwnd);
            click("option-25")?;
            if query()["view"]["windows_option"] != 25 {
                return Err("Last choice not selected".into());
            }
            let png = json!(dir.join("windows-choice.png").to_string_lossy());
            SendMessageW(hwnd, STUDIO_TEST, 6, &png as *const Value as isize);
            let sets = controller_sim
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count();
            click("save")?;
            wait_for(
                || query()["open"] == false && query()["pending"] == 0,
                "local save reply",
            )?;
            let saved = Store::load(&dir.join("profiles.json")).map_err(|e| e.to_string())?;
            let p = saved.devices[&id].active();
            if p.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][1]["win8ShortcutsGroup"]
                ["windowsShortcutAssignment"]
                != "OfficeZoomOut"
            {
                return Err("Wrong saved source ID".into());
            }
            if p.extra["opaque"] != json!({"keep":true})
                || p.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][0]["opaque"]
                    != json!([7, 8])
            {
                return Err("Opaque metadata changed".into());
            }
            if controller_sim
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count()
                != sets
            {
                return Err("Unexpected device SET".into());
            }
            open();
            if query()["view"]["windows_option"] != 25 || query()["dirty"] != false {
                return Err("Reopen did not restore saved choice".into());
            }
            click("options")?;
            click("option-24")?;
            click("cancel")?;
            open();
            click("options")?;click("option-24")?;click("close")?;
            if query()["prompt"]!=true {return Err("Dirty close did not ask to save".into());}
            click("save")?;wait_for(||query()["open"]==false && query()["pending"]==0,"dirty-close save")?;
            let stable = std::fs::read(dir.join("profiles.json")).unwrap();
            open();
            if query()["view"]["windows_option"] != 24 {
                return Err("Dirty-close Save did not persist choice".into());
            }
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
            let mut value = Value::Null;
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                1,
                &mut value as *mut Value as isize,
            );
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
            if std::fs::read(dir.join("profiles.json")).unwrap() != stable {
                return Err("Readonly changed store".into());
            }
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            Ok(())
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
    run_window(engine.clone(), Some(Duration::from_secs(20)), None).unwrap();
    controller.join().unwrap().unwrap();
    let stable = std::fs::read(directory.join("profiles.json")).unwrap();
    let mut engine = engine.lock().unwrap();
    engine.enforce_measurement_read_only().unwrap();
    assert!(server::dispatch(
        &mut engine,
        "POST",
        "/api/assignment-windows-local",
        &serde_json::to_vec(&bad).unwrap()
    )
    .is_err());
    assert_eq!(
        std::fs::read(directory.join("profiles.json")).unwrap(),
        stable
    );
    eprintln!("Windows assignment evidence: {}", directory.display());
}
