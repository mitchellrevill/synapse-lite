//! Discovery and request/reply transport over HID feature reports.

use crate::commands as cmd;
use crate::db::{Db, DeviceSpec, Kind, RAZER_VID};
use crate::error::{Error, Result};
use crate::proto::{Command, Report, Status, REPORT_LEN};
use crate::transport::Transport;
use hidapi::HidApi;
use std::ffi::CString;
use std::thread::sleep;

/// Synapse retries reads up to 30 times (5 ms apart, see [`Transport::read_delay`]).
const READ_RETRIES: u32 = 30;

/// Writes to the device's flash (macros) take longer: Synapse waits at least 10 ms between
/// reads and retries 40 times. Allow up to about two seconds.
const FLASH_RETRIES: u32 = 200;

/// Transaction ids seen across Razer devices, tried in order when the database has none.
pub const TXID_CANDIDATES: [u8; 4] = [0x1F, 0x3F, 0xFF, 0x9F];

pub struct Device {
    transport: Box<dyn Transport>,
    query_only: bool,
    pub pid: u16,
    /// Serial number read during discovery; empty if the device reports none.
    pub serial: String,
    pub interface: i32,
    pub path: CString,
    pub txid: u8,
    pub spec: Option<DeviceSpec>,
}

impl Device {
    /// Irreversibly restrict this handle to protocol GETs. Used by explicit
    /// read-only measurements; it does not alter the physical device.
    pub fn enforce_query_only(&mut self) {
        self.query_only = true;
    }
    pub fn is_query_only(&self) -> bool {
        self.query_only
    }

    /// Selected wheel SDK getters declare capacity80 for Smart Reel but decode
    /// two meaningful bytes. Keep this exception local to the explicit adapter.
    pub(crate) fn get_wheel_register(&self, control: crate::wheel_controls::WheelControl) -> Result<Report> {
        if !crate::wheel_controls::supports(self) {
            return Err(Error::Unsupported("Wheel transport is not source-supported".into()));
        }
        let command = control.command().get();
        self.permit_command(command)?;
        transact_with_response_minimum(self.transport.as_ref(), self.txid, command,
            control.size(), &[1], READ_RETRIES, 2)
    }
    fn permit_command(&self, command: Command) -> Result<()> {
        if self.query_only && command.id & 0x80 == 0 {
            return Err(Error::Unsupported(format!(
                "Read-only device handle refuses protocol SET {command}"
            )));
        }
        Ok(())
    }
    pub fn name(&self) -> String {
        match &self.spec {
            Some(s) => s.name.clone(),
            None => format!("Unknown Razer device {:04X}", self.pid),
        }
    }

    /// A Mouse Dock Pro is the docked mouse's receiver: every command reaches the mouse.
    /// Ask which mouse is paired and take its name, DPI range and lighting zones. Polling
    /// stays on the dock's `00:C0` command. If the answer is missing or isn't a mouse that
    /// pairs with the dock, keep the generic database entry.
    pub(crate) fn identify_docked_mouse(&mut self, db: &Db) {
        if self.pid != MOUSE_DOCK_PRO {
            return;
        }
        let Ok(reply) = self.get(cmd::WIRELESS_DEVICES, cmd::WIRELESS_DEVICES_SIZE, &[]) else {
            return;
        };
        let Some(pid) = connected_mouse_pid(reply.args()).filter(|p| DOCK_PRO_MICE.contains(p))
        else {
            return;
        };
        let Some(mouse) = db.get(pid).filter(|s| s.kind == Kind::Mouse) else {
            return;
        };
        if let Some(spec) = &mut self.spec {
            spec.name = format!(
                "{} on Mouse Dock Pro",
                mouse.name.trim_end_matches(" (Wireless)")
            );
            spec.dpi = mouse.dpi.clone();
            spec.lighting = mouse.lighting;
            spec.leds = mouse.leds.clone();
            spec.onboard_profiles = mouse.onboard_profiles;
        }
    }

