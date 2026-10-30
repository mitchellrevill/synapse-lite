//! Opt-in, one-frame installed-hardware probe. Never run by ordinary tests.
//! Usage: ornata_frame_probe <report.json> --write-one-frame
//! Use only through the reviewed tools/Test-OrnataFrame.ps1 wrapper, which
//! requires the configured stock owner and restarts it in finally. The wrapper
//! intent flag is not an exclusive owner lease. Exact RGB/phase restoration is
//! not claimed. No mode, brightness, input, macro or profile1 SET is issued.

use secbird_core::{
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    lighting_readback::{BrightnessReadback, DeviceModeReadback, EffectReadback},
    ornata_frame::{FrameTransferOutcome, FrameTransferStop, PreparedOrnataFrame, PIXELS},
    Db, Device, Error, Rgb,
};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

const OWNER_GUARD: &str = "SECBIRD_ORNATA_FRAME_OWNER_RESTART";
const OWNER_INTENT: &str = "reviewed-wrapper-finally-restart";

fn safe_error(error: &Error) -> String {
    match error {
        Error::Hid(_) | Error::System(_) => {
            "transport/system failure; backend details omitted".into()
        }
        other => other.to_string(),
    }
}

fn mode_json(mode: &DeviceModeReadback) -> Value {
    json!({"status":"success", "mode":mode.mode(), "source_name":mode.source_name(),
        "param":mode.param(), "declared_size":mode.declared_size(),
        "declared_args":mode.declared_args(), "raw_args_80":mode.raw_args().to_vec()})
}

fn effect_json(effect: &EffectReadback) -> Value {
    json!({"status":"success", "requested_profile":effect.requested.profile,
        "requested_region":effect.requested.region, "returned_profile":effect.returned.profile,
        "returned_region":effect.returned.region, "selectors_match":effect.selectors_match(),
        "declared_size":effect.declared_size(), "effect_id":effect.effect_id(),
        "flags":effect.flags(), "rate":effect.rate(), "color_count":effect.color_count(),
        "colors":effect.colors(), "raw_args_80":effect.raw_args().to_vec()})
}

fn brightness_json(brightness: &BrightnessReadback) -> Value {
    json!({"status":"success", "requested_profile":brightness.requested.profile,
        "requested_region":brightness.requested.region, "returned_profile":brightness.returned.profile,
        "returned_region":brightness.returned.region, "selectors_match":brightness.selectors_match(),
        "raw_brightness":brightness.raw_brightness(), "raw_args_3":brightness.raw_args()})
}

#[derive(Default)]
struct Capture {
    mode: Option<DeviceModeReadback>,
    effects: [Option<EffectReadback>; 2],
    brightness: [Option<BrightnessReadback>; 2],
    errors: Vec<String>,
}

impl Capture {
    /// Attempt all five GETs even if one fails. No fallback values are invented.
    fn read(device: &Device) -> Self {
        let mut capture = Self::default();
        match device.ornata_mode_readback() {
            Ok(value) => capture.mode = Some(value),
            Err(error) => capture.errors.push(format!("mode: {}", safe_error(&error))),
        }
        for profile in 0..=1 {
            match device.ornata_effect_readback(profile, 0) {
                Ok(value) => capture.effects[usize::from(profile)] = Some(value),
                Err(error) => capture
                    .errors
                    .push(format!("effect{profile}: {}", safe_error(&error))),
            }
        }
        for profile in 0..=1 {
            match device.ornata_brightness_readback(profile, 0) {
                Ok(value) => capture.brightness[usize::from(profile)] = Some(value),
                Err(error) => capture
                    .errors
                    .push(format!("brightness{profile}: {}", safe_error(&error))),
            }
        }
        capture
    }

    fn json(&self) -> Value {
        let unavailable = || json!({"status":"unavailable"});
        json!({"mode":self.mode.as_ref().map(mode_json).unwrap_or_else(unavailable),
            "effect0":self.effects[0].as_ref().map(effect_json).unwrap_or_else(unavailable),
            "effect1":self.effects[1].as_ref().map(effect_json).unwrap_or_else(unavailable),
            "brightness0":self.brightness[0].as_ref().map(brightness_json).unwrap_or_else(unavailable),
            "brightness1":self.brightness[1].as_ref().map(brightness_json).unwrap_or_else(unavailable),
            "errors":self.errors})
    }

