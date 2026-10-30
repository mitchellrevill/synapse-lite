//! Local diagnostic snapshot. No catalog entry grants a protocol query.
use crate::Device;

/// Explicit family/interface admission for the existing Protocol 2.5 identity GETs.
pub fn identity_queries_supported(pid: u16, interface: i32) -> bool {
    matches!((pid, interface), (0x025d, 2) | (0x00c4 | 0x00c5, 0) | (0x0099, 3))
}

/// Runs only on explicit user request, never during painting or startup.
pub fn diagnostic_text(device: &Device) -> String {
    let mut text = format!("Product: {}\r\nUSB VID:PID: 1532:{:04X}\r\nControl interface: {}\r\nConnection generation: {}\r\nDiscovery serial: {}\r\n", device.name(), device.pid, device.interface, device.connection_id(), if device.serial.is_empty() { "Not reported" } else { &device.serial });
    let observed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_secs()).unwrap_or(0);
    text.push_str(&format!("Snapshot time (Unix seconds): {observed}\r\n"));
    let route = match device.pid {
        0x00c4 => "USB cable",
        0x00c5 => "USB wireless receiver (radio state not inferred)",
        0x025d | 0x0099 => "USB cable",
        _ => "USB HID control handle; cable/radio route unverified",
    };
    text.push_str(&format!("Connection: {route}\r\n"));
    if identity_queries_supported(device.pid, device.interface) {
        match device.serial() {
            Ok(serial) if !serial.is_empty() && serial == device.serial => {
                text.push_str("Identity: responding; serial matches discovery\r\n");
                match device.firmware() {
                    Ok((major, minor)) => text.push_str(&format!("Firmware: {major}.{minor} (two reported components; build unknown)\r\n")),
                    Err(error) => text.push_str(&format!("Firmware read failed: {error}\r\n")),
                }
            }
            Ok(_) => text.push_str("Identity unavailable or changed; rescan devices before using this handle. Firmware not queried.\r\n"),
            Err(error) => text.push_str(&format!("Identity read failed (disconnected, asleep or busy): {error}\r\nFirmware not queried.\r\n")),
        }
    } else {
        text.push_str("Live serial/firmware: unavailable; no admitted family/interface adapter\r\n");
    }
    if let Some(spec) = &device.spec {
        text.push_str(&format!("\r\nDatabase declarations (not hardware verification):\r\nKind: {:?}\r\nLighting protocol: {:?}\r\nLighting zones: {}\r\nWireless flag: {}\r\nOnboard profile slots: {}\r\nSource: {}\r\n", spec.kind, spec.lighting, spec.leds.len(), spec.wireless, spec.onboard_profiles, spec.source));
        if let Some(polling) = spec.polling { text.push_str(&format!("Polling family: {polling:?} (not current rate)\r\n")); }
        if let Some(dpi) = &spec.dpi { text.push_str(&format!("DPI: {}–{}, step {}\r\n", dpi.min,dpi.max,dpi.step)); }
        if let Some(matrix) = spec.matrix { text.push_str(&format!("Lighting matrix: {} x {}\r\n",matrix.rows,matrix.cols)); }
    }
    text.push_str("\r\nLocal snapshot; may become stale after unplug/sleep. No settings were changed.");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_uses_gets_and_never_fabricates_build_components() {
        let sim = crate::sim::Sim::keyboard();
        let mut dev = sim.open(0x025d, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID)).unwrap();
        dev.interface = 2;
        dev.enforce_query_only();
        sim.state().log.clear();
        let text = diagnostic_text(&dev);
        assert!(text.contains("build unknown"));
        assert!(text.contains("not hardware verification"));
        assert!(!text.contains(".0.0"));
        assert!(sim.state().log.iter().all(|row| row.id & 0x80 != 0));
    }
    #[test]
    fn unsupported_family_does_not_query_and_changed_identity_skips_firmware() {
        let sim = crate::sim::Sim::keyboard();
        let mut dev = sim.open(0x025d, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID)).unwrap();
        dev.interface = 3;
        sim.state().log.clear();
        assert!(diagnostic_text(&dev).contains("no admitted"));
        assert!(sim.state().log.is_empty());
        dev.interface = 2;
        dev.serial = "different".into();
        assert!(diagnostic_text(&dev).contains("Firmware not queried"));
        assert!(sim.state().log.iter().all(|row| row.id != 0x81));
        sim.state().available = false;
        assert!(diagnostic_text(&dev).contains("Identity read failed"));
    }
    #[test]
    fn query_admission_is_exact_and_excludes_forwarded_and_unknown_families() {
        for (pid, interface) in [(0x025d,2),(0x00c4,0),(0x00c5,0),(0x0099,3)] { assert!(identity_queries_supported(pid,interface)); assert!(!identity_queries_supported(pid,interface+1)); }
        for pid in [0x00a4,0x00ac,0x0548,0xffff,0x00aa] { assert!(!identity_queries_supported(pid,0)); }
    }
}
