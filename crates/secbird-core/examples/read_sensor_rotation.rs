//! GET-only PID00C5 rotation inspection. No profile apply, SET, lease, restore,
//! DLL initialization or service operations. Output excludes serials and paths.
//! The parent requests termination after eight seconds; a stuck kernel request
//! may delay actual process termination, which is reported without waiting.

#[cfg(windows)]
fn inspect() -> serde_json::Value {
    use secbird_core::{hidapi::HidApi, sensor_rotation::SensorRotationReader, Db, Device};
    use serde_json::json;
    let operation = || -> Result<serde_json::Value, &'static str> {
        let api = HidApi::new().map_err(|_| "hid_enumeration_failed")?;
        // Protocol interface0 has multiple HID collections. Exact standard
        // mouse usage selects its physical HID path, not the first PID match.
        let found: Vec<_> = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == 0x1532
                    && d.product_id() == 0x00c5
                    && d.interface_number() == 0
                    && d.usage_page() == 1
                    && d.usage() == 2
            })
            .collect();
        if found.len() != 1 {
            let collections: Vec<_> = api
                .device_list()
                .filter(|d| d.vendor_id() == 0x1532 && matches!(d.product_id(), 0x00c4 | 0x00c5))
                .take(32)
                .map(|d| {
                    json!({"pid":format!("{:04X}",d.product_id()),
                    "interface":d.interface_number(),"usage_page":d.usage_page(),"usage":d.usage()})
                })
                .collect();
            return Ok(
                json!({"status":"blocked", "stage":"physical_mouse_collection_missing_or_ambiguous",
                "eligible_collection_count":found.len(), "mouse_collections":collections,
                "set_count":0,"restore_count":0,"restoring_drop_present":false}),
            );
        }
        let info = found[0];
        let path = info.path().to_owned();
        let handle = api.open_path(&path).map_err(|_| "hid_open_failed")?;
        let db = Db::builtin().map_err(|_| "database_failed")?;
        let spec = db.get(0x00c5).cloned();
        let mut device = Device::open(Box::new(handle), 0x00c5, 0, path, &[0x1f], spec)
            .ok_or("serial_get_did_not_answer")?;
        device.enforce_query_only();
        let mut reader = SensorRotationReader::open_for_device(&device)
            .map_err(|_| "exact_container_association_or_control_open_failed")?;
        let baseline = reader.angle().map_err(|_| "rotation_get_failed")?;
        let repeated = reader.angle().map_err(|_| "rotation_repeat_get_failed")?;
        Ok(json!({
            "status":"success", "pid":"00C5", "interface":0,
            "hid_usage_page":1, "hid_usage":2, "query_only":true,
            "association":"unique active HID/control ContainerId + exact PID/interface",
            "association_verified":true, "get_ioctl":"0x88883184",
            "rotation_get_count":2, "baseline_raw_degrees":baseline,
            "repeated_raw_degrees":repeated, "unchanged_between_reads":baseline==repeated,
            "set_count":0, "restore_count":0, "restoring_drop_present":false
        }))
    };
    operation().unwrap_or_else(|stage| {
        json!({"status":"blocked", "stage":stage,
        "set_count":0,"restore_count":0,"restoring_drop_present":false})
    })
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    if std::env::args().any(|arg| arg == "--get-only-child") {
        println!("{}", inspect());
        return Ok(());
    }
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--get-only-child")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    loop {
        if let Some(status) = child.try_wait()? {
            let mut output = String::new();
            if let Some(stdout) = child.stdout.take() {
                stdout.take(16_384).read_to_string(&mut output)?;
            }
            let report = serde_json::json!({
                "probe":"vendor_filter_rotation_get_only", "deadline_ms":8000,
                "elapsed_ms":start.elapsed().as_millis(), "child_exit_success":status.success(),
                "result":serde_json::from_str::<serde_json::Value>(&output).unwrap_or_else(|_| {
                    serde_json::json!({"status":"failed","stage":"invalid_child_report"})
                })
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }
        if start.elapsed() >= Duration::from_secs(8) {
            let requested = child.kill().is_ok();
            println!(
                "{}",
                serde_json::json!({"probe":"vendor_filter_rotation_get_only",
                "status":"deadline_exceeded","deadline_ms":8000,"termination_requested":requested,
                "set_count":0,"restore_count":0})
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This GET-only vendor-filter inspection requires Windows.");
}
