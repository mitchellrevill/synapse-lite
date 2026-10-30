//! Explicit Basilisk V3 / V3 Pro USB wheel registers. See docs/architecture.md.
use crate::{
    proto::{Command, Report},
    Device, Error, Result,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WheelControl {
    Mode,
    Acceleration,
    SmartReel,
}

impl WheelControl {
    pub const ALL: [Self; 3] = [Self::Mode, Self::Acceleration, Self::SmartReel];

    pub fn title(self) -> &'static str {
        match self {
            Self::Mode => "FREE-SPIN SCROLLING",
            Self::Acceleration => "SCROLL ACCELERATION",
            Self::SmartReel => "SMART REEL",
        }
    }

    pub(crate) fn command(self) -> Command {
        Command::new(
            2,
            match self {
                Self::Mode => 0x14,
                Self::Acceleration => 0x16,
                Self::SmartReel => 0x17,
            },
        )
    }

    pub(crate) fn size(self) -> u8 {
        // Both selected model SDKs declare80, despite OpenRazer's two-byte form.
        if self == Self::SmartReel {
            80
        } else {
            2
        }
    }
}

/// Source-supported transports only. BLE, generic dongles, dock forwarding and
/// mutable catalog entries cannot grant these capabilities.
pub fn supports(dev: &Device) -> bool {
    dev.txid == 0x1f && matches!((dev.pid, dev.interface), (0x0099, 3) | (0x00aa | 0x00ab, 0))
}

fn require_model(dev: &Device) -> Result<()> {
    if !supports(dev) {
        return Err(Error::Unsupported("Wheel controls require Basilisk V3 USB interface3 or V3 Pro USB/receiver interface0, transaction1F".into()));
    }
    Ok(())
}

fn require_slot(dev: &Device) -> Result<()> {
    require_model(dev)?;
    let r = dev.get(crate::commands::ACTIVE_SLOT, 1, &[])?;
    if r.0[5] != 1 || r.args()[0] != 1 {
        return Err(Error::Unsupported(
            "Wheel controls require active onboard slot1; no slot change was attempted".into(),
        ));
    }
    Ok(())
}

fn decode(control: WheelControl, r: &Report) -> Result<bool> {
    let n = usize::from(r.0[5]);
    if !(2..=usize::from(control.size())).contains(&n)
        || r.args()[0] != 1
        || r.args()[1] > 1
        || r.args()[2..n].iter().any(|b| *b != 0)
    {
        return Err(Error::Unsupported(
            "Unexpected wheel register size, slot, value or opaque data; mutation refused".into(),
        ));
    }
    Ok(r.args()[1] == 1)
}

impl Device {
    /// Exact slot1 register. `Mode=true` means free-spin; false means tactile.
    /// GET success is required per control, independently of catalog/firmware flags.
    pub fn wheel_control(&self, control: WheelControl) -> Result<bool> {
        require_slot(self)?;
        let r = self.get_wheel_register(control)?;
        decode(control, &r)
    }

