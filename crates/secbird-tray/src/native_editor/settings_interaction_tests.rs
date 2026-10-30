use super::*;

#[test]
#[ignore = "opens one isolated simulated Settings window; run alone with private assets"]
fn native_settings_navigation_persistence_and_read_only_are_consistent() {
    let folder = std::env::temp_dir().join(format!(
        "native-settings-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("dashboard")),
    ]);
    let tray = folder.join("tray.json");
    std::fs::write(&tray, br#"{"future":{"keep":[1,"opaque"]}}"#).unwrap();
    let sim = Sim::mouse();
    let mut engine = Engine::with_simulated_mouse(&sim).unwrap();
    let monitor = sim.clone();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let before = std::fs::read(&tray).unwrap();
    engine.enforce_measurement_read_only().unwrap();
    for key in ["autostart", "start_minimized", "popups", "away_lights"] {
        let body = serde_json::to_vec(&json!({"key":key,"on":true,"data":"dim"})).unwrap();
        assert!(server::dispatch(&mut engine, "POST", "/api/settings", &body).is_err());
    }
    assert_eq!(std::fs::read(&tray).unwrap(), before);
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let output = folder.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "Settings window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(
                        hwnd,
                        super::super::settings_view::TEST,
                        2,
                        &mut value as *mut Value as isize,
                    );
                    value
                };
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                redraw(hwnd);
                lighting_click(hwnd, width - 60., 60., scale);
                wait_for(
                    || {
                        query()["active"] == true
                            && query()["busy"] == false
                            && query()["data"].is_object()
                    },
                    "settings read",
                )?;
                let x = (width - ((width - 80.) / 2.).min(600.) * 2. - 20.) / 2.;
                monitor.state().log.clear();
                lighting_click(hwnd, x + 60., 408., scale);
                wait_for(
                    || query()["data"]["popups"] == false && query()["busy"] == false,
                    "popup persistence",
                )?;
                let settings = crate::settings::TraySettings::read(&tray)?;
                if !settings.popups_off || settings.extra["future"] != json!({"keep":[1,"opaque"]})
                {
                    return Err(
                        "Tray reader disagrees with native page or unknown data lost".into(),
                    );
                }
                // Simulate another settings client; Refresh must show its new value.
                crate::settings::change_local("auto_switch", false, 0, "", false)?;
                lighting_click(hwnd, 60., 60., scale);
                wait_for(
                    || query()["data"]["auto_switch"] == false && query()["busy"] == false,
                    "external settings refresh",
                )?;
                let right = x + ((width - 80.) / 2.).min(600.) + 20.;
                lighting_click(hwnd, right + 60., 292., scale);
                wait_for(
                    || {
                        query()["data"]["lighting_devices"][0]["lighting_control"] == false
                            && query()["busy"] == false
                    },
                    "lighting ownership save",
                )?;
                let store =
                    Store::load(&output.join("profiles.json")).map_err(|e| e.to_string())?;
                if !store
                    .devices
                    .values()
                    .next()
                    .is_some_and(|device| device.lighting_off)
                {
                    return Err("Lighting checkbox did not reach the retained device policy".into());
                }
                let capture = json!(output.join("synapse.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, width / 2. + 45., 106., scale);
                wait_for(
                    || query()["general"] == true && query()["busy"] == false,
                    "GENERAL navigation",
                )?;
                let capture = json!(output.join("general.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, width / 2. - 45., 106., scale);
                wait_for(
                    || query()["general"] == false && query()["busy"] == false,
                    "SYNAPSE navigation",
                )?;
                let before = std::fs::read(&tray).unwrap();
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, x + 60., 408., scale);
                std::thread::sleep(Duration::from_millis(100));
                if std::fs::read(&tray).unwrap() != before {
                    return Err("Read-only click changed settings".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                // A damaged file is retained and the failure is rendered, without a modal retry loop.
                std::fs::write(&tray, b"{bad").unwrap();
                lighting_click(hwnd, 60., 60., scale);
                wait_for(
                    || query()["data"]["settings_error"].is_string() && query()["busy"] == false,
                    "corrupt settings error",
                )?;
                lighting_click(hwnd, x + 60., 408., scale);
                if std::fs::read(&tray).unwrap() != b"{bad" {
                    return Err("Damaged settings were overwritten".into());
                }
                let capture = json!(output.join("failure.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    (1000. * scale) as i32,
                    (750. * scale) as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                redraw(hwnd);
                SendMessageW(
                    hwnd,
                    WM_MOUSEWHEEL,
                    (-120i16 as u16 as usize) << 16,
                    position(800., 600., scale),
                );
                redraw(hwnd);
                if query()["scroll"].as_i64().unwrap_or(0) <= 0 {
                    return Err("Settings wheel did not scroll the stacked view".into());
                }
                let capture = json!(output.join("narrow.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                if monitor
                    .state()
                    .log
                    .iter()
                    .any(|command| command.id & 0x80 == 0)
                {
                    return Err("Local settings issued a hardware SET".into());
                }
                Ok(
                    json!({"navigation":true,"shared_tray_persistence":true,"external_refresh":true,"unknown_preserved":true,"read_only_inert":true,"corruption_retained":true,"hardware_sets":0,"autostart_mutations":0,"stacked_view_scroll":true,"per_device_lighting_control":true,"engine_read_only_refuses":true}),
                )
            }
        })();
        unsafe {
            PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0);
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
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(30)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    eprintln!("Native Settings evidence: {}", folder.display());
}
