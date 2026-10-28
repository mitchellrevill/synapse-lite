//! Metadata-only identity validation. No Raw Input registration, input data read,
//! control-driver operations, SET, restoring lease or UI automation.
#[cfg(windows)]
fn inspect() -> serde_json::Value {
    use secbird_core::{hidapi::HidApi, mouse_input_identity, Db, Device};
    use serde_json::json;
    let operation = || -> Result<serde_json::Value, &'static str> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let choices: Vec<_> = args
            .iter()
            .filter_map(|arg| match arg.to_ascii_uppercase().as_str() {
                "--PID=00C4" | "--PID00C4" => Some(196),
                "--PID=00C5" | "--PID00C5" => Some(197),
                _ => None,
            })
            .collect();
        if choices.len() != 1 {
            return Err("exactly_one_explicit_pid_00C4_or_00C5_required");
        }
        let pid = choices[0];
        let api = HidApi::new().map_err(|_| "hid_enumeration_failed")?;
        let found: Vec<_> = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == 0x1532
                    && d.product_id() == pid
                    && d.interface_number() == 0
                    && d.usage_page() == 1
                    && d.usage() == 2
            })
            .collect();
        if found.len() != 1 {
            return Err("physical_mouse_collection_missing_or_ambiguous");
        }
        let path = found[0].path().to_owned();
        let handle = api.open_path(&path).map_err(|_| "hid_open_failed")?;
        let db = Db::builtin().map_err(|_| "database_failed")?;
        let mut device = Device::open(
            Box::new(handle),
            pid,
            0,
            path,
            &[0x1f],
            db.get(pid).cloned(),
        )
        .ok_or("serial_get_did_not_answer")?;
        device.enforce_query_only();
        let binding = mouse_input_identity::discover(&device)
            .map_err(|_| "exact_raw_mouse_association_failed")?;
        let live = binding
            .matches_live_handle(binding.handle())
            .map_err(|_| "raw_mouse_live_validation_failed")?;
        if !live {
            return Err("raw_mouse_handle_changed");
        }
        Ok(
            json!({"status":"success","pid":format!("{pid:04X}"),"interface":0,"query_only":true,
            "raw_mouse_count_for_exact_identity":1,"raw_handle_nonzero":binding.handle()!=0,
            "matching_active_container":true,"live_handle_revalidated":live,
            "selection":"VID/PID/interface + nonzero ContainerId; collection paths may differ",
            "input_registrations":0,"input_records_read":0,"set_count":0,"restore_count":0}),
        )
    };
    operation().unwrap_or_else(|stage| {
        json!({"status":"blocked","stage":stage,
        "input_registrations":0,"input_records_read":0,"set_count":0,"restore_count":0})
    })
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    if std::env::args().any(|arg| arg == "--metadata-child") {
        println!("{}", inspect());
        return Ok(());
    }
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe()?)
        .args(std::env::args().skip(1))
        .arg("--metadata-child")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    loop {
        if let Some(status) = child.try_wait()? {
            let mut output = String::new();
            if let Some(stdout) = child.stdout.take() {
                stdout.take(16384).read_to_string(&mut output)?;
            }
            let report = serde_json::json!({"probe":"raw_mouse_identity_metadata_only","deadline_ms":8000,
            "elapsed_ms":start.elapsed().as_millis(),"child_exit_success":status.success(),
            "result":serde_json::from_str::<serde_json::Value>(&output).unwrap_or_else(|_| {
                serde_json::json!({"status":"failed","stage":"invalid_child_report"})
            })});
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }
        if start.elapsed() >= Duration::from_secs(8) {
            let requested = child.kill().is_ok();
            println!(
                "{}",
                serde_json::json!({"probe":"raw_mouse_identity_metadata_only",
                "status":"deadline_exceeded","termination_requested":requested,"deadline_ms":8000,
                "input_registrations":0,"input_records_read":0,"set_count":0,"restore_count":0})
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Raw Input identity metadata requires Windows.");
}
