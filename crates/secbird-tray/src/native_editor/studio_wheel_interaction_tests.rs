use super::*;
#[test]
#[ignore = "opens an isolated simulated Wheel window with recovered local assets"]
fn native_studio_wheel_authoring_save_history_and_no_device_output() {
    let folder = std::env::temp_dir().join(format!("native-studio-wheel-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let frame = folder.join("frame.json");
    let source = folder.join("input.ChromaEffects");
    let mut profile: Value = serde_json::from_str(include_str!(
        "../../../secbird-core/tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    profile["devices"][0]["width"] = json!(960);
    profile["devices"][0]["height"] = json!(360);
    profile["layers"] = json!([{"id":"wheel","type":"layer","name":"wheel","value":13,"title":"WHEEL","visible":true,"params":{"centerPointX":125,"centerPointX2":5000,"centerPointY":123,"centerPointY2":5000,"counterclockwise":false},"regions":[]}]);
    let bytes = serde_json::to_vec(&json!({"version":1,"profiles":[profile]})).unwrap();
    std::fs::write(&source, &bytes).unwrap();
    let _env = Environment::set(&[
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
    let input = source.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "Wheel window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut v as *mut Value as isize);
                    v
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                monitor.state().log.clear();
                let path = json!(input.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "Wheel open",
                )?;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let right = width - 250.;
                let x = GetDlgItem(hwnd, 2131);
                let y = GetDlgItem(hwnd, 2132);
                if x.is_null() || y.is_null() {
                    return Err(format!("Wheel center fields missing: {}", query()));
                }
                SendMessageW(x, WM_SETTEXT, 0, wide("4960").as_ptr() as isize);
                SendMessageW(y, WM_SETTEXT, 0, wide("4880").as_ptr() as isize);
                lighting_click(hwnd, right + 100., 445., scale);
                if query()["dirty"] != true {
                    return Err("Center-only edit was not dirty".into());
                }
                lighting_click(hwnd, 450., 100., scale);
                if query()["dirty"] != false {
                    return Err("Center undo failed".into());
                }
                lighting_click(hwnd, 520., 100., scale);
                lighting_click(hwnd, right + 170., 360., scale);
                let fit = ((width - 540.) / 960.)
                    .min((height - 250.) / 360.)
                    .clamp(0.05, 4.);
                lighting_click(
                    hwnd,
                    width / 2. + (67. - 480.) * fit,
                    (118. + height) / 2. + (76. - 180.) * fit,
                    scale,
                );
                if query()["selected_leds"] != 1 {
                    return Err(format!("LED selection missed the canvas: {}", query()));
                }
                let speed = GetDlgItem(hwnd, 2130);
                SendMessageW(speed, WM_SETTEXT, 0, wide("120").as_ptr() as isize);
                lighting_click(hwnd, right + 170., 298., scale);
                let history = query()["undo"].as_u64().unwrap();
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, position(right + 10., 317., scale));
                SendMessageW(hwnd, WM_MOUSEMOVE, 1, position(right + 230., 317., scale));
                if query()["undo"].as_u64() != Some(history) {
                    return Err("Wheel slider committed before release".into());
                }
                SendMessageW(hwnd, WM_LBUTTONUP, 0, position(right + 230., 317., scale));
                if query()["undo"].as_u64() != Some(history + 1)
                    || edit_text(hwnd, 2130).as_deref() != Some("360")
                {
                    return Err("Wheel slider did not commit one bounded speed edit".into());
                }
                lighting_click(hwnd, 450., 100., scale);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 1, position(right + 10., 317., scale));
                SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
                if query()["undo"].as_u64() != Some(history)
                    || windows_sys::Win32::UI::Input::KeyboardAndMouse::GetCapture() == hwnd
                    || edit_text(hwnd, 2130).as_deref() != Some("120")
                {
                    return Err(
                        "Cancelled Wheel speed drag changed history or retained pointer capture"
                            .into(),
                    );
                }
                let capture = json!(output.join("wheel-properties.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, right + 100., 525., scale);
                let position = GetDlgItem(hwnd, 2103);
                let color = GetDlgItem(hwnd, 2104);
                if position.is_null() || color.is_null() {
                    return Err(format!("Wheel gradient missing: {}", query()));
                }
                SendMessageW(position, WM_SETTEXT, 0, wide("0").as_ptr() as isize);
                SendMessageW(color, WM_SETTEXT, 0, wide("112233").as_ptr() as isize);
                lighting_click(hwnd, right + 70., 493., scale);
                let capture = json!(output.join("wheel-gradient.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                let path = json!(output.join("wheel.ChromaEffects").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "Wheel save",
                )?;
                let saved =
                    std::fs::read(output.join("wheel.ChromaEffects")).map_err(|e| e.to_string())?;
                let archive =
                    secbird_core::studio::Archive::parse(&saved).map_err(|e| e.to_string())?;
                let row: Value = serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
                let row = &row["layers"][0];
                if row["params"]["centerPointX2"] != 4960
                    || row["params"]["centerPointY2"] != 4880
                    || row["params"]["counterclockwise"] != true
                    || row["regions"][0]["params"]["speed"] != 120
                    || row["regions"][0]["params"]["colorStops"][0]["Color"] != 0x112233
                {
                    return Err(format!("Wheel save differs: {row}"));
                }
                lighting_click(hwnd, 630., 100., scale);
                wait_for(
                    || {
                        query()["preview"]["current"] == true
                            && query()["preview"]["frames"].as_u64().unwrap_or(0) >= 3
                    },
                    "Wheel animated preview",
                )?;
                let capture = json!(output.join("wheel-preview.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                lighting_click(hwnd, 630., 100., scale);
                if query()["dirty"] != false {
                    return Err("Preview dirtied saved Wheel".into());
                }
                lighting_click(hwnd, right + 100., 290., scale);
                let selection = query()["selected_leds"].clone();
                lighting_click(hwnd, right + 100., 485., scale);
                lighting_click(hwnd, width / 2., (118. + height) / 2., scale);
                if query()["dirty"] != true || query()["selected_leds"] != selection {
                    return Err(
                        "Center picker changed LED membership or did not edit geometry".into(),
                    );
                }
                lighting_click(hwnd, 450., 100., scale);
                if query()["dirty"] != false {
                    return Err("Center picker undo failed".into());
                }
                let speed = GetDlgItem(hwnd, 2130);
                SendMessageW(speed, WM_SETTEXT, 0, wide("361").as_ptr() as isize);
                lighting_click(hwnd, right + 170., 298., scale);
                if query()["dirty"] != false || query()["failed"] != true {
                    return Err("Invalid speed admitted".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, right + 50., 360., scale);
                if query()["dirty"] != false {
                    return Err("Read-only Wheel edit admitted".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                studio_add_effect(hwnd,"Wheel",scale)?;
                if query()["rows"].as_array().map_or(0, Vec::len) != 2
                    || query()["selected"] == "wheel"
                {
                    return Err("Add Wheel menu did not create a layer".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Authoring issued device requests".into());
                }
                Ok(
                    json!({"center_only_dirty":true,"undo_redo":true,"speed_slider_single_commit":true,"center_picker_preserves_selection":true,"add_wheel":true,"animated_preview":true,"regional_speed":120,"layer_rotation":"counterclockwise","gradient_saved":true,"invalid_speed_atomic":true,"readonly_inert":true,"device_requests":0}),
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
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(40)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(source).unwrap(), bytes);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    eprintln!("Wheel native evidence: {}", folder.display());
}
