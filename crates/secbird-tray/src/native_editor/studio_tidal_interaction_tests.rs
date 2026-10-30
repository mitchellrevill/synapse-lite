use super::*;
#[test]
#[ignore = "opens an isolated simulated Tidal authoring window; requires local stock fixture and assets"]
fn native_studio_tidal_authoring_save_history_center_and_runtime_gate() {
    let input = PathBuf::from(
        std::env::var_os("SYNAPSE_LITE_STOCK_STUDIO_FILE").expect("select read-only stock fixture"),
    );
    let original = std::fs::read(&input).unwrap();
    let folder = std::env::temp_dir().join(format!("native-studio-tidal-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let output = std::env::var_os("SYNAPSE_LITE_TIDAL_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| folder.clone());
    std::fs::create_dir_all(&output).unwrap();
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
    let report = output.clone();
    let monitor = sim.clone();
    let source = input.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(|| frame.exists(), "Tidal window")?;
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
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                let height = f64::from(client.bottom) / scale;
                let right = width - 250.;
                studio_add_effect(hwnd,"Tidal",scale)?;
                let id = query()["selected"]
                    .as_str()
                    .ok_or("Tidal not selected")?
                    .to_owned();
                if !query()["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["id"] == id && r["title"] == "TIDAL")
                {
                    return Err("Tidal layer menu entry failed".into());
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
                let set = |id, text: &str| -> Result<(), String> {
                    let f = GetDlgItem(hwnd, id);
                    if f.is_null() {
                        return Err(format!("Tidal field {id} missing"));
                    }
                    SendMessageW(f, WM_SETTEXT, 0, wide(text).as_ptr() as isize);
                    Ok(())
                };
                set(2142, "10")?;
                lighting_click(hwnd, right + 155., 413., scale);
                if !capture_client(hwnd, &report.join("tidal-native-default.png")) {
                    return Err("Tidal native client screenshot failed".into());
                }
                set(2140, "112233")?;
                lighting_click(hwnd, right + 40., 320., scale);
                lighting_click(hwnd, right + 210., 320., scale);
                lighting_click(hwnd, right + 100., 354., scale);
                if windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(GetDlgItem(
                    hwnd, 2140,
                )) != 0
                {
                    return Err("Random did not disable colour input".into());
                }
                let undo = query()["undo"].clone();
                lighting_click(hwnd, right + 40., 320., scale);
                if query()["undo"] != undo {
                    return Err("Random colour permitted colour mutation".into());
                }
                lighting_click(hwnd, right + 100., 354., scale);
                set(2142, "0")?;
                lighting_click(hwnd, right + 155., 413., scale);
                lighting_click(hwnd, right + 40., 487., scale); // active Outward toggles inward, source behavior
                lighting_click(hwnd, right + 100., 450., scale);
                lighting_click(hwnd, width / 2., (168. + height) / 2., scale);
                lighting_click(hwnd, right + 197., 548., scale);
                set(2143, "7")?;
                lighting_click(hwnd, right + 155., 633., scale);
                if !capture_client(hwnd, &report.join("tidal-native-authored.png")) {
                    return Err("Tidal authored screenshot failed".into());
                }
                let saved_path = report.join("tidal-authored.ChromaEffects");
                let path = json!(saved_path.to_string_lossy());
                SendMessageW(hwnd, STUDIO_TEST, 3, &path as *const Value as isize);
                wait_for(
                    || query()["busy"] == false && query()["dirty"] == false,
                    "Tidal save",
                )?;
                let archive = secbird_core::studio::Archive::parse(
                    &std::fs::read(&saved_path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let saved: Value = serde_json::from_str(archive.profile_json(0).unwrap()).unwrap();
                let layer = saved["layers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|l| l["id"] == id)
                    .ok_or("Saved Tidal missing")?;
                let p = &layer["regions"][0]["params"];
                if p["color"] != 0x112233
                    || p["color2"] != 1677721600u32
                    || p["speed"] != 0
                    || p["randomColor"] != false
                    || p["playbackStart"] != "onSelectedKeys"
                    || p["playbackEnd"] != "after"
                    || p["cycles"] != 7
                    || p["angle"] != 160
                    || p["split"] != true
                {
                    return Err(format!("Tidal saved parameters mismatch: {p}"));
                }
                let layout = &layer["params"];
                if layout["counterclockwise"] != true
                    || layout["centerPointX2"] != 5000
                    || layout["centerPointY2"] != 5020
                    || layout["centerPointY"] != 124
                {
                    return Err(format!("Tidal canvas center mismatch: {layout}"));
                }
                if p.get("counterclockwise").is_some() {
                    return Err("Tidal copied direction into regional overrides".into());
                }
                lighting_click(hwnd, 450., 100., scale);
                if query()["dirty"] != true {
                    return Err("Tidal undo failed".into());
                }
                lighting_click(hwnd, 520., 100., scale);
                if query()["dirty"] != false {
                    return Err("Tidal redo failed".into());
                }
                set(2142, "51")?;
                lighting_click(hwnd, right + 155., 413., scale);
                if query()["dirty"] != false || query()["failed"] != true {
                    return Err("Invalid speed mutated Tidal".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                lighting_click(hwnd, right + 40., 487., scale);
                if query()["dirty"] != false {
                    return Err("Read-only Tidal direction changed".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                lighting_click(hwnd, 630., 100., scale);
                wait_for(|| query()["failed"] == true, "Tidal preview refusal")?;
                if query()["preview"]["current"] == true {
                    return Err("Tidal claimed unverified playback".into());
                }
                if !monitor.state().log.is_empty() {
                    return Err("Tidal authoring issued a device request".into());
                }
                Ok(
                    json!({"add_tidal":true,"colours_and_none_saved":true,"random_disables_colour":true,"speed_zero_saved":true,"active_direction_tab_toggles":true,"canvas_center_saved":true,"playback_start_end_cycles_saved":true,"save_undo_redo":true,"invalid_speed_atomic":true,"read_only_inert":true,"radial_preview_refused":true,"device_requests":0,"physical_discovery_disabled":true,"client_size":[client.right,client.bottom]}),
                )
            }
        })();
        unsafe {
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            SendMessageW(hwnd, STUDIO_TEST, 5, 0);
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        std::fs::write(
            report.join("interaction.json"),
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
    assert_eq!(std::fs::read(input).unwrap(), original);
    assert_eq!(
        std::fs::read(folder.join("profiles.json")).unwrap(),
        baseline
    );
    eprintln!("Tidal evidence: {}", output.display());
}
/// Capture only this test window, including its real child edit controls.
unsafe fn capture_client(hwnd: HWND, path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Graphics::GdiPlus as gp;
    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(hwnd: HWND, dc: HDC, flags: u32) -> i32;
    }
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let src = GetDC(hwnd);
    let dc = CreateCompatibleDC(src);
    let bitmap = CreateCompatibleBitmap(src, rect.right, rect.bottom);
    ReleaseDC(hwnd, src);
    if dc.is_null() || bitmap.is_null() {
        return false;
    }
    let old = SelectObject(dc, bitmap);
    let printed = PrintWindow(hwnd, dc, 1) != 0;
    SelectObject(dc, old);
    let mut image = null_mut();
    let made = gp::GdipCreateBitmapFromHBITMAP(bitmap, null_mut(), &mut image);
    let encoder = windows_sys::core::GUID {
        data1: 0x557cf406,
        data2: 0x1a04,
        data3: 0x11d3,
        data4: [0x9a, 0x73, 0, 0, 0xf8, 0x1e, 0xf3, 0x2e],
    };
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let saved = printed
        && made == gp::Ok
        && gp::GdipSaveImageToFile(image.cast(), name.as_ptr(), &encoder, null()) == gp::Ok;
    if !image.is_null() {
        gp::GdipDisposeImage(image.cast());
    }
    DeleteObject(bitmap);
    DeleteDC(dc);
    saved
}
