//! Source-backed rotation through the already installed Razer mouse filter.
//!
//! Windows only. No hooks, DLL initialization, redirection or driver installation.
//! The matching PID00C5 filter handles 0x88883180/184 at RVA0x78C0;
//! mapping_engine.dll calls them at RVA0x114130/0x113DEA. See validation manifest.
//! Handles and request buffers are synchronous and owned until completion. A
//! broken kernel driver can block: use a supervised worker when a deadline is
//! required. Drop attempts ownership-aware recovery, but callers should call
//! restore() explicitly to observe errors. A crash cannot run recovery.


use crate::device::Device;
use crate::driver_control::{enumerate_control_interfaces, ControlInterface};
use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::{null, null_mut};

const GET_ROTATION: u32 = 0x8888_3184;
const SET_ROTATION: u32 = 0x8888_3180;
const MAX_PATH_UNITS: usize = 32_768;
const MAX_DETAIL_BYTES: u32 = 65_536;
type Handle = *mut c_void;
const INVALID_HANDLE: Handle = -1_isize as Handle;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

impl Guid {
    fn is_zero(self) -> bool {
        self == Self {
            a: 0,
            b: 0,
            c: 0,
            d: [0; 8],
        }
    }
}

#[repr(C)]
struct PropertyKey {
    guid: Guid,
    id: u32,
}

// DEVPKEY_Device_ContainerId; DEVPROP_TYPE_GUID = 0x0D.
const CONTAINER_ID: PropertyKey = PropertyKey {
    guid: Guid {
        a: 0x8c7ed206,
        b: 0x3f8a,
        c: 0x4827,
        d: [0xb3, 0xab, 0xae, 0x9e, 0x1f, 0xae, 0xfc, 0x6c],
    },
    id: 2,
};

#[repr(C)]
struct InterfaceData {
    size: u32,
    guid: Guid,
    flags: u32,
    reserved: usize,
}

#[repr(C)]
struct DeviceInfoData {
    size: u32,
    guid: Guid,
    devinst: u32,
    reserved: usize,
}

