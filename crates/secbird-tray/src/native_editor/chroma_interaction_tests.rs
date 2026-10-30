use super::super::chroma_view;
use super::*;

#[test]
#[ignore = "opens isolated native windows with simulated keyboard only; run alone"]
fn native_chroma_global_device_persistence_readonly_and_stale_profile_refusal() {
    let folder = std::env::temp_dir().join(format!(
        "native-chroma-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let mut before_other = Value::Null;
    for phase in 0..2 {
        let frame = folder.join(format!("frame-{phase}.json"));
        let _environment = Environment::set(&[
            ("SYNAPSE_LITE_CONFIG_DIR", &folder),
            ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
            ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("chroma")),
        ]);
        let (mut engine, spy) = Engine::with_studio_output_simulator(folder.join("profiles.json"));
        engine.set_uniform_worker(
            true,
            None,
            Some(crate::uniform_worker::Signal::running_for_test()),
        );
        let sim = spy.sim.clone();
        let key = engine.devices()[0].key();
        let (mut second, second_spy) = crate::host_static::test_support::fixture();
        let second_sim = second_spy.sim.clone();
        second.serial = "CHROMA_SECOND".into();
        second.path = std::ffi::CString::new("sim:chroma-second").unwrap();
        second.interface = 2;
        let second_key = second.key();
        engine.add_simulated_device(second);
        if phase == 0 {
            let mut store = Store::load(&folder.join("profiles.json")).unwrap();
            let profiles = store.for_device(&engine.devices()[0]);
            profiles.profiles.clear();
            profiles.profiles.insert(
                "Work".into(),
                secbird_core::profile_compat::new_native_profile(605, "Work").unwrap(),
            );
            profiles.profiles.insert(
                "Other".into(),
                secbird_core::profile_compat::new_native_profile(605, "Other").unwrap(),
            );
            profiles.active = "Work".into();
            before_other = serde_json::to_value(&profiles.profiles["Other"]).unwrap();
            let second_profiles = profiles.clone();
            store.devices.insert(second_key.clone(), second_profiles);
            store.save().unwrap();
        }
        assert!(engine.rescan().is_empty());
        let observed = Arc::new(Mutex::new(engine));
        let monitor = sim.clone();
        let output = folder.clone();
        let owned = observed.clone();
        let target = key.clone();
        let other_target = second_key.clone();
        let other_monitor = second_sim.clone();
        let controller = std::thread::spawn(move || {
            let result = (|| -> Result<Value, String> {
                unsafe {
                    wait_for(|| frame.exists(), "Chroma window")?;
                    let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                    let query = || {
                        let mut value = Value::Null;
                        SendMessageW(
                            hwnd,
                            chroma_view::TEST,
                            1,
                            &mut value as *mut Value as isize,
                        );
                        value
                    };
                    wait_for(
                        || query()["open"] == true && query()["pending"] == 0,
                        "Chroma ready",
                    )?;
                    let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                    let mut rect: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    let width = (f64::from(rect.right) / scale) as i32;
                    let left = ((width - 1120) / 2).max(30);
                    let room = (width - 2 * left).min(1120);
                    if phase == 0 {
                        // Click the retained global Spectrum button, then explicit Apply.
                        redraw(hwnd);
                        lighting_click(hwnd, f64::from(left + 70), 202., scale);
                        if query()["effect"] != 3 {
                            return Err("Global effect strip did not select Spectrum".into());
                        }
                        lighting_click(hwnd, f64::from(left + room - 75), 240., scale);
                        wait_for(
                            || {
                                query()["busy"] == false
                                    && query()["report"]["results"][0]["ok"] == true
                            },
                            "global selection persistence",
                        )
                        .map_err(|e| format!("{e}: {}", query()))?;
                        let saved = Store::load(&output.join("profiles.json"))
                            .map_err(|e| e.to_string())?;
                        if secbird_core::profile_compat::source_quick_effects(
                            saved.devices[&target].active(),
                        )
                        .unwrap()["selectedEffectId"]
                            != 3
                        {
                            return Err("Global effect was not saved".into());
                        }
                        if query()["report"]["results"].as_array().map(Vec::len) != Some(2)
                            || saved.devices[&other_target].active != "Work"
                            || secbird_core::profile_compat::source_quick_effects(
                                saved.devices[&other_target].active(),
                            )
                            .unwrap()["selectedEffectId"]
                                != 3
                        {
                            return Err(
                                "Global selection did not reach both exact simulated devices"
                                    .into(),
                            );
                        }
                        let screenshot =
                            json!(output.join("chroma-dashboard.png").to_string_lossy());
                        SendMessageW(hwnd, STUDIO_TEST, 6, &screenshot as *const Value as isize);
                        redraw(hwnd);
                        lighting_click(hwnd, f64::from(left + 130), 523., scale);
                        if query()["modal"] != target {
                            return Err("Device card did not open lighting modal".into());
                        }
                        let x = (width - 900) / 2;
                        redraw(hwnd);
                        lighting_click(hwnd, f64::from(x + 24 + 162 + 60), 369., scale); // Static green
                        if query()["effect"] != 1 {
                            return Err(format!("Device strip did not select Static: {}", query()));
                        }
                        lighting_click(hwnd, f64::from(x + 90), 419., scale);
                        wait_for(
                            || {
                                query()["busy"] == false
                                    && query()["report"]["effect"] == 1
                                    && query()["report"]["results"][0]["ok"] == true
                            },
                            "device selection persistence",
                        )?;
                        let screenshot = json!(output.join("chroma-device.png").to_string_lossy());
                        SendMessageW(hwnd, STUDIO_TEST, 6, &screenshot as *const Value as isize);
                        let baseline = std::fs::read(output.join("profiles.json")).unwrap();
                        let packets = monitor.state().log.len();
                        SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                        SendMessageW(hwnd, chroma_view::TEST, 2, 2);
                        SendMessageW(hwnd, chroma_view::TEST, 3, 0);
                        if std::fs::read(output.join("profiles.json")).unwrap() != baseline
                            || monitor.state().log.len() != packets
                        {
                            return Err("Read-only dashboard changed disk or hardware".into());
                        }
                        SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                        SendMessageW(hwnd, chroma_view::TEST, 5, 0);
                        SendMessageW(hwnd, chroma_view::TEST, 6, 0);
                        if query()["open"] != false {
                            return Err("Home navigation failed".into());
                        }
                        SendMessageW(hwnd, chroma_view::TEST, 0, 0);
                        if query()["open"] != true {
                            return Err("Dashboard reopen failed".into());
                        }
                        // A later target changes profile. Whole-selection preflight
                        // must refuse before writing even the unchanged first target.
                        let mut saved = Store::load(&output.join("profiles.json")).unwrap();
                        saved.devices.get_mut(&other_target).unwrap().active = "Other".into();
                        saved.save().unwrap();
                        let baseline = std::fs::read(output.join("profiles.json")).unwrap();
                        monitor.state().log.clear();
                        other_monitor.state().log.clear();
                        SendMessageW(hwnd, chroma_view::TEST, 2, 3);
                        SendMessageW(hwnd, chroma_view::TEST, 3, 0);
                        wait_for(
                            || query()["busy"] == false && query()["failed"] == true,
                            "stale profile refusal",
                        )?;
                        if std::fs::read(output.join("profiles.json")).unwrap() != baseline
                            || monitor
                                .state()
                                .log
                                .iter()
                                .any(|packet| packet.id & 0x80 == 0)
                            || other_monitor
                                .state()
                                .log
                                .iter()
                                .any(|packet| packet.id & 0x80 == 0)
                        {
                            return Err("Stale global selection wrote data".into());
                        }
                        let mut saved = Store::load(&output.join("profiles.json")).unwrap();
                        saved.devices.get_mut(&other_target).unwrap().active = "Work".into();
                        saved.save().unwrap();
                    } else {
                        let state =
                            server::dispatch(&mut owned.lock().unwrap(), "GET", "/api/state", &[])
                                .map_err(|e| e.to_string())?;
                        if state["devices"][0]["source_quick_effects"]["selectedEffectId"] != 1 {
                            return Err(
                                "Per-device selection did not survive window/engine recreation"
                                    .into(),
                            );
                        }
                        monitor.state().log.clear();
                        SendMessageW(hwnd, chroma_view::TEST, 0, 0);
                        SendMessageW(hwnd, chroma_view::TEST, 6, 0);
                        if monitor
                            .state()
                            .log
                            .iter()
                            .any(|packet| packet.id & 0x80 == 0)
                        {
                            return Err("Navigation applied a saved effect automatically".into());
                        }
                    }
                    Ok(
                        json!({"phase":phase,"global_spectrum_saved":true,"device_static_saved":true,"selection_survives_restart":phase==1,"read_only_inert":true,"stale_profile_refused_before_set":true,"physical_discovery_disabled":true}),
                    )
                }
            })();
            unsafe {
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            std::fs::write(
                output.join(format!("interaction-{phase}.json")),
                serde_json::to_vec_pretty(&match &result {
                    Ok(v) => v.clone(),
                    Err(e) => json!({"error":e}),
                })
                .unwrap(),
            )
            .unwrap();
            result
        });
        let run = run_standalone(observed, Some(Duration::from_secs(40)));
        let result = controller.join().unwrap();
        assert!(run.is_ok(), "{run:?}");
        assert!(result.is_ok(), "{result:?}");
        let saved = Store::load(&folder.join("profiles.json")).unwrap();
        assert_eq!(saved.devices[&key].active, "Work");
        assert_eq!(saved.devices[&second_key].active, "Work");
        assert_eq!(
            secbird_core::profile_compat::source_quick_effects(saved.devices[&second_key].active())
                .unwrap()["selectedEffectId"],
            3
        );
        assert_eq!(
            serde_json::to_value(&saved.devices[&key].profiles["Other"]).unwrap(),
            before_other
        );
    }
    eprintln!("Chroma native evidence: {}", folder.display());
}

#[test]
#[ignore = "requires selected private stock export and pinned assets; isolated simulated shared-worker window"]
fn native_chroma_studio_navigation_conflict_stop_and_quick_recovery() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select private export"),
    );
    let original = std::fs::read(&input).unwrap();
    let folder = std::env::temp_dir().join(format!(
        "native-chroma-studio-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("chroma")),
    ]);
    let (engine, spy) = Engine::with_studio_output_simulator(folder.join("profiles.json"));
    let key = engine.devices()[0].key();
    let mut profile = secbird_core::profile_compat::new_native_profile(605, "Default").unwrap();
    crate::host_uniform::Program::Spectrum
        .opt_in(&mut profile)
        .unwrap();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    store
        .for_device(&engine.devices()[0])
        .profiles
        .insert("Default".into(), profile);
    store.save().unwrap();
    let baseline = std::fs::read(folder.join("profiles.json")).unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let mut worker = crate::uniform_worker::Worker::start(engine.clone()).unwrap();
    let owned = engine.clone();
    let output = folder.clone();
    let source = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "Chroma Studio window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut v as *mut Value as isize);
                    v
                };
                let chroma = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, chroma_view::TEST, 1, &mut v as *mut Value as isize);
                    v
                };
                wait_for(
                    || {
                        chroma()["open"] == true
                            && query()["pending"] == 0
                            && query()["output_control_ready"] == true
                    },
                    "dashboard worker ready",
                )?;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let width = (f64::from(rect.right) / scale) as i32;
                let left = ((width - 1120) / 2).max(30);
                redraw(hwnd);
                lighting_click(hwnd, f64::from(width - left - 230), 108., scale);
                if query()["open"] != true {
                    return Err("Dashboard Studio navigation failed".into());
                }
                let path = json!(source.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "saved composition load",
                )?;
                redraw(hwnd);
                lighting_click(hwnd, 914., 100., scale);
                wait_for(
                    || {
                        owned.lock().unwrap().studio_status()["accepted_frames"]
                            .as_u64()
                            .unwrap_or(0)
                            >= 3
                    },
                    "saved Apply through shared worker",
                )?;
                wait_for(|| query()["output_busy"] == false, "Apply completion")?;
                SendMessageW(hwnd, chroma_view::TEST, 0, 0);
                wait_for(|| chroma()["open"] == true, "return to Chroma")?;
                SendMessageW(hwnd, chroma_view::TEST, 2, 1);
                SendMessageW(hwnd, chroma_view::TEST, 3, 0);
                wait_for(
                    || chroma()["busy"] == false && chroma()["failed"] == true,
                    "Studio ownership conflict refusal",
                )?;
                if !chroma()["status"]
                    .as_str()
                    .unwrap_or("")
                    .contains("Stop Studio")
                {
                    return Err(format!("Ownership refusal was not explicit: {}", chroma()));
                }
                let screenshot = json!(output.join("chroma-studio-conflict.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &screenshot as *const Value as isize);
                redraw(hwnd);
                lighting_click(hwnd, f64::from(width - left - 80), 108., scale);
                wait_for(
                    || owned.lock().unwrap().studio_status()["state"] == "stopped",
                    "dashboard Stop drains Studio",
                )?;
                wait_for(|| query()["output_busy"] == false, "Stop completion")?;
                wait_for(
                    || {
                        owned.lock().unwrap().lighting_runtime_status()[&key]["uniform_host"]
                            ["accepted_frames"]
                            .as_u64()
                            .unwrap_or(0)
                            >= 1
                    },
                    "saved Quick resumes after Stop",
                )?;
                if owned.lock().unwrap().studio_holds(&key) {
                    return Err("Studio retained the device after Stop".into());
                }
                SendMessageW(hwnd, STUDIO_TEST, 0, 0);
                wait_for(|| query()["open"] == true, "Studio reopen")?;
                redraw(hwnd);
                lighting_click(hwnd, 100., 184., scale);
                SendMessageW(hwnd, STUDIO_TEST, 7, 0);
                if query()["dirty"] != true {
                    return Err("Expected unsaved Studio draft".into());
                }
                SendMessageW(hwnd, chroma_view::TEST, 0, 0);
                if chroma()["open"] == true || query()["dirty"] != true || IsWindow(hwnd) == 0 {
                    return Err("Chroma navigation lost unsaved Studio work".into());
                }
                Ok(
                    json!({"native_dashboard_studio_navigation":true,"shared_worker_apply":true,"quick_refused_during_studio":true,"dashboard_stop_drained":true,"saved_quick_resumed":true,"unsaved_studio_navigation_refused_without_exit":true,"hardware":"simulated"}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        std::fs::write(
            output.join("interaction.json"),
            serde_json::to_vec_pretty(&match &result {
                Ok(v) => v.clone(),
                Err(e) => json!({"error":e}),
            })
            .unwrap(),
        )
        .unwrap();
        result
    });
    let run = run_standalone(engine.clone(), Some(Duration::from_secs(40)));
    let result = controller.join().unwrap();
    assert!(worker.stop(Duration::from_secs(1)));
    engine.lock().unwrap().set_uniform_worker(false, None, None);
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(input).unwrap(), original);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    assert_eq!(spy.state.lock().unwrap().mode, 3);
    eprintln!("Chroma Studio native evidence: {}", folder.display());
}
