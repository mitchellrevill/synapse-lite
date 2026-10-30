use super::*;

unsafe fn query(hwnd: HWND) -> Value {
    let mut value = Value::Null;
    let _ = wait_for(
        || {
            value = Value::Null;
            SendMessageW(
                hwnd,
                ASSIGNMENT_TEST_QUERY,
                4,
                &mut value as *mut Value as isize,
            );
            value["hits"].is_array()
        },
        "assignment diagnostic UI-thread borrow",
    );
    value
}
unsafe fn click(hwnd: HWND, name: &str) -> Result<(), String> {
    let mut hit = Value::Null;
    wait_for(
        || {
            redraw(hwnd);
            let value = query(hwnd);
            hit = value["hits"]
                .as_array()
                .and_then(|rows| rows.iter().rev().find(|h| h["name"] == name))
                .cloned()
                .unwrap_or(Value::Null);
            !hit.is_null()
        },
        &format!("native hit {name}"),
    )?;
    let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
    native_click(
        hwnd,
        hit["x"].as_f64().unwrap() / scale,
        hit["y"].as_f64().unwrap() / scale,
        scale,
    );
    Ok(())
}
unsafe fn edit(hwnd: HWND, id: u16, text: &str) -> Result<(), String> {
    let child = GetDlgItem(hwnd, id as i32);
    if child.is_null() {
        return Err(format!("Missing DPI field {id}"));
    }
    SetWindowTextW(child, wide(text).as_ptr());
    SendMessageW(
        hwnd,
        WM_COMMAND,
        id as usize | ((EN_KILLFOCUS as usize) << 16),
        child as isize,
    );
    redraw(hwnd);
    Ok(())
}
/// Capture only this owned simulated window, without desktop or clipboard access.
unsafe fn capture(hwnd: HWND, path: &Path) {
    redraw(hwnd);
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let dc = GetDC(hwnd);
    let buffer = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, rect.right, rect.bottom);
    let old = SelectObject(buffer, bitmap);
    assert_ne!(
        BitBlt(buffer, 0, 0, rect.right, rect.bottom, dc, 0, 0, SRCCOPY),
        0
    );
    SelectObject(buffer, old);
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = rect.right;
    info.bmiHeader.biHeight = -rect.bottom;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    let mut pixels = vec![0u8; rect.right as usize * rect.bottom as usize * 4];
    assert_ne!(
        GetDIBits(
            dc,
            bitmap,
            0,
            rect.bottom as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS
        ),
        0
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0u8; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(std::slice::from_raw_parts(
        (&info.bmiHeader as *const BITMAPINFOHEADER).cast::<u8>(),
        40,
    ));
    bytes.extend_from_slice(&pixels);
    std::fs::write(path, bytes).unwrap();
    DeleteObject(bitmap);
    DeleteDC(buffer);
    ReleaseDC(hwnd, dc);
}

