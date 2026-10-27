//! Read the shipped Ornata physical edition/layout tuple with GET00:86.
//!
//! No cache, locale inference, fallback identity, or spatial-map resolution is
//! involved. Source evidence and scope: docs/ORNATA-IDENTITY.md.

use crate::proto::{Command, Report, Status, MAX_ARGS};
use crate::{Device, Error, Result};

const PRODUCT_ID: u16 = 0x025d;
const GET_EDITION: Command = Command::new(0x00, 0x86);
const EDITION_SIZE: u8 = 3;

/// A correlated device observation. A successful unknown tuple is distinct
/// from an unsupported GET or a malformed response. firmware_id is a raw ID,
/// not the firmware version. Padding is never promoted into identity bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrnataEditionReadback {
    raw_args: [u8; MAX_ARGS],
    declared_size: u8,
}

impl OrnataEditionReadback {
    pub fn from_reply(txid: u8, reply: &Report) -> Result<Self> {
        let minimum = if reply.status() == Status::Success {
            EDITION_SIZE
        } else {
            0
        };
        if !reply.valid_reply(txid, GET_EDITION, minimum) {
            return Err(Error::InvalidArg(
                "Malformed Ornata edition/layout reply".into(),
            ));
        }
        if reply.status() != Status::Success {
            return Err(Error::Device {
                cmd: GET_EDITION,
                status: reply.status(),
            });
        }
        Ok(Self {
            raw_args: reply.args().try_into().expect("fixed report argument size"),
            declared_size: reply.0[5],
        })
    }

    pub fn layout_id(&self) -> u8 {
        self.raw_args[0]
    }
    pub fn edition_id(&self) -> u8 {
        self.raw_args[1]
    }
    pub fn firmware_id(&self) -> u8 {
        self.raw_args[2]
    }
    pub fn declared_size(&self) -> u8 {
        self.declared_size
    }
    /// Declared bytes, including any uninterpreted extension after the tuple.
    pub fn declared_args(&self) -> &[u8] {
        &self.raw_args[..usize::from(self.declared_size)]
    }

    /// Tag the raw observation for a spatial caller. This does not certify
    /// manifest membership or select a fallback; OrnataSpatialMap validates it
    /// separately against its source record and dimensions.
    pub fn layout_identity(&self) -> crate::ornata_spatial::LayoutIdentity {
        crate::ornata_spatial::LayoutIdentity {
            vendor_id: crate::db::RAZER_VID,
            product_id: PRODUCT_ID,
            edition_id: u16::from(self.edition_id()),
            layout_id: u16::from(self.layout_id()),
        }
    }
}

