//! Bounded Ornata V2 physical frame transfer-out, without mode changes.
//!
//! Stock chunk2890 sends six71-byte rows and a6-byte CustomFrame8 commit with
//! transfer-in disabled. See docs/ORNATA-FRAMES.md for the execution boundary.
//! This module accepts physical6x22 RGB order; logical layout mapping is separate.
//! It provides no interprocess ownership, brightness change or restoration. Callers
//! arrange ownership/recovery. An accepted USB OUT is not firmware/optical proof.
use crate::{
    lighting_readback::DeviceModeReadback,
    proto::{Command, Report},
    Device, Error, Result, Rgb,
};
use std::time::{Duration, Instant};

pub const ROWS: usize = 6;
pub const COLS: usize = 22;
pub const PIXELS: usize = ROWS * COLS;
pub const PACKETS: usize = ROWS + 1;

/// A bounded physical frame, independent of device handles and logical layout.
#[derive(Debug, Clone)]
pub struct PreparedOrnataFrame {
    pixels: [Rgb; PIXELS],
}

/// Why this one bounded batch stopped. Errors after a transfer attempt do not
/// prove the failed packet was absent from the device.
#[derive(Debug)]
pub enum FrameTransferStop {
    CompleteUsbOut,
    Cancelled,
    Deadline,
    ModeQueryError(Error),
    NotDriverMode(u8),
    TransferError { packet_index: usize, error: Error },
}
#[derive(Debug)]
pub struct FrameTransferOutcome {
    /// Frame packets whose send_feature_report call returned Ok (not ACKs).
    pub accepted_packets: usize,
    /// Includes a failed OUT, which may have affected hardware. Excludes mode GET.
    pub attempted_packets: usize,
    pub mode_query_attempted: bool,
    /// Wall time spent in this batch's fresh mode GET, including its transport.
    pub mode_query_elapsed: Duration,
    /// Sum of actual frame OUT call durations, including a failed call.
    /// Excludes mode GET, packet construction, cancellation and loop overhead.
    pub usb_out_elapsed: Duration,
    /// Fresh preflight only; no post-write or unchanged-mode claim is made.
    pub observed_mode: Option<DeviceModeReadback>,
    pub stop: FrameTransferStop,
}
impl FrameTransferOutcome {
    pub fn usb_out_complete(&self) -> bool {
        matches!(self.stop, FrameTransferStop::CompleteUsbOut)
    }
    pub fn commit_accepted(&self) -> bool {
        self.accepted_packets == PACKETS
    }
}
impl PreparedOrnataFrame {
    pub fn new(physical_row_major: &[Rgb]) -> Result<Self> {
        let pixels = physical_row_major.try_into().map_err(|_| {
            Error::InvalidArg("Ornata physical frame requires exactly132 RGB pixels (6x22)".into())
        })?;
        Ok(Self { pixels })
    }
    /// Seven complete91-byte HID feature wrappers, including report-id0 and
    /// normal protocol checksum. Row0..5 precede a profile0/region0 commit.
    pub fn packets(&self, txid: u8) -> [[u8; 91]; PACKETS] {
        std::array::from_fn(|row| {
            let report = if row < ROWS {
                let mut args = [0; 71];
                args[..5].copy_from_slice(&[0, 0, row as u8, 0, 21]);
                for col in 0..COLS {
                    let Rgb(r, g, b) = self.pixels[row * COLS + col];
                    args[5 + 3 * col..8 + 3 * col].copy_from_slice(&[r, g, b]);
                }
                Report::request(txid, Command::new(15, 3), 71, &args)
            } else {
                Report::request(txid, Command::new(15, 2), 6, &[0, 0, 8, 0, 0, 0])
            };
            let mut packet = [0; 91];
            packet[1..].copy_from_slice(&report.0);
            packet
        })
    }
    /// The mutable handle serializes this batch only within this Rust borrow;
    /// cloned transports, other handles and other processes remain independent.
    /// Deadline/cancellation are cooperative before preflight and each packet.
    /// They cannot interrupt an in-flight mode query or HID syscall. No write is
    /// retried, and no firmware ACK is requested for rows or commit.
    pub fn transfer_out(
        &self,
        device: &mut Device,
        deadline: Instant,
        cancelled: impl FnMut() -> bool,
    ) -> Result<FrameTransferOutcome> {
        self.transfer_with_clock(device, deadline, cancelled, Instant::now)
    }
    fn transfer_with_clock(
        &self,
        device: &mut Device,
        deadline: Instant,
        mut cancelled: impl FnMut() -> bool,
        mut now: impl FnMut() -> Instant,
    ) -> Result<FrameTransferOutcome> {
        validate_device(device)?;
        let mut outcome = FrameTransferOutcome {
            accepted_packets: 0,
            attempted_packets: 0,
            mode_query_attempted: false,
            mode_query_elapsed: Duration::ZERO,
            usb_out_elapsed: Duration::ZERO,
            observed_mode: None,
            stop: FrameTransferStop::CompleteUsbOut,
        };
        if let Some(stop) = interrupted(deadline, &mut cancelled, &mut now) {
            outcome.stop = stop;
            return Ok(outcome);
        }
        outcome.mode_query_attempted = true;
        let mode_started = Instant::now();
        let mode_result = device.ornata_mode_readback();
        outcome.mode_query_elapsed = mode_started.elapsed();
        match mode_result {
            Ok(mode) => {
                let value = mode.mode();
                outcome.observed_mode = Some(mode);
                if value != 3 {
                    outcome.stop = FrameTransferStop::NotDriverMode(value);
                    return Ok(outcome);
                }
            }
            Err(error) => {
                outcome.stop = FrameTransferStop::ModeQueryError(error);
                return Ok(outcome);
            }
        }
        for (packet_index, packet) in self.packets(device.txid).iter().enumerate() {
            if let Some(stop) = interrupted(deadline, &mut cancelled, &mut now) {
                outcome.stop = stop;
                return Ok(outcome);
            }
            outcome.attempted_packets += 1;
            let out_started = Instant::now();
            let out_result = device.send_ornata_frame_packet(packet);
            outcome.usb_out_elapsed += out_started.elapsed();
            if let Err(error) = out_result {
                outcome.stop = FrameTransferStop::TransferError {
                    packet_index,
                    error,
                };
                return Ok(outcome);
            }
            outcome.accepted_packets += 1;
        }
        Ok(outcome)
    }
}
fn interrupted(
    deadline: Instant,
    cancelled: &mut impl FnMut() -> bool,
    now: &mut impl FnMut() -> Instant,
) -> Option<FrameTransferStop> {
    if cancelled() {
        Some(FrameTransferStop::Cancelled)
    } else if now() >= deadline {
        Some(FrameTransferStop::Deadline)
    } else {
        None
    }
}
pub(crate) fn validate_device(device: &Device) -> Result<()> {
    if device.pid != 0x025d
        || device.interface != 2
        || !device
            .spec
            .as_ref()
            .and_then(|s| s.matrix)
            .is_some_and(|m| m.rows == 6 && m.cols == 22)
    {
        return Err(Error::Unsupported(
            "Ornata frame transfer requires PID025D/interface2 with physical6x22 matrix".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        sim::{Sim, KEYBOARD_PID},
        Transport,
    };
    use std::{
        ffi::CString,
        sync::{Arc, Mutex},
        time::Duration,
    };
    #[derive(Default)]
    struct State {
        packets: Vec<[u8; 91]>,
        reply: Option<[u8; 91]>,
        reads: usize,
        frame_attempts: usize,
        fail_at: Option<usize>,
        mode: u8,
        mode_fail: bool,
    }
    #[derive(Clone)]
    struct Spy(Arc<Mutex<State>>);
    impl Transport for Spy {
        fn send_feature_report(&self, data: &[u8]) -> Result<()> {
            let packet: [u8; 91] = data.try_into().unwrap();
            let mut state = self.0.lock().unwrap();
            state.packets.push(packet);
            let command = Command::new(packet[7], packet[8]);
            if command == Command::new(0, 0x82) || command == Command::new(0, 0x84) {
                let args = if command.id == 0x84 {
                    vec![state.mode]
                } else {
                    vec![0; 22]
                };
                let mut report = Report::request(packet[2], command, args.len() as u8, &args);
                report.0[0] = if command.id == 0x84 && state.mode_fail {
                    5
                } else {
                    2
                };
                let mut reply = [0; 91];
                reply[1..].copy_from_slice(&report.0);
                state.reply = Some(reply);
                Ok(())
            } else {
                assert_eq!(
                    command.class, 15,
                    "no mode/input/brightness/profile command"
                );
                let index = state.frame_attempts;
                state.frame_attempts += 1;
                if state.fail_at == Some(index) {
                    Err(Error::System(format!("injected OUT failure at{index}")))
                } else {
                    Ok(())
                }
            }
        }
        fn get_feature_report(&self, buffer: &mut [u8]) -> Result<usize> {
            let mut state = self.0.lock().unwrap();
            state.reads += 1;
            let reply = state
                .reply
                .take()
                .expect("frame path must never request transfer-in");
            buffer[..91].copy_from_slice(&reply);
            Ok(91)
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    fn fixture() -> (Device, Spy) {
        let spy = Spy(Arc::new(Mutex::new(State {
            mode: 3,
            ..Default::default()
        })));
        let device = Device::open(
            Box::new(spy.clone()),
            0x025d,
            2,
            CString::new("frame-test").unwrap(),
            &[0x1f],
            Sim::spec_for(KEYBOARD_PID),
        )
        .unwrap();
        {
            let mut s = spy.0.lock().unwrap();
            s.packets.clear();
            s.reads = 0;
        }
        (device, spy)
    }
    fn pixels() -> Vec<Rgb> {
        (0..132)
            .map(|i| Rgb(i as u8, (i * 3) as u8, (255 - i) as u8))
            .collect()
    }
    fn frame() -> PreparedOrnataFrame {
        PreparedOrnataFrame::new(&pixels()).unwrap()
    }
    // Independent expected full HID report, with an explicit checksum loop.
    fn expected(txid: u8, command: u8, args: &[u8]) -> [u8; 91] {
        let mut p = [0; 91];
        p[2] = txid;
        p[6] = args.len() as u8;
        p[7] = 15;
        p[8] = command;
        p[9..9 + args.len()].copy_from_slice(args);
        let mut checksum = 0;
        for b in &p[3..89] {
            checksum ^= *b;
        }
        p[89] = checksum;
        p
    }
    #[test]
    fn physical_rows_and_commit_match_independent_full_hid_fixture() {
        for count in [0, 131, 133] {
            assert!(PreparedOrnataFrame::new(&vec![Rgb(0, 0, 0); count]).is_err());
        }
        let packets = frame().packets(0x1f);
        for row in 0..6 {
            let mut args = vec![0, 0, row as u8, 0, 21];
            for col in 0..22 {
                let index = row * 22 + col;
                args.extend([index as u8, (index * 3) as u8, (255 - index) as u8]);
            }
            assert_eq!(packets[row], expected(0x1f, 3, &args));
            assert_eq!(packets[row][6], 71);
            assert!(packets[row][80..89].iter().all(|b| *b == 0));
        }
        assert_eq!(
            &packets[0][77..80],
            &[21, 63, 234],
            "last pixel of first physical row"
        );
        assert_eq!(&packets[5][77..80], &[131, 137, 124], "last physical pixel");
        assert_eq!(packets[6], expected(0x1f, 2, &[0, 0, 8, 0, 0, 0]));
    }
    #[test]
    fn complete_out_has_fresh_mode_get_seven_writes_and_no_frame_reads() {
        let (mut device, spy) = fixture();
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                false
            })
            .unwrap();
        assert_eq!(
            (outcome.accepted_packets, outcome.attempted_packets),
            (7, 7)
        );
        assert!(outcome.usb_out_complete());
        assert!(outcome.commit_accepted());
        let mode = outcome.observed_mode.unwrap();
        assert_eq!(mode.mode(), 3);
        assert_eq!(mode.param(), None);
        let state = spy.0.lock().unwrap();
        assert_eq!(state.reads, 1);
        assert_eq!(state.packets.len(), 8);
        assert_eq!(&state.packets[0][7..9], &[0, 0x84]);
        assert_eq!(&state.packets[1..], &frame().packets(device.txid));
    }
    #[test]
    fn scope_failures_precede_all_io() {
        for variant in 0..5 {
            let (mut device, spy) = fixture();
            match variant {
                0 => device.pid = 0x00c4,
                1 => device.interface = 0,
                2 => device.spec = None,
                3 => device.spec.as_mut().unwrap().matrix.as_mut().unwrap().rows = 5,
                _ => device.spec.as_mut().unwrap().matrix.as_mut().unwrap().cols = 21,
            }
            assert!(frame()
                .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                    false
                })
                .is_err());
            let state = spy.0.lock().unwrap();
            assert!(state.packets.is_empty());
            assert_eq!(state.reads, 0);
        }
    }
    #[test]
    fn expired_or_cancelled_preflight_does_no_io_and_post_query_cancel_does_no_out() {
        let (mut device, spy) = fixture();
        let outcome = frame()
            .transfer_out(&mut device, Instant::now(), || false)
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Deadline));
        assert!(!outcome.mode_query_attempted);
        assert_eq!(outcome.mode_query_elapsed, Duration::ZERO);
        assert_eq!(outcome.usb_out_elapsed, Duration::ZERO);
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                true
            })
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Cancelled));
        assert!(!outcome.mode_query_attempted);
        assert!(spy.0.lock().unwrap().packets.is_empty());
        let mut calls = 0;
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                calls += 1;
                calls == 2
            })
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Cancelled));
        assert!(outcome.mode_query_attempted);
        assert_eq!(outcome.attempted_packets, 0);
        assert_eq!(outcome.usb_out_elapsed, Duration::ZERO);
        assert_eq!(spy.0.lock().unwrap().packets.len(), 1);
    }
    #[test]
    fn deadline_after_mode_query_prevents_first_out_and_each_batch_requeries_mode() {
        let (mut device, spy) = fixture();
        let start = Instant::now();
        let deadline = start + Duration::from_secs(1);
        let mut calls = 0;
        let outcome = frame()
            .transfer_with_clock(
                &mut device,
                deadline,
                || false,
                || {
                    calls += 1;
                    if calls == 2 {
                        deadline
                    } else {
                        start
                    }
                },
            )
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Deadline));
        assert!(outcome.mode_query_attempted);
        assert_eq!(outcome.attempted_packets, 0);
        assert_eq!(spy.0.lock().unwrap().frame_attempts, 0);
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                false
            })
            .unwrap();
        assert!(outcome.usb_out_complete());
        spy.0.lock().unwrap().mode = 0;
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                false
            })
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::NotDriverMode(0)));
        let state = spy.0.lock().unwrap();
        assert_eq!(state.reads, 3);
        assert_eq!(state.frame_attempts, 7);
    }
    #[test]
    fn partial_out_error_retains_possible_failed_attempt_and_never_retries_or_commits() {
        for fail_at in [0, 2, 6] {
            let (mut device, spy) = fixture();
            spy.0.lock().unwrap().fail_at = Some(fail_at);
            let outcome = frame()
                .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                    false
                })
                .unwrap();
            assert_eq!(outcome.accepted_packets, fail_at);
            assert_eq!(outcome.attempted_packets, fail_at + 1);
            assert!(!outcome.commit_accepted());
            assert!(!outcome.usb_out_complete());
            match outcome.stop {
                FrameTransferStop::TransferError {
                    packet_index,
                    error,
                } => {
                    assert_eq!(packet_index, fail_at);
                    assert!(error.to_string().contains("injected OUT"));
                }
                _ => panic!("expected transfer error"),
            }
            let state = spy.0.lock().unwrap();
            assert_eq!(state.frame_attempts, fail_at + 1);
            assert_eq!(state.reads, 1);
            assert_eq!(state.packets.len(), fail_at + 2);
        }
    }
    #[test]
    fn cancellation_and_deadline_between_rows_keep_counts_and_skip_commit() {
        let (mut device, spy) = fixture();
        let mut calls = 0;
        let outcome = frame()
            .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                calls += 1;
                calls == 4
            })
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Cancelled));
        assert_eq!(
            (outcome.accepted_packets, outcome.attempted_packets),
            (2, 2)
        );
        assert_eq!(spy.0.lock().unwrap().frame_attempts, 2);
        let (mut device, spy) = fixture();
        let start = Instant::now();
        let deadline = start + Duration::from_secs(1);
        let mut calls = 0;
        let outcome = frame()
            .transfer_with_clock(
                &mut device,
                deadline,
                || false,
                || {
                    calls += 1;
                    if calls == 4 {
                        deadline
                    } else {
                        start
                    }
                },
            )
            .unwrap();
        assert!(matches!(outcome.stop, FrameTransferStop::Deadline));
        assert_eq!(
            (outcome.accepted_packets, outcome.attempted_packets),
            (2, 2)
        );
        assert_eq!(spy.0.lock().unwrap().frame_attempts, 2);
    }
    #[test]
    fn mode_refusal_or_failed_query_sends_no_frame_and_does_not_claim_mode_unchanged() {
        for failed in [false, true] {
            let (mut device, spy) = fixture();
            {
                let mut state = spy.0.lock().unwrap();
                state.mode = 0;
                state.mode_fail = failed;
            }
            let outcome = frame()
                .transfer_out(&mut device, Instant::now() + Duration::from_secs(1), || {
                    false
                })
                .unwrap();
            assert_eq!(
                (outcome.accepted_packets, outcome.attempted_packets),
                (0, 0)
            );
            assert!(outcome.mode_query_attempted);
            if failed {
                assert!(matches!(outcome.stop, FrameTransferStop::ModeQueryError(_)));
                assert!(outcome.observed_mode.is_none());
            } else {
                assert!(matches!(outcome.stop, FrameTransferStop::NotDriverMode(0)));
                assert_eq!(outcome.observed_mode.unwrap().mode(), 0);
            }
            let state = spy.0.lock().unwrap();
            assert_eq!(state.frame_attempts, 0);
            assert_eq!(state.packets.len(), 1);
        }
    }
    #[test]
    fn private_frame_sender_rejects_other_commands_and_noncanonical_packets() {
        let (mut device, spy) = fixture();
        let mut packets = frame().packets(device.txid);
        packets[0][0] = 1;
        assert!(device.send_ornata_frame_packet(&packets[0]).is_err());
        let mut bad = expected(device.txid, 2, &[1, 0, 8, 0, 0, 0]);
        assert!(device.send_ornata_frame_packet(&bad).is_err());
        bad = expected(device.txid, 4, &[0, 0, 0]);
        assert!(device.send_ornata_frame_packet(&bad).is_err());
        let mut bad = frame().packets(device.txid)[0];
        bad[89] ^= 1;
        assert!(device.send_ornata_frame_packet(&bad).is_err());
        assert!(spy.0.lock().unwrap().packets.is_empty());
    }
}
