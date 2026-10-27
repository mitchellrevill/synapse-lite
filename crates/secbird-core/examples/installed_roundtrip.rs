//! Opt-in installed-hardware checks. Never runs as part of the normal test suite.
//!
//! Usage: installed_roundtrip <00C4|025D> <report.json> [--roundtrip [--fail-after-dpi]]
//! Close competing configuration clients before running. Default mode only reads.
//! The report is written before any settings change and contains no serials.
//! Only DPI, polling, sleep and brightness are exercised; no input maps/macros,
//! device modes, services, drivers, effects or profile files are changed.

use secbird_core::{commands as cmd, discover, hidapi::HidApi, proto::Command, Db, Device};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone)]
struct Register {
    name: &'static str,
    command: Command,
    size: u8,
    query: Vec<u8>,
    original: Vec<u8>,
    restore: bool,
}

impl Register {
    fn restore_packet(&self) -> Result<(u8, Vec<u8>), String> {
        let mut args = self.original.clone();
        if self.name == "dpi-stages" {
            let count = usize::from(args[2]);
            if !(1..=5).contains(&count) || !(1..=count as u8).contains(&args[1]) {
                return Err("Unrecognized DPI stage count/active index".into());
            }
            for index in 0..count {
                let slot = 3 + index * 7;
                if args[slot] != index as u8 + 1 {
                    return Err("Unrecognized GET stage slot numbering".into());
                }
                // Shipped MouseMW6120 expects GET slot = SET slot + 1.
                args[slot] -= 1;
            }
            args[0] = cmd::VARSTORE;
            args.truncate(3 + count * 7);
            return Ok((args.len() as u8, args));
        }
        if matches!(
            self.name,
            "dpi-saved" | "dpi-live" | "brightness-saved" | "brightness-live"
        ) {
            args[..self.query.len()].copy_from_slice(&self.query);
            if self.name.starts_with("brightness") {
                args[2] = self.original[2];
            }
        }
        Ok((self.size, args))
    }
    fn capture(
        device: &Device,
        name: &'static str,
        command: Command,
        size: u8,
        query: &[u8],
        restore: bool,
    ) -> Result<Self, String> {
        let original = device
            .get(command, size, query)
            .map_err(|e| format!("Cannot snapshot {name}: {e}"))?
            .args()[..size as usize]
            .to_vec();
        Ok(Self {
            name,
            command,
            size,
            query: query.to_vec(),
            original,
            restore,
        })
    }
    fn read(&self, device: &Device) -> Result<Vec<u8>, String> {
        device
            .get(self.command, self.size, &self.query)
            .map(|r| r.args()[..self.size as usize].to_vec())
            .map_err(|e| e.to_string())
    }
}

struct Recovery<'a> {
    device: &'a Device,
    registers: Vec<Register>,
    armed: bool,
}

impl Recovery<'_> {
    fn restore(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        // Restoring stages can switch the current DPI. Restore those first,
        // then the captured independent saved/live DPI values.
        let stages = self
            .registers
            .iter()
            .filter(|r| r.restore && r.name == "dpi-stages");
        let rest = self
            .registers
            .iter()
            .filter(|r| r.restore && r.name != "dpi-stages");
        for register in stages.chain(rest) {
            if register
                .read(self.device)
                .is_ok_and(|actual| actual == register.original)
            {
                continue;
            }
            let (size, args) = match register.restore_packet() {
                Ok(packet) => packet,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };
            if let Err(error) = self.device.transact(register.command, size, &args) {
                errors.push(format!("Restore {}: {error}", register.name));
            }
        }
        for register in &self.registers {
            match register.read(self.device) {
                Ok(actual) if actual == register.original => {}
                Ok(_) => errors.push(format!(
                    "Restore {} readback differs from snapshot",
                    register.name
                )),
                Err(error) => errors.push(format!("Restore {} readback: {error}", register.name)),
            }
        }
        self.armed = !errors.is_empty();
        errors
    }
}

impl Drop for Recovery<'_> {
    fn drop(&mut self) {
        if self.armed {
            for error in self.restore() {
                eprintln!("Recovery retry failed: {error}");
            }
        }
    }
}

