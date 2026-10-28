//! Exact passive Raw Input mouse identity. Metadata queries only: this module
//! never registers input, observes movement, opens control handles or issues SET.
//! Paths/containers remain private and deliberately implement neither Debug nor
//! Serialize. No new windows-sys features are needed; USER32 FFI is local.
//!
//! Lifetime: fully validate at observer start/first matching record. Do not run
//! SetupAPI per input packet. An observer may then compare hDevice locally only
//! while its DEVNOTIFY registration remains alive; on ANY device change stop,
//! discard this binding and discover/validate again. The full validation method
//! remains available and rejects changed names, disconnected interfaces and
//! reused handles. Windows provides no atomic metadata snapshot across hotplug.

#![cfg(windows)]

use crate::sensor_rotation::{hid_identity, interface_container_id};
use crate::Device;
use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::null_mut;

const RIM_TYPEMOUSE: u32 = 0;
const RIDI_DEVICENAME: u32 = 0x2000_0007;
const RIDI_DEVICEINFO: u32 = 0x2000_000b;
const UINT_ERROR: u32 = u32::MAX;
const MAX_DEVICES: u32 = 512;
const MAX_NAME_UNITS: u32 = 32_768;
const LIST_ATTEMPTS: usize = 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct DeviceListEntry {
    pub(crate) handle: usize,
    pub(crate) kind: u32,
}

// RID_DEVICE_INFO has a 24-byte union (keyboard is the largest member).
#[repr(C)]
struct DeviceInfo {
    size: u32,
    kind: u32,
    data: [u32; 6],
}

#[link(name = "user32")]
extern "system" {
    fn GetRawInputDeviceList(list: *mut DeviceListEntry, count: *mut u32, size: u32) -> u32;
    fn GetRawInputDeviceInfoW(
        handle: usize,
        command: u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> u32;
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn admit_count(count: u32) -> io::Result<usize> {
    if count > MAX_DEVICES {
        return Err(invalid(
            "Raw Input device count exceeds the512-device limit",
        ));
    }
    Ok(count as usize)
}

fn validate_entries(entries: &[DeviceListEntry]) -> io::Result<()> {
    if entries
        .iter()
        .any(|entry| entry.kind > 2 || (entry.kind == RIM_TYPEMOUSE && entry.handle == 0))
    {
        return Err(invalid(
            "Raw Input list returned an invalid type or null mouse handle",
        ));
    }
    Ok(())
}

pub(crate) fn enumerate() -> io::Result<Vec<DeviceListEntry>> {
    for _ in 0..LIST_ATTEMPTS {
        let mut required = 0;
        let sized = unsafe {
            GetRawInputDeviceList(
                null_mut(),
                &mut required,
                size_of::<DeviceListEntry>() as u32,
            )
        };
        if sized == UINT_ERROR {
            return Err(io::Error::last_os_error());
        }
        if sized != 0 {
            return Err(invalid("Unexpected Raw Input list sizing result"));
        }
        let length = admit_count(required)?;
        if length == 0 {
            return Ok(Vec::new());
        }
        let mut entries = vec![DeviceListEntry::default(); length];
        let mut capacity = required;
        let copied = unsafe {
            GetRawInputDeviceList(
                entries.as_mut_ptr(),
                &mut capacity,
                size_of::<DeviceListEntry>() as u32,
            )
        };
        if copied == UINT_ERROR {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(122) {
                continue;
            }
            return Err(error);
        }
        if copied > required || capacity > required {
            continue;
        }
        entries.truncate(copied as usize);
        validate_entries(&entries)?;
        return Ok(entries);
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "Raw Input device list changed during all three bounded attempts",
    ))
}

fn decode_name(units: &[u16]) -> io::Result<String> {
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| invalid("Raw Input name is not terminated"))?;
    if end == 0 {
        return Err(invalid("Raw Input mouse name is empty"));
    }
    String::from_utf16(&units[..end]).map_err(|_| invalid("Raw Input mouse name is invalid UTF-16"))
}