#[link(name = "setupapi")]
extern "system" {
    fn SetupDiCreateDeviceInfoList(class: *const Guid, parent: Handle) -> Handle;
    fn SetupDiOpenDeviceInterfaceW(
        set: Handle,
        path: *const u16,
        flags: u32,
        interface: *mut InterfaceData,
    ) -> i32;
    fn SetupDiGetDeviceInterfaceDetailW(
        set: Handle,
        interface: *const InterfaceData,
        detail: *mut c_void,
        bytes: u32,
        required: *mut u32,
        device: *mut c_void,
    ) -> i32;
    fn SetupDiGetDevicePropertyW(
        set: Handle,
        device: *mut DeviceInfoData,
        key: *const PropertyKey,
        property_type: *mut u32,
        data: *mut u8,
        bytes: u32,
        required: *mut u32,
        flags: u32,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(set: Handle) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        path: *const u16,
        access: u32,
        sharing: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn DeviceIoControl(
        handle: Handle,
        code: u32,
        input: *const c_void,
        input_bytes: u32,
        output: *mut c_void,
        output_bytes: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn conflict(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, message)
}

fn wide_path(path: &str) -> io::Result<Vec<u16>> {
    if path.is_empty() || path.contains('\0') {
        return Err(invalid("Empty or embedded-NUL interface path"));
    }
    let units: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    if units.len() > MAX_PATH_UNITS {
        return Err(invalid("Interface path exceeds the bounded path limit"));
    }
    Ok(units)
}

struct DeviceSet(Handle);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

/// Resolve a registered, currently active interface to its Windows device
/// container. No device handle or IOCTL is used by this function.
fn container_for_path(path: &str) -> io::Result<Guid> {
    let path = wide_path(path)?;
    let raw = unsafe { SetupDiCreateDeviceInfoList(null(), null_mut()) };
    if raw == INVALID_HANDLE {
        return Err(io::Error::last_os_error());
    }
    let set = DeviceSet(raw);
    let zero = Guid {
        a: 0,
        b: 0,
        c: 0,
        d: [0; 8],
    };
    let mut interface = InterfaceData {
        size: size_of::<InterfaceData>() as u32,
        guid: zero,
        flags: 0,
        reserved: 0,
    };
    if unsafe { SetupDiOpenDeviceInterfaceW(set.0, path.as_ptr(), 0, &mut interface) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SPINT_ACTIVE: do not associate retained paths for disconnected devices.
    if interface.flags & 1 == 0 {
        return Err(invalid("Interface is not currently active"));
    }
    let mut bytes = 0;
    let sized = unsafe {
        SetupDiGetDeviceInterfaceDetailW(set.0, &interface, null_mut(), 0, &mut bytes, null_mut())
    };
    if sized != 0 || io::Error::last_os_error().raw_os_error() != Some(122) {
        return Err(invalid("Unexpected interface-detail sizing result"));
    }
    if !(6..=MAX_DETAIL_BYTES).contains(&bytes) {
        return Err(invalid(
            "Interface detail exceeds the bounded descriptor limit",
        ));
    }
    let mut detail = vec![0_u32; (bytes as usize).div_ceil(4)];
    detail[0] = if size_of::<usize>() == 8 { 8 } else { 6 };
    let mut device = DeviceInfoData {
        size: size_of::<DeviceInfoData>() as u32,
        guid: zero,
        devinst: 0,
        reserved: 0,
    };
    if unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            detail.as_mut_ptr().cast(),
            bytes,
            null_mut(),
            (&mut device as *mut DeviceInfoData).cast(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut container = zero;
    let mut property_type = 0;
    let mut returned = 0;
    if unsafe {
        SetupDiGetDevicePropertyW(
            set.0,
            &mut device,
            &CONTAINER_ID,
            &mut property_type,
            (&mut container as *mut Guid).cast(),
            size_of::<Guid>() as u32,
            &mut returned,
            0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if property_type != 0x0d || returned != 16 || container.is_zero() {
        return Err(invalid(
            "Device has no exact nonzero GUID container identity",
        ));
    }
    Ok(container)
}

pub(crate) fn hid_identity(path: &str) -> io::Result<(u16, u8)> {
    let lower = path.to_ascii_lowercase();
    let identity = lower
        .strip_prefix(r"\\?\hid#")
        .and_then(|value| value.split('#').next())
        .ok_or_else(|| invalid("Device path is not a HID interface"))?;
    let parts: Vec<_> = identity.split('&').collect();
    if parts.first() != Some(&"vid_1532") {
        return Err(invalid("HID interface is not a Razer device"));
    }
    let values = |prefix: &str| -> io::Result<&str> {
        let matches: Vec<_> = parts
            .iter()
            .filter_map(|part| part.strip_prefix(prefix))
            .collect();
        if matches.len() != 1 {
            return Err(invalid("Ambiguous HID identity fields"));
        }
        Ok(matches[0])
    };
    let pid = values("pid_")?;
    let mi = values("mi_")?;
    if pid.len() != 4 || mi.len() != 2 {
        return Err(invalid("Malformed HID product/interface identity"));
    }
    Ok((
        u16::from_str_radix(pid, 16).map_err(|_| invalid("Invalid HID product"))?,
        u8::from_str_radix(mi, 16).map_err(|_| invalid("Invalid HID interface"))?,
    ))
}

/// Equality-only Windows container identity for sibling HID interfaces. The
/// representation is private to the core and is never a profile/device key.
pub(crate) fn interface_container_id(path: &str) -> io::Result<[u8; 16]> {
    let guid = container_for_path(path)?;
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&guid.a.to_le_bytes());
    bytes[4..6].copy_from_slice(&guid.b.to_le_bytes());
    bytes[6..8].copy_from_slice(&guid.c.to_le_bytes());
    bytes[8..].copy_from_slice(&guid.d);
    Ok(bytes)
}

fn unique_match<T>(mut candidates: Vec<T>) -> io::Result<T> {
    if candidates.len() != 1 {
        return Err(conflict("Rotation requires exactly one control interface for this HID container, product and interface"));
    }
    Ok(candidates.remove(0))
}

fn exact_association(hid: (u16, u8, Guid), control: (u16, u8, Guid)) -> bool {
    !hid.2.is_zero() && hid == control
}

/// An association established through SetupAPI, never an arbitrary device path.
#[derive(Debug, Clone)]
pub struct RotationTarget {
    interface: ControlInterface,
    hid_path: String,
    container: Guid,
    query_only: bool,
}

impl RotationTarget {
    pub fn control_path(&self) -> &str {
        self.interface.path()
    }
    pub fn hid_path(&self) -> &str {
        &self.hid_path
    }
}

/// Discover one exact matching endpoint without opening it or issuing IOCTLs.
/// Only PID00C5 is admitted: its Rotation handler was inspected directly.
pub fn discover_for_device(device: &Device) -> io::Result<RotationTarget> {
    if device.pid != 0x00c5 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Software Rotation handler is verified only for the installed PID00C5 filter",
        ));
    }
    let path = device
        .path
        .to_str()
        .map_err(|_| invalid("HID path is not UTF-8"))?;
    let (pid, mi) = hid_identity(path)?;
    if pid != device.pid || i32::from(mi) != device.interface {
        return Err(invalid(
            "HID path does not match the selected Device identity",
        ));
    }
    let container = container_for_path(path)?;
    let mut matches = Vec::new();
    for interface in enumerate_control_interfaces()? {
        if interface.product_id() == pid && interface.interface_number() == mi {
            // Failure resolving an otherwise eligible candidate is not absence:
            // refuse the association rather than silently ignoring ambiguity.
            if exact_association(
                (pid, mi, container),
                (
                    interface.product_id(),
                    interface.interface_number(),
                    container_for_path(interface.path())?,
                ),
            ) {
                matches.push(interface);
            }
        }
    }
    Ok(RotationTarget {
        interface: unique_match(matches)?,
        hid_path: path.to_owned(),
        container,
        query_only: device.is_query_only(),
    })
}

trait RotationIo {
    fn get(&mut self) -> io::Result<u32>;
    fn set(&mut self, angle: u32) -> io::Result<()>;
}

struct DriverIo(Handle);
// SAFETY: this handle is exclusively owned, has no thread-affine callbacks or
// overlapped requests, and each operation retains its buffers synchronously.
// Moving ownership between worker threads is safe; sharing concurrent access is
// deliberately not provided (no Sync implementation).
unsafe impl Send for DriverIo {}
impl Drop for DriverIo {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

impl RotationIo for DriverIo {
    fn get(&mut self) -> io::Result<u32> {
        let mut output = [0_u8; 4];
        let mut returned = 0;
        let success = unsafe {
            DeviceIoControl(
                self.0,
                GET_ROTATION,
                null(),
                0,
                output.as_mut_ptr().cast(),
                4,
                &mut returned,
                null_mut(),
            )
        };
        if success == 0 {
            return Err(io::Error::last_os_error());
        }
        if returned != 4 {
            return Err(invalid("Rotation GET must return exactly four bytes"));
        }
        Ok(u32::from_le_bytes(output))
    }
    fn set(&mut self, angle: u32) -> io::Result<()> {
        let input = angle.to_le_bytes();
        let mut returned = 0;
        let success = unsafe {
            DeviceIoControl(
                self.0,
                SET_ROTATION,
                input.as_ptr().cast(),
                4,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        };
        if success == 0 {
            return Err(io::Error::last_os_error());
        }
        if returned != 0 {
            return Err(invalid("Rotation SET unexpectedly returned output"));
        }
        Ok(())
    }
}

fn source_angle(degrees: i32) -> io::Result<u32> {
    if !(-44..=44).contains(&degrees) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Rotation must be -44 to 44 degrees",
        ));
    }
    Ok(degrees.rem_euclid(360) as u32)
}

struct Lease<I: RotationIo> {
    io: I,
    baseline: u32,
    owned: Option<u32>,
    query_only: bool,
}

impl<I: RotationIo> Lease<I> {
    fn new(mut io: I, query_only: bool) -> io::Result<Self> {
        let baseline = io.get()?;
        Ok(Self {
            io,
            baseline,
            owned: None,
            query_only,
        })
    }
    fn set_degrees(&mut self, degrees: i32) -> io::Result<()> {
        let desired = source_angle(degrees)?;
        if self.query_only {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Read-only device refuses Rotation SET",
            ));
        }
        let current = self.io.get()?;
        if current != self.owned.unwrap_or(self.baseline) {
            self.owned = None;
            return Err(conflict(
                "Rotation changed outside this lease; external value preserved",
            ));
        }
        if current == desired {
            return Ok(());
        }
        // Record intent before SET: a failed call may still have changed state.
        // Recovery only writes when a later GET equals this exact intended value.
        self.owned = Some(desired);
        self.io.set(desired)?;
        if self.io.get()? != desired {
            return Err(invalid(
                "Rotation SET readback does not match requested angle",
            ));
        }
        Ok(())
    }
    fn restore(&mut self) -> io::Result<()> {
        let Some(owned) = self.owned else {
            return Ok(());
        };
        let current = self.io.get()?;
        if current == self.baseline {
            self.owned = None;
            return Ok(());
        }
        if current != owned {
            self.owned = None;
            return Err(conflict(
                "Rotation ownership lost; external value preserved during recovery",
            ));
        }
        self.io.set(self.baseline)?;
        if self.io.get()? != self.baseline {
            return Err(invalid("Rotation baseline restoration failed readback"));
        }
        self.owned = None;
        Ok(())
    }
}

