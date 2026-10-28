//! Explicit indicator-only check; no input mode changes, selectors or key output.
//! Usage: <fresh-report.json> --read-indicator | --check-indicator-restoration
use secbird_core::{discover, hidapi::HidApi, Db, Device};
use serde_json::{json, Value};
use std::{fs::OpenOptions, io::Write};

struct Restore<'a> {
    device: &'a Device,
    baseline: bool,
    pending: bool,
}
impl Restore<'_> {
    fn restore(&mut self) -> bool {
        if self
            .device
            .set_ornata_gaming_indicator(self.baseline)
            .is_ok()
            && self.device.ornata_gaming_indicator().ok() == Some(self.baseline)
        {
            self.pending = false;
            true
        } else {
            false
        }
    }
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        if self.pending {
            let _ = self.restore();
        }
    }
}

fn sample(write: bool) -> Result<Value, Box<dyn std::error::Error>> {
    let api = HidApi::new()?;
    let database = Db::builtin()?;
    let mut devices: Vec<_> = discover(&api, &database)
        .into_iter()
        .filter(|device| device.pid == 605 && device.interface == 2)
        .collect();
    if devices.len() != 1 {
        return Err("Missing or ambiguous Ornata configuration interface".into());
    }
    let mut device = devices.remove(0);
    if !write {
        device.enforce_query_only();
    }
    let mode = device.ornata_mode_readback()?;
    if !matches!(mode.mode(), 0 | 3) {
        return Err("Indicator check requires observed normal0 or driver3 mode".into());
    }
    let baseline = device.ornata_gaming_indicator()?;
    if !write {
        return Ok(
            json!({"read_verified":true,"indicator":baseline,"input_mode":mode.mode(),"indicator_write_requested":false}),
        );
    }
    let mut recovery = Restore {
        device: &device,
        baseline,
        pending: true,
    };
    let changed = device
        .set_ornata_gaming_indicator(!baseline)
        .and_then(|_| device.ornata_gaming_indicator())
        .map(|on| on == !baseline);
    let restored = recovery.restore();
    let final_mode = device.ornata_mode_readback().ok().map(|mode| mode.mode());
    Ok(
        json!({"baseline_indicator":baseline,"requested_indicator":!baseline,
        "change_verified":changed.as_ref().copied().unwrap_or(false),
        "change_error":changed.err().map(|error| error.to_string()),
        "restoration_verified":restored,"input_mode_initial":mode.mode(),"input_mode_final":final_mode,
        "indicator_write_requested":true,"input_mode_write_requested":false,
        "selector_write_requested":false,"generated_input":false,"profile_write_requested":false}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2
        || !matches!(
            args[1].as_str(),
            "--read-indicator" | "--check-indicator-restoration"
        )
    {
        return Err(
            "Use <fresh-report.json> --read-indicator | --check-indicator-restoration".into(),
        );
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])?;
    let report = match sample(args[1] == "--check-indicator-restoration") {
        Ok(value) => value,
        Err(error) => json!({"setup_failed":true,"error":error.to_string()}),
    };
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    if report["setup_failed"] == true
        || (args[1] == "--check-indicator-restoration"
            && (report["change_verified"] != true
                || report["restoration_verified"] != true
                || report["input_mode_final"] != report["input_mode_initial"]))
    {
        return Err("Indicator verification incomplete; inspect the saved report".into());
    }
    Ok(())
}
