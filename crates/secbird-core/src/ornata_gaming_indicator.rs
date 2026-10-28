//! Ornata's Gaming Mode indicator only. Shortcut blocking is a host workflow.
//! Shipped605 taskRunnerSetGameMode uses LED class0/id8, not GAME_MODE bits.
use crate::{commands, Device, Error, Result};

fn scope(device: &Device) -> Result<()> {
    if device.pid != 0x025d || device.interface != 2 {
        return Err(Error::Unsupported(
            "Gaming indicator requires Ornata V2 configuration interface2".into(),
        ));
    }
    Ok(())
}

fn decode(args: &[u8]) -> Result<bool> {
    match args {
        [0, 8, value @ 0..=1] => Ok(*value != 0),
        _ => Err(Error::Unsupported(
            "Gaming indicator reply does not match LED class0/id8 boolean state".into(),
        )),
    }
}

impl Device {
    /// Read the indicator. This is not a query of shortcut interception or
    /// effective Gaming Mode; a lit indicator cannot prove either is active.
    pub fn ornata_gaming_indicator(&self) -> Result<bool> {
        scope(self)?;
        let reply = self.get(commands::LED_STATE, 3, &[0, 8])?;
        decode(&reply.args()[..usize::from(reply.0[5])])
    }

    /// Explicit indicator-only change with readback. The caller owns restoration
    /// on error/cancellation; failed acknowledgement may still change hardware.
    /// Never called by the legacy GameMode/profile shortcut projection.
    pub fn set_ornata_gaming_indicator(&self, on: bool) -> Result<()> {
        scope(self)?;
        // Establish the exact supported GET contract before attempting a SET.
        if self.ornata_gaming_indicator()? == on {
            return Ok(());
        }
        self.transact(commands::LED_STATE, 3, &[0, 8, u8::from(on)])?;
        if self.ornata_gaming_indicator()? != on {
            return Err(Error::Unsupported(
                "Gaming indicator readback mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Sim, KEYBOARD_PID};
    fn fixture(pid: u16, interface: i32) -> (Sim, Device) {
        let sim = Sim::keyboard();
        sim.state().leds.push(8);
        let device = Device::open(
            Box::new(sim.clone()),
            pid,
            interface,
            std::ffi::CString::new("sim-gaming-indicator").unwrap(),
            &[0xff],
            Sim::spec_for(KEYBOARD_PID),
        )
        .unwrap();
        sim.state().log.clear();
        (sim, device)
    }
    #[test]
    fn indicator_roundtrip_uses_separate_led_command_without_changing_input_mode() {
        let (sim, device) = fixture(605, 2);
        assert!(!device.ornata_gaming_indicator().unwrap());
        device.set_ornata_gaming_indicator(true).unwrap();
        assert!(device.ornata_gaming_indicator().unwrap());
        device.set_ornata_gaming_indicator(false).unwrap();
        assert!(!device.ornata_gaming_indicator().unwrap());
        let state = sim.state();
        assert_eq!(state.device_mode, Some(0));
        assert_eq!(state.game_mode, Some(0));
        assert!(state
            .log
            .iter()
            .all(|entry| *entry == commands::LED_STATE || *entry == commands::LED_STATE.get()));
    }
    #[test]
    fn scope_and_query_only_gate_precede_indicator_writes() {
        for (pid, interface) in [(604, 2), (605, 0)] {
            let (sim, device) = fixture(pid, interface);
            assert!(device.ornata_gaming_indicator().is_err());
            assert!(device.set_ornata_gaming_indicator(true).is_err());
            assert!(sim.state().log.is_empty());
        }
        let (sim, mut device) = fixture(605, 2);
        device.enforce_query_only();
        assert!(device.set_ornata_gaming_indicator(true).is_err());
        assert!(sim
            .state()
            .log
            .iter()
            .all(|entry| *entry == commands::LED_STATE.get()));
        assert!(!sim.state().led_on.get(&8).copied().unwrap_or(false));
    }
    #[test]
    fn indicator_decoder_does_not_accept_wrong_led_unknown_state_or_padding() {
        assert_eq!(decode(&[0, 8, 0]).unwrap(), false);
        assert_eq!(decode(&[0, 8, 1]).unwrap(), true);
        for bytes in [
            &[0, 8][..],
            &[0, 5, 1],
            &[1, 8, 1],
            &[0, 8, 2],
            &[0, 8, 1, 0],
        ] {
            assert!(decode(bytes).is_err());
        }
    }
}
