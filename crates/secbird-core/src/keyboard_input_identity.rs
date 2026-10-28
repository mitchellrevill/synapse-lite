//! Passive metadata identities for the two current Ornata V2 keyboard endpoints.
//!
//! No registration, input reads, hooks, device handles or settings writes occur.
//! The selected PID025D configuration interface is MI02; Raw Input keyboard
//! endpoints MI00 and MI01 must share its active, nonzero Windows ContainerId.
//! This establishes candidate identity, not physical-key or notification routing.
//!
//! Validate at observer start, first matching record and end, not per packet.
//! Stop on any device notification and discard the set before rediscovery: a
//! Windows handle can be reused and metadata has no atomic hotplug snapshot.

#![cfg(windows)]

use crate::mouse_input_identity::{enumerate, raw_name, verify_raw_type};
use crate::sensor_rotation::{hid_identity, interface_container_id};
use crate::Device;
use std::io;

const ORNATA_PID: u16 = 0x025d;
const CONFIGURATION_MI: u8 = 2;
const KEYBOARD_TYPE: u32 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    pid: u16,
    interface: u8,
    container: [u8; 16],
}

impl Identity {
    fn for_path(path: &str) -> io::Result<Self> {
        let (pid, interface) = hid_identity(path)?;
        let container = interface_container_id(path)?;
        if container == [0; 16] {
            return Err(invalid("Keyboard container identity is zero"));
        }
        Ok(Self {
            pid,
            interface,
            container,
        })
    }

    fn selected_configuration(self) -> bool {
        self.pid == ORNATA_PID && self.interface == CONFIGURATION_MI && self.container != [0; 16]
    }

    fn keyboard_for(self, selected: Self) -> bool {
        selected.selected_configuration()
            && self.pid == ORNATA_PID
            && matches!(self.interface, 0 | 1)
            && self.container == selected.container
    }
}

/// Paths and container values stay private; intentionally no Debug or Serialize.
pub struct RawKeyboardBinding {
    handle: usize,
    raw_name: String,
    selected_hid_path: String,
    selected: Identity,
    keyboard: Identity,
}

impl RawKeyboardBinding {
    pub fn handle(&self) -> usize {
        self.handle
    }

    pub fn interface(&self) -> u8 {
        self.keyboard.interface
    }

    /// Requery type, full collection name and both active container identities.
    /// False means a reused/changed identity; OS failures must also stop the
    /// observer. Callers must not fall back to all-keyboard or PID-only capture.
    pub fn validate(&self) -> io::Result<bool> {
        if self.handle == 0 {
            return Ok(false);
        }
        verify_raw_type(self.handle, KEYBOARD_TYPE)?;
        let name = raw_name(self.handle)?;
        if !same_collection_name(&name, &self.raw_name) {
            return Ok(false);
        }
        let selected = Identity::for_path(&self.selected_hid_path)?;
        let keyboard = Identity::for_path(&name)?;
        Ok(selected == self.selected
            && keyboard == self.keyboard
            && keyboard.keyboard_for(selected))
    }
}

/// Exact endpoint bindings plus every keyboard handle in the initial snapshot.
/// The latter supports DEVNOTIFY initial-arrival accounting without broadening
/// the set of endpoints whose input records an observer may accept.
pub struct RawKeyboardSet {
    bindings: Vec<RawKeyboardBinding>,
    initial_handles: Vec<usize>,
}

impl RawKeyboardSet {
    pub fn bindings(&self) -> &[RawKeyboardBinding] {
        &self.bindings
    }

    pub fn initial_handles(&self) -> &[usize] {
        &self.initial_handles
    }
}

/// Enumerate metadata only, using the shared bounded native helpers: at most
/// 512 entries, three list attempts and 32768 UTF-16 units per collection name.
/// Exactly one MI00 and one MI01 endpoint must share the selected MI02 container.
pub fn discover(device: &Device) -> io::Result<RawKeyboardSet> {
    if device.pid != ORNATA_PID || device.interface != i32::from(CONFIGURATION_MI) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Keyboard identity is scoped to Ornata V2 PID025D configuration MI02",
        ));
    }
    let path = device
        .path
        .to_str()
        .map_err(|_| invalid("Selected HID path is not UTF-8"))?;
    let selected = Identity::for_path(path)?;
    if !selected.selected_configuration() {
        return Err(invalid(
            "Selected configuration path does not match the Device identity",
        ));
    }
    let mut initial_handles = Vec::new();
    let mut bindings = Vec::new();
    for entry in enumerate()?
        .into_iter()
        .filter(|entry| entry.kind == KEYBOARD_TYPE)
    {
        admit_initial_handle(&mut initial_handles, entry.handle)?;
        verify_raw_type(entry.handle, KEYBOARD_TYPE)?;
        let name = raw_name(entry.handle)?;
        let Ok((pid, interface)) = hid_identity(&name) else {
            continue;
        };
        if pid != ORNATA_PID || !matches!(interface, 0 | 1) {
            continue;
        }
        // An eligible endpoint whose container cannot be read is an error;
        // never silently omit a possible competing candidate.
        let keyboard = Identity::for_path(&name)?;
        if keyboard.keyboard_for(selected) {
            bindings.push(RawKeyboardBinding {
                handle: entry.handle,
                raw_name: name,
                selected_hid_path: path.to_owned(),
                selected,
                keyboard,
            });
        }
    }
    require_one_per_interface(&bindings)?;
    bindings.sort_by_key(RawKeyboardBinding::interface);
    for binding in &bindings {
        if !binding.validate()? {
            return Err(invalid("Keyboard identity changed during discovery"));
        }
    }
    Ok(RawKeyboardSet {
        bindings,
        initial_handles,
    })
}

