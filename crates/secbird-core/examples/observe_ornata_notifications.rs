//! Explicit passive driver subscription probe, never an automatic test/runtime.
//! Usage: observe_ornata_notifications <fresh-report.json> --observe-passive <1..10>
//! Aggregate counts only. No selectors, key output, redirection or HID SETs.
#[cfg(windows)]
fn sample(seconds: u64) -> serde_json::Value {
    use secbird_core::{
        discover,
        hidapi::HidApi,
        keyboard_notifications::{Session, Target},
        Db,
    };
    use serde_json::json;
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let inspect = || -> Result<serde_json::Value, &'static str> {
        let api = HidApi::new().map_err(|_| "hid_discovery_failed")?;
        let db = Db::builtin().map_err(|_| "database_failed")?;
        let mut found: Vec<_> = discover(&api, &db)
            .into_iter()
            .filter(|device| device.pid == 0x025d && device.interface == 2)
            .collect();
        if found.len() != 1 {
            return Err("missing_or_ambiguous_ornata");
        }
        let mut device = found.remove(0);
        device.enforce_query_only();
        let target = Target::discover(&device).map_err(|_| "exact_control_association_failed")?;
        let mut session = Session::start(target).map_err(|_| "notification_worker_failed")?;
        let start = Instant::now();
        while !session.snapshot().subscription_enabled
            && !session.snapshot().finished
            && start.elapsed() < Duration::from_secs(4)
        {
            thread::sleep(Duration::from_millis(5));
        }
        let start = Instant::now();
        let (mut makes, mut breaks) = (0u64, 0u64);
        while start.elapsed() < Duration::from_secs(seconds) && !session.snapshot().finished {
            while let Some(event) = session.try_next() {
                if event.flags & 1 == 0 {
                    makes += 1;
                } else {
                    breaks += 1;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        let stopped = session.stop();
        while let Some(event) = session.try_next() {
            if event.flags & 1 == 0 {
                makes += 1;
            } else {
                breaks += 1;
            }
        }
        let state = session.snapshot();
        Ok(
            json!({"scope":"explicit passive selected-Ornata filter notifications; no interception or generated input",
            "snapshot":state,"stop_completed":stopped,"observed_make_records":makes,"observed_break_records":breaks,
            "physical_input_delivery_verified":makes+breaks>0,
            "selector_mutations":0,"input_submissions":0,"redirection_changes":0,"hid_sets":0,
            "key_identities_or_text_emitted":false,"paths_or_serials_emitted":false}),
        )
    };
    inspect().unwrap_or_else(|stage|json!({"stage":stage,"status":"failed","selector_mutations":0,"input_submissions":0,"hid_sets":0}))
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--passive-child") {
        if args.len() != 2 {
            return Err("Invalid child arguments".into());
        }
        let seconds = args[1].parse::<u64>()?;
        if !(1..=10).contains(&seconds) {
            return Err("Invalid observation duration".into());
        }
        println!("{}", sample(seconds));
        return Ok(());
    }
    if args.len() != 3 || args[1] != "--observe-passive" {
        return Err("Use <fresh-report.json> --observe-passive <1..10>".into());
    }
    let seconds = args[2].parse::<u64>()?;
    if !(1..=10).contains(&seconds) || std::path::Path::new(&args[0]).exists() {
        return Err("Fresh report path and duration1..10 required".into());
    }
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--passive-child", &seconds.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(seconds + 10) {
            child.kill()?;
            child.wait()?;
            return Err(
                "Passive child exceeded its deadline; clean completion is unverified".into(),
            );
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or("Missing report pipe")?
        .take(65537)
        .read_to_end(&mut bytes)?;
    if !status.success() || bytes.len() > 65536 {
        return Err("Passive child failed or exceeded report bound".into());
    }
    let report: serde_json::Value = serde_json::from_slice(&bytes)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    println!("{}", report);
    if report["snapshot"]["subscription_enabled"] != true
        || report["snapshot"]["subscription_disabled"] != true
        || report["snapshot"]["pending_drained"] != true
        || report["stop_completed"] != true
        || !report["snapshot"]["error"].is_null()
    {
        return Err("Passive notification lifecycle did not pass".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The selected-device notification probe requires Windows");
}
