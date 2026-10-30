//! Actual native hit testing and worker dispatch, isolated simulated mouse only.
use super::*;
use secbird_core::{sim::Sim, Store};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "opens a bounded native Calibration window; simulated transport only, run alone"]
fn native_calibration_reads_writes_and_preserves_named_profiles() {
    let directory = std::env::temp_dir().join(format!(
        "native-calibration-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let frame = directory.join("frame.json");
    let _environment = Environment::set(&[
        ("SYNAPSE_LITE_CONFIG_DIR", &directory),
        ("SYNAPSE_LITE_SMOKE_REPORT", &frame),
        ("SYNAPSE_LITE_SMOKE_VIEW", Path::new("mouse-calibration")),
    ]);
    let sim = Sim::wireless_mouse();
    sim.state().sensor_calibration = Some([0, 0, 0]);
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let key = engine.devices()[0].key();
    let mut store = Store::load(&directory.join("profiles.json")).unwrap();
    store
        .for_device(&engine.devices()[0])
        .active_mut()
        .extra
        .insert("opaque".into(), json!({"keep":true}));
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
                    "Calibration presented",
                )?;
                let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
                let query = || {
                    let mut v = Value::Null;
                    SendMessageW(hwnd, RUNTIME_TEST_QUERY, 0, &mut v as *mut Value as isize);
                    v
                };
                wait_for(
                    || query()["calibration"]["snapshot"]["calibration"]["available"] == true,
                    "Calibration hardware read",
                )?;
                let initial = query();
                if !initial["calibration"]["snapshot"]["calibration"]["tracking_distance_mm"]
                    .is_null()
                {
                    return Err("Invented a radio value for non-UI hardware mode".into());
                }
                let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let width = f64::from(client.right) / scale;
                lighting_click(hwnd, (width - 744.0) / 2.0 + 725.0, 167.0, scale);
                redraw(hwnd);
                if query()["calibration"]["welcome"] != false {
                    return Err("Welcome did not dismiss".into());
                }
                let prefs: Value = serde_json::from_slice(
                    &std::fs::read(folder.join("sensor-ui.json")).map_err(|e| e.to_string())?,
                )
                .unwrap();
                if prefs["calibration_welcome_dismissed"] != true {
                    return Err("Welcome preference was not saved".into());
                }
                for (offset, mm) in [(50.0, 1u8), (146.0, 2u8)] {
                    SendMessageW(hwnd, READ_ONLY_TEST_SET, 1, 0);
                    redraw(hwnd);
                    let log = controller_sim.state().log.len();
                    let bytes = std::fs::read(folder.join("profiles.json")).unwrap();
                    lighting_click(hwnd, (width - 600.0) / 2.0 + offset, 320.0, scale);
                    if controller_sim.state().log.len() != log
                        || std::fs::read(folder.join("profiles.json")).unwrap() != bytes
                    {
                        return Err("Read-only Calibration changed hardware or profiles".into());
                    }
                    SendMessageW(hwnd, READ_ONLY_TEST_SET, 0, 0);
                    redraw(hwnd);
                    lighting_click(hwnd, (width - 600.0) / 2.0 + offset, 320.0, scale);
                    wait_for(
                        || {
                            query()["pending"] == 0
                                && query()["calibration"]["snapshot"]["calibration"]
                                    ["tracking_distance_mm"]
                                    == mm
                        },
                        "Calibration write and readback",
                    )?;
                    if controller_sim.state().sensor_calibration != Some([1, 1, mm - 1]) {
                        return Err("Wrong Calibration registers".into());
                    }
                    let saved =
                        Store::load(&folder.join("profiles.json")).map_err(|e| e.to_string())?;
                    if saved.devices[&key].profiles != profiles
                        || saved.devices[&key]
                            .sensor_calibration
                            .as_ref()
                            .and_then(|c| c.tracking_distance_mm)
                            != Some(mm)
                    {
                        return Err("Incorrect Calibration/profile scope".into());
                    }
                }
                Ok(
                    json!({"actual_hwnd":true,"simulated_only":true,"low_high_readback":true,"readonly_no_io_or_save":true,"named_profiles_unchanged":true,"welcome_saved":true}),
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
        Some(Duration::from_secs(18)),
        None,
    )
    .unwrap();
    let evidence = controller.join().unwrap().unwrap();
    std::fs::write(
        directory.join("interaction.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Calibration native interaction evidence: {}",
        directory.display()
    );
}
