//! Separate explicit physical diagnostic. Ordinary tests run only pure guards.
//! No worker, Engine, profile apply, Store, stock-process control or runtime flag.
use crate::{
    host_uniform::io::submit_prepared,
    host_wave::{preparation::prepare, Program},
};
use secbird_core::{
    device::{candidates, TXID_CANDIDATES},
    hidapi::HidApi,
    ornata_frame::PreparedOrnataFrame,
    proto::{checksum, Command, Report},
    Db, Device, Error, Transport,
};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const TOKEN: &str = "left-two-frames-v1";
const MAX_FRAME_OUT: usize = 14;
const MAX_GET_OUT: usize = 48;
const MAX_HEADERS: usize = 64;
const SERIAL: Command = Command::new(0, 0x82);
const EDITION: Command = Command::new(0, 0x86);

struct Options {
    token: String,
    manifest: PathBuf,
    report: PathBuf,
}
impl Options {
    fn validate(&self, simulated: bool) -> Result<(), String> {
        if self.token != TOKEN || simulated {
            return Err(
                "Physical Wave requires its exact opt-in token and refuses SECBIRD_SIM".into(),
            );
        }
        if !self.manifest.is_absolute() || !self.report.is_absolute() {
            return Err("Explicit absolute manifest and report paths are required".into());
        }
        if self.report.exists() || self.report.with_extension("baseline.json").exists() {
            return Err("Report and baseline paths must both be new".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum SendKind {
    Get,
    Frame,
}
struct Guard {
    txids: Vec<u8>,
    selected: Option<u8>,
    serial_out: usize,
    get_out: usize,
    frame_out: usize,
    accepted_get: usize,
    accepted_frame: usize,
    denied: usize,
    write_failed: bool,
    partial_aborted: bool,
    armed: Option<[[u8; 91]; 7]>,
    next_packet: usize,
    frames_armed: usize,
    active_get: bool,
    reads_for_get: usize,
    total_reads: usize,
    headers: Vec<Value>,
    omitted_headers: usize,
}
impl Guard {
    fn new(txids: Vec<u8>) -> Self {
        Self {
            txids,
            selected: None,
            serial_out: 0,
            get_out: 0,
            frame_out: 0,
            accepted_get: 0,
            accepted_frame: 0,
            denied: 0,
            write_failed: false,
            partial_aborted: false,
            armed: None,
            next_packet: 0,
            frames_armed: 0,
            active_get: false,
            reads_for_get: 0,
            total_reads: 0,
            headers: Vec::new(),
            omitted_headers: 0,
        }
    }
    fn refuse<T>(&mut self, message: &str) -> secbird_core::Result<T> {
        self.denied += 1;
        Err(Error::Unsupported(message.into()))
    }
    fn finish_baseline(&mut self, txid: u8) {
        self.selected = Some(txid);
    }
    fn arm(&mut self, index: usize, frame: &PreparedOrnataFrame) -> secbird_core::Result<()> {
        if self.selected.is_none()
            || self.write_failed
            || self.partial_aborted
            || self.armed.is_some()
            || index != self.frames_armed
            || index >= 2
            || self.frame_out + 7 > MAX_FRAME_OUT
        {
            return self.refuse("Wave probe cannot arm another frame or reuse a partial batch");
        }
        self.armed = Some(frame.packets(self.selected.unwrap()));
        self.next_packet = 0;
        self.frames_armed += 1;
        Ok(())
    }
    fn permit(&mut self, packet: &[u8]) -> secbird_core::Result<SendKind> {
        if packet.len() != 91 || packet[0] != 0 {
            return self.refuse("Probe denied malformed HID wrapper");
        }
        let body: [u8; 90] = packet[1..].try_into().unwrap();
        let report = Report(body);
        let command = report.command();
        if command.id & 0x80 == 0 {
            if self.write_failed || self.partial_aborted || self.frame_out >= MAX_FRAME_OUT {
                return self.refuse("Probe output is stopped or exhausted");
            }
            if self
                .armed
                .as_ref()
                .is_none_or(|packets| packets[self.next_packet].as_slice() != packet)
            {
                return self
                    .refuse("Probe denied SET outside the exact armed physical frame sequence");
            }
            self.frame_out += 1;
            self.next_packet += 1;
            self.active_get = false;
            if self.next_packet == 7 {
                self.armed = None;
                self.next_packet = 0;
            }
            return Ok(SendKind::Frame);
        }
        if self.get_out >= MAX_GET_OUT {
            return self.refuse("Probe GET command budget exhausted");
        }
        let expected = match command {
            SERIAL
                if self.selected.is_none()
                    && self.serial_out < self.txids.len()
                    && self.txids.contains(&report.txid()) =>
            {
                Some(Report::request(report.txid(), command, 22, &[]))
            }
            EDITION if self.selected == Some(report.txid()) => {
                Some(Report::request(report.txid(), command, 3, &[]))
            }
            Command { class: 0, id: 0x84 } if self.selected == Some(report.txid()) => {
                Some(Report::request(report.txid(), command, 2, &[]))
            }
            Command {
                class: 15,
                id: 0x82,
            } if self.selected == Some(report.txid()) && body[8] <= 1 && body[9] == 0 => {
                Some(Report::request(report.txid(), command, 80, &[body[8], 0]))
            }
            Command {
                class: 15,
                id: 0x84,
            } if self.selected == Some(report.txid()) && body[8] <= 1 && body[9] == 0 => {
                Some(Report::request(report.txid(), command, 3, &[body[8], 0]))
            }
            _ => None,
        };
        if expected.as_ref().is_none_or(|expected| expected.0 != body) {
            return self.refuse("Probe denied a noncanonical or unapproved GET");
        }
        // A valid diagnostic GET abandons an incomplete row batch. Validate
        // canonicality first so malformed GETs cannot change this state. No
        // remaining rows, commit, rearm or retry are permitted after this point.
        if self.armed.is_some() && self.next_packet != 0 {
            self.partial_aborted = true;
        }
        self.get_out += 1;
        if command == SERIAL {
            self.serial_out += 1;
        }
        self.active_get = true;
        self.reads_for_get = 0;
        Ok(SendKind::Get)
    }
    fn sent(&mut self, kind: SendKind, success: bool) {
        match kind {
            SendKind::Get => self.accepted_get += usize::from(success),
            SendKind::Frame => {
                self.accepted_frame += usize::from(success);
                self.write_failed |= !success;
            }
        }
    }
    fn before_read(&mut self) -> secbird_core::Result<()> {
        if !self.active_get || self.reads_for_get >= 30 {
            return self.refuse("Probe refused transfer-in outside a bounded GET");
        }
        self.reads_for_get += 1;
        self.total_reads += 1;
        Ok(())
    }
    fn observe(&mut self, data: &[u8], length: Option<usize>) {
        let header = if let Some(n) = length.filter(|&n| (n == 90 || n == 91) && n <= data.len()) {
            let body: [u8; 90] = data[n - 90..n].try_into().unwrap();
            json!({"bytes_read":n,"status":body[0],"transaction":body[1],"declared_size":body[5],"class":body[6],"id":body[7],"checksum_stored":body[88],"checksum_computed":checksum(&body)})
        } else {
            json!({"bytes_read":length,"read_error":length.is_none()})
        };
        if self.headers.len() < MAX_HEADERS {
            self.headers.push(header);
        } else {
            self.omitted_headers += 1;
        }
    }
    fn report(&self) -> Value {
        json!({"frame_out_attempts":self.frame_out,"frame_out_accepted_returns":self.accepted_frame,
            "frame_out_limit":MAX_FRAME_OUT,"get_out_attempts":self.get_out,"get_out_limit":MAX_GET_OUT,
            "get_out_accepted_returns":self.accepted_get,"serial_get_attempts":self.serial_out,"read_calls":self.total_reads,
            "denied_requests":self.denied,"failed_frame_out_latched":self.write_failed,"partial_batch_aborted_for_cleanup":self.partial_aborted,"frames_armed":self.frames_armed,
            "headers":self.headers,"omitted_headers":self.omitted_headers,"header_limit":MAX_HEADERS,
            "serial_arguments_or_paths_retained":false,"allowlist_installed_before_device_open":true,
            "forwarded_frame_packets_matched_exact_armed_bytes":true})
    }
}
struct Guarded {
    inner: Box<dyn Transport>,
    guard: Arc<Mutex<Guard>>,
}
impl Transport for Guarded {
    fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
        let kind = self.guard.lock().unwrap().permit(data)?;
        let result = self.inner.send_feature_report(data);
        self.guard.lock().unwrap().sent(kind, result.is_ok());
        result
    }
    fn get_feature_report(&self, data: &mut [u8]) -> secbird_core::Result<usize> {
        self.guard.lock().unwrap().before_read()?;
        let result = self.inner.get_feature_report(data);
        self.guard
            .lock()
            .unwrap()
            .observe(data, result.as_ref().ok().copied());
        result
    }
    fn read_delay(&self) -> Duration {
        self.inner.read_delay()
    }
}
fn safe_error(error: impl std::fmt::Display) -> String {
    // Device messages can contain private HID backend paths. Only this fixed
    // category enters the public diagnostic; frame counts remain separate.
    eprintln!("Wave probe private diagnostic: {error}");
    "Wave probe stage failed; private backend detail omitted".into()
}
fn owner_clear() -> Result<(), String> {
    let mut runtime = crate::lighting_runtime::Runtime::default();
    runtime.sample_owner();
    match runtime.environment.owner {
        crate::lighting_runtime::Owner::Clear => Ok(()),
        crate::lighting_runtime::Owner::Competing(_) => {
            Err("A known competing lighting owner is running".into())
        }
        crate::lighting_runtime::Owner::Unknown(_) => {
            Err("Known-owner process sampling failed".into())
        }
    }
}
fn registers(device: &Device) -> Result<Value, String> {
    let mode = device.ornata_mode_readback().map_err(safe_error)?;
    let mut effects = Vec::new();
    let mut brightness = Vec::new();
    for profile in [0, 1] {
        let effect = device
            .ornata_effect_readback(profile, 0)
            .map_err(safe_error)?;
        let level = device
            .ornata_brightness_readback(profile, 0)
            .map_err(safe_error)?;
        if !effect.selectors_match() || !level.selectors_match() {
            return Err("Register selectors differ from the requested profiles/region".into());
        }
        effects.push(json!({"profile":profile,"declared_size":effect.declared_size(),"declared_args":&effect.raw_args()[..usize::from(effect.declared_size())]}));
        brightness.push(json!({"profile":profile,"raw":level.raw_brightness()}));
    }
    Ok(
        json!({"mode":mode.mode(),"mode_parameter":mode.param(),"mode_parameter_known":mode.param().is_some(),"mode_declared_size":mode.declared_size(),"effects":effects,"brightness":brightness}),
    )
}
fn admit_registers(snapshot: &Value) -> Result<(), String> {
    let custom = snapshot["effects"][0]["declared_args"]
        .as_array()
        .is_some_and(|args| {
            args.starts_with(&[json!(0), json!(0), json!(8), json!(0), json!(0), json!(0)])
        });
    if snapshot["mode"] != 3
        || !custom
        || snapshot["brightness"][0]["raw"]
            .as_u64()
            .is_none_or(|raw| raw == 0)
    {
        return Err("Probe requires existing DriverMode3, live CustomFrame8/flags0/rate0/count0, and positive live brightness; no acquisition is permitted".into());
    }
    Ok(())
}
fn save_new(path: &Path, value: &Value) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(value).map_err(safe_error)?;
    if data.len() > 256 * 1024 {
        return Err("Probe report exceeded its bounded output size".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(safe_error)?;
    file.write_all(&data).map_err(safe_error)?;
    file.sync_all().map_err(safe_error)
}
fn exercise(device: &mut Device, guard: &Arc<Mutex<Guard>>, options: &Options) -> Value {
    let mut report = json!({"scope":"two explicit source Left Wave frames at indices0 and1; no worker, Store or product startup",
        "optically_verified":false,"cadence_certified":false,"input_recovery_verified":false,
        "rgb_or_animation_phase_restored":false,"stock_restart":"external reviewed wrapper required",
        "no_brightness_mode_input_macro_or_saved_profile_setters":true,
        "known_owner_gate":"fresh native process sample before open, baseline and each frame; not an exclusive lease",
        "wdl_or_third_party_exclusivity_verified":false});
    let result = (|| -> Result<(), String> {
        owner_clear()?;
        let before = registers(device)?;
        admit_registers(&before)?;
        report["before"] = before.clone();
        let ready = prepare(device, Program::Left, &options.manifest).map_err(safe_error)?;
        report["preparation"] = ready.metadata_at(Instant::now());
        save_new(
            &options.report.with_extension("baseline.json"),
            &json!({"before":before,"preparation":report["preparation"],"scope":"readable declared controls only; not a resumable RGB/animation/input snapshot"}),
        )?;
        let window = Instant::now();
        let deadline = window + Duration::from_secs(3);
        let mut phases = Vec::new();
        let mut previous_start = None;
        for index in [0, 1] {
            if let Some(previous) = previous_start {
                let earliest: Instant = previous + Duration::from_millis(40);
                std::thread::sleep(earliest.saturating_duration_since(Instant::now()));
            }
            if Instant::now() >= deadline {
                return Err("Frame admission window expired; no next frame was attempted".into());
            }
            owner_clear()?;
            let admission = registers(device)?;
            admit_registers(&admission)?;
            if admission != before {
                return Err(
                    "Readable registers drifted before output; no next frame was attempted".into(),
                );
            }
            let frame = ready.prepared().render_frame(index).map_err(safe_error)?;
            guard
                .lock()
                .unwrap()
                .arm(index, &frame)
                .map_err(safe_error)?;
            let started = Instant::now();
            let submission = submit_prepared(
                device,
                &frame,
                (started + Duration::from_millis(500)).min(deadline),
                || Instant::now() >= deadline,
            );
            let entry = json!({"index":index,"start_offset_us":started.duration_since(window).as_micros(),"since_previous_start_us":previous_start.map(|previous|started.duration_since(previous).as_micros()),
                "accepted_packets":submission.completion.accepted_packets,"attempted_packets":submission.completion.attempted_packets,
                "submission_failed":submission.completion.error.is_some(),"control_state_verified":submission.control_verified,
                "mode_before":submission.observed_mode,"parameter_before":submission.observed_parameter,"mode_after":submission.mode_after,"parameter_after":submission.parameter_after,
                "elapsed_us":submission.elapsed.as_micros(),"fresh_mode_get_us":submission.mode_query_elapsed.as_micros(),"usb_out_us":submission.usb_out_elapsed.as_micros()});
            phases.push(entry);
            report["frames"] = json!(phases);
            previous_start = Some(started);
            if submission.completion.error.is_some()
                || submission.completion.accepted_packets != 7
                || submission.completion.attempted_packets != 7
                || !submission.control_verified
            {
                return Err("Frame did not complete the guarded USB/control boundary; partial output is possible and no retry follows".into());
            }
            let after = registers(device)?;
            report["after_frame"] = after.clone();
            phases.last_mut().unwrap()["after_registers"] = after.clone();
            report["frames"] = json!(phases);
            if after != before {
                return Err(
                    "Readable controls changed after output; no next frame was attempted".into(),
                );
            }
        }
        report["success"] = json!(true);
        Ok(())
    })();
    if let Err(error) = result {
        report["success"] = json!(false);
        report["error"] = json!(error);
    }
    let final_read = registers(device);
    match final_read {
        Ok(after) => {
            report["after_final"] = after.clone();
            report["readable_controls_unchanged"] = json!(report.get("before") == Some(&after));
        }
        Err(error) => {
            report["after_final_error"] = json!(error);
            report["readable_controls_unchanged"] = Value::Null;
        }
    }
    report["trace"] = guard.lock().unwrap().report();
    if report["readable_controls_unchanged"] != true {
        report["success"] = json!(false);
    }
    report
}

#[test]
#[ignore = "requires exact physical opt-in, explicit new report/manifest paths and reviewed external stock isolation/restart; NOT ordinary validation"]
fn two_left_wave_indices_preserve_readable_controls() {
    let options = Options {
        token: std::env::var("SYNAPSE_LITE_PHYSICAL_WAVE_PROBE")
            .expect("explicit physical Wave token"),
        manifest: PathBuf::from(
            std::env::var_os("SYNAPSE_LITE_PHYSICAL_WAVE_MANIFEST")
                .expect("explicit manifest path"),
        ),
        report: PathBuf::from(
            std::env::var_os("SYNAPSE_LITE_PHYSICAL_WAVE_REPORT").expect("explicit report path"),
        ),
    };
    options
        .validate(std::env::var_os("SECBIRD_SIM").is_some())
        .unwrap();
    owner_clear().unwrap();
    let api = HidApi::new().expect("real HID enumeration");
    let targets: Vec<_> = candidates(&api)
        .into_iter()
        .filter(|c| c.pid == 605 && c.interface == 2)
        .collect();
    assert_eq!(
        targets.len(),
        1,
        "Exactly one physical Ornata MI02 is required"
    );
    let target = &targets[0];
    assert!(
        target
            .path
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("\\\\?\\hid#"),
        "Physical Windows HID path required"
    );
    let db = Db::builtin().unwrap();
    let spec = db.get(605).cloned();
    assert!(spec
        .as_ref()
        .and_then(|s| s.matrix)
        .is_some_and(|m| m.rows == 6 && m.cols == 22));
    let mut txids = Vec::new();
    if let Some(txid) = spec.as_ref().and_then(|s| s.txid) {
        txids.push(txid);
    }
    for txid in TXID_CANDIDATES {
        if !txids.contains(&txid) {
            txids.push(txid);
        }
    }
    let guard = Arc::new(Mutex::new(Guard::new(txids.clone())));
    let hid = api.open_path(&target.path).expect("physical handle open");
    let transport = Guarded {
        inner: Box::new(hid),
        guard: guard.clone(),
    };
    let device = Device::open(
        Box::new(transport),
        605,
        2,
        target.path.clone(),
        &txids,
        spec,
    );
    let result = if let Some(mut device) = device {
        guard.lock().unwrap().finish_baseline(device.txid);
        exercise(&mut device, &guard, &options)
    } else {
        json!({"success":false,"error":"Serial baseline failed before frame admission","trace":guard.lock().unwrap().report()})
    };
    if let Err(error) = save_new(&options.report, &result) {
        let counts = guard.lock().unwrap().report();
        eprintln!("Final diagnostic report write failed after accepted {} / attempted {} frame OUTs. This is a report persistence failure, not a frame rejection or restoration proof. External stock recovery is still required.",counts["frame_out_accepted_returns"],counts["frame_out_attempts"]);
        panic!("Final report persistence failed: {error}");
    }
    assert_eq!(result["success"],true,"Reviewed physical probe failed; consult its private report and run external stock recovery");
}

#[test]
fn pure_opt_in_and_register_admission_fail_closed() {
    let options = Options {
        token: TOKEN.into(),
        manifest: PathBuf::from("C:/explicit/manifest.json"),
        report: PathBuf::from("C:/explicit/fresh-probe-result.json"),
    };
    assert!(options.validate(true).is_err());
    assert!(Options {
        token: "wrong".into(),
        ..options
    }
    .validate(false)
    .is_err());
    let good =
        json!({"mode":3,"effects":[{"declared_args":[0,0,8,0,0,0]}],"brightness":[{"raw":128}]});
    assert!(admit_registers(&good).is_ok());
    for (field, value) in [
        ("mode", json!(0)),
        ("effects", json!([{"declared_args":[0,0,3,0,0,0]}])),
        ("brightness", json!([{"raw":0}])),
    ] {
        let mut bad = good.clone();
        bad[field] = value;
        assert!(admit_registers(&bad).is_err());
    }
    let mut guard = Guard::new(vec![0x1f]);
    guard.finish_baseline(0x1f);
    let mut query = vec![0];
    query.extend(Report::request(0x1f, Command::new(0, 0x84), 2, &[]).0);
    for _ in 0..MAX_GET_OUT {
        assert!(guard.permit(&query).is_ok());
    }
    assert!(guard.permit(&query).is_err());
    assert_eq!(guard.get_out, MAX_GET_OUT);
}
#[test]
fn pure_allowlist_has_exact_two_frame_script_and_no_other_setters() {
    let mut guard = Guard::new(vec![0x1f]);
    let packet = |command, size, args: &[u8]| {
        let mut p = vec![0];
        p.extend(Report::request(0x1f, command, size, args).0);
        p
    };
    assert!(guard.permit(&packet(EDITION, 3, &[])).is_err());
    assert!(guard.permit(&packet(SERIAL, 22, &[])).is_ok());
    guard.finish_baseline(0x1f);
    for command in [
        Command::new(0, 4),
        Command::new(15, 4),
        Command::new(2, 12),
        Command::new(6, 8),
        Command::new(15, 2),
    ] {
        assert!(guard
            .permit(&packet(command, 6, &[1, 0, 8, 0, 0, 0]))
            .is_err());
    }
    for index in [0, 1] {
        let pixels: Vec<_> = (0..132)
            .map(|pixel| secbird_core::Rgb(index as u8, pixel as u8, 7))
            .collect();
        let frame = PreparedOrnataFrame::new(&pixels).unwrap();
        guard.arm(index, &frame).unwrap();
        let packets = frame.packets(0x1f);
        assert!(guard.permit(&packets[1]).is_err(), "row order is fixed");
        let mut altered = packets[0];
        altered[14] ^= 1;
        let body: [u8; 90] = altered[1..].try_into().unwrap();
        altered[89] = checksum(&body);
        assert!(
            guard.permit(&altered).is_err(),
            "even checksummed RGB must equal the armed frame"
        );
        for packet in packets {
            let kind = guard.permit(&packet).unwrap();
            guard.sent(kind, true);
        }
    }
    assert_eq!((guard.frame_out, guard.accepted_frame), (14, 14));
    assert!(guard
        .arm(
            2,
            &PreparedOrnataFrame::new(&[secbird_core::Rgb(0, 0, 0); 132]).unwrap()
        )
        .is_err());
}
#[test]
fn pure_partial_failure_latches_output_and_read_traces_omit_arguments() {
    let mut guard = Guard::new(vec![0x1f]);
    guard.finish_baseline(0x1f);
    let frame = PreparedOrnataFrame::new(&[secbird_core::Rgb(1, 2, 3); 132]).unwrap();
    guard.arm(0, &frame).unwrap();
    let packets = frame.packets(0x1f);
    let kind = guard.permit(&packets[0]).unwrap();
    guard.sent(kind, false);
    assert!(guard.permit(&packets[1]).is_err());
    assert_eq!((guard.frame_out, guard.accepted_frame), (1, 0));
    guard.active_get = true;
    let mut reply = Report::request(0x1f, SERIAL, 22, b"PRIVATE_SERIAL_MARKER");
    reply.0[0] = 2;
    let mut data = vec![0];
    data.extend(reply.0);
    for _ in 0..30 {
        guard.before_read().unwrap();
        guard.observe(&data, Some(91));
    }
    assert!(guard.before_read().is_err());
    // Test observer capacity independently of the transport's30-read limit.
    for _ in 0..70 {
        guard.observe(&data, Some(91));
    }
    assert_eq!(guard.headers.len(), MAX_HEADERS);
    assert_eq!(guard.omitted_headers, 100 - MAX_HEADERS);
    assert!(!guard.report().to_string().contains("PRIVATE_SERIAL_MARKER"));
    let mut cancelled = Guard::new(vec![0x1f]);
    cancelled.finish_baseline(0x1f);
    cancelled.arm(0, &frame).unwrap();
    let kind = cancelled.permit(&packets[0]).unwrap();
    cancelled.sent(kind, true);
    let mut invalid = vec![0];
    invalid.extend(Report::request(0x1f, Command::new(0, 0x84), 3, &[]).0);
    assert!(cancelled.permit(&invalid).is_err());
    assert!(
        !cancelled.partial_aborted,
        "invalid GET must not change abort state"
    );
    let mut query = vec![0];
    query.extend(Report::request(0x1f, Command::new(0, 0x84), 2, &[]).0);
    assert!(cancelled.permit(&query).is_ok());
    assert!(cancelled.partial_aborted);
    assert!(
        !cancelled.write_failed,
        "cooperative stop is not a failed USB OUT"
    );
    assert!(cancelled.permit(&packets[1]).is_err());
    assert!(cancelled.arm(1, &frame).is_err());
    assert_eq!((cancelled.frame_out, cancelled.accepted_frame), (1, 1));
}
