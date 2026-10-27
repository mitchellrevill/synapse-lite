use super::*;

unsafe fn snapshot(hwnd: HWND) -> Value {
    let mut outer: RECT = std::mem::zeroed();
    let mut client: RECT = std::mem::zeroed();
    let mut origin = POINT { x: 0, y: 0 };
    GetWindowRect(hwnd, &mut outer);
    GetClientRect(hwnd, &mut client);
    ClientToScreen(hwnd, &mut origin);
    json!({"outer":[outer.right-outer.left,outer.bottom-outer.top],"client":[client.right,client.bottom],"inset":[origin.x-outer.left,origin.y-outer.top],"maximized":IsZoomed(hwnd)!=0})
}
unsafe fn layout_ready(hwnd: HWND) -> bool {
    let mut state = Value::Null;
    SendMessageW(
        hwnd,
        ASSIGNMENT_TEST_QUERY,
        3,
        &mut state as *mut Value as isize,
    );
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);
    state["width"] == client.right && state["height"] == client.bottom
}

unsafe fn screen_param(hwnd: HWND, x: i32, y: i32) -> isize {
    let mut point = POINT { x, y };
    ClientToScreen(hwnd, &mut point);
    (point.x as u16 as u32 | ((point.y as u16 as u32) << 16)) as isize
}
unsafe fn hit(hwnd: HWND, x: i32, y: i32) -> LRESULT {
    SendMessageW(hwnd, WM_NCHITTEST, 0, screen_param(hwnd, x, y))
}
fn full_client(value: &Value) -> bool {
    value["outer"] == value["client"] && value["inset"] == json!([0, 0])
}