#[test]
#[ignore = "opens one owned simulated native window; run alone on an interactive desktop"]
fn native_sensitivity_clutch_save_reopen_and_filters() {
    let directory = std::env::var_os("SYNAPSE_LITE_SENSITIVITY_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("sensitivity-native-{}", std::process::id()))
        });
    std::fs::create_dir_all(&directory).unwrap();
    let config = directory.join("fixture-config");
    std::fs::create_dir_all(&config).unwrap();
    let frame = directory.join("frame.json");
    if frame.exists() {
        std::fs::remove_file(&frame).unwrap();
    }
    let _env = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &config),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-performance")),
    ]);
    let sim = Sim::wireless_mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    assert!(engine.event_paths().is_empty());
    let key = engine.devices()[0].key();
    let mut store = Store::load(&config.join("profiles.json")).unwrap();
    let mut profile = secbird_core::profile_compat::new_native_profile(196, "Fixture").unwrap();
    profile
        .extra
        .insert("opaque-sentinel".into(), json!({"future":[1,2,3]}));
    let p = store.for_device(&engine.devices()[0]);
    p.profiles.clear();
    p.profiles.insert("Fixture".into(), profile.clone());
    p.active = "Fixture".into();
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_directory = directory.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
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
            unsafe {
                click(hwnd, "tab:CUSTOMIZE")?;
                let before = sim.state().log.len();
                click(hwnd, "open:Button5")?;
                click(hwnd, "category:SENSITIVITY")?;
                capture(hwnd, &controller_directory.join("clutch-linked.bmp"));
                edit(hwnd, assignment_view::DPI_X, "401")?;
                if query(hwnd)["draft"]["x"] != 401 || query(hwnd)["draft"]["y"] != 401 {
                    return Err("Linked numeric input rounded or did not commit".into());
                }
                edit(hwnd, assignment_view::DPI_X, "99999")?;
                if query(hwnd)["draft"]["x"] != 26000 {
                    return Err("DPI upper clamp failed".into());
                }
                click(hwnd, "xy")?;
                edit(hwnd, assignment_view::DPI_Y, "1200")?;
                edit(hwnd, assignment_view::DPI_X, "401")?;
                click(hwnd, "slider:1")?;
                let stepped = query(hwnd);
                if stepped["draft"]["y"].as_u64().unwrap() % 50 != 0
                    || stepped["draft"]["y"] == 1200
                    || stepped["draft"]["x"] != 401
                {
                    return Err("Y slider step or axis isolation failed".into());
                }
                edit(hwnd, assignment_view::DPI_Y, "1200")?;
                capture(hwnd, &controller_directory.join("clutch-independent.bmp"));
                click(hwnd, "save")?;
                wait_for(
                    || !query(hwnd)["open"].as_bool().unwrap_or(true),
                    "saved draft snapshot",
                )?;
                click(hwnd, "open:Button5")?;
                let reopened = query(hwnd);
                if reopened["category"] != "SENSITIVITY"
                    || reopened["draft"]["x"] != 401
                    || reopened["draft"]["y"] != 1200
                    || reopened["draft"]["independent"] != true
                {
                    return Err(format!("Saved draft did not reopen: {reopened}"));
                }
                capture(hwnd, &controller_directory.join("clutch-reopened.bmp"));
                click(hwnd, "options")?;
                click(hwnd, "option:4")?;
                capture(hwnd, &controller_directory.join("cycle-up.bmp"));
                click(hwnd, "cancel")?;
                click(hwnd, "open:DKM_SB_03")?;
                click(hwnd, "category:SENSITIVITY")?;
                click(hwnd, "options")?;
                let menu = query(hwnd);
                let count = menu["hits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|h| h["name"].as_str().is_some_and(|n| n.starts_with("option:")))
                    .count();
                if count != 4 {
                    return Err(format!("DPI button source exclusions failed: {menu}"));
                }
                capture(hwnd, &controller_directory.join("dpi-button-options.bmp"));
                click(hwnd, "cancel")?;
                let disk = Store::load(&config.join("profiles.json")).unwrap();
                let saved = &disk.devices[&key].profiles["Fixture"];
                if saved.buttons != profile.buttons
                    || saved.extra["opaque-sentinel"] != profile.extra["opaque-sentinel"]
                    || saved.extra["_synapseLiteSynapse4"] != profile.extra["_synapseLiteSynapse4"]
                {
                    return Err("Saved draft changed executable/source/opaque data".into());
                }
                let commands = sim.state().log[before..].to_vec();
                if commands.iter().any(|c| c.id & 0x80 == 0) {
                    return Err(format!("Unexpected simulated device SET: {commands:?}"));
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"linked_numeric_401_unrounded":true,"upper_bound_26000":true,"independent_y_1200":true,"save_reopen":true,"dpi_button_options":count,"source_and_executable_unchanged":true,"device_sets":0,"reads":commands.len(),"physical_discovery":false}),
                )
            }
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
    let window = run_window(engine, Some(Duration::from_secs(25)), None);
    let result = controller
        .join()
        .unwrap()
        .expect("Sensitivity native workflow failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Sensitivity evidence: {}", directory.display());
}