    /// A HyperPolling dongle is its paired mouse's receiver: ask which mouse it reports and
    /// label the device with that mouse, so the editor shows the mouse rather than the
    /// dongle. The dongle keeps its own transport, capabilities and profile key — the
    /// database entry for `00B3` describes the pair, and polling stays on the dongle's
    /// `00:C0` command. If the answer is missing or isn't a mouse, keep the generic entry.
    pub(crate) fn identify_hyperpolling_mouse(&mut self, db: &Db) {
        if self.pid != HYPERPOLLING_DONGLE {
            return;
        }
        let Ok(reply) = self.get(cmd::WIRELESS_DEVICES, cmd::WIRELESS_DEVICES_SIZE, &[]) else {
            return;
        };
        let Some(pid) = connected_mouse_pid(reply.args()) else {
            return;
        };
        let Some(mouse) = db.get(pid).filter(|s| s.kind == Kind::Mouse) else {
            return;
        };
        if let Some(spec) = &mut self.spec {
            spec.name = format!(
                "{} (via HyperPolling)",
                mouse.name.trim_end_matches(" (Wireless)")
            );
        }
    }

    /// Receivers answer for the device they pair with, so label the one they report.
    pub(crate) fn identify_receiver(&mut self, db: &Db) {
        self.identify_docked_mouse(db);
        self.identify_hyperpolling_mouse(db);
        self.add_dock_light();
    }

    /// A mouse dock's own light is one more zone beside the docked mouse's (#39).
    fn add_dock_light(&mut self) {
        if !self.is_mouse_dock() {
            return;
        }
        if let Some(spec) = self
            .spec
            .as_mut()
            .filter(|s| !s.leds.iter().any(|l| l.id == DOCK_LED))
        {
            spec.leds.push(crate::db::Led {
                name: "dock".into(),
                id: DOCK_LED,
            });
        }
    }

    /// Whether `led` is a dock's own light, reached with [`DOCK_TXID`] and the newer
    /// lighting commands whatever the docked mouse uses.
    pub(crate) fn is_dock_led(&self, led: u8) -> bool {
        self.is_mouse_dock() && led == DOCK_LED
    }

    /// [`Device::transact`] for a lighting command to `led`, sent to the dock for its light.
    pub(crate) fn transact_led(
        &self,
        led: u8,
        command: Command,
        size: u8,
        args: &[u8],
    ) -> Result<Report> {
        if self.is_dock_led(led) {
            self.transact_as(DOCK_TXID, command, size, args)
        } else {
            self.transact(command, size, args)
        }
    }

    /// Ornata's inspected frame path disables transfer-in checks. This private
    /// seam sends one canonical row/commit HID wrapper, with no read or retry.
    /// USB acceptance is not firmware acknowledgement or optical verification.
    pub(crate) fn send_ornata_frame_packet(&mut self, packet: &[u8; 91]) -> Result<()> {
        if self.query_only {
            return Err(Error::Unsupported("Read-only device handle refuses raw lighting frame OUT".into()));
        }
        crate::ornata_frame::validate_device(self)?;
        let report = Report(packet[1..].try_into().expect("fixed HID wrapper"));
        let size = report.0[5];
        let args = report.args();
        let row = report.command() == cmd::MATRIX_FRAME
            && size == 71
            && args[0] == 0
            && args[1] == 0
            && args[2] < 6
            && args[3] == 0
            && args[4] == 21;
        let commit =
            report.command() == cmd::MATRIX_EFFECT && size == 6 && args[..6] == [0, 0, 8, 0, 0, 0];
        if packet[0] != 0
            || (!row && !commit)
            || report.0
                != Report::request(
                    self.txid,
                    report.command(),
                    size,
                    &args[..usize::from(size)],
                )
                .0
        {
            return Err(Error::InvalidArg(
                "Noncanonical Ornata frame row/commit packet".into(),
            ));
        }
        self.transport.send_feature_report(packet)
    }

