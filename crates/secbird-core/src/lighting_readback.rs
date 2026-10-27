//! Read the inspected Ornata V2 firmware lighting registers without changing them.
//!
//! Source: `outputs/research/ornata-lighting-transport.md`, shipped lighting modules
//! 4001/5668/6411. GET 0F:82 declares 80 argument bytes; GET 0F:84 declares 3.
//! A custom-frame effect (8) does not capture a host animation or its ownership.
//! These snapshots deliberately provide no setter or restoration operation.

use crate::proto::{Command, Report, Status, MAX_ARGS};
use crate::{Device, Error, Result};

const ORNATA_V2: u16 = 0x025d;
const GET_EFFECT: Command = Command::new(0x0f, 0x82);
const GET_BRIGHTNESS: Command = Command::new(0x0f, 0x84);
const GET_MODE: Command = Command::new(0x00, 0x84);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingSelector {
    pub profile: u8,
    pub region: u8,
}

fn selector(profile: u8, region: u8) -> Result<LightingSelector> {
    if profile > 1 || region != 0 {
        return Err(Error::InvalidArg(
            "Ornata readback accepts only profile 0 or 1 and region 0".into(),
        ));
    }
    Ok(LightingSelector { profile, region })
}

fn validate_reply(reply: &Report, txid: u8, command: Command, minimum: u8) -> Result<()> {
    if !reply.valid_reply(txid, command, minimum) {
        return Err(Error::InvalidArg(format!(
            "Malformed {command} lighting reply"
        )));
    }
    if reply.status() != Status::Success {
        return Err(Error::Device {
            cmd: command,
            status: reply.status(),
        });
    }
    Ok(())
}

/// A firmware register snapshot, not a resumable host animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectReadback {
    pub requested: LightingSelector,
    pub returned: LightingSelector,
    raw_args: [u8; MAX_ARGS],
    declared_size: u8,
    color_count: u8,
}

impl EffectReadback {
    /// Decode a correlated, checksummed successful reply. Returned selectors are
    /// retained even when they differ: profile aliases are not established yet.
    pub fn from_reply(profile: u8, region: u8, txid: u8, reply: &Report) -> Result<Self> {
        let requested = selector(profile, region)?;
        validate_reply(reply, txid, GET_EFFECT, 6)?;
        let declared_size = reply.0[5];
        let raw_args: [u8; MAX_ARGS] = reply.args().try_into().expect("fixed report argument size");
        let color_count = raw_args[5];
        if usize::from(color_count) > (MAX_ARGS - 6) / 3
            || 6 + usize::from(color_count) * 3 > usize::from(declared_size)
        {
            return Err(Error::InvalidArg(
                "Lighting color count exceeds declared reply data".into(),
            ));
        }
        Ok(Self {
            requested,
            returned: LightingSelector {
                profile: raw_args[0],
                region: raw_args[1],
            },
            raw_args,
            declared_size,
            color_count,
        })
    }

    /// All 80 bytes, including padding outside the declared response length.
    pub fn raw_args(&self) -> &[u8; MAX_ARGS] {
        &self.raw_args
    }
    pub fn declared_size(&self) -> u8 {
        self.declared_size
    }
    pub fn effect_id(&self) -> u8 {
        self.raw_args[2]
    }
    pub fn flags(&self) -> u8 {
        self.raw_args[3]
    }
    pub fn rate(&self) -> u8 {
        self.raw_args[4]
    }
    pub fn color_count(&self) -> u8 {
        self.color_count
    }
    /// Only the declared colors, excluding unused padding.
    pub fn colors(&self) -> &[u8] {
        &self.raw_args[6..6 + usize::from(self.color_count) * 3]
    }
    pub fn selectors_match(&self) -> bool {
        self.requested == self.returned
    }
    /// This mode contains no host effect identity, animation phase or frame data.
    pub fn is_custom_frame(&self) -> bool {
        self.effect_id() == 8
    }
}

/// Keep the raw brightness byte: converting via a percentage loses precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrightnessReadback {
    pub requested: LightingSelector,
    pub returned: LightingSelector,
    raw_args: [u8; 3],
}