    /// Change one register without saving any named profile or changing selection.
    /// A lost acknowledgement can follow a mutation, so verify recovery with GET.
    pub fn set_wheel_control(&self, control: WheelControl, enabled: bool) -> Result<()> {
        if self.is_query_only() {
            return Err(Error::Unsupported(
                "Read-only handle refuses wheel changes".into(),
            ));
        }
        let original = self.wheel_control(control)?;
        if original == enabled {
            return Ok(());
        }
        let write = || -> Result<()> {
            require_slot(self)?;
            self.transact(control.command(), control.size(), &[1, u8::from(enabled)])?;
            if self.wheel_control(control)? != enabled {
                return Err(Error::Unsupported(
                    "Wheel readback differs from requested value".into(),
                ));
            }
            Ok(())
        };
        if let Err(error) = write() {
            // Never recover into a different active hardware profile.
            let recovery = (|| -> Result<()> {
                require_slot(self)?;
                let sent =
                    self.transact(control.command(), control.size(), &[1, u8::from(original)]);
                match self.wheel_control(control) {
                    Ok(value) if value == original => Ok(()),
                    observed => Err(Error::Unsupported(format!(
                        "Wheel recovery unverified: write={:?}, readback={observed:?}",
                        sent.err()
                    ))),
                }
            })();
            return Err(Error::Unsupported(match recovery {
                Ok(()) => format!("{error}; original wheel register restored and verified"),
                Err(recovery) => format!("{error}; {recovery}"),
            }));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{proto::checksum, Transport};
    use std::{
        ffi::CString,
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[derive(Default)]
    struct State {
        values: [u8; 3],
        requests: Vec<Vec<u8>>,
        reply: Option<Report>,
        slot: u8,
        malformed: u8,
        unsupported: Option<usize>,
        lose_ack: bool,
        refuse_restore: bool,
        short_smart_reel: bool,
    }
    #[derive(Clone)]
    struct Wire(Arc<Mutex<State>>);

    impl Transport for Wire {
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
        fn send_feature_report(&self, bytes: &[u8]) -> Result<()> {
            assert_eq!(bytes.len(), 91);
            assert_eq!(bytes[0], 0);
            let request = Report(bytes[1..].try_into().unwrap());
            assert_eq!(checksum(&request.0), request.0[88]);
            let mut state = self.0.lock().unwrap();
            state.requests.push(bytes.to_vec());
            let command = request.command();
            let mut reply = request.clone();
            reply.0[0] = 2;
            if command == crate::commands::SERIAL.get() {
                reply.0[8..12].copy_from_slice(b"TEST");
            } else if command == crate::commands::ACTIVE_SLOT.get() {
                reply.0[8] = state.slot;
            } else {
                let index = match (command.class, command.id & 0x7f) {
                    (2, 0x14) => 0,
                    (2, 0x16) => 1,
                    (2, 0x17) => 2,
                    _ => panic!("unexpected command {command}"),
                };
                assert_eq!(request.0[5], if index == 2 { 80 } else { 2 });
                assert_eq!(request.args()[0], 1);
                assert!(request.args()[2..].iter().all(|b| *b == 0));
                if state.unsupported == Some(index) {
                    reply.0[0] = 5;
                    reply.0[5] = 0;
                } else if command.id & 0x80 != 0 {
                    reply.0[9] = state.values[index];
                    if index == 2 && state.short_smart_reel {
                        reply.0[5] = 2;
                    }
                    match state.malformed {
                        1 => reply.0[8] = 2,
                        2 => reply.0[9] = 2,
                        3 => reply.0[5] = 1,
                        4 => reply.0[10] = 99,
                        5 => reply.0[5] = 81,
                        _ => {}
                    }
                } else if state.refuse_restore && request.args()[1] == 0 {
                    reply.0[0] = 3;
                } else {
                    state.values[index] = request.args()[1];
                    if state.lose_ack {
                        state.lose_ack = false;
                        reply.0[0] = 3;
                    }
                }
            }
            reply.0[88] = checksum(&reply.0);
            state.reply = Some(reply);
            Ok(())
        }
        fn get_feature_report(&self, out: &mut [u8]) -> Result<usize> {
            out[0] = 0;
            out[1..91].copy_from_slice(&self.0.lock().unwrap().reply.as_ref().unwrap().0);
            Ok(91)
        }
    }

    fn device(pid: u16, interface: i32) -> (Wire, Device) {
        let wire = Wire(Arc::new(Mutex::new(State {
            slot: 1,
            ..Default::default()
        })));
        let dev = Device::open(
            Box::new(wire.clone()),
            pid,
            interface,
            CString::new("sim:wheel").unwrap(),
            &[0x1f],
            None,
        )
        .unwrap();
        wire.0.lock().unwrap().requests.clear();
        (wire, dev)
    }

    #[test]
    fn all_explicit_usb_models_round_trip_independent_registers_and_restore() {
        for (pid, interface) in [(0x0099, 3), (0x00aa, 0), (0x00ab, 0)] {
            let (wire, dev) = device(pid, interface);
            for control in WheelControl::ALL {
                assert!(!dev.wheel_control(control).unwrap());
                dev.set_wheel_control(control, true).unwrap();
                assert!(dev.wheel_control(control).unwrap());
                dev.set_wheel_control(control, false).unwrap();
            }
            assert_eq!(wire.0.lock().unwrap().values, [0; 3]);
        }
    }

    #[test]
    fn packet_vectors_match_model_sdk_including_eighty_byte_smart_reel() {
        let (wire, dev) = device(0x0099, 3);
        for control in WheelControl::ALL {
            dev.set_wheel_control(control, true).unwrap();
        }
        let state = wire.0.lock().unwrap();
        let sets: Vec<_> = state.requests.iter().filter(|b| b[8] & 0x80 == 0).collect();
        assert_eq!(sets.len(), 3);
        for (packet, (size, id, crc)) in
            sets.iter()
                .zip([(2, 0x14, 0x14), (2, 0x16, 0x16), (80, 0x17, 0x45)])
        {
            assert_eq!(&packet[..11], &[0, 0, 0x1f, 0, 0, 0, size, 2, id, 1, 1]);
            assert_eq!(packet[89], crc);
            assert!(packet[11..89].iter().all(|b| *b == 0));
        }
    }

    #[test]
    fn no_set_for_unsupported_transport_model_slot_or_readonly_handle() {
        for (pid, interface) in [
            (0x0099, 0),
            (0x00aa, 3),
            (0x00ac, 0),
            (0x00a4, 0),
            (0x00b3, 0),
            (196, 0),
            (0xffff, 0),
        ] {
            let (wire, dev) = device(pid, interface);
            assert!(dev.set_wheel_control(WheelControl::Mode, true).is_err());
            assert!(wire.0.lock().unwrap().requests.is_empty());
        }
        let (wire, mut dev) = device(0x0099, 3);
        dev.txid = 0x3f;
        assert!(dev.wheel_control(WheelControl::Mode).is_err());
        assert!(wire.0.lock().unwrap().requests.is_empty());
        dev.txid = 0x1f;
        wire.0.lock().unwrap().slot = 2;
        assert!(dev.set_wheel_control(WheelControl::Mode, true).is_err());
        assert!(wire
            .0
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|b| b[8] & 0x80 != 0));
        wire.0.lock().unwrap().requests.clear();
        dev.enforce_query_only();
        assert!(dev.set_wheel_control(WheelControl::Mode, true).is_err());
        assert!(wire.0.lock().unwrap().requests.is_empty());
    }

    #[test]
    fn malformed_and_unsupported_registers_refuse_mutation_per_capability() {
        for fault in 1..=5 {
            let (wire, dev) = device(0x0099, 3);
            wire.0.lock().unwrap().malformed = fault;
            assert!(dev
                .set_wheel_control(WheelControl::SmartReel, true)
                .is_err());
            assert!(wire
                .0
                .lock()
                .unwrap()
                .requests
                .iter()
                .all(|b| b[8] & 0x80 != 0));
        }
        let (wire, dev) = device(0x0099, 3);
        wire.0.lock().unwrap().unsupported = Some(2);
        assert!(dev
            .set_wheel_control(WheelControl::SmartReel, true)
            .is_err());
        dev.set_wheel_control(WheelControl::Acceleration, true)
            .unwrap();
    }

    #[test]
    fn short_meaningful_smart_reel_reply_and_noop_are_supported() {
        let (wire, dev) = device(0x0099, 3);
        wire.0.lock().unwrap().short_smart_reel = true;
        dev.set_wheel_control(WheelControl::SmartReel, false)
            .unwrap();
        assert!(wire
            .0
            .lock()
            .unwrap()
            .requests
            .iter()
            .all(|b| b[8] & 0x80 != 0));
        dev.set_wheel_control(WheelControl::SmartReel, true)
            .unwrap();
    }

    #[test]
    fn mutation_followed_by_lost_ack_restores_original_and_reports_failure() {
        let (wire, dev) = device(0x0099, 3);
        wire.0.lock().unwrap().lose_ack = true;
        let error = dev
            .set_wheel_control(WheelControl::Mode, true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("restored and verified"), "{error}");
        assert_eq!(wire.0.lock().unwrap().values[0], 0);
        wire.0.lock().unwrap().lose_ack = true;
        wire.0.lock().unwrap().refuse_restore = true;
        let error = dev
            .set_wheel_control(WheelControl::Mode, true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("recovery unverified"), "{error}");
        assert_eq!(wire.0.lock().unwrap().values[0], 1);
    }
}