    /// Send one command and wait for its reply. `size` is the argument length the
    /// device expects for this command.
    pub fn transact(&self, command: Command, size: u8, args: &[u8]) -> Result<Report> {
        self.permit_command(command)?;
        transact(
            self.transport.as_ref(),
            self.txid,
            command,
            size,
            args,
            READ_RETRIES,
        )
    }

    /// [`Device::transact`] for commands that write flash and answer slowly.
    pub(crate) fn transact_slow(&self, command: Command, size: u8, args: &[u8]) -> Result<Report> {
        self.permit_command(command)?;
        transact(
            self.transport.as_ref(),
            self.txid,
            command,
            size,
            args,
            FLASH_RETRIES,
        )
    }

    /// [`Device::transact`] with another transaction id (see [`DOCK_TXID`]).
    pub fn transact_as(&self, txid: u8, command: Command, size: u8, args: &[u8]) -> Result<Report> {
        self.permit_command(command)?;
        transact(
            self.transport.as_ref(),
            txid,
            command,
            size,
            args,
            READ_RETRIES,
        )
    }

    /// Whether this is a mouse dock, whose own settings take [`DOCK_TXID`].
    pub fn is_mouse_dock(&self) -> bool {
        matches!(self.pid, MOUSE_DOCK_PRO | MOUSE_DOCK_V2_PRO)
    }

    pub fn get(&self, command: Command, size: u8, args: &[u8]) -> Result<Report> {
        self.transact(command.get(), size, args)
    }

    /// The installed Ornata returns two declared edition/layout bytes to the
    /// shipped three-byte GET00:86 request. Firmware ID is then unavailable.
    /// Keep the measured minimum exception local; ordinary GET stays strict.
    pub(crate) fn get_ornata_edition_register(&self) -> Result<Report> {
        if self.pid != 0x025d || self.interface != 2 {
            return Err(Error::Unsupported(
                "edition/layout readback is verified only for Ornata V2 interface2".into(),
            ));
        }
        self.permit_command(Command::new(0, 0x86))?;
        transact_with_response_minimum(
            self.transport.as_ref(),
            self.txid,
            Command::new(0, 0x86),
            3,
            &[],
            READ_RETRIES,
            2,
        )
    }

    /// The installed Ornata returns one mode byte to the source's two-byte
    /// GET00:84 request. Keep this measured exception local to that interface.
    pub(crate) fn get_ornata_mode_register(&self) -> Result<Report> {
        if self.pid != 0x025d || self.interface != 2 {
            return Err(Error::Unsupported(
                "mode readback is verified only for Ornata V2 interface2".into(),
            ));
        }
        self.permit_command(Command::new(0, 0x84))?;
        transact_with_response_minimum(
            self.transport.as_ref(),
            self.txid,
            Command::new(0, 0x84),
            2,
            &[],
            READ_RETRIES,
            1,
        )
    }

    /// The inspected Ornata GET0F82 sends80 bytes but can return only its
    /// six-byte effect header. This exception does not alter other commands.
    pub(crate) fn get_lighting_effect_register(&self, profile: u8, region: u8) -> Result<Report> {
        if self.pid != 0x025d || self.interface != 2 {
            return Err(Error::Unsupported(
                "effect readback is verified only for Ornata V2 interface2".into(),
            ));
        }
        if profile > 1 || region != 0 {
            return Err(Error::InvalidArg(
                "effect readback requires profile0/1 and region0".into(),
            ));
        }
        self.permit_command(Command::new(0x0f, 0x82))?;
        transact_with_response_minimum(
            self.transport.as_ref(),
            self.txid,
            Command::new(0x0f, 0x82),
            80,
            &[profile, region],
            READ_RETRIES,
            6,
        )
    }

