use super::*;
#[test]
#[ignore = "opens one simulated native window with private stock export/assets"]
fn native_studio_property_clipboard_context_history_and_save() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select stock export"),
    );
    let original = std::fs::read(&input).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "native-studio-clipboard-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("studio")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let original_store = std::fs::read(directory.join("profiles.json")).unwrap();
    let folder = directory.clone();
    let monitor = sim.clone();
    let source = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for_studio_window(|| frame.exists(), "clipboard window")?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(hwnd, STUDIO_TEST, 2, &mut value as *mut Value as isize);
                    value
                };
                wait_for(|| query()["pending"] == 0, "initial reads")?;
                let path = json!(source.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 1, &path as *const Value as isize);
                wait_for(
                    || query()["profiles"] == 1 && query()["busy"] == false,
                    "composition",
                )?;
                monitor.state().log.clear();
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = client.right as f64 / scale;
                let height = client.bottom as f64 / scale;
                studio_effect_click(hwnd,"Static",scale)?; // new Static
                let fit = ((width - 540.) / 960.)
                    .min((height - 250.) / 360.)
                    .clamp(0.05, 4.);
                let x = width / 2. + (67. - 480.) * fit;
                let y = (118. + height) / 2. + (76. - 180.) * fit;
                lighting_click(hwnd, x, y, scale);
                if query()["selected_leds"] != 1 {
                    return Err(format!("LED targeting failed: {}", query()));
                }
                let set_color = |text: &str| studio_set_color(hwnd,false,text,scale);
                let open = || {
                    let px = (x * scale).round() as u32;
                    let py = (y * scale).round() as u32;
                    SendMessageW(hwnd, WM_RBUTTONUP, 0, ((py << 16) | (px & 0xffff)) as isize);
                };
                let menu = |index: usize| {
                    open();
                    let value = query();
                    let pos = value["property_menu"].as_array().unwrap();
                    lighting_click(
                        hwnd,
                        pos[0].as_i64().unwrap() as f64 + 50.,
                        pos[1].as_i64().unwrap() as f64
                            + 14.
                            + index as f64 * 27.
                            + if index >= 2 { 9. } else { 0. },
                        scale,
                    );
                };
                set_color("123456")?;
                let before = query()["undo"].as_u64().unwrap();
                menu(0);
                if query()["undo"] != before || query()["property_clipboard"] != true {
                    return Err("Copy mutated history or failed".into());
                }
                set_color("ABCDEF")?;
                menu(1);
                if query()["color"] != "123456" {
                    return Err(format!("Paste failed: {}", query()));
                }
                open();
                let capture = json!(folder.join("clipboard-menu.png").to_string_lossy());
                if SendMessageW(hwnd, STUDIO_TEST, 6, &capture as *const Value as isize) != 1 {
                    return Err("Owned menu render failed".into());
                }
                // Dismiss outside the popup before opening Reset.
                SendMessageW(hwnd, WM_CANCELMODE, 0, 0);
                menu(2);
                if query()["color"] != "00FF00" {
                    return Err(format!("Reset failed: {}", query()));
                }
                studio_click(hwnd,"undo",scale)?;
                if query()["color"] != "123456" {
                    return Err("Reset undo failed".into());
                }
                studio_click(hwnd,"redo",scale)?;
                if query()["color"] != "00FF00" {
                    return Err("Reset redo failed".into());
                }
                menu(3);
                if query()["selected_leds"] != 0 {
                    return Err("Remove retained selection".into());
                }
                studio_click(hwnd,"undo",scale)?;
                lighting_click(hwnd, x, y, scale);
                if query()["color"] != "00FF00" {
                    return Err("Remove undo did not restore membership".into());
                }
                // A popup opened in read-only mode must never mutate the document.
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                let history = query()["undo"].clone();
                open();
                if query()["undo"] != history || !query()["property_menu"].is_null() {
                    return Err("Read-only property edit".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                let output = folder.join("clipboard.ChromaEffects");
                let path = json!(output.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(|| query()["busy"] == false, "clipboard save")?;
                if query()["failed"] != false || query()["dirty"] != false {
                    return Err(format!("Save failed: {}", query()));
                }
                if !monitor.state().log.is_empty() {
                    return Err("Property editing made device requests".into());
                }
                Ok(
                    json!({"copy_read_only":true,"paste":true,"reset":true,"remove":true,"undo_redo":true,"saved":true,"simulated_device_requests":0,"physical_discovery_disabled":true}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        std::fs::write(
            folder.join("interaction.json"),
            serde_json::to_vec_pretty(&match &result {
                Ok(value) => value.clone(),
                Err(error) => json!({"error":error}),
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
    assert_eq!(std::fs::read(input).unwrap(), original);
    assert_eq!(
        std::fs::read(directory.join("profiles.json")).unwrap(),
        original_store
    );
    eprintln!("Clipboard native evidence: {}", directory.display());
}
