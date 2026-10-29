use super::*;

#[test]
#[ignore = "requires private stock export/assets; opens isolated simulated Ambient authoring window"]
fn native_studio_ambient_region_blur_save_history_and_invalid_values() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"),
    );
    let original = std::fs::read(&input).unwrap();
    let folder = std::env::temp_dir().join(format!(
        "native-studio-ambient-{}-{}",
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
                // Loading the isolated retained layout can exceed the general4s
                // interaction wait on a busy build host. Keep startup bounded.
                let startup_deadline=std::time::Instant::now()+Duration::from_secs(20);
                while !frame.exists() && std::time::Instant::now()<startup_deadline {
                    std::thread::sleep(Duration::from_millis(20));
                }
                if !frame.exists() {return Err("Timed out: Ambient window (20s startup budget)".into());}
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                monitor.state().log.clear();
                let path = json!(source.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "composition load",
                )?;
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
                    .ok_or("Ambient layer not selected")?
                    .to_owned();
                if !query()["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == id && row["title"] == "AMBIENT")
                {
                    return Err("Ambient layer was not created by Layer Actions".into());
                }
                let fit = ((width - 540.) / 960.)
                    .min((height - 250.) / 360.)
                    .clamp(0.05, 4.);
                lighting_click(
                    hwnd,
                    width / 2. + (67. - 480.) * fit,
                    (168. + height) / 2. + (76. - 180.) * fit,
                    scale,
                );
                lighting_click(hwnd, right + 112., 290., scale); // Left strip + source defaults.
                for (id, text) in [
                    (2110, "123.5"),
                    (2111, "250"),
                    (2112, "60000.25"),
                    (2113, "65535"),
                ] {
                    let field = GetDlgItem(hwnd, id);
                    if field.is_null() {
                        return Err(format!("Ambient field {id} missing"));
                    }
                    SendMessageW(field, WM_SETTEXT, 0, wide(text).as_ptr() as isize);
                }
                lighting_click(hwnd, right + 100., 477., scale);
                let field = GetDlgItem(hwnd, 2114);
                SendMessageW(field, WM_SETTEXT, 0, wide("9").as_ptr() as isize);
                lighting_click(hwnd, right + 150., 547., scale);
                let capture = json!(output.join("ambient-properties.png").to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                let saved_path = output.join("ambient.ChromaEffects");
                let path = json!(saved_path.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "Ambient save",
                )?;
                let saved = secbird_core::studio::Archive::parse(
                    &std::fs::read(saved_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let saved: Value = serde_json::from_str(saved.profile_json(0).unwrap()).unwrap();
                let layer = saved["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["id"] == id)
                    .ok_or("saved Ambient layer absent")?;
                let params = &layer["regions"][0]["params"];
                if params["screen"] != "custom"
                    || params["blur"] != 9
                    || params["screenRect"]
                        != json!({"Left":123.5,"Top":250.0,"Right":60000.25,"Bottom":65535.0})
                {
                    return Err(format!("Ambient save mismatch: {params}"));
                }
                lighting_click(hwnd, 450., 100., scale);
                if query()["dirty"] != true {
                    return Err("Ambient Undo failed".into());
                }
                lighting_click(hwnd, 520., 100., scale);
                if query()["dirty"] != false {
                    return Err("Ambient Redo failed".into());
                }
                lighting_click(hwnd, 630., 100., scale);
                wait_for(
                    || {
                        query()["preview"]["current"] == true
                            && query()["preview"]["frames"].as_u64().unwrap_or(0) > 1
                    },
                    "synthetic Ambient preview",
                )?;
                if query()["preview"]["synthetic_ambient"] != true || query()["dirty"] != false {
                    return Err(
                        "Ambient preview lacked synthetic provenance or changed saved composition"
                            .into(),
                    );
                }
                let capture = json!(output
                    .join("ambient-synthetic-preview.png")
                    .to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize);
                let deadline=std::time::Instant::now()+Duration::from_secs(12);
                while query()["preview"]["requested"]==true && std::time::Instant::now()<deadline {
                    std::thread::sleep(Duration::from_millis(30));
                }
                if query()["preview"]["requested"]!=false || query()["preview"]["current"]!=false {
                    return Err("Ambient ten-second deadline retained its frame/timer request".into());
                }
                lighting_click(hwnd,630.,100.,scale);
                wait_for(||query()["preview"]["current"]==true,"restarted synthetic preview")?;
                lighting_click(hwnd, 630., 100., scale);
                if query()["preview"]["current"] != false
                    || query()["preview"]["requested"] != false
                {
                    return Err(
                        "Ambient synthetic preview Stop retained its frame/timer request".into(),
                    );
                }
                let before = query()["undo"].clone();
                for text in ["0", "10", "-1"] {
                    SendMessageW(
                        GetDlgItem(hwnd, 2114),
                        WM_SETTEXT,
                        0,
                        wide(text).as_ptr() as isize,
                    );
                    lighting_click(hwnd, right + 150., 547., scale);
                    if query()["dirty"] != false
                        || query()["undo"] != before
                        || query()["failed"] != true
                    {
                        return Err("Invalid Ambient blur changed composition/history".into());
                    }
                }
                SendMessageW(
                    GetDlgItem(hwnd, 2110),
                    WM_SETTEXT,
                    0,
                    wide("NaN").as_ptr() as isize,
                );
                lighting_click(hwnd, right + 100., 477., scale);
                if query()["dirty"] != false
                    || query()["undo"] != before
                    || query()["failed"] != true
                {
                    return Err("Nonfinite Ambient region changed composition/history".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, right + 35., 290., scale);
                if query()["dirty"] != false {
                    return Err("Read-only Ambient changed the composition".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                if !monitor.state().log.is_empty() {
                    return Err("Ambient authoring made device requests".into());
                }
                Ok(
                    json!({"ambient_layer_added":true,"custom_fractional_rect_saved":true,"blur9_saved":true,
                "save_undo_redo":true,"invalid_blur_atomic":true,"nonfinite_rect_atomic":true,
                "read_only_inert":true,"synthetic_preview_and_stop":true,"ten_second_deadline_cleanup":true,
                "device_requests":0,"desktop_capture":false,"physical_discovery_disabled":true}),
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
                Ok(value) => value.clone(),
                Err(error) => json!({"error":error}),
            })
            .unwrap(),
        )
        .unwrap();
        result
    });
    let run = run_standalone(Arc::new(Mutex::new(engine)), Some(Duration::from_secs(60)));
    let result = controller.join().unwrap();
    assert!(run.is_ok(), "{run:?}");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(std::fs::read(input).unwrap(), original);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    eprintln!("Ambient native evidence: {}", folder.display());
}
