//! Explicit reversible physical check. No personal profiles or driver changes.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use secbird_core::{
        dynamic_sensitivity::DynamicSensitivity,
        hidapi::HidApi,
        sensor_rotation_control::{Rotation, RotationController},
        Db, Device, Store,
    };
    use serde_json::json;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let selected_pid = match args.as_slice() {
        [action] if action == "--verify-and-restore" => None,
        [action, flag, pid] if action == "--verify-and-restore" && flag == "--pid" => {
            Some(u16::from_str_radix(pid, 16)?)
        }
        _ => return Err("Use --verify-and-restore [--pid 00C4|00C5]".into()),
    };
    if selected_pid.is_some_and(|pid| !matches!(pid, 196 | 197)) {
        return Err("Unsupported sensor-test product".into());
    }
    if !secbird_core::debloat::synapse_running().is_empty() {
        return Err("Close Synapse before the sensor round-trip".into());
    }
    let api = HidApi::new()?;
    let db = Db::builtin()?;
    // Interface0 exposes several collections. Bind the unique standard mouse
    // collection, as in the verified GET-only filter reader, not first PID match.
    let candidates: Vec<_> = api
        .device_list()
        .filter(|c| {
            c.vendor_id() == 0x1532
                && matches!(c.product_id(), 196 | 197)
                && selected_pid.is_none_or(|pid| c.product_id() == pid)
                && c.interface_number() == 0
                && c.usage_page() == 1
                && c.usage() == 2
        })
        .collect();
    if candidates.len() != 1 {
        return Err("Expected exactly one selected interface0 mouse collection; specify --pid when both connection modes are present".into());
    }
    let c = &candidates[0];
    let dev = Device::open(
        Box::new(api.open_path(c.path())?),
        c.product_id(),
        c.interface_number(),
        c.path().to_owned(),
        &[31],
        db.get(c.product_id()).cloned(),
    )
    .ok_or("Mouse handshake failed")?;
    let folder = std::env::current_dir()?.join("../../work/sensor-roundtrip-store");
    std::fs::create_dir_all(&folder)?;
    let mut store = Store::load(&folder.join("profiles.json"))?;
    let original_calibration = dev.calibration_raw()?;
    let mut calibration_checks = Vec::new();
    for mm in [1, 2] {
        let outcome =
            secbird_core::mouse_sensor::set_tracking_distance(&dev, &mut store, &dev.key(), mm);
        let observed = dev.calibration_raw();
        let passed = outcome.is_ok();
        calibration_checks.push(json!({"requested_mm":mm,"after":observed.as_ref().ok(),"write_verified":passed,"error":outcome.err().map(|e|e.to_string())}));
        if !passed {
            break;
        }
    }
    let recovery = dev.restore_calibration_raw(original_calibration);
    let calibration = json!({"before":original_calibration,"checks":calibration_checks,"restored":recovery.is_ok()});
    if let Err(error) = recovery {
        println!("{}", json!({"calibration":calibration}));
        return Err(error.into());
    }
    // Capture every curve byte before any sensitivity write, including hidden data.
    let sensitivity = match dev.dynamic_raw(true) {
        Ok(before) => {
            let mut custom = DynamicSensitivity {
                state: 1,
                mode: 3,
                ..Default::default()
            };
            custom.seed(0)?;
            let outcome = dev.apply_dynamic_sensitivity(&custom);
            let observed = dev.dynamic_raw(false);
            let recovery = dev.restore_dynamic_raw(&before);
            let result = json!({"before":{"state":before.state,"mode":before.mode},"after":observed.as_ref().ok(),"custom_curve_write_and_250_sample_readback":outcome.is_ok(),"error":outcome.err().map(|e|e.to_string()),"restored":recovery.is_ok()});
            if let Err(error) = recovery {
                println!(
                    "{}",
                    json!({"calibration":calibration,"sensitivity":result})
                );
                return Err(error.into());
            }
            result
        }
        Err(error) => json!({"write_attempted":false,"checkpoint_error":error.to_string()}),
    };
    let rotation = match RotationController::open(&dev) {
        Ok(mut controller) => {
            let before = controller.baseline();
            let mut checks = Vec::new();
            for angle in [5, -5] {
                let desired = Rotation {
                    is_enabled: true,
                    value: angle,
                    ..Default::default()
                };
                let outcome = controller.apply(&dev, &desired);
                let observed = controller.observe(&dev);
                let restored = controller.restore(&dev);
                let passed = outcome.is_ok();
                checks.push(json!({"requested_degrees":angle,"after":observed.as_ref().ok(),"write_verified":passed,"error":outcome.err().map(|e|e.to_string()),"restored":restored.is_ok()}));
                if let Err(error) = restored {
                    println!(
                        "{}",
                        json!({"calibration":calibration,"sensitivity":sensitivity,"rotation":{"before":before,"checks":checks}})
                    );
                    return Err(error.into());
                }
                if !passed {
                    break;
                }
            }
            json!({"before":before,"checks":checks})
        }
        Err(error) => json!({"write_attempted":false,"checkpoint_error":error.to_string()}),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"pid":format!("{:04X}",dev.pid),"firmware":dev.firmware_version().ok(),"calibration":calibration,"sensitivity":sensitivity,"rotation":rotation,"personal_profiles_modified":false,"driver_installation":false})
        )?
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This verification requires Windows");
}