#[test]
#[ignore = "opens one bounded native frame using simulated transport only"]
fn native_full_client_bounds_resize_hit_zones_and_maximize_preserve_state() {
    let directory = std::env::temp_dir().join(format!(
        "synapse-native-frame-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("keyboard-customize")),
    ]);
    let sim = Sim::keyboard();
    let engine = Engine::with_simulated_keyboard(&sim, directory.join("profiles.json")).unwrap();
    assert!(engine.event_paths().is_empty());
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store.for_device(&engine.devices()[0]);
    store.save().unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_directory = directory.clone();
    let controller_sim = sim.clone();
    let controller = std::thread::spawn(move || -> Result<Value, String> {
        let result = (|| -> Result<Value, String> {
            wait_for(
                || {
                    std::fs::read(&frame)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_some_and(|f| f["matched"] == true && f["presented"] == true)
                },
                "Full-client frame readiness",
            )?;
            let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
            unsafe {
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
                let before = controller_sim.state().log.len();
                let before_store =
                    std::fs::read(controller_directory.join("profiles.json")).unwrap();
                let mut observations = Vec::new();
                let normal = snapshot(hwnd);
                if !full_client(&normal) {
                    return Err(format!("Initial outer/client still differ: {normal}"));
                }
                observations.push(json!({"mode":"normal","bounds":normal}));
                let mut limits: MINMAXINFO = std::mem::zeroed();
                SendMessageW(
                    hwnd,
                    WM_GETMINMAXINFO,
                    0,
                    &mut limits as *mut MINMAXINFO as isize,
                );
                if (limits.ptMinTrackSize.x, limits.ptMinTrackSize.y)
                    != (
                        (1000.0 * scale).round() as i32,
                        (650.0 * scale).round() as i32,
                    )
                {
                    return Err("DPI minimum track bounds changed".into());
                }
                for (mode, width, height) in [
                    ("minimum", 1000.0, 650.0),
                    ("narrow", 1060.0, 700.0),
                    ("normal-restored", 1280.0, 720.0),
                ] {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        0,
                        0,
                        (width * scale).round() as i32,
                        (height * scale).round() as i32,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    wait_for(|| layout_ready(hwnd), "Current resized layout size")?;
                    redraw(hwnd);
                    let value = snapshot(hwnd);
                    if !full_client(&value)
                        || value["client"]
                            != json!([
                                (width * scale).round() as i32,
                                (height * scale).round() as i32
                            ])
                    {
                        return Err(format!(
                            "{mode} source viewport does not fill window: {value}"
                        ));
                    }
                    observations.push(json!({"mode":mode,"bounds":value}));
                }
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let (w, h) = (client.right, client.bottom);
                for (x, y, expected) in [
                    (1, 1, HTTOPLEFT),
                    (w - 2, 1, HTTOPRIGHT),
                    (1, h - 2, HTBOTTOMLEFT),
                    (w - 2, h - 2, HTBOTTOMRIGHT),
                    (1, h / 2, HTLEFT),
                    (w - 2, h / 2, HTRIGHT),
                    (w / 2, 1, HTTOP),
                    (w / 2, h - 2, HTBOTTOM),
                ] {
                    let actual = hit(hwnd, x, y);
                    if actual != expected as LRESULT {
                        return Err(format!(
                            "Resize target({x},{y}) expected{expected}, got{actual}"
                        ));
                    }
                }
                if hit(hwnd, w / 2, (20.0 * scale).round() as i32) != HTCAPTION as LRESULT {
                    return Err("Source42px header lost drag behavior".into());
                }
                if hit(
                    hwnd,
                    w - (23.0 * scale).round() as i32,
                    (20.0 * scale).round() as i32,
                ) != HTCLIENT as LRESULT
                {
                    return Err("Close control became a caption/resize target".into());
                }
                let host_boundary = (42.0 * scale).round() as i32;
                if hit(hwnd, w / 2, host_boundary - 1) != HTCAPTION as LRESULT
                    || hit(hwnd, w / 2, host_boundary) != HTCLIENT as LRESULT
                {
                    return Err("Source42px host boundary changed".into());
                }
                let monitor = window_frame::monitor(hwnd).ok_or("No native monitor work area")?;
                let mut normal_position: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut normal_position);
                // Exercise the actual native maximize button and posted system
                // command, rather than bypassing the editor's action path.
                lighting_click(hwnd, w as f64 / scale - 72.0, 20.0, scale);
                wait_for(|| IsZoomed(hwnd) != 0, "Native maximized state")?;
                wait_for(|| layout_ready(hwnd), "Current maximized layout size")?;
                redraw(hwnd);
                let max = snapshot(hwnd);
                let expected = json!([
                    monitor.rcWork.right - monitor.rcWork.left,
                    monitor.rcWork.bottom - monitor.rcWork.top
                ]);
                if !full_client(&max) || max["client"] != expected {
                    return Err(format!(
                        "Maximized client escaped work area: {max}, expected{expected}"
                    ));
                }
                let mut outer: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut outer);
                if (outer.left, outer.top) != (monitor.rcWork.left, monitor.rcWork.top) {
                    return Err("Maximized origin differs from work area".into());
                }
                if hit(hwnd, 1, 1) != HTCAPTION as LRESULT {
                    return Err("Maximized frame retained resize corner".into());
                }
                observations.push(json!({"mode":"maximized","bounds":max}));
                lighting_click(
                    hwnd,
                    max["client"][0].as_f64().unwrap() / scale - 72.0,
                    20.0,
                    scale,
                );
                wait_for(|| IsZoomed(hwnd) == 0, "Native restore")?;
                wait_for(|| layout_ready(hwnd), "Current restored layout size")?;
                redraw(hwnd);
                let restored = snapshot(hwnd);
                if !full_client(&restored)
                    || restored["client"]
                        != json!([
                            (1280.0 * scale).round() as i32,
                            (720.0 * scale).round() as i32
                        ])
                {
                    return Err(format!("Restore lost normal client geometry: {restored}"));
                }
                GetWindowRect(hwnd, &mut outer);
                if (outer.left, outer.top) != (normal_position.left, normal_position.top) {
                    return Err("Restore changed the normal window position".into());
                }
                observations.push(json!({"mode":"restore","bounds":restored}));
                if controller_sim.state().log.len() != before
                    || std::fs::read(controller_directory.join("profiles.json")).unwrap()
                        != before_store
                {
                    return Err("Frame operations changed simulated device/profile state".into());
                }
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                Ok(
                    json!({"scale":scale,"observations":observations,"all_eight_resize_targets":true,"header_drag_and_close_control":true,"minimum_track_size_css":[1000,650],"host_offset_css":42,"observed_host_boundary_px":host_boundary,"cached_layout_tracks_client":true,"maximize_restore_buttons_work":true,"restore_position_unchanged":true,"profile_bytes_unchanged":true,"sim_commands_added":0,"physical_discovery_disabled":true}),
                )
            }
        })();
        if result.is_err() {
            unsafe {
                PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0);
            }
        }
        result
    });
    let window = run_window(engine, Some(Duration::from_secs(15)), None);
    let result = controller
        .join()
        .unwrap()
        .expect("Native frame regression failed");
    window.unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!("Native full-client frame evidence: {}", directory.display());
}