impl BrightnessReadback {
    pub fn from_reply(profile: u8, region: u8, txid: u8, reply: &Report) -> Result<Self> {
        let requested = selector(profile, region)?;
        validate_reply(reply, txid, GET_BRIGHTNESS, 3)?;
        if reply.0[5] != 3 {
            return Err(Error::InvalidArg(
                "Brightness reply must declare exactly 3 bytes".into(),
            ));
        }
        let raw_args = [reply.args()[0], reply.args()[1], reply.args()[2]];
        Ok(Self {
            requested,
            returned: LightingSelector {
                profile: raw_args[0],
                region: raw_args[1],
            },
            raw_args,
        })
    }
    pub fn raw_args(&self) -> &[u8; 3] {
        &self.raw_args
    }
    pub fn raw_brightness(&self) -> u8 {
        self.raw_args[2]
    }
    pub fn selectors_match(&self) -> bool {
        self.requested == self.returned
    }
}

/// Device mode from protocol25 GET00:84. Normal0 and Driver3 are source-backed
/// names; a normal-mode reply does not establish exclusive lighting ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceModeReadback {
    raw_args: [u8; MAX_ARGS],
    declared_size: u8,
}

impl DeviceModeReadback {
    pub fn from_reply(txid: u8, reply: &Report) -> Result<Self> {
        validate_reply(reply, txid, GET_MODE, 1)?;
        if !(1..=2).contains(&reply.0[5]) {
            return Err(Error::InvalidArg(
                "Device mode reply must declare 1 or 2 bytes".into(),
            ));
        }
        Ok(Self {
            raw_args: reply.args().try_into().expect("fixed report argument size"),
            declared_size: reply.0[5],
        })
    }
    pub fn raw_args(&self) -> &[u8; MAX_ARGS] {
        &self.raw_args
    }
    pub fn declared_size(&self) -> u8 {
        self.declared_size
    }
    pub fn declared_args(&self) -> &[u8] {
        &self.raw_args[..usize::from(self.declared_size)]
    }
    pub fn mode(&self) -> u8 {
        self.raw_args[0]
    }
    /// A one-byte response does not supply a parameter, even when padding is0.
    pub fn param(&self) -> Option<u8> {
        (self.declared_size == 2).then_some(self.raw_args[1])
    }
    pub fn source_name(&self) -> &'static str {
        match self.mode() {
            0 => "Normal_Mode",
            1 => "Bootloader_Mode",
            2 => "Testing_Mode",
            3 => "Driver_Mode",
            4 => "Download_Mode",
            5 => "Pairing_Mode",
            6 => "Standby_Mode",
            8 => "RF_Test_Mode",
            9 => "Retail_Mode",
            _ => "Unknown",
        }
    }
}

impl Device {
    /// Issue only GET00:84, size2, with no arguments. The returned parameter is
    /// retained without assuming a meaning or changing device mode.
    pub fn ornata_mode_readback(&self) -> Result<DeviceModeReadback> {
        self.check_ornata_lighting_query(0, 0)?;
        let reply = self.get_ornata_mode_register()?;
        DeviceModeReadback::from_reply(self.txid, &reply)
    }

    /// Issue only GET 0F:82. Profiles 0/1 and region 0 are the bounded inspected
    /// selectors; their relationship to current rendering remains unverified.
    /// The command-specific transport sends 80 argument bytes and accepts at
    /// least 6 response argument bytes. The installed-device trace confirms a
    /// correlated, checksummed 6-byte response; other commands keep their limits.
    pub fn ornata_effect_readback(&self, profile: u8, region: u8) -> Result<EffectReadback> {
        self.check_ornata_lighting_query(profile, region)?;
        let reply = self.get_lighting_effect_register(profile, region)?;
        EffectReadback::from_reply(profile, region, self.txid, &reply)
    }

    /// Issue only GET 0F:84 and retain the exact 3 raw argument bytes.
    pub fn ornata_brightness_readback(
        &self,
        profile: u8,
        region: u8,
    ) -> Result<BrightnessReadback> {
        self.check_ornata_lighting_query(profile, region)?;
        let reply = self.get(GET_BRIGHTNESS, 3, &[profile, region])?;
        BrightnessReadback::from_reply(profile, region, self.txid, &reply)
    }