    /// Pure preflight: a failed/missing observation refuses before frame OUT.
    fn preflight_errors(&self) -> Vec<String> {
        let mut errors = self.errors.clone();
        if self.mode.as_ref().is_none_or(|m| m.mode() != 3) {
            errors
                .push("Fresh Driver_Mode3 readback is required; no mode change is allowed".into());
        }
        for profile in 0..=1usize {
            if !self.effects[profile].as_ref().is_some_and(|e| {
                e.selectors_match()
                    && e.requested.profile == profile as u8
                    && e.requested.region == 0
            }) {
                errors.push(format!(
                    "Complete exact-selector profile{profile} effect is required"
                ));
            }
            if !self.brightness[profile].as_ref().is_some_and(|b| {
                b.selectors_match()
                    && b.requested.profile == profile as u8
                    && b.requested.region == 0
                    && b.raw_brightness() != 0
            }) {
                errors.push(format!("Profile{profile} raw brightness must be nonzero and selector-exact; output will not be enabled"));
            }
        }
        if !self.effects[0].as_ref().is_some_and(|e| {
            e.effect_id() == 8 && e.flags() == 0 && e.rate() == 0 && e.color_count() == 0
        }) {
            errors
                .push("Profile0 must already be canonical CustomFrame8/flags0/rate0/count0".into());
        }
        errors
    }
}

fn effect_equal(a: &EffectReadback, b: &EffectReadback) -> bool {
    a.requested == b.requested
        && a.returned == b.returned
        && a.effect_id() == b.effect_id()
        && a.flags() == b.flags()
        && a.rate() == b.rate()
        && a.color_count() == b.color_count()
        && a.colors() == b.colors()
}

fn comparisons(before: &Capture, after: &Capture) -> Value {
    let mode = before
        .mode
        .as_ref()
        .zip(after.mode.as_ref())
        .map(|(a, b)| a.mode() == b.mode() && a.param() == b.param());
    let effects: [Option<bool>; 2] = std::array::from_fn(|p| {
        before.effects[p]
            .as_ref()
            .zip(after.effects[p].as_ref())
            .map(|(a, b)| effect_equal(a, b))
    });
    let brightness: [Option<bool>; 2] = std::array::from_fn(|p| {
        before.brightness[p]
            .as_ref()
            .zip(after.brightness[p].as_ref())
            .map(|(a, b)| {
                a.requested == b.requested
                    && a.returned == b.returned
                    && a.raw_args() == b.raw_args()
            })
    });
    let all = mode == Some(true)
        && effects.iter().all(|v| *v == Some(true))
        && brightness.iter().all(|v| *v == Some(true))
        && after.errors.is_empty();
    json!({"mode_and_optional_param_unchanged":mode, "effect0_meaningful_fields_unchanged":effects[0],
        "effect1_meaningful_fields_unchanged":effects[1], "brightness0_raw_unchanged":brightness[0],
        "brightness1_raw_unchanged":brightness[1], "all_readable_control_fields_unchanged":all,
        "rgb_frame_restoration_proven":false, "optical_effect_proven":false})
}

fn transfer_json(outcome: &FrameTransferOutcome) -> Value {
    let stop = match &outcome.stop {
        FrameTransferStop::CompleteUsbOut => json!({"kind":"complete_usb_out"}),
        FrameTransferStop::Cancelled => json!({"kind":"cancelled"}),
        FrameTransferStop::Deadline => json!({"kind":"cooperative_deadline"}),
        FrameTransferStop::ModeQueryError(error) => {
            json!({"kind":"mode_query_error", "error":safe_error(error)})
        }
        FrameTransferStop::NotDriverMode(mode) => json!({"kind":"not_driver_mode", "mode":mode}),
        FrameTransferStop::TransferError {
            packet_index,
            error,
        } => {
            json!({"kind":"transfer_error", "packet_index":packet_index, "error":safe_error(error),
                "failed_packet_may_have_affected_hardware":true})
        }
    };
    let packets: Vec<_> = (0..7).map(|index| json!({"index":index,
        "role":if index < 6 {"row"} else {"custom_frame_commit"},
        "attempted":index < outcome.attempted_packets, "accepted_usb_out":index < outcome.accepted_packets})).collect();
    json!({"accepted_packets":outcome.accepted_packets, "attempted_packets":outcome.attempted_packets,
        "mode_query_attempted":outcome.mode_query_attempted,
        "fresh_mode":outcome.observed_mode.as_ref().map(mode_json), "stop":stop, "packets":packets,
        "usb_out_complete":outcome.usb_out_complete(), "commit_accepted_usb_out":outcome.commit_accepted(),
        "firmware_ack_requested":false, "optical_verification":false})
}

fn save(path: &Path, value: &Value, create_new: bool) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "Cannot open diagnostic report".to_string())?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "Cannot encode diagnostic report")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot persist diagnostic report".into())
}