fn admit_initial_handle(handles: &mut Vec<usize>, handle: usize) -> io::Result<()> {
    if handle == 0 || handles.contains(&handle) {
        return Err(invalid("Raw Input keyboard handles are null or repeated"));
    }
    handles.push(handle);
    Ok(())
}

fn require_one_per_interface(bindings: &[RawKeyboardBinding]) -> io::Result<()> {
    if bindings.len() != 2
        || bindings
            .iter()
            .filter(|binding| binding.interface() == 0)
            .count()
            != 1
        || bindings
            .iter()
            .filter(|binding| binding.interface() == 1)
            .count()
            != 1
    {
        return Err(io::Error::new(io::ErrorKind::NotFound,
            "Exactly one Ornata keyboard endpoint per MI00 and MI01 must match the selected container"));
    }
    Ok(())
}

fn same_collection_name(current: &str, saved: &str) -> bool {
    !current.is_empty() && current.eq_ignore_ascii_case(saved)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected() -> Identity {
        Identity {
            pid: ORNATA_PID,
            interface: CONFIGURATION_MI,
            container: [1; 16],
        }
    }

    fn binding(interface: u8) -> RawKeyboardBinding {
        RawKeyboardBinding {
            handle: usize::from(interface) + 1,
            raw_name: String::new(),
            selected_hid_path: String::new(),
            selected: selected(),
            keyboard: Identity {
                interface,
                ..selected()
            },
        }
    }

    #[test]
    fn only_keyboard_interfaces_of_the_exact_configuration_container_match() {
        let selected = selected();
        for interface in [0, 1] {
            let keyboard = Identity {
                interface,
                ..selected
            };
            assert!(keyboard.keyboard_for(selected));
            assert!(!Identity {
                container: [2; 16],
                ..keyboard
            }
            .keyboard_for(selected));
            assert!(!Identity {
                pid: 197,
                ..keyboard
            }
            .keyboard_for(selected));
            assert!(!keyboard.keyboard_for(Identity {
                interface: 0,
                ..selected
            }));
            assert!(!keyboard.keyboard_for(Identity {
                container: [0; 16],
                ..selected
            }));
        }
        for interface in [2, 3, 255] {
            assert!(!Identity {
                interface,
                ..selected
            }
            .keyboard_for(selected));
        }
    }

    #[test]
    fn missing_or_ambiguous_interfaces_fail_closed() {
        assert!(require_one_per_interface(&[]).is_err());
        assert!(require_one_per_interface(&[binding(0)]).is_err());
        assert!(require_one_per_interface(&[binding(0), binding(0)]).is_err());
        assert!(require_one_per_interface(&[binding(1), binding(1)]).is_err());
        assert!(require_one_per_interface(&[binding(0), binding(1), binding(1)]).is_err());
        assert!(require_one_per_interface(&[binding(0), binding(1)]).is_ok());
    }

    #[test]
    fn initial_snapshot_rejects_null_and_repeated_keyboard_handles() {
        let mut handles = Vec::new();
        assert!(admit_initial_handle(&mut handles, 0).is_err());
        admit_initial_handle(&mut handles, 10).unwrap();
        assert!(admit_initial_handle(&mut handles, 10).is_err());
        admit_initial_handle(&mut handles, 20).unwrap();
        assert_eq!(handles, [10, 20]);
    }

    #[test]
    fn collection_name_change_rejects_reused_handle() {
        assert!(same_collection_name(
            r"\\?\HID#VID_1532&PID_025D&MI_00#A",
            r"\\?\hid#vid_1532&pid_025d&mi_00#a"
        ));
        assert!(!same_collection_name(
            r"\\?\hid#vid_1532&pid_025d&mi_01#a",
            r"\\?\hid#vid_1532&pid_025d&mi_00#a"
        ));
        assert!(!same_collection_name("", ""));
        let mut zero = binding(0);
        zero.handle = 0;
        assert!(!zero.validate().unwrap());
    }
}
