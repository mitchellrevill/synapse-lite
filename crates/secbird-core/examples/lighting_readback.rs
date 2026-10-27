//! Explicitly opt-in real-device GETs. Never run by ordinary tests.
//! Usage: lighting_readback <0|1> <report.json> --read-only
//! Opens exactly one PID025D/interface2 candidate; refuses SECBIRD_SIM.
//! Does not change device mode, brightness, effects, profiles or driver state.

use secbird_core::{
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    proto::{checksum, Command, Report, Status, REPORT_LEN},
    Db, Device, Error, Transport,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const MAX_TRACE_HEADERS: usize = 64;

#[derive(Clone, Copy)]
struct EffectRequest {
    txid: u8,
}

#[derive(Default)]
struct EffectTrace {
    active: Option<EffectRequest>,
    headers: Vec<(Value, u64)>,
    total_reads: u64,
    omitted_reads: u64,
}

impl EffectTrace {
    fn observe(&mut self, request: EffectRequest, buffer: &[u8], read: Option<usize>) {
        self.total_reads = self.total_reads.saturating_add(1);
        let header = match read {
            None => json!({"read_error": true}),
            Some(n) if (n == REPORT_LEN || n == REPORT_LEN + 1) && n <= buffer.len() => {
                let body: &[u8; REPORT_LEN] = buffer[n - REPORT_LEN..n].try_into().unwrap();
                let reply = Report(*body);
                let command = Command::new(0x0f, 0x82);
                let minimum = if reply.status() == Status::Success {
                    6
                } else {
                    0
                };
                let frame_valid = reply.valid_reply(request.txid, command, minimum);
                let disposition = if !frame_valid {
                    "rejected_by_current_transport"
                } else {
                    match reply.status() {
                        Status::Success => "accepted_success",
                        Status::Busy | Status::New => "retry_busy_or_new",
                        _ => "device_status_error",
                    }
                };
                // No argument bytes are recorded, even for a stale serial reply.
                json!({
                    "bytes_read": n, "hid_report_id": if n == REPORT_LEN + 1 { Some(buffer[0]) } else { None },
                    "status": body[0], "status_name": format!("{:?}", reply.status()),
                    "requested_transaction": request.txid, "returned_transaction": body[1],
                    "remaining_packets_bytes": [body[2], body[3]], "protocol_type": body[4],
                    "declared_size": body[5], "command_class": body[6], "command_id": body[7],
                    "checksum_stored": body[88], "checksum_computed": checksum(body),
                    "checksum_valid": body[88] == checksum(body), "reserved": body[89],
                    "transaction_matches": body[1] == request.txid,
                    "command_matches": reply.command() == command,
                    "request_declared_size": 80, "required_success_minimum": 6,
                    "meets_current_success_minimum": body[5] >= 6,
                    "passes_current_frame_checks": frame_valid, "disposition": disposition
                })
            }
            Some(n) => json!({"bytes_read": n, "disposition": "invalid_report_length"}),
        };
        if let Some((_, count)) = self.headers.iter_mut().find(|(known, _)| *known == header) {
            *count = count.saturating_add(1);
        } else if self.headers.len() < MAX_TRACE_HEADERS {
            self.headers.push((header, 1));
        } else {
            self.omitted_reads = self.omitted_reads.saturating_add(1);
        }
    }

    fn report(&self) -> Value {
        json!({
            "scope": "response headers while GET0F82 is active only; no argument payloads",
            "max_distinct_headers": MAX_TRACE_HEADERS, "total_reads": self.total_reads,
            "omitted_reads_at_capacity": self.omitted_reads,
            "headers": self.headers.iter().map(|(header, count)| json!({"header": header, "count": count})).collect::<Vec<_>>()
        })
    }
}

/// Transparent observer: forwarding and read delays are unchanged. Serial
/// discovery and brightness reads never activate observation.
struct TracedEffectTransport<T> {
    inner: T,
    trace: Arc<Mutex<EffectTrace>>,
}

impl<T: Transport> Transport for TracedEffectTransport<T> {
    fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
        let active = if data.len() == REPORT_LEN + 1 && data[7..9] == [0x0f, 0x82] {
            Some(EffectRequest { txid: data[2] })
        } else {
            None
        };
        self.trace.lock().unwrap().active = active;
        self.inner.send_feature_report(data)
    }

    fn get_feature_report(&self, buffer: &mut [u8]) -> secbird_core::Result<usize> {
        let active = self.trace.lock().unwrap().active;
        let result = self.inner.get_feature_report(buffer);
        if let Some(request) = active {
            self.trace
                .lock()
                .unwrap()
                .observe(request, buffer, result.as_ref().ok().copied());
        }
        result
    }

    fn read_delay(&self) -> std::time::Duration {
        self.inner.read_delay()
    }
}

