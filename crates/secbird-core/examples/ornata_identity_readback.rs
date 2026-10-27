//! Opt-in real-device GET probe; never executed by ordinary tests.
//! Usage: ornata_identity_readback <report.json> --read-only
//! See docs/ORNATA-IDENTITY.md. No device/process/configuration changes.

use secbird_core::{
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    proto::{checksum, Command, Report, Status},
    Db, Device, DeviceSpec, Error, Transport,
};
use serde_json::{json, Value};
use std::{
    ffi::CString,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const SERIAL: Command = Command::new(0, 0x82);
const EDITION: Command = Command::new(0, 0x86);
const TRACE_CAP: usize = 64;

#[derive(Clone, Copy)]
struct Request {
    txid: u8,
    command: Command,
    request_size: u8,
    minimum: u8,
}
#[derive(Default)]
struct Trace {
    active: Option<Request>,
    serial_attempts: usize,
    edition_attempts: usize,
    successful_outs: usize,
    denied_requests: usize,
    denied_sets: usize,
    read_count: usize,
    omitted_headers: usize,
    request_headers: Vec<Value>,
    response_headers: Vec<Value>,
    correlated_edition: Option<Value>,
}

impl Trace {
    fn admit(&mut self, data: &[u8], max_serial: usize) -> secbird_core::Result<()> {
        let is_set = data.len() == 91 && data[8] & 0x80 == 0;
        let request = if data.len() == 91 && data[0] == 0 {
            let body: [u8; 90] = data[1..].try_into().unwrap();
            let report = Report(body);
            let command = report.command();
            let size = match command {
                SERIAL => Some(22),
                EDITION => Some(3),
                _ => None,
            };
            size.filter(|size| report.0 == Report::request(report.txid(), command, *size, &[]).0)
                .map(|request_size| Request {
                    txid: report.txid(),
                    command,
                    request_size,
                    minimum: if command == EDITION { 2 } else { request_size },
                })
        } else {
            None
        };
        let request = request.filter(|r| {
            if r.command == SERIAL {
                self.edition_attempts == 0 && self.serial_attempts < max_serial
            } else {
                self.edition_attempts == 0
            }
        });
        let Some(request) = request else {
            self.denied_requests += 1;
            self.denied_sets += usize::from(is_set);
            return Err(Error::Unsupported(
                "GET-only probe transport denied a noncanonical or unapproved request".into(),
            ));
        };
        if request.command == SERIAL {
            self.serial_attempts += 1;
        } else {
            self.edition_attempts += 1;
        }
        if self.request_headers.len() < TRACE_CAP {
            self.request_headers.push(json!({
                "transaction":request.txid,"command_class":request.command.class,
                "command_id":request.command.id,"declared_size":request.request_size,
                "required_success_minimum":request.minimum,
                "hid_report_id":0,"packet_bytes":91,"canonical_zero_payload":true,
                "checksum_valid":true
            }));
        }
        self.active = Some(request);
        Ok(())
    }
    fn observe(&mut self, buffer: &[u8], length: Option<usize>) {
        self.read_count += 1;
        let Some(request) = self.active else { return };
        let header = if let Some(n) = length.filter(|&n| (n == 90 || n == 91) && n <= buffer.len())
        {
            let body: [u8; 90] = buffer[n - 90..n].try_into().unwrap();
            let report = Report(body);
            let min = if report.status() == Status::Success {
                request.minimum
            } else {
                0
            };
            let valid = report.valid_reply(request.txid, request.command, min);
            if request.command == EDITION && valid && report.status() == Status::Success {
                self.correlated_edition = Some(json!({
                    "body_bytes":report.0.to_vec(),"carrier_bytes":n,
                    "hid_report_id":if n==91 {Some(buffer[0])} else {None},
                    "declared_args":report.args()[..usize::from(body[5])].to_vec(),
                    "scope":"correlated successful edition GET only; serial argument bytes are not retained"
                }));
            }
            json!({"carrier_bytes":n,"status":body[0],"transaction":body[1],
                "command_class":body[6],"command_id":body[7],"declared_size":body[5],
                "checksum_stored":body[88],"checksum_computed":checksum(&body),
                "passes_current_frame_checks":valid,"for_serial_baseline":request.command==SERIAL})
        } else {
            json!({"carrier_bytes":length,"read_error":length.is_none(),"invalid_length":length.is_some()})
        };
        if self.response_headers.len() < TRACE_CAP {
            self.response_headers.push(header);
        } else {
            self.omitted_headers += 1;
        }
    }
    fn report(&self) -> Value {
        json!({
            "guard":"strict canonical allowlist installed before Device::open: serial GET00:82 size22, then one edition GET00:86 size3",
            "serial_get_forward_attempts":self.serial_attempts,"edition_get_forward_attempts":self.edition_attempts,
            "successful_usb_out_returns":self.successful_outs,"forwarded_set_commands":0,
            "denied_requests":self.denied_requests,"denied_set_requests":self.denied_sets,
            "read_count":self.read_count,"header_capacity":TRACE_CAP,"omitted_response_headers":self.omitted_headers,
            "request_headers":self.request_headers,"response_headers":self.response_headers,
            "correlated_edition_response":self.correlated_edition,
            "serial_arguments_and_device_paths_retained":false
        })
    }
}

struct GuardedTransport {
    inner: Box<dyn Transport>,
    trace: Arc<Mutex<Trace>>,
    max_serial: usize,
}
impl Transport for GuardedTransport {
    fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
        self.trace.lock().unwrap().admit(data, self.max_serial)?;
        self.inner.send_feature_report(data)?;
        self.trace.lock().unwrap().successful_outs += 1;
        Ok(())
    }
    fn get_feature_report(&self, buffer: &mut [u8]) -> secbird_core::Result<usize> {
        let result = self.inner.get_feature_report(buffer);
        self.trace
            .lock()
            .unwrap()
            .observe(buffer, result.as_ref().ok().copied());
        result
    }
    fn read_delay(&self) -> std::time::Duration {
        self.inner.read_delay()
    }
}