    fn check_ornata_lighting_query(&self, profile: u8, region: u8) -> Result<()> {
        if self.pid != ORNATA_V2 || self.interface != 2 {
            return Err(Error::Unsupported(
                "Lighting readback is inspected only for Ornata V2 PID025D/interface2".into(),
            ));
        }
        selector(profile, region)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::checksum;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn reply(command: Command, size: u8, args: &[u8]) -> Report {
        let mut report = Report::request(0x1f, command, size, args);
        report.0[0] = 2;
        report
    }

    #[test]
    fn effect_keeps_all_padding_and_declared_colors() {
        let mut args = [0xa5; 80];
        args[..12].copy_from_slice(&[1, 0, 2, 2, 40, 2, 10, 20, 30, 40, 50, 60]);
        let effect = EffectReadback::from_reply(1, 0, 0x1f, &reply(GET_EFFECT, 12, &args)).unwrap();
        assert_eq!(effect.raw_args(), &args);
        assert_eq!(effect.declared_size(), 12);
        assert_eq!(effect.color_count(), 2);
        assert_eq!(effect.colors(), &[10, 20, 30, 40, 50, 60]);
        assert_eq!(
            (effect.effect_id(), effect.flags(), effect.rate()),
            (2, 2, 40)
        );
        assert!(effect.selectors_match());
    }

    #[test]
    fn returned_selectors_are_never_relabelled_as_requested() {
        let effect =
            EffectReadback::from_reply(0, 0, 0x1f, &reply(GET_EFFECT, 6, &[1, 5, 8, 0, 0, 0]))
                .unwrap();
        assert!(!effect.selectors_match());
        assert_eq!(
            effect.returned,
            LightingSelector {
                profile: 1,
                region: 5
            }
        );
        assert!(effect.is_custom_frame());
        assert!(effect.colors().is_empty());
    }

    #[test]
    fn rejects_impossible_or_truncated_color_counts() {
        for (size, count) in [(5, 0), (6, 1), (11, 2), (80, 25), (80, 255)] {
            assert!(EffectReadback::from_reply(
                1,
                0,
                0x1f,
                &reply(GET_EFFECT, size, &[1, 0, 1, 0, 0, count])
            )
            .is_err());
        }
        assert!(EffectReadback::from_reply(
            1,
            0,
            0x1f,
            &reply(GET_EFFECT, 80, &[1, 0, 1, 0, 0, 24])
        )
        .is_ok());
    }

    #[test]
    fn rejects_bad_status_framing_command_and_transaction() {
        let good = reply(GET_EFFECT, 80, &[1, 0, 3, 0, 0, 0]);
        for index in [0, 1, 2, 5, 6, 7, 88, 89] {
            let mut bad = good.clone();
            bad.0[index] ^= 0xff;
            // Recompute checksum to show that status/identity/length checks also matter.
            if index != 88 {
                bad.0[88] = checksum(&bad.0);
            }
            assert!(
                EffectReadback::from_reply(1, 0, 0x1f, &bad).is_err(),
                "byte {index}"
            );
        }
    }

    #[test]
    fn brightness_keeps_raw_byte_and_requires_exact_size() {
        let brightness =
            BrightnessReadback::from_reply(0, 0, 0x1f, &reply(GET_BRIGHTNESS, 3, &[1, 0, 128]))
                .unwrap();
        assert_eq!(brightness.raw_args(), &[1, 0, 128]);
        assert_eq!(brightness.raw_brightness(), 128);
        assert!(!brightness.selectors_match());
        for size in [0, 2, 4, 80] {
            assert!(BrightnessReadback::from_reply(
                1,
                0,
                0x1f,
                &reply(GET_BRIGHTNESS, size, &[1, 0, 128])
            )
            .is_err());
        }
    }

    #[test]
    fn device_mode_preserves_parameter_and_unknown_values() {
        for (mode, name) in [(0, "Normal_Mode"), (3, "Driver_Mode"), (255, "Unknown")] {
            let snapshot =
                DeviceModeReadback::from_reply(0x1f, &reply(GET_MODE, 2, &[mode, 7])).unwrap();
            assert_eq!(snapshot.declared_args(), &[mode, 7]);
            assert_eq!(snapshot.declared_size(), 2);
            assert_eq!(snapshot.mode(), mode);
            assert_eq!(snapshot.param(), Some(7));
            assert_eq!(snapshot.source_name(), name);
        }
    }

    #[test]
    fn device_mode_rejects_wrong_size_identity_and_status() {
        for size in [0, 3, 80] {
            assert!(DeviceModeReadback::from_reply(0x1f, &reply(GET_MODE, size, &[0, 0])).is_err());
        }
        assert!(DeviceModeReadback::from_reply(0x3f, &reply(GET_MODE, 2, &[0, 0])).is_err());
        assert!(
            DeviceModeReadback::from_reply(0x1f, &reply(GET_BRIGHTNESS, 3, &[0, 0, 0])).is_err()
        );
        let mut failed = reply(GET_MODE, 2, &[0, 0]);
        failed.0[0] = 5;
        assert!(DeviceModeReadback::from_reply(0x1f, &failed).is_err());
    }

    #[test]
    fn one_byte_mode_preserves_padding_without_inventing_parameter() {
        let mut args = [0xa5; MAX_ARGS];
        args[0] = 3;
        let snapshot = DeviceModeReadback::from_reply(0x1f, &reply(GET_MODE, 1, &args)).unwrap();
        assert_eq!(snapshot.raw_args(), &args);
        assert_eq!(snapshot.declared_args(), &[3]);
        assert_eq!(snapshot.declared_size(), 1);
        assert_eq!(snapshot.param(), None);
        assert_eq!(snapshot.source_name(), "Driver_Mode");
    }

    struct ProbeOnly(Arc<AtomicUsize>);
    impl crate::Transport for ProbeOnly {
        fn send_feature_report(&self, data: &[u8]) -> Result<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            assert_eq!(
                &data[7..9],
                &[0, 0x82],
                "only discovery's serial query is allowed"
            );
            Ok(())
        }
        fn get_feature_report(&self, buffer: &mut [u8]) -> Result<usize> {
            let report = reply(
                crate::commands::SERIAL.get(),
                crate::commands::SERIAL_SIZE,
                &[],
            );
            buffer[0] = 0;
            buffer[1..91].copy_from_slice(&report.0);
            Ok(91)
        }
        fn read_delay(&self) -> std::time::Duration {
            std::time::Duration::ZERO
        }
    }

