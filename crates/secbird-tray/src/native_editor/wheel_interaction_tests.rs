//! Bounded owned HWND test; simulated transport and isolated profile store only.
use super::*;

#[test]
#[ignore = "opens an isolated simulated native wheel page; run alone"]
fn native_wheel_buttons_verify_registers_and_preserve_profiles() {
    let folder = std::env::temp_dir().join(format!(
        "native-wheel-{}-{}",
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
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-advanced")),
    ]);
    let (sim, _) = secbird_core::sim::preset("basilisk-v3").unwrap();
    let engine = Engine::with_simulated_wheel(&sim).unwrap();
    let mut store = Store::load(&folder.join("profiles.json")).unwrap();
    let profiles = store.for_device(&engine.devices()[0]);
    profiles
        .active_mut()
        .extra
        .insert("opaque".into(), json!({"retain":42}));
    profiles.profiles.insert("Other".into(), Default::default());
    store.save().unwrap();
    let disk = std::fs::read(folder.join("profiles.json")).unwrap();
    let engine = Arc::new(Mutex::new(engine));
    let controller_sim = sim.clone();
    let controller_folder = folder.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "wheel page presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let x = f64::from(client.right) / scale / 2. + 200.;
                let sets = || {
                    controller_sim
                        .state()
                        .log
                        .iter()
                        .filter(|c| c.id & 0x80 == 0)
                        .count()
                };
                let baseline = sets();
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                redraw(hwnd);
                lighting_click(hwnd, x, 263., scale);
                if sets() != baseline {
                    return Err("Read-only wheel page wrote".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                redraw(hwnd);
                for (index, y) in [(0, 263.), (1, 378.), (2, 493.)] {
                    lighting_click(hwnd, x, y, scale);
                    wait_for(
                        || controller_sim.state().wheel_controls.unwrap()[index] == 1,
                        "native button register write",
                    )?;
                    wait_for(
                        || {
                            let mut status = Value::Null;
                            SendMessageW(
                                hwnd,
                                RUNTIME_TEST_QUERY,
                                0,
                                &mut status as *mut Value as isize,
                            );
                            status["pending"] == 0
                        },
                        "wheel job completed",
                    )?;
                    redraw(hwnd);
                    lighting_click(hwnd, x, y, scale);
                    wait_for(
                        || controller_sim.state().wheel_controls.unwrap()[index] == 0,
                        "native button raw restoration",
                    )?;
                    wait_for(
                        || {
                            let mut status = Value::Null;
                            SendMessageW(
                                hwnd,
                                RUNTIME_TEST_QUERY,
                                0,
                                &mut status as *mut Value as isize,
                            );
                            status["pending"] == 0
                        },
                        "wheel restoration completed",
                    )?;
                    redraw(hwnd);
                }
                if sets() != baseline + 6 {
                    return Err("Unexpected duplicate wheel SET".into());
                }
                if std::fs::read(controller_folder.join("profiles.json")).unwrap() != disk {
                    return Err("Wheel controls changed profile bytes".into());
                }
                Ok(())
            }
        })();
        unsafe {
            PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0);
        }
        result
    });
    run_window(engine, Some(Duration::from_secs(20)), None).unwrap();
    controller.join().unwrap().unwrap();
    eprintln!("Wheel HWND test evidence: {}", folder.display());
}