    /// Stable identity of this physical device: `PID:SERIAL`, or just `PID` when the
    /// device has no usable serial. Profiles are stored under this key, so two
    /// identical mice keep separate settings.
    pub fn key(&self) -> String {
        if self.serial.is_empty() {
            format!("{:04X}", self.pid)
        } else {
            format!("{:04X}:{}", self.pid, self.serial)
        }
    }
}

/// Serial from a GET serial reply: printable ASCII only; placeholders count as none.
fn parse_serial(args: &[u8]) -> String {
    let raw = &args[..(cmd::SERIAL_SIZE as usize).min(args.len())];
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let s: String = raw[..end]
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string();
    let usable = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && s.chars().any(|c| c != '0' && c != 'F' && c != 'f');
    if usable {
        s
    } else {
        String::new()
    }
}

pub(crate) const MOUSE_DOCK_PRO: u16 = 0x00A4;
/// Mouse Dock V2 Pro. Not in the database yet; assumed to work like the Mouse Dock Pro.
pub(crate) const MOUSE_DOCK_V2_PRO: u16 = 0x00F1;

/// Transaction id that reaches a mouse dock itself rather than the docked mouse.
/// Synapse's dock class (`rzDevice25Linker`) sends `0xE0 | n`; not yet seen on hardware.
pub const DOCK_TXID: u8 = 0xFF;
/// The dock's own light (Synapse `DockedLED`, the dock's brightness region).
pub const DOCK_LED: u8 = 0x0F;

/// The HyperPolling wireless dongle. It answers for the mouse paired to it.
pub(crate) const HYPERPOLLING_DONGLE: u16 = 0x00B3;

/// Mice that pair with the Mouse Dock Pro (Synapse's `DualDongleCompatibleDevices.json`).
const DOCK_PRO_MICE: [u16; 11] = [
    0x00A8, 0x00AB, 0x00B0, 0x00CD, 0x00D7, 0x00DB, 0x00DF, 0x00E3, 0x00E8, 0x00EA, 0xB00B,
];

/// The one connected mouse in a paired-mice reply. `None` if there is none, or more than one.
fn connected_mouse_pid(args: &[u8]) -> Option<u16> {
    let count = *args.first()? as usize;
    let entries = args.get(1..1 + count * 3)?;
    let mut connected = entries.as_chunks::<3>().0.iter().filter_map(|e| {
        let pid = u16::from_be_bytes([e[1], e[2]]);
        (e[0] == 1 && pid != 0 && pid != 0xFFFF).then_some(pid)
    });
    let pid = connected.next()?;
    connected.next().is_none().then_some(pid)
}

/// Accept only a complete, correlated reply. Another lighting controller may
/// overwrite the shared response buffer between our write and read.
fn transact(
    hid: &dyn Transport,
    txid: u8,
    command: Command,
    size: u8,
    args: &[u8],
    retries: u32,
) -> Result<Report> {
    transact_with_response_minimum(hid, txid, command, size, args, retries, size)
}

/// Request capacity and minimum successful reply size are separate for the
/// observed variable-length lighting getter. Callers of transact retain the
/// original request-sized minimum; all framing/correlation checks remain shared.
fn transact_with_response_minimum(
    hid: &dyn Transport,
    txid: u8,
    command: Command,
    size: u8,
    args: &[u8],
    retries: u32,
    minimum_success_args: u8,
) -> Result<Report> {
    if minimum_success_args > size {
        return Err(Error::InvalidArg(
            "reply minimum exceeds request capacity".into(),
        ));
    }
    let req = Report::checked_request(txid, command, size, args)?;
    let mut buf = [0u8; REPORT_LEN + 1]; // byte 0 is the HID report id (0)
    buf[1..].copy_from_slice(&req.0);
    hid.send_feature_report(&buf)?;

    for _ in 0..retries {
        sleep(hid.read_delay());
        let mut reply = [0u8; REPORT_LEN + 1];
        let n = hid.get_feature_report(&mut reply)?;
        if n != REPORT_LEN && n != REPORT_LEN + 1 {
            continue;
        }
        let mut body = [0u8; REPORT_LEN];
        body.copy_from_slice(&reply[n - REPORT_LEN..n]);
        let rep = Report(body);
        // Match the complete frame before trusting status or feature values.
        // No transaction alias has been verified for the attached devices.
        let minimum_args = if rep.status() == Status::Success {
            minimum_success_args
        } else {
            0
        };
        if !rep.valid_reply(txid, command, minimum_args) {
            continue;
        }
        match rep.status() {
            Status::Success => return Ok(rep),
            Status::Busy | Status::New => continue,
            status => {
                return Err(Error::Device {
                    cmd: command,
                    status,
                })
            }
        }
    }
    Err(Error::NoReply(command))
}