    fn test_device(pid: u16) -> (Device, Arc<AtomicUsize>) {
        let sends = Arc::new(AtomicUsize::new(0));
        let device = Device::open(
            Box::new(ProbeOnly(sends.clone())),
            pid,
            2,
            std::ffi::CString::new("test-only").unwrap(),
            &[0x1f],
            None,
        )
        .unwrap();
        sends.store(0, Ordering::Relaxed);
        (device, sends)
    }

    #[test]
    fn unsupported_device_and_selectors_send_nothing() {
        for pid in [0x00c4, 0x00c5, 0xfff3] {
            let (device, sends) = test_device(pid);
            assert!(device.ornata_mode_readback().is_err());
            assert!(device.ornata_effect_readback(1, 0).is_err());
            assert!(device.ornata_brightness_readback(1, 0).is_err());
            assert_eq!(sends.load(Ordering::Relaxed), 0);
        }
        let (device, sends) = test_device(ORNATA_V2);
        for (profile, region) in [(2, 0), (255, 0), (1, 1), (0, 255)] {
            assert!(device.ornata_effect_readback(profile, region).is_err());
            assert!(device.ornata_brightness_readback(profile, region).is_err());
        }
        assert_eq!(sends.load(Ordering::Relaxed), 0);
        for interface in [-1, 0, 1, 3] {
            let (mut device, sends) = test_device(ORNATA_V2);
            device.interface = interface;
            assert!(device.ornata_effect_readback(1, 0).is_err());
            assert!(device.ornata_brightness_readback(1, 0).is_err());
            assert_eq!(sends.load(Ordering::Relaxed), 0);
            let (mut wrong_interface, sends) = test_device(ORNATA_V2);
            wrong_interface.interface = 0;
            assert!(wrong_interface.ornata_mode_readback().is_err());
            assert_eq!(sends.load(Ordering::Relaxed), 0);
        }
    }
}
