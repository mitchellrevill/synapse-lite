//! Own only the volatile CustomFrame effect; keyboard mode is never written.
use secbird_core::{proto::Command, Device, Error, Result};

pub(crate) struct Session {
    key: String,
    connection: u64,
    original: [u8; 80],
    meaningful_len: usize,
    owned: bool,
}

fn refused(message: &str) -> Error {
    Error::Unsupported(message.into())
}

impl Session {
    pub(crate) fn capture(device: &Device) -> Result<Self> {
        if device.is_query_only() || device.pid != 605 || device.interface != 2 {
            return Err(refused(
                "Normal lighting needs a writable Ornata interface2",
            ));
        }
        if device.ornata_mode_readback()?.mode() != 0 {
            return Err(refused("Normal lighting does not change keyboard mode"));
        }
        let effect = device.ornata_effect_readback(0, 0)?;
        if !effect.selectors_match() || effect.is_custom_frame() {
            return Err(refused(
                "An existing host animation cannot be captured for restoration",
            ));
        }
        Ok(Self {
            key: device.key(),
            connection: device.connection_id(),
            original: *effect.raw_args(),
            meaningful_len: 6 + usize::from(effect.color_count()) * 3,
            owned: false,
        })
    }

    pub(crate) fn matches(&self, device: &Device) -> bool {
        self.connection == device.connection_id() && self.key == device.key()
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn start(&mut self, device: &Device) -> Result<()> {
        if !self.matches(device) || device.ornata_mode_readback()?.mode() != 0 {
            return Err(refused(
                "Keyboard connection or mode changed before lighting setup",
            ));
        }
        let effect = device.ornata_effect_readback(0, 0)?;
        if effect.raw_args()[..self.meaningful_len] != self.original[..self.meaningful_len] {
            return Err(refused(
                "Lighting changed before setup; the external effect was retained",
            ));
        }
        // A failed ACK can still mean that firmware changed. Keep the baseline
        // before attempting SET so the caller can restore on every failure path.
        self.owned = true;
        device.transact(Command::new(15, 2), 6, &[0, 0, 8, 0, 0, 0])?;
        self.check(device)
    }

    pub(crate) fn check(&self, device: &Device) -> Result<()> {
        if !self.matches(device) || device.ornata_mode_readback()?.mode() != 0 {
            return Err(refused("Normal lighting connection or mode changed"));
        }
        let effect = device.ornata_effect_readback(0, 0)?;
        if !effect.selectors_match() || effect.raw_args()[..6] != [0, 0, 8, 0, 0, 0] {
            return Err(refused(
                "Normal lighting effect changed outside this session",
            ));
        }
        Ok(())
    }

    pub(crate) fn restore(&mut self, device: &Device) -> Result<()> {
        if !self.owned {
            return Ok(());
        }
        if !self.matches(device) || device.ornata_mode_readback()?.mode() != 0 {
            return Err(refused(
                "Lighting restoration requires the original connection in Normal mode",
            ));
        }
        let current = device.ornata_effect_readback(0, 0)?;
        if current.raw_args()[..self.meaningful_len] == self.original[..self.meaningful_len] {
            self.owned = false;
            return Ok(());
        }
        if !current.selectors_match() || current.raw_args()[..6] != [0, 0, 8, 0, 0, 0] {
            self.owned = false;
            return Err(refused(
                "Lighting ownership changed; the external effect was retained",
            ));
        }
        let acknowledgement = device.transact(Command::new(15, 2), 80, &self.original);
        let restored = device.ornata_effect_readback(0, 0)?;
        if restored.raw_args()[..self.meaningful_len] != self.original[..self.meaningful_len] {
            acknowledgement?;
            return Err(refused(
                "Original lighting effect failed restoration readback",
            ));
        }
        self.owned = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_static::test_support;

    #[test]
    fn normal_effect_setup_and_restoration_never_write_input_mode_or_brightness() {
        let (device, spy) = test_support::fixture();
        spy.state.lock().unwrap().mode = 0;
        spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
        let mut session = Session::capture(&device).unwrap();
        session.start(&device).unwrap();
        session.check(&device).unwrap();
        session.restore(&device).unwrap();
        assert_eq!(&spy.sim.state().effects[&0][..4], &[3, 0, 0, 0]);
        let commands = &spy.state.lock().unwrap().commands;
        assert!(commands
            .iter()
            .all(|command| command.id >= 0x80 || *command == Command::new(15, 2)));
    }

    #[test]
    fn external_effect_and_reopened_connection_are_never_overwritten() {
        let (device, spy) = test_support::fixture();
        spy.state.lock().unwrap().mode = 0;
        spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
        let mut session = Session::capture(&device).unwrap();
        session.start(&device).unwrap();
        spy.sim.state().effects.insert(0, vec![7, 0, 0, 0]);
        assert!(session.restore(&device).is_err());
        assert_eq!(&spy.sim.state().effects[&0][..4], &[7, 0, 0, 0]);
        let (replacement, _) = test_support::fixture();
        assert!(!session.matches(&replacement));
    }

    #[test]
    fn failed_setup_keeps_the_baseline_and_restoration_is_idempotent() {
        let (device, spy) = test_support::fixture();
        spy.state.lock().unwrap().mode = 0;
        spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
        let mut session = Session::capture(&device).unwrap();
        spy.state.lock().unwrap().fail_at = Some(0);
        assert!(session.start(&device).is_err());
        session.restore(&device).unwrap();
        let before = spy.state.lock().unwrap().commands.len();
        session.restore(&device).unwrap();
        assert_eq!(spy.state.lock().unwrap().commands.len(), before);
        assert_eq!(&spy.sim.state().effects[&0][..4], &[3, 0, 0, 0]);
    }
}