/// A Razer HID interface seen during enumeration, before we know if it speaks Protocol 2.5.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub pid: u16,
    pub interface: i32,
    pub path: CString,
}

pub fn candidates(api: &HidApi) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = api
        .device_list()
        .filter(|d| d.vendor_id() == RAZER_VID)
        .map(|d| Candidate {
            pid: d.product_id(),
            interface: d.interface_number(),
            path: d.path().to_owned(),
        })
        .collect();
    out.sort_by(|a, b| (a.pid, a.interface, &a.path).cmp(&(b.pid, b.interface, &b.path)));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

/// Find every connected Razer device that answers Protocol 2.5, one handle per physical
/// device (several units of the same model are told apart by serial number).
///
/// Simulated devices named in `SECBIRD_SIM` (e.g. `mouse,keyboard`) are added at the end.
pub fn discover(api: &HidApi, db: &Db) -> Vec<Device> {
    let mut found = discover_hid(api, db);
    for dev in &mut found {
        dev.identify_receiver(db);
    }
    found.extend(crate::sim::from_env());
    found
}

fn discover_hid(api: &HidApi, db: &Db) -> Vec<Device> {
    let all = candidates(api);
    let mut pids: Vec<u16> = all.iter().map(|c| c.pid).collect();
    pids.dedup();

    pids.into_iter()
        .filter_map(|pid| -> Option<Vec<Device>> {
            let spec = db.get(pid).cloned();
            let mut ifaces: Vec<&Candidate> = all.iter().filter(|c| c.pid == pid).collect();
            // Known interface first; the rest still get a chance.
            if let Some(want) = spec.as_ref().and_then(|s| s.interface) {
                ifaces.sort_by_key(|c| c.interface != want);
            }
            let txids: Vec<u8> = match spec.as_ref().and_then(|s| s.txid) {
                Some(t) => std::iter::once(t).chain(TXID_CANDIDATES).collect(),
                None => TXID_CANDIDATES.to_vec(),
            };
            let mut iter = ifaces.into_iter();
            let first = iter
                .by_ref()
                .find_map(|c| probe(api, c, &txids, spec.clone()))?;
            // More units of the same model answer on the same interface number.
            let mut units = vec![first];
            let rest: Vec<&Candidate> =
                iter.filter(|c| c.interface == units[0].interface).collect();
            for c in rest {
                if let Some(dev) = probe(api, c, &[units[0].txid], spec.clone()) {
                    if !dev.serial.is_empty() && units.iter().all(|u| u.serial != dev.serial) {
                        units.push(dev);
                    }
                }
            }
            Some(units)
        })
        .flatten()
        .collect()
}

fn probe(api: &HidApi, c: &Candidate, txids: &[u8], spec: Option<DeviceSpec>) -> Option<Device> {
    let hid = api.open_path(&c.path).ok()?;
    Device::open(
        Box::new(hid),
        c.pid,
        c.interface,
        c.path.clone(),
        txids,
        spec,
    )
}