fn safe_error(error: Error) -> String {
    match error {
        // HID backend messages can contain paths; omit them from this report.
        Error::Hid(_) => "HID transport failed; backend detail omitted".into(),
        other => other.to_string(),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || args[2] != "--read-only" || !matches!(args[0].as_str(), "0" | "1") {
        return Err("usage: lighting_readback <0|1> <report.json> --read-only".into());
    }
    if std::env::var_os("SECBIRD_SIM").is_some() {
        return Err("Lighting readback example refuses SECBIRD_SIM".into());
    }
    let profile: u8 = args[0].parse().map_err(|_| "Invalid profile")?;
    let api = HidApi::new().map_err(|_| "Cannot initialize real HID transport")?;
    let found: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|candidate| candidate.pid == 0x025d && candidate.interface == 2)
        .collect();
    if found.len() != 1 {
        return Err(format!(
            "Expected exactly one PID025D/interface2 candidate, found {}",
            found.len()
        ));
    }
    let candidate = &found[0];
    let db = Db::builtin().map_err(safe_error)?;
    let spec = db.get(0x025d).cloned();
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
        .map_err(|_| "Cannot open the sole Ornata transport")?;
    let trace = Arc::new(Mutex::new(EffectTrace::default()));
    let device = Device::open(
        Box::new(TracedEffectTransport {
            inner: hid,
            trace: trace.clone(),
        }),
        0x025d,
        2,
        candidate.path.clone(),
        &txids,
        spec,
    )
    .ok_or("The sole Ornata transport did not answer the read-only discovery query")?;
    let effect = match device.ornata_effect_readback(profile, 0) {
        Ok(snapshot) => json!({
            "status": "success", "requested_profile": snapshot.requested.profile,
            "requested_region": snapshot.requested.region,
            "returned_profile": snapshot.returned.profile, "returned_region": snapshot.returned.region,
            "selectors_match": snapshot.selectors_match(), "declared_size": snapshot.declared_size(),
            "effect_id": snapshot.effect_id(), "flags": snapshot.flags(), "rate": snapshot.rate(),
            "color_count": snapshot.color_count(), "colors": snapshot.colors(),
            "raw_args_80": snapshot.raw_args().to_vec(), "custom_frame": snapshot.is_custom_frame(),
            "restoration_proven": false
        }),
        Err(error) => json!({"status": "error", "message": safe_error(error)}),
    };
    let brightness = match device.ornata_brightness_readback(profile, 0) {
        Ok(snapshot) => json!({
            "status": "success", "requested_profile": snapshot.requested.profile,
            "requested_region": snapshot.requested.region,
            "returned_profile": snapshot.returned.profile, "returned_region": snapshot.returned.region,
            "selectors_match": snapshot.selectors_match(), "raw_args_3": snapshot.raw_args(),
            "raw_brightness": snapshot.raw_brightness()
        }),
        Err(error) => json!({"status": "error", "message": safe_error(error)}),
    };
    let report: Value = json!({
        "pid": "025D", "interface": 2, "read_only": true, "real_transport": true,
        "effect": effect, "brightness": brightness,
        "effect_transport_headers": trace.lock().unwrap().report(),
        "owner": "unknown", "host_animation_captured": false,
        "limitation": "Firmware CustomFrame8 does not capture host animation, phase, frames or ownership. No restoration operation is provided."
    });
    std::fs::write(
        &args[1],
        serde_json::to_vec_pretty(&report).map_err(|_| "Cannot encode report")?,
    )
    .map_err(|_| "Cannot save readback report")?;
    println!("Saved read-only Ornata report; restoration and host ownership remain unproven.");
    if report["effect"]["status"] != "success" || report["brightness"]["status"] != "success" {
        return Err("One or more lighting reads failed; details saved in report".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FixedReply {
        bytes: Vec<u8>,
        sends: Arc<AtomicUsize>,
    }

    impl Transport for FixedReply {
        fn send_feature_report(&self, _: &[u8]) -> secbird_core::Result<()> {
            self.sends.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn get_feature_report(&self, buffer: &mut [u8]) -> secbird_core::Result<usize> {
            buffer[..self.bytes.len()].copy_from_slice(&self.bytes);
            Ok(self.bytes.len())
        }
        fn read_delay(&self) -> std::time::Duration {
            std::time::Duration::from_millis(7)
        }
    }

    fn response(txid: u8, command: Command, size: u8, status: u8) -> Vec<u8> {
        let mut report = Report::request(txid, command, size, &[]);
        report.0[0] = status;
        let mut bytes = vec![0];
        bytes.extend_from_slice(&report.0);
        bytes
    }

    fn request(command: Command) -> Vec<u8> {
        let mut bytes = vec![0];
        bytes.extend_from_slice(&Report::request(0x1f, command, 80, &[1, 0]).0);
        bytes
    }

    #[test]
    fn tracing_excludes_serial_and_brightness_and_forwards_unchanged() {
        let mut bytes = response(0x1f, Command::new(0, 0x82), 22, 2);
        let secret = b"PRIVATE_SERIAL_MARKER";
        bytes[9..9 + secret.len()].copy_from_slice(secret);
        let sends = Arc::new(AtomicUsize::new(0));
        let trace = Arc::new(Mutex::new(EffectTrace::default()));
        let pipe = TracedEffectTransport {
            inner: FixedReply {
                bytes: bytes.clone(),
                sends: sends.clone(),
            },
            trace: trace.clone(),
        };
        let mut buffer = [0; 91];
        pipe.send_feature_report(&request(Command::new(0, 0x82)))
            .unwrap();
        assert_eq!(pipe.get_feature_report(&mut buffer).unwrap(), 91);
        assert_eq!(&buffer[..], bytes.as_slice());
        assert_eq!(trace.lock().unwrap().total_reads, 0);
        pipe.send_feature_report(&request(Command::new(0x0f, 0x82)))
            .unwrap();
        pipe.get_feature_report(&mut buffer).unwrap();
        assert_eq!(trace.lock().unwrap().total_reads, 1);
        let text = trace.lock().unwrap().report().to_string();
        assert!(!text.contains("PRIVATE_SERIAL_MARKER"));
        assert!(!text.contains("raw_args"));
        pipe.send_feature_report(&request(Command::new(0x0f, 0x84)))
            .unwrap();
        pipe.get_feature_report(&mut buffer).unwrap();
        assert_eq!(trace.lock().unwrap().total_reads, 1);
        assert_eq!(sends.load(Ordering::Relaxed), 3);
        assert_eq!(pipe.read_delay(), std::time::Duration::from_millis(7));
    }

    #[test]
    fn header_diagnostics_distinguish_short_alias_unsupported_and_bad_checksum() {
        let request = EffectRequest { txid: 0x1f };
        let mut trace = EffectTrace::default();
        let short = response(0x1f, Command::new(0x0f, 0x82), 5, 2);
        trace.observe(request, &short, Some(91));
        let header = &trace.headers[0].0;
        assert_eq!(header["declared_size"], 5);
        assert_eq!(header["checksum_valid"], true);
        assert_eq!(header["transaction_matches"], true);
        assert_eq!(header["passes_current_frame_checks"], false);
        let alias = response(0x3f, Command::new(0x0f, 0x82), 80, 2);
        trace.observe(request, &alias, Some(91));
        assert_eq!(trace.headers[1].0["transaction_matches"], false);
        let unsupported = response(0x1f, Command::new(0x0f, 0x82), 0, 5);
        trace.observe(request, &unsupported, Some(91));
        assert_eq!(trace.headers[2].0["disposition"], "device_status_error");
        let mut corrupt = response(0x1f, Command::new(0x0f, 0x82), 80, 2);
        corrupt[89] ^= 1;
        trace.observe(request, &corrupt, Some(91));
        assert_eq!(trace.headers[3].0["checksum_valid"], false);
        trace.observe(request, &[0; 12], Some(12));
        assert_eq!(trace.headers[4].0["disposition"], "invalid_report_length");
        let observed_six_byte_reply = response(0x1f, Command::new(0x0f, 0x82), 6, 2);
        trace.observe(request, &observed_six_byte_reply, Some(91));
        assert_eq!(trace.headers[5].0["request_declared_size"], 80);
        assert_eq!(trace.headers[5].0["required_success_minimum"], 6);
        assert_eq!(trace.headers[5].0["passes_current_frame_checks"], true);
        assert_eq!(trace.headers[5].0["disposition"], "accepted_success");
    }

    #[test]
    fn trace_bounds_distinct_headers_and_deduplicates_after_capacity() {
        let mut trace = EffectTrace::default();
        let request = EffectRequest { txid: 0x1f };
        for txid in 0..70 {
            let bytes = response(txid, Command::new(0x0f, 0x82), 80, 2);
            trace.observe(request, &bytes, Some(91));
        }
        let repeated = response(0, Command::new(0x0f, 0x82), 80, 2);
        trace.observe(request, &repeated, Some(91));
        assert_eq!(trace.headers.len(), MAX_TRACE_HEADERS);
        assert_eq!(trace.headers[0].1, 2);
        assert_eq!(trace.total_reads, 71);
        assert_eq!(trace.omitted_reads, 6);
    }
}
