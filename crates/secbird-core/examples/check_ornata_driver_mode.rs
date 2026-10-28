//! Explicit reversible initialization experiment. Not a passive-only probe.
//! Usage: check_ornata_driver_mode <fresh-report.json> --stock-driver-mode <1..10>
//! Parent owns mode recovery even if its passive notification child times out.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use secbird_core::{discover, hidapi::HidApi, ornata_driver_mode::ModeLease, Db};
    use serde_json::json;
    use std::os::windows::process::CommandExt;
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
    }
    fn quiet() -> bool {
        let start = Instant::now();
        let mut released = None;
        while start.elapsed() < Duration::from_secs(5) {
            // High bits only, never toggle history; exclude mouse virtual keys.
            let held = (8..=254).any(|key| unsafe { GetAsyncKeyState(key) } < 0);
            if held {
                released = None;
            } else {
                let since = released.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_millis(250) {
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3
        || !matches!(
            args[1].as_str(),
            "--stock-driver-mode" | "--stock-driver-mode-gated" | "--stock-driver-mode-raw-gated"
        )
    {
        return Err("Use <fresh-report.json> --stock-driver-mode <1..10>".into());
    }
    let seconds = args[2].parse::<u64>()?;
    let raw = args[1] == "--stock-driver-mode-raw-gated";
    let gated = raw || args[1] == "--stock-driver-mode-gated";
    let ready = std::path::Path::new(&args[0]).with_extension("ready.json");
    let signal = std::path::Path::new(&args[0]).with_extension("gate");
    if gated && (ready.exists() || signal.exists()) {
        return Err("Gate paths must be fresh".into());
    }
    if !(1..=10).contains(&seconds) || std::path::Path::new(&args[0]).exists() {
        return Err("Fresh report path and duration1..10 required".into());
    }
    let child_exe = std::env::current_exe()?
        .parent()
        .ok_or("Missing executable directory")?
        .join(if raw {
            "observe_ornata_raw_sources.exe"
        } else {
            "observe_ornata_notifications.exe"
        });
    if !child_exe.is_file() {
        return Err("Build the adjacent passive observation example first".into());
    }
    let api = HidApi::new()?;
    let db = Db::builtin()?;
    let mut devices: Vec<_> = discover(&api, &db)
        .into_iter()
        .filter(|device| device.pid == 0x025d && device.interface == 2)
        .collect();
    if devices.len() != 1 {
        return Err("One unambiguous Ornata configuration interface required".into());
    }
    let device = devices.remove(0);
    // Physical binding is resolved before any SET, not inferred from PID alone.
    let _target = secbird_core::keyboard_notifications::Target::discover(&device)?;
    let mut lease = ModeLease::open_source_startup(&device)?;
    let baseline = lease.baseline();
    if baseline.mode != 0 {
        return Err("This comparison requires the observed Normal0/0 baseline".into());
    }
    if !quiet() {
        return Err("Keyboard did not become quiet; no mode SET attempted".into());
    }
    let mut driver_readback = None;
    let observed = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        lease.enable_driver()?;
        driver_readback = Some(lease.current()?);
        let mut command = Command::new(child_exe);
        command.creation_flags(0x08000000);
        if gated {
            command
                .arg("--gated-child")
                .arg(seconds.to_string())
                .arg(&ready)
                .arg(&signal);
        } else {
            command.args(["--passive-child", &seconds.to_string()]);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(seconds + if gated { 38 } else { 8 }) {
                child.kill()?;
                child.wait()?;
                return Err(
                    "Passive child deadline exceeded; notification cleanup unverified".into(),
                );
            }
            thread::sleep(Duration::from_millis(10));
        };
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .ok_or("Missing child output")?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if !status.success() || bytes.len() > 65536 {
            return Err("Passive child failed or report exceeded bound".into());
        }
        Ok(serde_json::from_slice(&bytes)?)
    })();
    let quiet_before_restore = quiet();
    // Restore is independent of child success/timeout. A changed external mode
    // is preserved and reported rather than overwritten by this lease.
    let restored = lease.restore();
    let final_mode = lease.current().ok();
    let set_attempts = lease.set_attempts();
    let mode_restored = restored.is_ok() && final_mode == Some(baseline);
    let child_error = observed
        .as_ref()
        .err()
        .map(|_| "notification child or mode setup failed");
    let child_report = observed.as_ref().ok();
    let child_lifecycle_ok = child_report.is_some_and(|report| {
        report["snapshot"]["subscription_enabled"] == true
            && report["snapshot"]["subscription_disabled"] == true
            && report["snapshot"]["pending_drained"] == true
            && report["stop_completed"] == true
            && report["snapshot"]["error"].is_null()
            && (!raw
                || (report["raw_observation_valid"] == true
                    && report["raw_registration_removed"] == true))
    });
    let report = json!({"scope":"explicit source startup mode3 experiment with parent-owned observed-mode rollback; source canonical zero parameter; passive notifications only",
        "baseline_mode":baseline,"driver_readback":driver_readback,"final_mode":final_mode,
        "command_parameter_policy":"stock source canonical zero; unreported readback remains null",
        "original_parameter_readback_available":baseline.parameter.is_some(),
        "mode_restored":mode_restored,"mode_set_attempts":set_attempts,"quiet_before_restore":quiet_before_restore,
        "child":child_report,"child_error":child_error,"child_lifecycle_ok":child_lifecycle_ok,"selector_mutations":0,"redirection_changes":0,
        "input_submissions":0,"brightness_sets":0,"lighting_frames":0,"paths_or_serials_emitted":false});
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])?;
    output.write_all(&serde_json::to_vec_pretty(&report)?)?;
    output.sync_all()?;
    println!("{report}");
    if !mode_restored || !quiet_before_restore || observed.is_err() || !child_lifecycle_ok {
        return Err("Initialization comparison or recovery failed; inspect report".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The explicit mode experiment requires Windows");
}
