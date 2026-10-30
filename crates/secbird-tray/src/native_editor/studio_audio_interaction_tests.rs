use super::*;
#[test]
#[ignore = "requires private stock export/assets; opens an isolated simulated Audio authoring window"]
fn native_studio_audio_parameters_gradient_save_history_and_capture_refusal() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"),
    );
    let original = std::fs::read(&input).unwrap();
    let folder = std::env::temp_dir().join(format!(
        "native-studio-audio-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let baseline = std::fs::read(folder.join("profiles.json")).unwrap();
    let output = folder.clone();
    let monitor = sim.clone();
    let source = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for_studio_window(|| frame.exists(), "Audio window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut v as *mut Value as isize);
                    v
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                monitor.state().log.clear();
                let path = json!(source.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "composition load",
                )?;
                // Completion updates the model before the next WM_PAINT builds
                // its hit regions. Paint the loaded composition before clicking.
                redraw(hwnd);
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let right = width - 250.;
                lighting_click(hwnd, 775., 100., scale);
                lighting_click(hwnd, 800., 468., scale);
                let id = query()["selected"]
                    .as_str()
                    .ok_or("Audio Meter not selected")?
                    .to_owned();
                if !query()["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["id"] == id && r["title"] == "AUDIO_METER")
                {
                    return Err(format!("Audio Meter layer was not added: {}", query()));
                }
                let fit = ((width - 540.) / 960.)
                    .min((height - 250.) / 360.)
                    .clamp(0.05, 4.);
                lighting_click(
                    hwnd,
                    width / 2. + (67. - 480.) * fit,
                    (118. + height) / 2. + (76. - 180.) * fit,
                    scale,
                );
                let set = |field, text: &str| -> Result<(), String> {
                    let field = GetDlgItem(hwnd, field);
                    if field.is_null() {
                        return Err("Audio property field missing".into());
                    }
                    SendMessageW(field, WM_SETTEXT, 0, wide(text).as_ptr() as isize);
                    Ok(())
                };
                set(2120, "2.25")?;
                lighting_click(hwnd, right + 65., 328., scale);
                set(2121, "0.3")?;
                lighting_click(hwnd, right + 65., 475., scale);
                lighting_click(hwnd, right + 50., 368., scale);
                if windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(GetDlgItem(
                    hwnd, 2120,
                )) != 0
                {
                    return Err("Auto left Boost enabled".into());
                }
                let history = query()["undo"].clone();
                lighting_click(hwnd, right + 65., 328., scale);
                if query()["undo"] != history {
                    return Err("Auto permitted Boost editing".into());
                }
                lighting_click(hwnd, right + 170., 368., scale);
                let capture = json!(output.join("audio-properties.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, right + 65., 515., scale);
                lighting_click(hwnd, right + 75., 360., scale); // preset2
                set(2103, "0")?;
                set(2104, "112233")?;
                lighting_click(hwnd, right + 65., 493., scale);
                let capture = json!(output.join("audio-gradient.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                let saved_path = output.join("audio.ChromaEffects");
                let path = json!(saved_path.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "Audio save",
                )?;
                let saved = secbird_core::studio::Archive::parse(
                    &std::fs::read(&saved_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let saved: Value = serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();
                let layer = saved["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == id)
                    .ok_or("saved Audio absent")?;
                let params = &layer["regions"][0]["params"];
                if params["boost"] != 2.25
                    || params["decay"] != 0.3
                    || params["autoBoost"] != false
                    || params["colorStops"].as_array().map(Vec::len) != Some(7)
                    || params["colorStops"][0]["Color"] != 0x112233
                    || params["colorStopsCustom"] != true
                    || params.get("duration").is_some()
                {
                    return Err(format!("Audio save mismatch: {params}"));
                }
                lighting_click(hwnd, 450., 100., scale);
                if query()["dirty"] != true {
                    return Err("Audio Undo failed".into());
                }
                lighting_click(hwnd, 520., 100., scale);
                if query()["dirty"] != false {
                    return Err("Audio Redo failed".into());
                }
                lighting_click(hwnd, right + 190., 493., scale);
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "Audio None save",
                )?;
                let saved = secbird_core::studio::Archive::parse(
                    &std::fs::read(&saved_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let saved: Value = serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();
                let layer = saved["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == id)
                    .unwrap();
                if layer["regions"][0]["params"]["colorStops"][0]
                    .get("Color")
                    .is_some()
                {
                    return Err("None saved a colour".into());
                }
                lighting_click(hwnd, right + 65., 291., scale);
                set(2120, "0.3")?;
                lighting_click(hwnd, right + 65., 328., scale);
                if query()["dirty"] != false || query()["failed"] != true {
                    return Err("Off-step Boost changed draft".into());
                }
                set(2121, "2.1")?;
                lighting_click(hwnd, right + 65., 475., scale);
                if query()["dirty"] != false || query()["failed"] != true {
                    return Err("Out-of-range Decay changed draft".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                set(2121, "1.2")?;
                lighting_click(hwnd, right + 65., 475., scale);
                if query()["dirty"] != false {
                    return Err("Read-only Audio changed draft".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                lighting_click(hwnd, 630., 100., scale);
                wait_for(|| query()["failed"] == true, "Audio preview refusal")?;
                if query()["preview"]["current"] == true {
                    return Err("Audio falsely presented live playback".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Audio authoring issued a device request".into());
                }
                Ok(
                    json!({"add_audio":true,"boost":2.25,"decay_seconds":0.3,"auto_disables_boost":true,"gradient_stops":7,"custom_colour":0x112233,"none_omits_colour":true,"save_undo_redo":true,"invalid_boost_decay_atomic":true,"read_only_inert":true,"unsupported_preview_refused":true,"device_requests":0,"physical_discovery_disabled":true}),
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
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(35)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(input).unwrap(), original);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    eprintln!("Audio native evidence: {}", folder.display());
}