impl Device {
    /// Wrap a transport, keeping the first txid that answers a harmless GET (serial number).
    pub fn open(
        transport: Box<dyn Transport>,
        pid: u16,
        interface: i32,
        path: CString,
        txids: &[u8],
        spec: Option<DeviceSpec>,
    ) -> Option<Device> {
        let (txid, reply) = txids.iter().copied().find_map(|t| {
            transact(
                transport.as_ref(),
                t,
                cmd::SERIAL.get(),
                cmd::SERIAL_SIZE,
                &[],
                READ_RETRIES,
            )
            .ok()
            .map(|r| (t, r))
        })?;
        let serial = parse_serial(reply.args());
        Some(Device {
            transport,
            query_only: false,
            pid,
            serial,
            interface,
            path,
            txid,
            spec,
        })
    }
}

/// Razer products that are plugged in but didn't answer Protocol 2.5 (e.g. Kraken
/// headsets), so the user sees them instead of wondering where they went.
pub fn unanswered(api: &HidApi, found: &[Device]) -> Vec<u16> {
    let mut pids: Vec<u16> = candidates(api)
        .iter()
        .map(|c| c.pid)
        .filter(|p| !found.iter().any(|d| d.pid == *p))
        .collect();
    pids.dedup();
    pids
}

#[cfg(test)]
mod tests {
    use super::{connected_mouse_pid, parse_serial};
    use crate::proto::{checksum, Command, Report, Status};
    use crate::{Error, Transport};
    use std::collections::VecDeque;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    struct QueuedTransport {
        replies: Mutex<VecDeque<Vec<u8>>>,
        writes: AtomicUsize,
        reads: AtomicUsize,
        requests: Mutex<Vec<Vec<u8>>>,
    }

