//! Real native menu/hit/worker messages; explicit simulated Device196 only.
use super::*;
fn wait_for(mut predicate: impl FnMut() -> bool, reason: &str) -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(12) {
        if predicate() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("Timed out: {reason}"))
}
#[test]
#[ignore = "opens one bounded native window; simulated onboard registers only, run alone"]
fn native_onboard_reads_saves_restores_and_refuses_readonly_writes() {
    let directory = std::env::temp_dir().join(format!("native-onboard-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-performance")),
    ]);
    let sim = Sim::wireless_mouse();
    sim.state().onboard_polling = Some(1);
    sim.state().dpi_stages = Some((1, vec![(800, 800)]));
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let key = engine.devices()[0].key();
    let original = secbird_core::onboard::read(&engine.devices()[0]).unwrap();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    let profile = store.for_device(&engine.devices()[0]).active_mut();
    profile.dpi = Some((1600, 1800));
    profile.polling = Some(500);
    profile.extra.insert("keep".into(), json!({"opaque":true}));
    let profiles = store.for_device(&engine.devices()[0]).profiles.clone();
    store.save().unwrap();
    let controller_sim = sim.clone();
    let folder = directory.clone();
    let controller = std::thread::spawn(move || {
        let result = (|| -> Result<Value, String> {
            unsafe {
                wait_for(
                    || {
                        std::fs::read(&frame)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                            .is_some_and(|v| v["presented"] == true)
                    },
                    "Performance presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut value = Value::Null;
                    SendMessageW(
                        hwnd,
                        RUNTIME_TEST_QUERY,
                        0,
                        &mut value as *mut Value as isize,
                    );
                    value
                };
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                lighting_click(hwnd, 300., 103., scale);
                redraw(hwnd);
                lighting_click(hwnd, 350., 273., scale);
                wait_for(
                    || query()["onboard"]["snapshot"]["readback"]["available"] == true,
                    "Onboard readback",
                )?;
                if controller_sim.state().log.iter().any(|c| c.id & 0x80 == 0) {
                    return Err("Opening onboard memory sent a SET".into());
                }
                let x = ((width - 690.) / 2.).max(20.);
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                redraw(hwnd);
                let before = std::fs::read(folder.join("profiles.json")).unwrap();
                let count = controller_sim.state().log.len();
                lighting_click(hwnd, x + 240., 512., scale);
                if std::fs::read(folder.join("profiles.json")).unwrap() != before
                    || controller_sim.state().log.len() != count
                {
                    return Err("Read-only save mutated state".into());
                }
                SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                redraw(hwnd);
                lighting_click(hwnd, x + 240., 512., scale);
                wait_for(
                    || {
                        query()["pending"] == 0
                            && query()["onboard"]["snapshot"]["recovery"]["phase"] == "applied"
                    },
                    "Save and verified readback",
                )?;
                if controller_sim.state().onboard_polling != Some(2)
                    || controller_sim.state().dpi != Some((1600, 1800))
                {
                    return Err("Wrong onboard register projection".into());
                }
                redraw(hwnd);
                lighting_click(hwnd, x + 420., 512., scale);
                wait_for(
                    || {
                        query()["pending"] == 0
                            && query()["onboard"]["snapshot"]["recovery"].is_null()
                    },
                    "Original settings restored",
                ).map_err(|error| format!("{error}; UI snapshot: {}", query()))?;
                let dev = controller_sim
                    .open(196, secbird_core::Db::builtin().unwrap().get(196).cloned())
                    .unwrap();
                if secbird_core::onboard::read(&dev).map_err(|e| e.to_string())? != original {
                    return Err("Original settings did not read back".into());
                }
                let mut disk =
                    Store::load(&folder.join("profiles.json")).map_err(|e| e.to_string())?;
                if disk.for_device(&dev).profiles != profiles
                    || disk.for_device(&dev).active != "Default"
                    || disk.devices.len() != 1
                    || !disk.devices.contains_key(&key)
                {
                    return Err("Profiles or identity changed".into());
                }
                Ok(
                    json!({"actual_hwnd":true,"simulated_only":true,"read_open_sends_no_set":true,"save_restore_readback":true,"readonly_no_io":true,"profiles_unchanged":true}),
                )
            }
        })();
        unsafe {
            PostMessageW(WINDOW.load(Ordering::Acquire) as HWND, WM_CLOSE, 0, 0);
        }
        result
    });
    run_window(
        Arc::new(Mutex::new(engine)),
        Some(Duration::from_secs(45)),
        None,
    )
    .unwrap();
    let result = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Onboard native interaction evidence: {}",
        directory.display()
    );
}