impl Device {
    /// Query only this Ornata V2 control interface. Errors remain explicit;
    /// cached UI identity and historical physical observations are not consulted.
    /// Uses ordinary strict GET correlation/checksum/declared-length validation.
    pub fn ornata_edition_readback(&self) -> Result<OrnataEditionReadback> {
        if self.pid != PRODUCT_ID || self.interface != 2 {
            return Err(Error::Unsupported(
                "Edition/layout GET is verified only for Ornata V2 interface2".into(),
            ));
        }
        let reply = self.get(GET_EDITION, EDITION_SIZE, &[])?;
        OrnataEditionReadback::from_reply(self.txid, &reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{proto::checksum, sim::Sim, transport::Transport};
    use std::{
        ffi::CString,
        sync::{Arc, Mutex},
        time::Duration,
    };

    fn reply(size: u8, args: &[u8]) -> Report {
        let mut r = Report::request(0x1f, GET_EDITION, size, args);
        r.0[0] = 2;
        r
    }

    #[test]
    fn baseline_and_unknown_tuples_preserve_raw_ids_and_extensions() {
        for args in [[6, 0, 9, 77], [0, 128, 255, 42], [255, 254, 253, 252]] {
            let value = OrnataEditionReadback::from_reply(0x1f, &reply(4, &args)).unwrap();
            assert_eq!(
                (value.layout_id(), value.edition_id(), value.firmware_id()),
                (args[0], args[1], args[2])
            );
            assert_eq!(value.declared_args(), args);
            assert_eq!(value.declared_size(), 4);
            let tag = value.layout_identity();
            assert_eq!(
                (tag.vendor_id, tag.product_id, tag.edition_id, tag.layout_id),
                (0x1532, 605, u16::from(args[1]), u16::from(args[0]))
            );
        }
    }

    #[test]
    fn short_declared_data_never_uses_matching_padding_and_malformed_headers_fail() {
        for size in [0, 1, 2, 81, 255] {
            assert!(OrnataEditionReadback::from_reply(0x1f, &reply(size, &[6, 0, 9])).is_err());
        }
        for byte in [0, 1, 2, 3, 4, 6, 7, 88, 89] {
            let mut bad = reply(3, &[6, 0, 9]);
            bad.0[byte] ^= 0x7f;
            if byte != 88 {
                bad.0[88] = checksum(&bad.0);
            }
            assert!(
                OrnataEditionReadback::from_reply(0x1f, &bad).is_err(),
                "byte{byte}"
            );
        }
        let mut unsupported = reply(0, &[]);
        unsupported.0[0] = 5;
        assert!(matches!(
            OrnataEditionReadback::from_reply(0x1f, &unsupported),
            Err(Error::Device {
                status: Status::NotSupported,
                ..
            })
        ));
    }

    struct State {
        requests: Vec<Vec<u8>>,
        response: Report,
        pending: bool,
        returned_length: usize,
        reads: usize,
    }
    struct Spy {
        sim: Sim,
        state: Arc<Mutex<State>>,
    }
    impl Transport for Spy {
        fn send_feature_report(&self, data: &[u8]) -> Result<()> {
            let mut state = self.state.lock().unwrap();
            state.requests.push(data.to_vec());
            if data[7] == 0 && data[8] == 0x86 {
                state.pending = true;
                Ok(())
            } else {
                drop(state);
                self.sim.send_feature_report(data)
            }
        }
        fn get_feature_report(&self, out: &mut [u8]) -> Result<usize> {
            let mut state = self.state.lock().unwrap();
            if state.pending {
                state.reads += 1;
                let n = state.returned_length;
                if n == 90 {
                    out[..90].copy_from_slice(&state.response.0);
                } else {
                    out[0] = 0;
                    out[1..91].copy_from_slice(&state.response.0);
                }
                Ok(n)
            } else {
                drop(state);
                self.sim.get_feature_report(out)
            }
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    fn fixture(response: Report, length: usize) -> (Device, Arc<Mutex<State>>) {
        let state = Arc::new(Mutex::new(State {
            requests: Vec::new(),
            response,
            pending: false,
            returned_length: length,
            reads: 0,
        }));
        let sim = Sim::keyboard();
        sim.state().txid = 0x1f;
        let transport = Spy {
            sim,
            state: state.clone(),
        };
        let dev = Device::open(
            Box::new(transport),
            605,
            2,
            CString::new("sim:edition").unwrap(),
            &[0x1f],
            None,
        )
        .unwrap();
        state.lock().unwrap().requests.clear();
        (dev, state)
    }

    #[test]
    fn query_only_get_has_exact_zero_selector_packet_and_accepts_both_hid_lengths() {
        for length in [90, 91] {
            let (mut dev, state) = fixture(reply(3, &[6, 0, 9]), length);
            dev.enforce_query_only();
            assert_eq!(
                dev.ornata_edition_readback().unwrap().declared_args(),
                [6, 0, 9]
            );
            let state = state.lock().unwrap();
            let mut expected = [0u8; 91];
            expected[2] = 0x1f;
            expected[6] = 3;
            expected[8] = 0x86;
            expected[89] = 0x85;
            assert_eq!(state.requests, vec![expected.to_vec()]);
            assert_eq!(state.reads, 1);
        }
    }

    #[test]
    fn wrong_scope_is_rejected_without_any_io() {
        for (pid, interface) in [(605, 0), (605, 1), (605, 3), (196, 2)] {
            let (mut dev, state) = fixture(reply(3, &[6, 0, 9]), 91);
            dev.pid = pid;
            dev.interface = interface;
            assert!(matches!(
                dev.ornata_edition_readback(),
                Err(Error::Unsupported(_))
            ));
            let state = state.lock().unwrap();
            assert!(state.requests.is_empty());
            assert_eq!(state.reads, 0);
        }
    }

    #[test]
    fn unsupported_stays_distinct_from_unknown_and_invalid_data_gets_no_fallback_or_resend() {
        let mut unsupported = reply(0, &[]);
        unsupported.0[0] = 5;
        let (dev, state) = fixture(unsupported, 91);
        assert!(matches!(
            dev.ornata_edition_readback(),
            Err(Error::Device {
                status: Status::NotSupported,
                ..
            })
        ));
        assert_eq!(state.lock().unwrap().requests.len(), 1);
        assert_eq!(state.lock().unwrap().reads, 1);
        for (mut response, length) in [
            (reply(2, &[6, 0, 9]), 91),
            (reply(3, &[6, 0, 9]), 89),
            (reply(3, &[6, 0, 9]), 91),
        ] {
            if response.0[5] == 3 && length == 91 {
                response.0[88] ^= 1;
            }
            let (dev, state) = fixture(response, length);
            assert!(matches!(
                dev.ornata_edition_readback(),
                Err(Error::NoReply(_))
            ));
            let state = state.lock().unwrap();
            assert_eq!(
                state.requests.len(),
                1,
                "no extra transaction or cache fallback"
            );
            assert_eq!(state.reads, 30, "existing bounded GET reads only");
        }
    }
}