    impl QueuedTransport {
        fn new(replies: Vec<Vec<u8>>) -> Self {
            Self {
                replies: Mutex::new(replies.into()),
                writes: AtomicUsize::new(0),
                reads: AtomicUsize::new(0),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl Transport for QueuedTransport {
        fn send_feature_report(&self, data: &[u8]) -> crate::Result<()> {
            self.writes.fetch_add(1, Ordering::Relaxed);
            self.requests.lock().unwrap().push(data.to_vec());
            Ok(())
        }

        fn get_feature_report(&self, buffer: &mut [u8]) -> crate::Result<usize> {
            self.reads.fetch_add(1, Ordering::Relaxed);
            let reply = self.replies.lock().unwrap().pop_front().unwrap_or_default();
            buffer[..reply.len()].copy_from_slice(&reply);
            Ok(reply.len())
        }

        fn read_delay(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
    }

    fn wire(txid: u8, command: Command, args: &[u8]) -> Vec<u8> {
        let mut response = Report::request(txid, command, args.len() as u8, args);
        response.0[0] = 2;
        let mut wire = vec![0];
        wire.extend_from_slice(&response.0);
        wire
    }

    #[test]
    fn query_only_handle_blocks_all_set_ids_across_transaction_and_raw_frame_paths() {
        let sim = crate::sim::Sim::keyboard();
        let mut dev = sim
            .open(0x025d, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        dev.interface = 2;
        assert!(!dev.is_query_only());
        assert!(
            dev.set_brightness(5, 43).is_ok(),
            "ordinary handle still permits writes"
        );
        dev.enforce_query_only();
        dev.enforce_query_only();
        assert!(dev.is_query_only());
        sim.state().log.clear();
        for class in [0, 2, 4, 15, 127, 255] {
            for id in 0..128 {
                let command = Command::new(class, id);
                assert!(dev.transact(command, 0, &[]).is_err());
                assert!(dev.transact_as(0x3f, command, 0, &[]).is_err());
                assert!(dev.transact_slow(command, 0, &[]).is_err());
            }
        }
        let packets = crate::ornata_frame::PreparedOrnataFrame::new(&[crate::Rgb(1, 2, 3); 132])
            .unwrap()
            .packets(dev.txid);
        for packet in &packets {
            assert!(dev.send_ornata_frame_packet(packet).is_err());
        }
        assert!(
            sim.state().log.is_empty(),
            "all SET/raw-frame attempts refused before transport"
        );
        assert!(dev.firmware_version().is_ok());
        assert!(dev.brightness(5).is_ok());
        assert!(sim.state().log.iter().all(|command| command.id & 0x80 != 0));
    }
    #[test]
    fn stale_transaction_cannot_supply_a_feature_value() {
        let command = Command::new(0x0f, 0x84);
        let pipe = QueuedTransport::new(vec![
            wire(0x16, command, &[1, 5, 22]),
            wire(0x1f, command, &[1, 5, 128]),
        ]);
        let reply = super::transact(&pipe, 0x1f, command, 3, &[1, 5], 2).unwrap();
        assert_eq!(reply.args()[2], 128);
        assert_eq!(pipe.reads.load(Ordering::Relaxed), 2);
        assert_eq!(pipe.writes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn effect_header_reply_uses_explicit_minimum_without_shrinking_request() {
        let command = Command::new(0x0f, 0x82);
        let packet = wire(0x1f, command, &[1, 0, 8, 0, 0, 0]);
        let ordinary = QueuedTransport::new(vec![packet.clone()]);
        assert!(matches!(
            super::transact(&ordinary, 0x1f, command, 80, &[1, 0], 1),
            Err(Error::NoReply(_))
        ));
        let getter = QueuedTransport::new(vec![packet]);
        let reply =
            super::transact_with_response_minimum(&getter, 0x1f, command, 80, &[1, 0], 1, 6)
                .unwrap();
        assert_eq!(reply.0[5], 6);
        let requests = getter.requests.lock().unwrap();
        assert_eq!(requests[0].len(), 91);
        assert_eq!(requests[0][6], 80);
        assert_eq!(&requests[0][7..11], &[0x0f, 0x82, 1, 0]);
    }

    #[test]
    fn one_byte_mode_reply_keeps_two_byte_request_and_other_limits() {
        let command = Command::new(0, 0x84);
        let packet = wire(0x1f, command, &[3]);
        let ordinary = QueuedTransport::new(vec![packet.clone()]);
        assert!(matches!(
            super::transact(&ordinary, 0x1f, command, 2, &[], 1),
            Err(Error::NoReply(_))
        ));
        let getter = QueuedTransport::new(vec![packet]);
        let reply =
            super::transact_with_response_minimum(&getter, 0x1f, command, 2, &[], 1, 1).unwrap();
        assert_eq!(reply.0[5], 1);
        assert_eq!(reply.args()[0], 3);
        let requests = getter.requests.lock().unwrap();
        assert_eq!(requests[0].len(), 91);
        assert_eq!(&requests[0][6..11], &[2, 0, 0x84, 0, 0]);
    }

    #[test]
    fn one_byte_mode_reply_still_rejects_stale_corrupt_and_empty_frames() {
        let command = Command::new(0, 0x84);
        let mut corrupt = wire(0x1f, command, &[3]);
        corrupt[9] ^= 1;
        let pipe = QueuedTransport::new(vec![
            wire(0x3f, command, &[3]),
            corrupt,
            wire(0x1f, command, &[]),
            wire(0x1f, command, &[3]),
        ]);
        let reply =
            super::transact_with_response_minimum(&pipe, 0x1f, command, 2, &[], 4, 1).unwrap();
        assert_eq!(reply.args()[0], 3);
        assert_eq!(pipe.reads.load(Ordering::Relaxed), 4);
        assert_eq!(pipe.writes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn shorter_effect_reply_still_rejects_bad_frames_and_insufficient_headers() {
        let command = Command::new(0x0f, 0x82);
        let mut corrupt = wire(0x1f, command, &[1, 0, 8, 0, 0, 0]);
        corrupt[9] ^= 1;
        let pipe = QueuedTransport::new(vec![
            wire(0x3f, command, &[1, 0, 8, 0, 0, 0]),
            corrupt,
            wire(0x1f, command, &[1, 0, 8, 0, 0]),
            wire(0x1f, command, &[1, 0, 8, 0, 0, 0]),
        ]);
        let reply =
            super::transact_with_response_minimum(&pipe, 0x1f, command, 80, &[1, 0], 4, 6).unwrap();
        assert_eq!(reply.args()[2], 8);
        assert_eq!(pipe.reads.load(Ordering::Relaxed), 4);
        assert_eq!(pipe.writes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn invalid_reply_minimum_is_rejected_before_transport() {
        let pipe = QueuedTransport::new(vec![]);
        assert!(matches!(
            super::transact_with_response_minimum(
                &pipe,
                0x1f,
                Command::new(0x0f, 0x82),
                6,
                &[1, 0],
                1,
                7
            ),
            Err(Error::InvalidArg(_))
        ));
        assert_eq!(pipe.writes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn malformed_frames_are_skipped_before_decoding() {
        let command = Command::new(4, 0x85);
        let mut corrupt = wire(0x1f, command, &[0, 0, 100, 0, 100, 0, 0]);
        corrupt[9] ^= 0x80;
        let short = wire(0x1f, command, &[0]);
        let good = wire(0x1f, command, &[0, 3, 32, 3, 32, 0, 0]);
        let pipe = QueuedTransport::new(vec![corrupt, short, good]);
        let reply = super::transact(&pipe, 0x1f, command, 7, &[], 3).unwrap();
        assert_eq!(&reply.args()[1..5], &[3, 32, 3, 32]);
        assert_eq!(pipe.reads.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn invalid_requests_never_touch_the_transport() {
        let pipe = QueuedTransport::new(vec![]);
        assert!(matches!(
            super::transact(&pipe, 0x1f, Command::new(4, 5), 7, &[0; 81], 1),
            Err(Error::InvalidArg(_))
        ));
        assert_eq!(pipe.writes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn unsupported_reply_may_have_no_arguments() {
        let command = Command::new(4, 0x85);
        let mut body = Report::request(0x1f, command, 0, &[]);
        body.0[0] = 5;
        body.0[88] = checksum(&body.0);
        let pipe = QueuedTransport::new(vec![body.0.to_vec()]);
        assert!(matches!(
            super::transact(&pipe, 0x1f, command, 7, &[], 1),
            Err(Error::Device {
                status: Status::NotSupported,
                ..
            })
        ));
    }

    #[test]
    fn paired_mice_reply_needs_exactly_one_connected_mouse() {
        assert_eq!(
            connected_mouse_pid(&[2, 1, 0, 0xAB, 0, 0xFF, 0xFF]),
            Some(0x00AB)
        );
        for args in [
            &[][..],
            &[0],
            &[2, 1, 0, 0xAB],
            &[255, 1, 0, 0xAB],
            &[1, 0, 0, 0xAB],
            &[1, 1, 0xFF, 0xFF],
            &[1, 1, 0, 0],
            &[2, 1, 0, 0xAB, 1, 0, 0xA8],
        ] {
            assert_eq!(connected_mouse_pid(args), None, "{args:?}");
        }
    }

    #[test]
    fn serials_are_cleaned() {
        assert_eq!(
            parse_serial(b"AB1234C56789012\0\0\0\0\0\0\0"),
            "AB1234C56789012"
        );
        assert_eq!(parse_serial(b"  ABC123  \0"), "ABC123");
        assert_eq!(parse_serial(&[0u8; 22]), "");
        assert_eq!(parse_serial(b"000000000000\0"), "");
        assert_eq!(parse_serial(&[0xFF; 22]), "");
        assert_eq!(parse_serial(b"AB\x01CD\0"), "");
    }
}