fn safe_error(error: Error) -> String {
    match error {
        Error::Device { cmd, status } => format!("Device rejected {cmd}: {status:?}"),
        Error::NoReply(cmd) => format!("No valid reply to {cmd}"),
        Error::Unsupported(_) => "Unsupported identity query".into(),
        _ => "Identity query failed; private backend detail omitted".into(),
    }
}
fn probe(
    inner: Box<dyn Transport>,
    path: CString,
    txids: &[u8],
    spec: Option<DeviceSpec>,
) -> (Value, bool) {
    let trace = Arc::new(Mutex::new(Trace::default()));
    let guarded = GuardedTransport {
        inner,
        trace: trace.clone(),
        max_serial: txids.len(),
    };
    let device = Device::open(Box::new(guarded), 605, 2, path, txids, spec);
    let (identity, error, query_only) = match device {
        None => (
            None,
            Some("Serial GET baseline did not open the selected device".to_string()),
            false,
        ),
        Some(mut dev) => {
            dev.enforce_query_only();
            match dev.ornata_edition_readback() {
                Ok(value) => (
                    Some(
                        json!({"layout_id":value.layout_id(),"edition_id":value.edition_id(),
                    "firmware_id":value.firmware_id(),"declared_size":value.declared_size(),
                    "declared_args":value.declared_args(),"spatial_map_resolved":false,
                    "raw_unknown_ids_preserved":true}),
                    ),
                    None,
                    true,
                ),
                Err(error) => (None, Some(safe_error(error)), true),
            }
        }
    };
    let success = error.is_none();
    (
        json!({"success":success,"device_scope":{"vendor_id":0x1532,"product_id":605,"interface":2},
        "protocol":{"class":0,"get_id":0x86,"declared_request_size":3,"minimum_success_response_size":2,"firmware_id_optional":true,"input_selectors":[],"hid_report_id":0,"body_bytes":90},
        "provenance":{"source_release":"aef3665b36baca6993b26cdc296cda8bdab28bbd",
            "source_sha256":"0a463302ecaf113705ebb648f71f377d90d37d7194d6d73578644af080c12a7f",
            "source_evidence":"ev_5be7f3ba9bd58ae2b835c1a4a739dfb209cea9dbcfd175629a86868a4915ac31",
            "method":"current GET observation; no cache/locale/default/dock fallback"},
        "query_only_device_handle":query_only,"identity":identity,"error":error,"trace":trace.lock().unwrap().report(),
        "limits":"No SETs, process changes, driver changes, configuration writes, or spatial rendering; file output only"}),
        success,
    )
}

