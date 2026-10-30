//! Explicit source startup-mode lease. Not a complete input ownership lease.
//! Callers coordinate competing owners and physical held-key transitions.
use crate::{commands, Device, Error, Result};

const DRIVER: [u8; 2] = [3, 0];
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Observation {
    pub mode: u8,
    pub parameter: Option<u8>,
}
trait Port {
    fn read(&mut self) -> Result<Observation>;
    fn write(&mut self, value: [u8; 2]) -> Result<()>;
}
struct DevicePort<'a>(&'a Device);
impl Port for DevicePort<'_> {
    fn read(&mut self) -> Result<Observation> {
        let value = self.0.ornata_mode_readback()?;
        Ok(Observation {
            mode: value.mode(),
            parameter: value.param(),
        })
    }
    fn write(&mut self, value: [u8; 2]) -> Result<()> {
        self.0
            .transact(commands::DEVICE_MODE, commands::DEVICE_MODE_SIZE, &value)?;
        Ok(())
    }
}
fn failure(message: &str) -> Error {
    Error::Unsupported(message.into())
}

struct Lease<I: Port> {
    io: I,
    baseline: Observation,
    source_zero: bool,
    owned: bool,
    sets: usize,
}
impl<I: Port> Lease<I> {
    fn open(io: I) -> Result<Self> {
        Self::open_policy(io, false)
    }
    fn open_policy(mut io: I, source_zero: bool) -> Result<Self> {
        let baseline = io.read()?;
        if !matches!(baseline.mode, 0 | 3)
            || baseline
                .parameter
                .map_or(!source_zero, |parameter| parameter != 0)
        {
            return Err(failure(
                "Only observed Normal0/0 and Driver3/0 modes are admitted",
            ));
        }
        Ok(Self {
            io,
            baseline,
            source_zero,
            owned: false,
            sets: 0,
        })
    }
    fn matches(&self, current: Observation, mode: u8) -> bool {
        current.mode == mode
            && current
                .parameter
                .map_or(self.source_zero, |parameter| parameter == 0)
    }
    fn enable(&mut self) -> Result<()> {
        let current = self.io.read()?;
        let expected = if self.owned { 3 } else { self.baseline.mode };
        if !self.matches(current, expected) {
            return Err(failure("Device mode changed outside this lease"));
        }
        if current.mode == 3 {
            return Ok(());
        }
        // Failed acknowledgement may still have changed hardware. Retain intent
        // before SET so the explicit/Drop recovery can inspect actual state.
        self.owned = true;
        self.sets += 1;
        self.io.write(DRIVER)?;
        let readback = self.io.read()?;
        if !self.matches(readback, 3) {
            return Err(failure("Driver mode readback mismatch"));
        }
        Ok(())
    }
    fn restore(&mut self) -> Result<()> {
        if !self.owned {
            return Ok(());
        }
        let current = self.io.read()?;
        if self.matches(current, self.baseline.mode) {
            self.owned = false;
            return Ok(());
        }
        if !self.matches(current, 3) {
            self.owned = false;
            return Err(failure(
                "Device mode ownership lost; external mode retained",
            ));
        }
        self.sets += 1;
        self.io.write([self.baseline.mode, 0])?;
        let readback = self.io.read()?;
        if !self.matches(readback, self.baseline.mode) {
            return Err(failure("Original mode restoration failed readback"));
        }
        self.owned = false;
        Ok(())
    }
}
impl<I: Port> Drop for Lease<I> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Snapshot/enter/restore for source605's explicit startup register change.
/// Opening only reads. No profile, lighting frame, brightness, selector or key
/// is written. This does not establish that mode3 changes keyboard report paths.
pub struct ModeLease<'a> {
    lease: Lease<DevicePort<'a>>,
}
impl<'a> ModeLease<'a> {
    pub fn open(device: &'a Device) -> Result<Self> {
        Self::open_policy(device, false)
    }
    /// Explicit stock-contract experiment: parameter0 is the source's command
    /// argument when firmware omits its readback. It remains None in observations
    /// and exact restoration of an unreported parameter is not claimed.
    pub fn open_source_startup(device: &'a Device) -> Result<Self> {
        Self::open_policy(device, true)
    }
    fn open_policy(device: &'a Device, source_zero: bool) -> Result<Self> {
        if device.pid != 0x025d || device.interface != 2 || device.is_query_only() {
            return Err(failure(
                "Mode lease requires an explicitly writable Ornata interface2",
            ));
        }
        Ok(Self {
            lease: if source_zero {
                Lease::open_policy(DevicePort(device), true)?
            } else {
                Lease::open(DevicePort(device))?
            },
        })
    }
    pub fn baseline(&self) -> Observation {
        self.lease.baseline
    }
    pub fn enable_driver(&mut self) -> Result<()> {
        self.lease.enable()
    }
    pub fn current(&mut self) -> Result<Observation> {
        self.lease.io.read()
    }
    pub fn restore(&mut self) -> Result<()> {
        self.lease.restore()
    }
    pub fn set_attempts(&self) -> usize {
        self.lease.sets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    #[derive(Default)]
    struct State {
        value: [u8; 2],
        writes: Vec<[u8; 2]>,
        fail_after_write: bool,
        unreported_parameter: bool,
    }
    struct Mock(Rc<RefCell<State>>);
    impl Port for Mock {
        fn read(&mut self) -> Result<Observation> {
            let state = self.0.borrow();
            Ok(Observation {
                mode: state.value[0],
                parameter: (!state.unreported_parameter).then_some(state.value[1]),
            })
        }
        fn write(&mut self, value: [u8; 2]) -> Result<()> {
            let mut state = self.0.borrow_mut();
            state.value = value;
            state.writes.push(value);
            if std::mem::take(&mut state.fail_after_write) {
                Err(failure("ack lost"))
            } else {
                Ok(())
            }
        }
    }
    fn fixture(value: [u8; 2]) -> (Rc<RefCell<State>>, Lease<Mock>) {
        let state = Rc::new(RefCell::new(State {
            value,
            ..Default::default()
        }));
        let lease = Lease::open(Mock(state.clone())).unwrap();
        (state, lease)
    }
    #[test]
    fn source_startup_and_exact_restore_are_read_back_and_idempotent() {
        let (state, mut lease) = fixture([0, 0]);
        assert!(state.borrow().writes.is_empty());
        lease.enable().unwrap();
        lease.enable().unwrap();
        assert_eq!(state.borrow().writes, [[3, 0]]);
        lease.restore().unwrap();
        lease.restore().unwrap();
        drop(lease);
        assert_eq!(state.borrow().writes, [[3, 0], [0, 0]]);
        assert_eq!(state.borrow().value, [0, 0]);
    }
    #[test]
    fn already_driver_and_unobserved_modes_never_write() {
        let (state, mut lease) = fixture([3, 0]);
        lease.enable().unwrap();
        lease.restore().unwrap();
        drop(lease);
        assert!(state.borrow().writes.is_empty());
        for value in [[0, 7], [3, 1], [1, 0], [2, 0], [4, 0], [255, 255]] {
            let state = Rc::new(RefCell::new(State {
                value,
                ..Default::default()
            }));
            assert!(Lease::open(Mock(state.clone())).is_err());
            assert!(state.borrow().writes.is_empty());
        }
    }
    #[test]
    fn external_changes_are_preserved_on_entry_and_recovery() {
        let (state, mut lease) = fixture([0, 0]);
        state.borrow_mut().value = [2, 0];
        assert!(lease.enable().is_err());
        assert!(state.borrow().writes.is_empty());
        let (state, mut lease) = fixture([0, 0]);
        lease.enable().unwrap();
        state.borrow_mut().value = [3, 7];
        assert!(lease.restore().is_err());
        drop(lease);
        assert_eq!(state.borrow().value, [3, 7]);
        assert_eq!(state.borrow().writes, [[3, 0]]);
    }
    #[test]
    fn ambiguous_failed_enable_recovers_and_drop_restores_owned_mode() {
        let (state, mut lease) = fixture([0, 0]);
        state.borrow_mut().fail_after_write = true;
        assert!(lease.enable().is_err());
        lease.restore().unwrap();
        assert_eq!(state.borrow().value, [0, 0]);
        let (state, mut lease) = fixture([0, 0]);
        lease.enable().unwrap();
        drop(lease);
        assert_eq!(state.borrow().value, [0, 0]);
    }
    #[test]
    fn query_only_and_wrong_devices_are_rejected_without_mode_commands() {
        let sim = crate::sim::Sim::keyboard();
        let mut device = sim
            .open(605, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        device.pid = 0x025d;
        device.interface = 2;
        device.enforce_query_only();
        let before = sim.state().log.len();
        assert!(ModeLease::open(&device).is_err());
        assert_eq!(sim.state().log.len(), before);
        for (pid, interface) in [(0x025d, 0), (0x00c4, 2)] {
            let mut device = sim
                .open(605, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
                .unwrap();
            device.pid = pid;
            device.interface = interface;
            let before = sim.state().log.len();
            assert!(ModeLease::open(&device).is_err());
            assert_eq!(sim.state().log.len(), before);
        }
    }

    #[test]
    fn unreported_parameter_requires_explicit_source_policy_and_stays_unobserved() {
        let state = Rc::new(RefCell::new(State {
            value: [0, 0],
            unreported_parameter: true,
            ..Default::default()
        }));
        assert!(Lease::open(Mock(state.clone())).is_err());
        assert!(state.borrow().writes.is_empty());
        let mut lease = Lease::open_policy(Mock(state.clone()), true).unwrap();
        assert_eq!(
            lease.baseline,
            Observation {
                mode: 0,
                parameter: None
            }
        );
        lease.enable().unwrap();
        assert_eq!(
            lease.io.read().unwrap(),
            Observation {
                mode: 3,
                parameter: None
            }
        );
        lease.restore().unwrap();
        assert_eq!(state.borrow().writes, [[3, 0], [0, 0]]);
        assert_eq!(
            lease.io.read().unwrap(),
            Observation {
                mode: 0,
                parameter: None
            }
        );
    }
}