impl<I: RotationIo> Drop for Lease<I> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Owns a verified vendor-filter endpoint and its original exact angle.
/// Changing angle is explicit; opening and reading do not change device state.
pub struct SensorRotation {
    lease: Lease<DriverIo>,
    target: RotationTarget,
}

/// GET-only reader. It has no lease, setter, restore method or restoring Drop;
/// destruction only closes the owned synchronous handle.
pub struct SensorRotationReader {
    io: DriverIo,
    target: RotationTarget,
}

impl SensorRotationReader {
    pub fn open_for_device(device: &Device) -> io::Result<Self> {
        if !device.is_query_only() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Rotation reader requires an explicitly query-only Device",
            ));
        }
        let target = discover_for_device(device)?;
        let io = open_target(&target)?;
        Ok(Self { io, target })
    }
    pub fn angle(&mut self) -> io::Result<u32> {
        self.io.get()
    }
    pub fn target(&self) -> &RotationTarget {
        &self.target
    }
}

fn open_target(target: &RotationTarget) -> io::Result<DriverIo> {
    if container_for_path(&target.hid_path)? != target.container
        || container_for_path(target.interface.path())? != target.container
    {
        return Err(conflict(
            "Device container changed during Rotation discovery",
        ));
    }
    let path = wide_path(target.interface.path())?;
    let handle = unsafe { CreateFileW(path.as_ptr(), 0, 3, null(), 3, 0, null_mut()) };
    if handle == INVALID_HANDLE {
        return Err(io::Error::last_os_error());
    }
    Ok(DriverIo(handle))
}