fn parse_args(args: &[String], simulation_present: bool) -> Result<PathBuf, String> {
    if args.len() != 2
        || args[1] != "--read-only"
        || args[0].is_empty()
        || args[0].starts_with("--")
    {
        return Err("usage: ornata_identity_readback <report.json> --read-only".into());
    }
    if simulation_present {
        return Err("Identity readback refuses SECBIRD_SIM".into());
    }
    Ok(PathBuf::from(&args[0]))
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let output = parse_args(&args, std::env::var_os("SECBIRD_SIM").is_some())?;
    let api = HidApi::new().map_err(|_| "Cannot initialize real HID transport")?;
    let found: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|c| c.pid == 605 && c.interface == 2)
        .collect();
    if found.len() != 1 {
        return Err(format!(
            "Expected exactly one PID025D/interface2 candidate, found {}",
            found.len()
        ));
    }
    let candidate = &found[0];
    let db = Db::builtin().map_err(|_| "Cannot load builtin device specification")?;
    let spec = db.get(605).cloned();
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
    let (report, success) = probe(Box::new(hid), candidate.path.clone(), &txids, spec);
    let bytes = serde_json::to_vec_pretty(&report).map_err(|_| "Cannot encode probe report")?;
    std::fs::write(output, bytes).map_err(|_| "Cannot write requested probe report")?;
    if !success {
        return Err("Identity GET failed; see requested report for sanitized evidence".into());
    }
    println!("Read-only identity report written; no SETs forwarded");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opt_in_and_simulation_refusal_happen_before_discovery() {
        assert!(parse_args(&[], false).is_err());
        assert!(parse_args(&["out.json".into()], false).is_err());
        assert!(parse_args(&["out.json".into(), "--read-only".into()], true).is_err());
        assert!(parse_args(&["out.json".into(), "--read-only".into()], false).is_ok());
    }
    #[test]
    fn guard_refuses_set_other_get_noncanonical_and_repeated_edition() {
        let packet = |command, size| {
            let mut p = vec![0];
            p.extend(Report::request(0x1f, command, size, &[]).0);
            p
        };
        let mut trace = Trace::default();
        for bytes in [
            packet(Command::new(0, 6), 3),
            packet(Command::new(0, 0x84), 2),
            packet(EDITION, 2),
        ] {
            assert!(trace.admit(&bytes, 4).is_err());
        }
        let mut dirty = packet(EDITION, 3);
        dirty[9] = 7;
        let body: [u8; 90] = dirty[1..].try_into().unwrap();
        dirty[89] = checksum(&body);
        assert!(trace.admit(&dirty, 4).is_err());
        assert!(trace.admit(&packet(SERIAL, 22), 4).is_ok());
        assert!(trace.admit(&packet(EDITION, 3), 4).is_ok());
        assert!(trace.admit(&packet(EDITION, 3), 4).is_err());
        assert!(trace.admit(&packet(SERIAL, 22), 4).is_err());
        assert_eq!(trace.denied_sets, 1);
        assert_eq!((trace.serial_attempts, trace.edition_attempts), (1, 1));
    }

    #[test]
    fn denied_commands_never_reach_inner_transport_and_trace_is_bounded_private() {
        let sim = secbird_core::sim::Sim::keyboard();
        let trace = Arc::new(Mutex::new(Trace::default()));
        let guarded = GuardedTransport {
            inner: Box::new(sim.clone()),
            trace: trace.clone(),
            max_serial: 4,
        };
        for command in [Command::new(0, 6), Command::new(0, 0x84)] {
            let mut packet = vec![0];
            packet.extend(Report::request(0xff, command, 3, &[]).0);
            assert!(guarded.send_feature_report(&packet).is_err());
        }
        assert!(sim.state().log.is_empty());
        let mut state = trace.lock().unwrap();
        state.active = Some(Request {
            txid: 0xff,
            command: SERIAL,
            request_size: 22,
            minimum: 22,
        });
        let mut packet = vec![0];
        let mut response = Report::request(0xff, SERIAL, 22, b"PRIVATE_SERIAL_MARKER");
        response.0[0] = 2;
        packet.extend(response.0);
        for _ in 0..100 {
            state.observe(&packet, Some(91));
        }
        assert_eq!(state.response_headers.len(), TRACE_CAP);
        assert_eq!(state.omitted_headers, 100 - TRACE_CAP);
        assert!(state.correlated_edition.is_none());
        assert!(!state.report().to_string().contains("PRIVATE_SERIAL_MARKER"));
    }

    struct Fake {
        sim: secbird_core::sim::Sim,
        last: Mutex<Option<Report>>,
        unsupported: bool,
        declared_size: u8,
    }
    impl Transport for Fake {
        fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
            if data[8] == 0x86 {
                let mut report = Report::request(
                    data[2],
                    EDITION,
                    if self.unsupported {
                        0
                    } else {
                        self.declared_size
                    },
                    if self.unsupported {
                        &[]
                    } else {
                        &[255, 128, 9]
                    },
                );
                report.0[0] = if self.unsupported { 5 } else { 2 };
                *self.last.lock().unwrap() = Some(report);
                Ok(())
            } else {
                self.sim.send_feature_report(data)
            }
        }
        fn get_feature_report(&self, out: &mut [u8]) -> secbird_core::Result<usize> {
            if let Some(report) = self.last.lock().unwrap().as_ref() {
                out[0] = 0;
                out[1..91].copy_from_slice(&report.0);
                Ok(91)
            } else {
                self.sim.get_feature_report(out)
            }
        }
        fn read_delay(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
    }
    #[test]
    fn full_probe_guards_baseline_then_one_get_and_omits_private_identity() {
        for (unsupported, declared_size) in [(false, 2), (false, 3), (true, 0)] {
            let fake = Fake {
                sim: secbird_core::sim::Sim::keyboard(),
                last: Mutex::new(None),
                unsupported,
                declared_size,
            };
            let (report, success) = probe(
                Box::new(fake),
                CString::new("PRIVATE_DEVICE_PATH").unwrap(),
                &TXID_CANDIDATES,
                None,
            );
            assert_eq!(success, !unsupported);
            assert_eq!(report["trace"]["serial_get_forward_attempts"], 3);
            assert_eq!(report["trace"]["edition_get_forward_attempts"], 1);
            assert_eq!(report["trace"]["forwarded_set_commands"], 0);
            assert!(report["query_only_device_handle"].as_bool().unwrap());
            if success {
                assert_eq!(report["identity"]["layout_id"], 255);
                assert_eq!(
                    report["identity"]["firmware_id"],
                    if declared_size >= 3 {
                        json!(9)
                    } else {
                        Value::Null
                    }
                );
                assert_eq!(
                    report["trace"]["correlated_edition_response"]["declared_args"]
                        .as_array()
                        .unwrap()
                        .len(),
                    usize::from(declared_size)
                );
                assert_eq!(report["trace"]["request_headers"][3]["declared_size"], 3);
                assert_eq!(
                    report["trace"]["request_headers"][3]["required_success_minimum"],
                    2
                );
            }
            let encoded = report.to_string();
            assert!(!encoded.contains("SIMKEYBOARD1"));
            assert!(!encoded.contains("PRIVATE_DEVICE_PATH"));
            assert!(!encoded.contains("serialNumber"));
        }
    }
}