fn write_report(path: &Path, report: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| format!("Cannot save report: {e}"))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let roundtrip = args.get(2).is_some_and(|arg| arg == "--roundtrip");
    let inject_failure = args.get(3).is_some_and(|arg| arg == "--fail-after-dpi");
    if !(args.len() == 2
        || (args.len() == 3 && roundtrip)
        || (args.len() == 4 && roundtrip && inject_failure))
    {
        return Err("usage: installed_roundtrip <00C4|025D> <report.json> [--roundtrip]".into());
    }
    if std::env::var_os("SECBIRD_SIM").is_some() {
        return Err("Installed-hardware checks refuse simulator mode".into());
    }
    let pid = match args[0].to_ascii_uppercase().as_str() {
        "00C4" => 0x00c4,
        "025D" => 0x025d,
        _ => {
            return Err(
                "Only the inspected installed product IDs 00C4 and 025D are accepted".into(),
            )
        }
    };
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let db = Db::builtin().map_err(|e| e.to_string())?;
    let devices: Vec<_> = discover(&api, &db)
        .into_iter()
        .filter(|d| d.pid == pid)
        .collect();
    if devices.len() != 1 {
        return Err(format!(
            "Expected one {pid:04X} device, found {}",
            devices.len()
        ));
    }
    let device = &devices[0];
    let mut registers = Vec::new();
    if pid == 0x00c4 {
        registers.push(Register::capture(
            device,
            "dpi-saved",
            cmd::DPI,
            cmd::DPI_SIZE,
            &[cmd::VARSTORE],
            true,
        )?);
        registers.push(Register::capture(
            device,
            "dpi-live",
            cmd::DPI,
            cmd::DPI_SIZE,
            &[cmd::NOSTORE],
            true,
        )?);
        registers.push(Register::capture(
            device,
            "polling",
            cmd::POLLING,
            cmd::POLLING_SIZE,
            &[],
            true,
        )?);
        registers.push(Register::capture(
            device,
            "idle-seconds",
            cmd::IDLE_TIME,
            cmd::IDLE_TIME_SIZE,
            &[],
            true,
        )?);
        registers.push(Register::capture(
            device,
            "dpi-stages",
            cmd::DPI_STAGES,
            cmd::DPI_STAGES_SIZE,
            &[cmd::VARSTORE],
            true,
        )?);
    } else {
        registers.push(Register::capture(
            device,
            "brightness-saved",
            cmd::MATRIX_BRIGHTNESS,
            3,
            &[cmd::VARSTORE, cmd::BACKLIGHT_LED, 0],
            true,
        )?);
        registers.push(Register::capture(
            device,
            "brightness-live",
            cmd::MATRIX_BRIGHTNESS,
            3,
            &[cmd::NOSTORE, cmd::BACKLIGHT_LED, 0],
            true,
        )?);
    }
    if inject_failure && pid != 0x00c4 {
        return Err("Failure injection applies only to the mouse DPI check".into());
    }
    let mut report = json!({"product_id":format!("{pid:04X}"),"transport":"real-hid", "mode":if roundtrip {"roundtrip"} else {"read-only"},
        "snapshot":registers.iter().map(|r|json!({"name":r.name,"bytes":r.original})).collect::<Vec<_>>(),"steps":[],"restore_verified":false});
    let path = Path::new(&args[1]);
    write_report(path, &report)?;
    if !roundtrip {
        println!("Read-only snapshot saved for {pid:04X}");
        return Ok(());
    }
    // Avoid assuming independent live/saved semantics when current values differ.
    let equal = if pid == 0x00c4 {
        registers[0].original[1..] == registers[1].original[1..]
    } else {
        registers[0].original[2] == registers[1].original[2]
    };
    if !equal {
        return Err("Live and saved values differ; no settings were changed because exact restoration semantics need further validation".into());
    }
    for register in &registers {
        register.restore_packet()?;
    }
    let mut recovery = Recovery {
        device,
        registers,
        armed: true,
    };
    let mut steps = Vec::new();
    let exercise = (|| -> Result<(), String> {
        if pid == 0x00c4 {
            let before = device.dpi().map_err(|e| e.to_string())?;
            let target = if before.0 == 800 { 850 } else { 800 };
            let sent = device
                .set_dpi(target, before.1)
                .map_err(|e| e.to_string())?;
            let actual = device.dpi().map_err(|e| e.to_string())?;
            steps.push(
                json!({"setting":"dpi","requested":sent,"actual":actual,"passed":actual==sent}),
            );
            if actual != sent {
                return Err("DPI readback differs".into());
            }
            let stages = recovery
                .registers
                .iter()
                .find(|r| r.name == "dpi-stages")
                .ok_or("Missing stage snapshot")?;
            steps.last_mut().unwrap()["stage_register_unchanged_after_dpi"] =
                json!(stages.read(device)? == stages.original);
            if inject_failure {
                return Err("Injected workflow failure after the verified DPI write".into());
            }
            let before = device.polling_rate().map_err(|e| e.to_string())?;
            let target = if before == 500 { 1000 } else { 500 };
            device.set_polling_rate(target).map_err(|e| e.to_string())?;
            let actual = device.polling_rate().map_err(|e| e.to_string())?;
            steps.push(json!({"setting":"polling","requested":target,"actual":actual,"passed":actual==target}));
            if actual != target {
                return Err("Polling readback differs".into());
            }
            let before = device
                .sleep_timer()
                .map_err(|e| e.to_string())?
                .ok_or("Missing sleep timer")?;
            let target = if before.minutes == 5 { 6 } else { 5 };
            device
                .set_sleep_minutes(target)
                .map_err(|e| e.to_string())?;
            let actual = device
                .sleep_timer()
                .map_err(|e| e.to_string())?
                .ok_or("Missing sleep timer")?
                .minutes;
            steps.push(json!({"setting":"sleep-minutes","requested":target,"actual":actual,"passed":actual==target}));
            if actual != target {
                return Err("Sleep readback differs".into());
            }
            let mut target = device.dpi_stages().map_err(|e| e.to_string())?;
            let stage = target.stages.first_mut().ok_or("No DPI stages")?;
            stage.0 = if stage.0 == 650 { 700 } else { 650 };
            let sent = device.set_dpi_stages(&target).map_err(|e| e.to_string())?;
            let actual = device.dpi_stages().map_err(|e| e.to_string())?;
            steps.push(json!({"setting":"dpi-stages","requested":sent,"actual":actual,"passed":actual==sent}));
            if actual != sent {
                return Err("DPI stage readback differs".into());
            }
            let mut intent = secbird_core::performance::Performance::from_hardware(&actual)
                .map_err(|e| e.to_string())?;
            intent.set_visible(1, false).map_err(|e| e.to_string())?;
            intent.active = 4;
            intent.enabled = false;
            for (name, enabled) in [
                ("performance-disabled", false),
                ("performance-hidden-rows", true),
            ] {
                intent.enabled = enabled;
                let sent = intent.apply(device).map_err(|e| e.to_string())?;
                let packed = sent.pack().map_err(|e| e.to_string())?;
                let stages = device.dpi_stages().map_err(|e| e.to_string())?;
                let dpi = device.dpi().map_err(|e| e.to_string())?;
                let current = packed.stages[usize::from(packed.active - 1)];
                let passed =
                    stages == packed && dpi == current && sent.stages[1] == intent.stages[1];
                steps.push(json!({"setting":name,"passed":passed,"active_hardware_stage":stages.active,"hardware_stage_count":stages.stages.len(),"retained_row_count":sent.stages.len()}));
                if !passed {
                    return Err(format!("{name} projection/current-DPI readback differs"));
                }
            }
            let profile = secbird_core::Profile {
                performance: Some(intent.clone()),
                dpi: Some((800, 800)),
                ..Default::default()
            };
            let results = profile.apply(device);
            let expected = intent.pack().map_err(|e| e.to_string())?;
            let passed = results.iter().all(|r| r.result.is_ok())
                && device.dpi().map_err(|e| e.to_string())?
                    == expected.stages[usize::from(expected.active - 1)];
            steps.push(json!({"setting":"performance-profile-activation","passed":passed}));
            if !passed {
                return Err("Profile activation did not prioritize stage intent".into());
            }
        } else {
            let before = device
                .brightness(cmd::BACKLIGHT_LED)
                .map_err(|e| e.to_string())?;
            let target = if before == 50 { 60 } else { 50 };
            device
                .set_brightness(cmd::BACKLIGHT_LED, target)
                .map_err(|e| e.to_string())?;
            let actual = device
                .brightness(cmd::BACKLIGHT_LED)
                .map_err(|e| e.to_string())?;
            steps.push(json!({"setting":"brightness-percent","requested":target,"actual":actual,"passed":actual==target}));
            if actual != target {
                return Err("Brightness readback differs".into());
            }
        }
        Ok(())
    })();
    let errors = recovery.restore();
    report["steps"] = json!(steps);
    report["exercise_error"] = json!(exercise.as_ref().err());
    report["restore_errors"] = json!(errors);
    report["restore_verified"] = json!(errors.is_empty());
    write_report(path, &report)?;
    if !errors.is_empty() {
        return Err("Original settings did not verify after restoration; recovery snapshot is in the report".into());
    }
    exercise?;
    println!(
        "{pid:04X}: {} setting checks passed; every captured register restored exactly",
        steps.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_restore_converts_slot_ids_and_preserves_extra_axis_bytes() {
        let register = Register {
            name: "dpi-stages",
            command: cmd::DPI_STAGES,
            size: 38,
            query: vec![1],
            original: [
                vec![
                    1, 2, 2, 1, 2, 88, 2, 88, 0x12, 0x34, 2, 3, 32, 3, 32, 0x56, 0x78,
                ],
                vec![0; 21],
            ]
            .concat(),
            restore: true,
        };
        let (size, args) = register.restore_packet().unwrap();
        assert_eq!(size, 17);
        assert_eq!(
            args,
            vec![1, 2, 2, 0, 2, 88, 2, 88, 0x12, 0x34, 1, 3, 32, 3, 32, 0x56, 0x78]
        );
        let mut bad = register;
        bad.original[3] = 0;
        assert!(bad.restore_packet().is_err());
    }
}
