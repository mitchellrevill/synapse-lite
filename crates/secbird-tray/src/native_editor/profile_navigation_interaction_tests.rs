//! Real message-loop workflow; simulated transport and private files only.
use super::*;

#[test]
#[ignore = "owned simulated native window, serial only"]
fn native_profile_navigation_selectors_save_reopen_cancel_and_readonly() {
    let folder = std::env::temp_dir().join(format!(
        "native-profile-navigation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let frame = folder.join("frame.json");
    let source = folder.join("effects.ChromaEffects");
    std::fs::write(&source,br#"{"version":1,"profiles":[{"id":"effect-one","title":"Desk","devices":[],"layers":[]},{"id":"effect-two","title":"Night","devices":[],"layers":[]}]}"#).unwrap();
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &folder),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-customize")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, folder.join("profiles.json")).unwrap();
    let id = engine.devices()[0].key();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    let profiles = store.for_device(&engine.devices()[0]);
    profiles.profiles.clear();
    profiles.active = "Fixture".into();
    let mut current = secbird_core::profile_compat::new_native_profile(605, "Fixture").unwrap();
    current.extra.get_mut("_synapseLiteSynapse4").unwrap()["decoded"]["mappings"] = json!([
        {"inputID":"KEY_A","inputType":"KeyInput","isHyperShift":true,"outputType":"future","opaque":[7,8]}
    ]);
    profiles.profiles.insert("Fixture".into(), current);
    let target = secbird_core::profile_compat::new_native_profile(605, "Target").unwrap();
    let target_guid = target.extra["_synapseLiteSynapse4"]["decoded"]["guid"]
        .as_str()
        .unwrap()
        .to_string();
    profiles.profiles.insert("Target".into(), target);
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let directory = folder.clone();
    let observer = sim.clone();
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
            let studio_query = || {
                let mut v = Value::Null;
                SendMessageW(hwnd, STUDIO_TEST, 2, &mut v as *mut Value as isize);
                v
            };
            let source = json!(source.to_string_lossy());
            SendMessageW(hwnd, STUDIO_TEST, 1, &source as *const Value as isize);
            wait_for(
                || studio_query()["profiles"] == 2 && studio_query()["pending"] == 0,
                "local Studio library",
            )?;
            let mut size = Value::Null;
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                3,
                &mut size as *mut Value as isize,
            );
            let scale = size["scale"].as_f64().unwrap();
            let width = size["width"].as_f64().unwrap() / scale;
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
                .ok_or("KEY_A geometry")?;
            let open = || {
                native_click(
                    hwnd,
                    (key.left + key.right) / 2.0,
                    (key.top + key.bottom) / 2.0,
                    scale,
                )
            };
            let click = |name: &str| -> Result<(), String> {
                redraw(hwnd);
                let v = query();
                let control = v["view"]["controls"]
                    .as_array()
                    .and_then(|items| items.iter().find(|c| c["key"] == name))
                    .ok_or_else(|| format!("Missing {name}: {v}"))?;
                native_click(
                    hwnd,
                    control["x"].as_f64().unwrap(),
                    control["y"].as_f64().unwrap(),
                    scale,
                );
                Ok(())
            };
            open();
            click("profile")?;
            click("nav-action-4")?;
            click("nav-targets")?;
            click("option-0")?;
            click("nav-link")?;
            click("nav-effects")?;
            click("option-1")?;
            if query()["view"]["can_save"] != true {
                return Err(format!("Valid navigation is not saveable: {}", query()));
            }
            let png = json!(directory
                .join("profile-navigation-linked.png")
                .to_string_lossy());
            SendMessageW(hwnd, STUDIO_TEST, 6, &png as *const Value as isize);
            let sets = observer
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count();
            let mut renamed =
                Store::load(&directory.join("profiles.json")).map_err(|e| e.to_string())?;
            renamed
                .devices
                .get_mut(&id)
                .unwrap()
                .rename("Target", "Renamed")
                .map_err(|e| e.to_string())?;
            renamed.save().map_err(|e| e.to_string())?;
            click("save")?;
            wait_for(
                || query()["open"] == false && query()["pending"] == 0,
                "local navigation save",
            )?;
            let saved = Store::load(&directory.join("profiles.json")).map_err(|e| e.to_string())?;
            let rows =
                &saved.devices[&id].active().extra["_synapseLiteSynapse4"]["decoded"]["mappings"];
            if rows[1]["profileNavigationGroup"]
                != json!({"profileNavigationAssignment":"Specific","guid":target_guid,"lightpacId":"effect-two"})
                || rows[0]["opaque"] != json!([7, 8])
            {
                return Err("Saved identities or opposite layer changed".into());
            }
            if saved.devices[&id].active != "Fixture" {
                return Err("Save activated a profile".into());
            }
            if !saved.devices[&id].profiles.contains_key("Renamed") {
                return Err("Save overwrote target rename".into());
            }
            let bytes = std::fs::read(directory.join("profiles.json")).unwrap();
            open();
            click("nav-targets")?;
            click("option-0")?;
            if query()["view"]["can_save"] != true {
                return Err("Reopen lost target or lighting selection".into());
            }
            click("nav-action-0")?;
            click("undo")?;
            if query()["dirty"] != false {
                return Err("Undo failed".into());
            }
            click("nav-action-1")?;
            click("cancel")?;
            if std::fs::read(directory.join("profiles.json")).unwrap() != bytes {
                return Err("Cancel changed store".into());
            }
            open();
            click("nav-action-2")?;
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
            SendMessageW(hwnd, ASSIGNMENT_TEST_QUERY, 5, 0);
            SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
            if std::fs::read(directory.join("profiles.json")).unwrap() != bytes {
                return Err("Read-only Save changed store".into());
            }
            let mut ignored = Value::Null;
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                1,
                &mut ignored as *mut Value as isize,
            );
            if observer
                .state()
                .log
                .iter()
                .filter(|c| c.id & 0x80 == 0)
                .count()
                != sets
            {
                return Err("Unexpected device SET".into());
            }
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                1,
                &mut ignored as *mut Value as isize,
            );
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
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
            }
        }
        result
    });
    run_window(engine.clone(), Some(Duration::from_secs(30)), None).unwrap();
    controller.join().unwrap().unwrap();
    // Exercise the same save boundary directly, after the GUI has exited. The
    // owned error-modal acknowledgement needs a separate harness investigation.
    let mut engine = engine.lock().unwrap();
    assert!(engine.event_paths().is_empty());
    let state = server::dispatch(&mut engine, "GET", "/api/state", &[]).unwrap();
    let device = &state["devices"][0];
    let id = device["id"].as_str().unwrap().to_string();
    let mut context = assignment_model::Context::from_device(device).unwrap();
    let archive_path = folder.join("effects.ChromaEffects");
    let archive_bytes = std::fs::read(&archive_path).unwrap();
    let archive = secbird_core::studio::Archive::parse(&archive_bytes).unwrap();
    let catalog = context.navigation.as_mut().unwrap();
    catalog.path = Some(archive_path.clone());
    catalog.effects = assignment_navigation::effect_choices(&archive);
    let input = assignment_model::Input {
        key: "KEY_A".into(),
        input_id: "KEY_A".into(),
        input_type: "KeyInput".into(),
        label: "A".into(),
        default_label: "A".into(),
        functions: vec!["SWITCH_PROFILE".into()],
        enabled: true,
    };
    let mut draft = assignment_model::Draft::new(
        Arc::new(context),
        input,
        assignment_model::Anchor::Key("KEY_A".into()),
        false,
    );
    draft.navigation_action(4);
    assert!(draft.can_save());
    let bytes = std::fs::read(folder.join("profiles.json")).unwrap();
    let sets = sim.state().log.iter().filter(|c| c.id & 0x80 == 0).count();
    std::fs::write(&archive_path, br#"{"version":1,"profiles":[]}"#).unwrap();
    let error = assignment_backend::save(&mut engine, &draft).unwrap_err();
    assert!(error.contains("Linked Chroma effect"), "{error}");
    assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(), bytes);
    std::fs::write(&archive_path, &archive_bytes).unwrap();
    let mut missing = Store::load(&folder.join("profiles.json")).unwrap();
    missing
        .devices
        .get_mut(&id)
        .unwrap()
        .profiles
        .remove("Renamed");
    missing.save().unwrap();
    let deleted_bytes = std::fs::read(folder.join("profiles.json")).unwrap();
    let error = assignment_backend::save(&mut engine, &draft).unwrap_err();
    assert!(
        error.contains("Another profile") || error.contains("Target profile"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        deleted_bytes
    );
    engine.enforce_measurement_read_only().unwrap();
    assert!(assignment_backend::save(&mut engine, &draft)
        .unwrap_err()
        .contains("Read-only"));
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        deleted_bytes
    );
    assert_eq!(
        sim.state().log.iter().filter(|c| c.id & 0x80 == 0).count(),
        sets
    );
    eprintln!("Profile navigation native evidence: {}", folder.display());
}