pub(crate) fn raw_name(handle: usize) -> io::Result<String> {
    if handle == 0 {
        return Err(invalid("Null Raw Input device handle"));
    }
    let mut required = 0;
    let sized =
        unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut required) };
    if sized == UINT_ERROR {
        return Err(io::Error::last_os_error());
    }
    if sized != 0 || !(1..=MAX_NAME_UNITS).contains(&required) {
        return Err(invalid(
            "Raw Input mouse name exceeds the bounded character limit",
        ));
    }
    // One initialized sentinel also tolerates APIs whose returned character
    // count excludes the terminator. Embedded malformed names still fail below.
    let mut name = vec![0_u16; required as usize + 1];
    let mut capacity = name.len() as u32;
    let copied = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICENAME,
            name.as_mut_ptr().cast(),
            &mut capacity,
        )
    };
    if copied == UINT_ERROR {
        return Err(io::Error::last_os_error());
    }
    if copied == 0 || copied > name.len() as u32 || capacity > name.len() as u32 {
        return Err(invalid(
            "Raw Input name changed size during its metadata query",
        ));
    }
    decode_name(&name)
}

fn verify_mouse_type(handle: usize) -> io::Result<()> {
    verify_raw_type(handle, RIM_TYPEMOUSE)
}

/// Metadata only; callers must independently reject null handles and bind names.
pub(crate) fn verify_raw_type(handle: usize, expected: u32) -> io::Result<()> {
    let mut info = DeviceInfo {
        size: size_of::<DeviceInfo>() as u32,
        kind: UINT_ERROR,
        data: [0; 6],
    };
    let mut bytes = size_of::<DeviceInfo>() as u32;
    let copied = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICEINFO,
            (&mut info as *mut DeviceInfo).cast(),
            &mut bytes,
        )
    };
    if copied == UINT_ERROR {
        return Err(io::Error::last_os_error());
    }
    if copied != size_of::<DeviceInfo>() as u32
        || bytes != size_of::<DeviceInfo>() as u32
        || info.size != size_of::<DeviceInfo>() as u32
        || info.kind != expected
    {
        return Err(invalid(
            "Raw Input handle no longer identifies the expected device type",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PhysicalIdentity {
    pid: u16,
    interface: u8,
    container: [u8; 16],
}

impl PhysicalIdentity {
    fn for_path(path: &str) -> io::Result<Self> {
        let (pid, interface) = hid_identity(path)?;
        let container = interface_container_id(path)?;
        if container == [0; 16] {
            return Err(invalid("Mouse container identity is zero"));
        }
        Ok(Self {
            pid,
            interface,
            container,
        })
    }
    fn matches(self, candidate: Self) -> bool {
        self.container != [0; 16] && self == candidate
    }
}

fn unique<T>(mut candidates: Vec<T>) -> io::Result<T> {
    if candidates.len() != 1 {
        return Err(io::Error::new(io::ErrorKind::NotFound,
            "Exactly one Raw Input mouse must match the selected active HID container, product and interface"));
    }
    Ok(candidates.remove(0))
}

/// Private path-bearing identity; intentionally not Debug or Serialize.
#[derive(Clone)]
pub struct RawMouseBinding {
    handle: usize,
    raw_name: String,
    selected_hid_path: String,
    identity: PhysicalIdentity,
}

impl RawMouseBinding {
    pub fn handle(&self) -> usize {
        self.handle
    }

    /// Full metadata revalidation for start/rebind, not every1kHz input packet.
    /// False rejects unrelated/reused handles. OS lookup errors must stop the
    /// observer rather than falling back to PID-only or all-mouse monitoring.
    pub fn matches_live_handle(&self, handle: usize) -> io::Result<bool> {
        if handle == 0 || handle != self.handle {
            return Ok(false);
        }
        verify_mouse_type(handle)?;
        let name = raw_name(handle)?;
        if !name.eq_ignore_ascii_case(&self.raw_name) {
            return Ok(false);
        }
        let selected = PhysicalIdentity::for_path(&self.selected_hid_path)?;
        let candidate = PhysicalIdentity::for_path(&name)?;
        Ok(self.identity.matches(selected) && self.identity.matches(candidate))
    }
}

/// Discover metadata only. Collection paths may differ: association requires
/// exact nonzero ContainerId plus VID1532/PID/interface, not path equality.
/// Unknown eligible identities, zero/multiple matches and inactive interfaces
/// fail closed. This does not call RegisterRawInputDevices or read input data.
pub fn discover(device: &Device) -> io::Result<RawMouseBinding> {
    if !matches!(device.pid, 196 | 197) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Advanced feedback identity is currently scoped to mouse196/197",
        ));
    }
    let path = device
        .path
        .to_str()
        .map_err(|_| invalid("Selected HID path is not UTF-8"))?;
    let selected = PhysicalIdentity::for_path(path)?;
    if selected.pid != device.pid || i32::from(selected.interface) != device.interface {
        return Err(invalid(
            "Selected HID path does not match the Device identity",
        ));
    }
    let mut matches = Vec::new();
    for entry in enumerate()?
        .into_iter()
        .filter(|entry| entry.kind == RIM_TYPEMOUSE)
    {
        verify_mouse_type(entry.handle)?;
        let name = raw_name(entry.handle)?;
        // Non-Razer names/other products are not eligible. An eligible candidate
        // whose active ContainerId cannot be established is an error, not an
        // absent competing device silently ignored during ambiguity checks.
        let Ok((pid, interface)) = hid_identity(&name) else {
            continue;
        };
        if pid != selected.pid || interface != selected.interface {
            continue;
        }
        let candidate = PhysicalIdentity::for_path(&name)?;
        if selected.matches(candidate) {
            matches.push(RawMouseBinding {
                handle: entry.handle,
                raw_name: name,
                selected_hid_path: path.to_owned(),
                identity: selected,
            });
        }
    }
    let binding = unique(matches)?;
    if !binding.matches_live_handle(binding.handle)? {
        return Err(invalid("Raw Input identity changed during discovery"));
    }
    Ok(binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn containers_and_interfaces_must_match_even_when_pid_matches() {
        let a = PhysicalIdentity {
            pid: 197,
            interface: 0,
            container: [1; 16],
        };
        assert!(a.matches(a));
        assert!(!a.matches(PhysicalIdentity {
            container: [2; 16],
            ..a
        }));
        assert!(!a.matches(PhysicalIdentity { interface: 1, ..a }));
        assert!(!a.matches(PhysicalIdentity { pid: 196, ..a }));
        let zero = PhysicalIdentity {
            container: [0; 16],
            ..a
        };
        assert!(!zero.matches(zero));
    }
    #[test]
    fn different_collection_names_can_share_an_identity() {
        let a = hid_identity(r"\\?\hid#vid_1532&pid_00c5&mi_00&col01#a#{guid}").unwrap();
        let b = hid_identity(r"\\?\hid#vid_1532&pid_00c5&mi_00&col02#b#{guid}").unwrap();
        assert_eq!(a, b);
    }
    #[test]
    fn ambiguity_empty_lists_and_name_allocation_are_bounded() {
        assert!(unique::<usize>(vec![]).is_err());
        assert!(unique(vec![1, 2]).is_err());
        assert_eq!(unique(vec![3]).unwrap(), 3);
        assert_eq!(admit_count(512).unwrap(), 512);
        assert!(admit_count(513).is_err());
        assert!(decode_name(&[1, 2]).is_err());
        assert!(decode_name(&[0]).is_err());
        assert!(decode_name(&[0xd800, 0]).is_err());
        assert_eq!(decode_name(&[65, 0]).unwrap(), "A");
    }
    #[test]
    fn unrelated_or_zero_handle_does_not_query_windows() {
        let binding = RawMouseBinding {
            handle: 7,
            raw_name: String::new(),
            selected_hid_path: String::new(),
            identity: PhysicalIdentity {
                pid: 197,
                interface: 0,
                container: [1; 16],
            },
        };
        assert!(!binding.matches_live_handle(0).unwrap());
        assert!(!binding.matches_live_handle(8).unwrap());
    }
    #[test]
    fn native_metadata_layout_matches_windows_abi() {
        assert_eq!(
            size_of::<DeviceListEntry>(),
            if size_of::<usize>() == 8 { 16 } else { 8 }
        );
        assert_eq!(size_of::<DeviceInfo>(), 32);
        assert_eq!(RIDI_DEVICENAME, 0x20000007);
        assert_eq!(RIDI_DEVICEINFO, 0x2000000b);
    }
    #[test]
    fn unknown_types_and_null_mouse_handles_are_rejected() {
        assert!(validate_entries(&[
            DeviceListEntry { handle: 1, kind: 0 },
            DeviceListEntry { handle: 2, kind: 1 },
            DeviceListEntry { handle: 3, kind: 2 }
        ])
        .is_ok());
        assert!(validate_entries(&[DeviceListEntry { handle: 0, kind: 0 }]).is_err());
        assert!(validate_entries(&[DeviceListEntry { handle: 1, kind: 3 }]).is_err());
    }
}
