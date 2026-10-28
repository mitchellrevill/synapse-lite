//! Explicit reversible physical check. No personal profiles or driver changes.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use secbird_core::{
        device::candidates,
        dynamic_sensitivity::DynamicSensitivity,
        hidapi::HidApi,
        sensor_rotation_control::{Rotation, RotationController},
        Db, Device, Store,
    };
    use serde_json::json;
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--verify-and-restore"] {
        return Err("Use --verify-and-restore for the explicit reversible sensor check".into());
    }
    if !secbird_core::debloat::synapse_running().is_empty() {
        return Err("Close Synapse before the sensor round-trip".into());
    }
    let api = HidApi::new()?;
    let db = Db::builtin()?;
    let candidates: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|c| matches!(c.pid, 196 | 197) && c.interface == 0)
        .collect();
    if candidates.len() != 1 {
        return Err("Expected exactly one196/197 interface0 mouse".into());
    }
    let c = &candidates[0];
    let dev = Device::open(
        Box::new(api.open_path(&c.path)?),
        c.pid,
        c.interface,
        c.path.clone(),
        &[31],
        db.get(c.pid).cloned(),
    )
    .ok_or("Mouse handshake failed")?;
    let folder = std::env::current_dir()?.join("../../work/sensor-roundtrip-store");
    std::fs::create_dir_all(&folder)?;
    let mut store = Store::load(&folder.join("profiles.json"))?;
    let original_calibration = dev.calibration_raw()?;
    let outcome =
        secbird_core::mouse_sensor::set_tracking_distance(&dev, &mut store, &dev.key(), 1);
    let observed_calibration = dev.calibration_raw();
    let recovery = dev.restore_calibration_raw(original_calibration);
    let calibration = json!({"requested_mm":1,"before":original_calibration,"after":observed_calibration.as_ref().ok(),"write_verified":outcome.is_ok(),"error":outcome.err().map(|e|e.to_string()),"restored":recovery.is_ok()});
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
            let desired = Rotation {
                is_enabled: true,
                value: 5,
                ..Default::default()
            };
            let outcome = controller.apply(&dev, &desired);
            let observed = controller.observe(&dev);
            let restored = controller.restore(&dev);
            let result = json!({"before":before,"after":observed.as_ref().ok(),"write_verified":outcome.is_ok(),"error":outcome.err().map(|e|e.to_string()),"restored":restored.is_ok()});
            if let Err(error) = restored {
                println!(
                    "{}",
                    json!({"calibration":calibration,"sensitivity":sensitivity,"rotation":result})
                );
                return Err(error.into());
            }
            result
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