fn probe(path: &Path, report: &mut Value) -> Result<(), String> {
    if std::env::var_os("SECBIRD_SIM").is_some() {
        return Err("Physical frame probe refuses SECBIRD_SIM".into());
    }
    if std::env::var(OWNER_GUARD).ok().as_deref() != Some(OWNER_INTENT) {
        return Err("Use the reviewed outer stock-owner finally-restart wrapper".into());
    }
    let api = HidApi::new().map_err(|_| "Cannot initialize real HID transport")?;
    let found: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|c| c.pid == 0x025d && c.interface == 2)
        .collect();
    if found.len() != 1 {
        return Err(format!(
            "Expected exactly one PID025D/interface2 candidate, found{}",
            found.len()
        ));
    }
    let candidate = &found[0];
    let db = Db::builtin().map_err(|error| safe_error(&error))?;
    let spec = db.get(0x025d).cloned();
    if !spec
        .as_ref()
        .and_then(|s| s.matrix)
        .is_some_and(|m| m.rows == 6 && m.cols == 22)
    {
        return Err("Installed Ornata spec must have physical6x22 matrix".into());
    }
    let mut txids = Vec::new();
    if let Some(txid) = spec.as_ref().and_then(|s| s.txid) {
        txids.push(txid);
    }
    for txid in TXID_CANDIDATES {
        if !txids.contains(&txid) {
            txids.push(txid);
        }
    }
    let hid = api
        .open_path(&candidate.path)
        .map_err(|_| "Cannot open sole Ornata transport")?;
    let mut device = Device::open(
        Box::new(hid),
        0x025d,
        2,
        candidate.path.clone(),
        &txids,
        spec,
    )
    .ok_or("Sole Ornata transport did not answer discovery GET")?;
    let baseline = Capture::read(&device);
    report["baseline"] = baseline.json();
    let errors = baseline.preflight_errors();
    report["preflight_errors"] = json!(errors);
    if !errors.is_empty() {
        report["phase"] = json!("preflight_refused_no_frame_out");
        return Err("Frame probe preflight refused; no frame OUT attempted".into());
    }
    let frame = PreparedOrnataFrame::new(&vec![Rgb(16, 32, 48); PIXELS])
        .map_err(|error| safe_error(&error))?;
    report["phase"] = json!("baseline_ready_before_first_frame_out");
    report["planned_frame_packets_91"] = json!(frame.packets(device.txid).map(|p| p.to_vec()));
    report["baseline_persisted_before_frame_out"] = json!(false);
    // Keep an immutable sidecar in addition to the chosen complete report so a
    // later reporting failure cannot destroy the only baseline checkpoint.
    save(&path.with_extension("baseline.json"), report, true)?;
    report["baseline_persisted_before_frame_out"] = json!(true);
    save(path, report, false)?;
    let started = Instant::now();
    let outcome = frame.transfer_out(&mut device, started + Duration::from_secs(1), || false);
    report["transfer_elapsed_ms"] = json!(started.elapsed().as_millis());
    let complete = match outcome {
        Ok(outcome) => {
            let complete = outcome.usb_out_complete();
            report["transfer"] = transfer_json(&outcome);
            complete
        }
        Err(error) => {
            report["transfer"] = json!({"kind":"pre_transfer_error", "error":safe_error(&error),
            "accepted_packets":0,"attempted_packets":0});
            false
        }
    };
    report["phase"] = json!("frame_attempt_complete_post_gets_pending");
    let intermediate_save_error = save(path, report, false).err();
    let intermediate_save_failed = intermediate_save_error.is_some();
    let after = Capture::read(&device);
    report["post_readback"] = after.json();
    report["comparison"] = comparisons(&baseline, &after);
    report["intermediate_report_save_error"] = json!(intermediate_save_error);
    report["phase"] = json!("post_readback_complete_outer_owner_restart_required");
    if !complete
        || intermediate_save_failed
        || report["comparison"]["all_readable_control_fields_unchanged"] != true
    {
        return Err("Frame transfer incomplete or post-readback controls differ/unavailable; outer stock restart still required".into());
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[1] != "--write-one-frame" {
        eprintln!("usage: ornata_frame_probe <report.json> --write-one-frame");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    let mut report = json!({"schema_version":1,"phase":"created_no_frame_out", "pid":"025D", "interface":2,
        "requested_uniform_rgb":[16,32,48],"physical_pixels":132,"maximum_frame_packets":7,
        "cooperative_frame_deadline_ms":1000,"frame_write_retries":0,
        "outer_owner_restart_required":true,"owner_handoff_is_not_exclusive_lease":true,
        "mode_set_allowed":false,"brightness_set_allowed":false,"profile1_set_allowed":false,
        "input_macro_or_profile_file_changes_allowed":false,
        "exact_rgb_or_animation_phase_or_input_restoration_proven":false});
    if let Err(error) = save(path, &report, true) {
        eprintln!("{error}; no device opened");
        std::process::exit(1);
    }
    let result = probe(path, &mut report);
    if let Err(error) = &result {
        report["failure"] = json!(error);
    }
    report["probe_completed_without_reported_error"] = json!(result.is_ok());
    let saved = save(path, &report, false);
    if let Err(error) = &saved {
        eprintln!("{error}; immutable baseline sidecar may remain");
    }
    if result.is_err() || saved.is_err() {
        eprintln!("Probe did not complete cleanly; inspect report and outer restart record");
        std::process::exit(1);
    }
    println!("One frame USB OUT completed and readable controls matched. Optical/frame/phase restoration is not proven; outer stock restart is required.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::proto::{Command, Report};
    fn reply(command: Command, size: u8, args: &[u8]) -> Report {
        let mut r = Report::request(0x1f, command, size, args);
        r.0[0] = 2;
        r
    }
    fn fixture() -> Capture {
        let mode =
            DeviceModeReadback::from_reply(0x1f, &reply(Command::new(0, 0x84), 1, &[3])).unwrap();
        let effect = |p, e| {
            EffectReadback::from_reply(
                p,
                0,
                0x1f,
                &reply(Command::new(15, 0x82), 6, &[p, 0, e, 0, 0, 0]),
            )
            .unwrap()
        };
        let brightness = |p, b| {
            BrightnessReadback::from_reply(
                p,
                0,
                0x1f,
                &reply(Command::new(15, 0x84), 3, &[p, 0, b]),
            )
            .unwrap()
        };
        Capture {
            mode: Some(mode),
            effects: [Some(effect(0, 8)), Some(effect(1, 3))],
            brightness: [Some(brightness(0, 255)), Some(brightness(1, 255))],
            errors: vec![],
        }
    }
    #[test]
    fn preflight_accepts_only_existing_driver_custom_frame_and_nonzero_output() {
        let mut value = fixture();
        assert!(value.preflight_errors().is_empty());
        assert_eq!(value.mode.as_ref().unwrap().param(), None);
        value.mode = Some(
            DeviceModeReadback::from_reply(0x1f, &reply(Command::new(0, 0x84), 1, &[0])).unwrap(),
        );
        assert!(!value.preflight_errors().is_empty());
        let mut value = fixture();
        value.effects[0] = Some(
            EffectReadback::from_reply(
                0,
                0,
                0x1f,
                &reply(Command::new(15, 0x82), 6, &[0, 0, 3, 0, 0, 0]),
            )
            .unwrap(),
        );
        assert!(!value.preflight_errors().is_empty());
        for profile in 0..2 {
            let mut value = fixture();
            value.brightness[profile] = Some(
                BrightnessReadback::from_reply(
                    profile as u8,
                    0,
                    0x1f,
                    &reply(Command::new(15, 0x84), 3, &[profile as u8, 0, 0]),
                )
                .unwrap(),
            );
            assert!(!value.preflight_errors().is_empty());
        }
    }
    #[test]
    fn missing_failed_or_mismatched_capture_refuses_without_defaults() {
        let mut value = fixture();
        value.effects[1] = None;
        assert!(!value.preflight_errors().is_empty());
        let mut value = fixture();
        value.errors.push("injected GET failure".into());
        assert!(!value.preflight_errors().is_empty());
        let mut value = fixture();
        value.effects[0] = Some(
            EffectReadback::from_reply(
                0,
                0,
                0x1f,
                &reply(Command::new(15, 0x82), 6, &[1, 0, 8, 0, 0, 0]),
            )
            .unwrap(),
        );
        assert!(!value.preflight_errors().is_empty());
    }
    #[test]
    fn comparisons_ignore_unused_padding_but_preserve_unknown_parameter_and_real_changes() {
        let before = fixture();
        let mut after = fixture();
        let mut args = [0xa5; 80];
        args[..6].copy_from_slice(&[0, 0, 8, 0, 0, 0]);
        after.effects[0] = Some(
            EffectReadback::from_reply(0, 0, 0x1f, &reply(Command::new(15, 0x82), 6, &args))
                .unwrap(),
        );
        assert_eq!(
            comparisons(&before, &after)["all_readable_control_fields_unchanged"],
            true
        );
        after.mode = Some(
            DeviceModeReadback::from_reply(0x1f, &reply(Command::new(0, 0x84), 2, &[3, 0]))
                .unwrap(),
        );
        assert_eq!(
            comparisons(&before, &after)["mode_and_optional_param_unchanged"],
            false
        );
        after.mode = None;
        assert_eq!(
            comparisons(&before, &after)["mode_and_optional_param_unchanged"],
            Value::Null
        );
    }
}