impl SensorRotation {
    pub fn open_for_device(device: &Device) -> io::Result<Self> {
        let target = discover_for_device(device)?;
        // Re-resolve immediately before opening; a cached target never permits
        // an arbitrary path or replacement device to bypass association checks.
        let io = open_target(&target)?;
        // Once open, the handle keeps its original device object even if the
        // registered path is subsequently reused by another device.
        let lease = Lease::new(io, target.query_only)?;
        Ok(Self { lease, target })
    }
    pub fn target(&self) -> &RotationTarget {
        &self.target
    }
    /// Raw stored degrees, preserving even values outside the UI's range.
    pub fn angle(&mut self) -> io::Result<u32> {
        self.lease.io.get()
    }
    pub fn baseline_angle(&self) -> u32 {
        self.lease.baseline
    }
    /// Source UI range; zero explicitly disables rotation in the vendor filter.
    pub fn set_degrees(&mut self, degrees: i32) -> io::Result<()> {
        self.lease.set_degrees(degrees)
    }
    /// Restore exactly the original raw value, only while still owning state.
    pub fn restore(&mut self) -> io::Result<()> {
        self.lease.restore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct State {
        value: u32,
        writes: Vec<u32>,
        ignore_write: bool,
        fail_set_after_change: bool,
    }
    struct Mock(Rc<RefCell<State>>);
    impl RotationIo for Mock {
        fn get(&mut self) -> io::Result<u32> {
            Ok(self.0.borrow().value)
        }
        fn set(&mut self, angle: u32) -> io::Result<()> {
            let mut state = self.0.borrow_mut();
            state.writes.push(angle);
            if !state.ignore_write {
                state.value = angle;
            }
            if state.fail_set_after_change {
                state.fail_set_after_change = false;
                return Err(io::Error::other("ambiguous completion"));
            }
            Ok(())
        }
    }
    fn fixture(value: u32) -> (Lease<Mock>, Rc<RefCell<State>>) {
        let state = Rc::new(RefCell::new(State {
            value,
            ..State::default()
        }));
        (Lease::new(Mock(state.clone()), false).unwrap(), state)
    }
    #[test]
    fn source_range_and_wire_bytes() {
        assert_eq!(source_angle(-44).unwrap().to_le_bytes(), [60, 1, 0, 0]);
        assert_eq!(source_angle(44).unwrap().to_le_bytes(), [44, 0, 0, 0]);
        assert!(source_angle(-45).is_err());
        assert!(source_angle(45).is_err());
        assert_eq!(GET_ROTATION, 0x88883184);
        assert_eq!(SET_ROTATION, 0x88883180);
    }
    #[test]
    fn exact_hid_identity_and_ambiguity() {
        assert_eq!(
            hid_identity(r"\\?\hid#vid_1532&pid_00c5&mi_00&col01#a#{guid}").unwrap(),
            (0xc5, 0)
        );
        assert!(hid_identity(r"\\?\hid#vid_0001&pid_00c5&mi_00#a").is_err());
        assert!(hid_identity(r"\\?\hid#vid_1532&pid_00c5&mi_00&mi_01#a").is_err());
        assert!(unique_match::<u8>(vec![]).is_err());
        assert!(unique_match(vec![1, 2]).is_err());
        assert_eq!(unique_match(vec![7]).unwrap(), 7);
        assert!(wide_path("x\0y").is_err());
    }
    #[test]
    fn same_product_in_another_container_is_never_associated() {
        let a = Guid {
            a: 1,
            b: 2,
            c: 3,
            d: [4; 8],
        };
        let b = Guid { a: 2, ..a };
        assert!(exact_association((0xc5, 0, a), (0xc5, 0, a)));
        assert!(!exact_association((0xc5, 0, a), (0xc5, 0, b)));
        assert!(!exact_association((0xc5, 0, a), (0xc5, 1, a)));
        assert!(!exact_association((0xc5, 0, a), (0xc4, 0, a)));
        let zero = Guid {
            a: 0,
            b: 0,
            c: 0,
            d: [0; 8],
        };
        assert!(!exact_association((0xc5, 0, zero), (0xc5, 0, zero)));
    }
    #[test]
    fn restores_exact_non_ui_baseline_and_drop_is_idempotent() {
        let (mut lease, state) = fixture(359);
        lease.set_degrees(-12).unwrap();
        assert_eq!(state.borrow().value, 348);
        lease.restore().unwrap();
        drop(lease);
        assert_eq!(state.borrow().writes, [348, 359]);
    }
    #[test]
    fn drop_restores_owned_value() {
        let (mut lease, state) = fixture(17);
        lease.set_degrees(10).unwrap();
        drop(lease);
        assert_eq!(state.borrow().writes, [10, 17]);
    }
    #[test]
    fn refuses_external_changes_before_set_and_restore() {
        let (mut lease, state) = fixture(0);
        state.borrow_mut().value = 15;
        assert!(lease.set_degrees(10).is_err());
        assert!(state.borrow().writes.is_empty());
        drop(lease);
        assert_eq!(state.borrow().value, 15);
        let (mut lease, state) = fixture(0);
        lease.set_degrees(10).unwrap();
        state.borrow_mut().value = 15;
        assert!(lease.restore().is_err());
        drop(lease);
        assert_eq!(state.borrow().writes, [10]);
        assert_eq!(state.borrow().value, 15);
    }
    #[test]
    fn verification_and_ambiguous_completion_recovery() {
        let (mut lease, state) = fixture(0);
        state.borrow_mut().ignore_write = true;
        assert!(lease.set_degrees(10).is_err());
        lease.restore().unwrap(); // unchanged baseline needs no recovery write
        assert_eq!(state.borrow().writes, [10]);
        let (mut lease, state) = fixture(7);
        state.borrow_mut().fail_set_after_change = true;
        assert!(lease.set_degrees(12).is_err());
        lease.restore().unwrap();
        assert_eq!(state.borrow().writes, [12, 7]);
    }
    #[test]
    fn query_only_and_invalid_range_never_write() {
        let (mut lease, state) = fixture(0);
        assert!(lease.set_degrees(45).is_err());
        lease.query_only = true;
        assert!(lease.set_degrees(10).is_err());
        drop(lease);
        assert!(state.borrow().writes.is_empty());
    }
    #[test]
    fn abi_sizes_match_windows_layout() {
        assert_eq!(size_of::<Guid>(), 16);
        assert_eq!(size_of::<PropertyKey>(), 20);
        assert_eq!(
            size_of::<InterfaceData>(),
            if size_of::<usize>() == 8 { 32 } else { 28 }
        );
        assert_eq!(
            size_of::<DeviceInfoData>(),
            if size_of::<usize>() == 8 { 32 } else { 28 }
        );
    }
}
