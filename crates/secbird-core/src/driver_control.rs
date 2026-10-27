//! Read-only access to the installed Razer control driver.
//!
//! This is a vendor-driver-dependent probe, not a replacement input driver.
//! The only request exposed here is EnumInputHook (0x88883030): a four-byte
//! index in, a 292-byte selector out. Its shipped caller and kernel dispatch
//! are documented in `outputs/research/driver-abi.md` in the investigation.
//! Specifically, mapping_engine.dll SHA-256 6eabdfde…8f3320a builds that call at
//! RVA 0x111912–0x111942; RzCommon.sys SHA-256 64b72ef9…587e59 validates/copies
//! the reply at RVA 0x352e–0x35d0 and retrieves the item at 0x6698–0x6774.
//! No hook registration, redirection, input injection, or notification receive
//! operation is exposed.
//!
//! Queries use synchronous handles. This keeps every buffer alive until the
//! kernel finishes using it. Record counts and allocations are bounded, but a
//! broken driver can still block a query. Run probes in a separate process if
//! the caller needs an enforceable wall-clock deadline.

use std::io;

/// Exact selector size checked by the inspected RzCommon dispatch.
pub const INPUT_HOOK_BYTES: usize = 0x124;
/// Maximum number of records one prefix probe will request.
pub const MAX_HOOK_RECORDS: usize = 64;

const ENUM_INPUT_HOOK: u32 = 0x8888_3030;
const CONTROL_CLASS: Guid = Guid {
    data1: 0xe3be005d,
    data2: 0xd130,
    data3: 0x4910,
    data4: [0x88, 0xff, 0x09, 0xae, 0x02, 0xf6, 0x80, 0xe9],
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// One present interface obtained from Windows SetupAPI.
///
/// Fields are private so the probe cannot be pointed at arbitrary device paths.
/// The IDs are parsed from the registered interface name, not USB descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlInterface {
    path: String,
    product_id: u16,
    interface_number: u8,
}

impl ControlInterface {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn interface_number(&self) -> u8 {
        self.interface_number
    }

    /// The control ABI has only been inspected for these installed packages.
    pub fn query_supported(&self) -> bool {
        matches!(self.product_id, 0x025d | 0x00c4 | 0x00c5)
    }

    fn from_registered_path(path: String) -> Option<Self> {
        let lower = path.to_ascii_lowercase();
        let mut components = lower.split('#');
        if components.next()? != r"\\?\rzcontrol" {
            return None;
        }
        let identity = components.next()?;
        let mut fields = identity.split('&');
        if fields.next()? != "vid_1532" {
            return None;
        }
        let product = fields.next()?.strip_prefix("pid_")?;
        let interface = fields.next()?.strip_prefix("mi_")?;
        if product.len() != 4 || interface.len() != 2 || fields.next().is_some() {
            return None;
        }
        // A path from another interface class must not reach CreateFile.
        if components.next()?.is_empty()
            || components.next()? != "{e3be005d-d130-4910-88ff-09ae02f680e9}"
            || components.next().is_some()
        {
            return None;
        }
        let product_id = u16::from_str_radix(product, 16).ok()?;
        let interface_number = u8::from_str_radix(interface, 16).ok()?;
        Some(Self {
            path,
            product_id,
            interface_number,
        })
    }
}

/// The keyboard part of a selector, matching KEYBOARD_INPUT_DATA's wire layout.
/// ExtraInformation is preserved raw; its complete vendor semantics are unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardSelector {
    pub unit_id: u16,
    pub make_code: u16,
    pub flags: u16,
    pub reserved: u16,
    pub extra_information: u32,
}

/// Kind 2's 24-byte payload, corroborated in the shipped caller and driver.
/// ButtonData remains raw: wheel interpretation depends on ButtonFlags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseSelector {
    pub unit_id: u16,
    pub flags: u16,
    pub button_flags: u16,
    pub button_data: u16,
    pub raw_buttons: u32,
    pub last_x: i32,
    pub last_y: i32,
    pub extra_information: u32,
}

/// A driver selector, with opaque bytes retained for unresolved record kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputHookRecord {
    raw: [u8; INPUT_HOOK_BYTES],
}

impl InputHookRecord {
    fn from_reply(raw: [u8; INPUT_HOOK_BYTES], returned: usize) -> io::Result<Self> {
        if returned != INPUT_HOOK_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("EnumInputHook returned {returned} bytes; expected {INPUT_HOOK_BYTES}"),
            ));
        }
        Ok(Self { raw })
    }

    pub fn raw(&self) -> &[u8; INPUT_HOOK_BYTES] {
        &self.raw
    }

    /// Preserved without assigning an unverified meaning to the routing field.
    pub fn routing_field(&self) -> u32 {
        u32_at(&self.raw, 0)
    }

    pub fn kind(&self) -> u32 {
        u32_at(&self.raw, 4)
    }

    /// Interpret only the corroborated keyboard kind.
    pub fn keyboard(&self) -> Option<KeyboardSelector> {
        if self.kind() != 1 {
            return None;
        }
        Some(KeyboardSelector {
            unit_id: u16_at(&self.raw, 8),
            make_code: u16_at(&self.raw, 10),
            flags: u16_at(&self.raw, 12),
            reserved: u16_at(&self.raw, 14),
            extra_information: u32_at(&self.raw, 16),
        })
    }

    pub fn mouse(&self) -> Option<MouseSelector> {
        if self.kind() != 2 {
            return None;
        }
        Some(MouseSelector {
            unit_id: u16_at(&self.raw, 8),
            flags: u16_at(&self.raw, 10),
            button_flags: u16_at(&self.raw, 12),
            button_data: u16_at(&self.raw, 14),
            raw_buttons: u32_at(&self.raw, 16),
            last_x: u32_at(&self.raw, 20) as i32,
            last_y: u32_at(&self.raw, 24) as i32,
            extra_information: u32_at(&self.raw, 28),
        })
    }
}

fn u16_at(bytes: &[u8; INPUT_HOOK_BYTES], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8; INPUT_HOOK_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// A prefix is not an atomic snapshot: another client can edit hooks meanwhile.
#[derive(Debug)]
pub struct HookProbe {
    pub records: Vec<InputHookRecord>,
    pub stop: ProbeStop,
}

#[derive(Debug)]
pub enum ProbeStop {
    LimitReached,
    /// An out-of-range index produces a generic kernel failure in the inspected
    /// driver. Preserve the error: the same code can indicate another failure.
    QueryFailed {
        index: u32,
        error: io::Error,
    },
}

#[cfg(windows)]
pub use windows::{enumerate_control_interfaces, InputHookReader};

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};

    type RawHandle = *mut c_void;
    const INVALID_HANDLE: RawHandle = -1_isize as RawHandle;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const ERROR_NO_MORE_ITEMS: u32 = 259;
    const MAX_CONTROL_INTERFACES: u32 = 256;
    const MAX_DETAIL_BYTES: usize = 65_536;

    #[repr(C)]
    struct InterfaceData {
        size: u32,
        class_guid: Guid,
        flags: u32,
        reserved: usize,
    }

    #[link(name = "setupapi")]
    extern "system" {
        fn SetupDiGetClassDevsW(
            class: *const Guid,
            enumerator: *const u16,
            parent: RawHandle,
            flags: u32,
        ) -> RawHandle;
        fn SetupDiEnumDeviceInterfaces(
            set: RawHandle,
            device: *const c_void,
            class: *const Guid,
            index: u32,
            data: *mut InterfaceData,
        ) -> i32;
        fn SetupDiGetDeviceInterfaceDetailW(
            set: RawHandle,
            interface: *const InterfaceData,
            detail: *mut c_void,
            detail_bytes: u32,
            required_bytes: *mut u32,
            device: *mut c_void,
        ) -> i32;
        fn SetupDiDestroyDeviceInfoList(set: RawHandle) -> i32;
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
            template: RawHandle,
        ) -> RawHandle;
        fn CloseHandle(handle: RawHandle) -> i32;
        fn GetLastError() -> u32;
        fn DeviceIoControl(
            handle: RawHandle,
            code: u32,
            input: *const c_void,
            input_bytes: u32,
            output: *mut c_void,
            output_bytes: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }

    struct DeviceSet(RawHandle);

    impl Drop for DeviceSet {
        fn drop(&mut self) {
            // This owns exactly the set returned by SetupDiGetClassDevsW.
            unsafe { SetupDiDestroyDeviceInfoList(self.0) };
        }
    }

    /// Enumerate present RZCONTROL endpoints. This sends no driver requests.
    pub fn enumerate_control_interfaces() -> io::Result<Vec<ControlInterface>> {
        // 0x12 = DIGCF_PRESENT | DIGCF_DEVICEINTERFACE.
        let raw_set = unsafe { SetupDiGetClassDevsW(&CONTROL_CLASS, null(), null_mut(), 0x12) };
        if raw_set == INVALID_HANDLE {
            return Err(io::Error::last_os_error());
        }
        let set = DeviceSet(raw_set);
        let mut found = Vec::new();

        for index in 0..MAX_CONTROL_INTERFACES {
            let mut interface = InterfaceData {
                size: size_of::<InterfaceData>() as u32,
                class_guid: CONTROL_CLASS,
                flags: 0,
                reserved: 0,
            };
            let success = unsafe {
                SetupDiEnumDeviceInterfaces(set.0, null(), &CONTROL_CLASS, index, &mut interface)
            };
            if success == 0 {
                let code = unsafe { GetLastError() };
                if code == ERROR_NO_MORE_ITEMS {
                    return Ok(found);
                }
                return Err(io::Error::from_raw_os_error(code as i32));
            }
            let path = interface_path(&set, &interface)?;
            if let Some(interface) = ControlInterface::from_registered_path(path) {
                found.push(interface);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RZCONTROL enumeration exceeded the 256-interface limit",
        ))
    }

    fn interface_path(set: &DeviceSet, interface: &InterfaceData) -> io::Result<String> {
        let mut required = 0;
        let success = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                interface,
                null_mut(),
                0,
                &mut required,
                null_mut(),
            )
        };
        let code = unsafe { GetLastError() };
        if success != 0 || code != ERROR_INSUFFICIENT_BUFFER {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unexpected SetupAPI sizing result: success={success}, error={code}"),
            ));
        }
        if !(6..=MAX_DETAIL_BYTES).contains(&(required as usize)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "RZCONTROL interface path exceeds the descriptor size limit",
            ));
        }

        // u32 storage gives the detail header its required alignment. Windows'
        // cbSize is 8 on x64 and 6 on x86; the UTF-16 path starts at byte 4.
        let mut detail = vec![0_u32; (required as usize).div_ceil(4)];
        detail[0] = if size_of::<usize>() == 8 { 8 } else { 6 };
        let success = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                interface,
                detail.as_mut_ptr().cast(),
                required,
                null_mut(),
                null_mut(),
            )
        };
        if success == 0 {
            return Err(io::Error::last_os_error());
        }
        let units = unsafe {
            std::slice::from_raw_parts(
                detail.as_ptr().cast::<u8>().add(4).cast::<u16>(),
                (required as usize - 4) / 2,
            )
        };
        let end = units.iter().position(|unit| *unit == 0).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Unterminated RZCONTROL path")
        })?;
        String::from_utf16(&units[..end]).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-16 RZCONTROL path")
        })
    }

    /// Owns a zero-access synchronous handle; never registers hooks or redirects
    /// input. Closing it releases only this probe's handle.
    pub struct InputHookReader {
        handle: RawHandle,
    }

    impl Drop for InputHookReader {
        fn drop(&mut self) {
            // No overlapped I/O is issued, so no pending buffers outlive self.
            unsafe { CloseHandle(self.handle) };
        }
    }

    impl InputHookReader {
        pub fn open(interface: &ControlInterface) -> io::Result<Self> {
            if !interface.query_supported() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "The EnumInputHook ABI has not been inspected for this product",
                ));
            }
            let path: Vec<u16> = interface.path.encode_utf16().chain(Some(0)).collect();
            let handle = unsafe {
                // No read/write access, share read/write, OPEN_EXISTING, no
                // FILE_FLAG_OVERLAPPED. This prevents pending buffer lifetimes.
                CreateFileW(path.as_ptr(), 0, 3, null(), 3, 0, null_mut())
            };
            if handle == INVALID_HANDLE {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { handle })
        }

        /// Read one selector by index. A generic failure is not interpreted as
        /// an empty list: it also covers other driver failures.
        pub fn read_hook(&mut self, index: u32) -> io::Result<InputHookRecord> {
            let input = index.to_le_bytes();
            let mut output = [0_u8; INPUT_HOOK_BYTES];
            let mut returned = 0;
            let success = unsafe {
                DeviceIoControl(
                    self.handle,
                    ENUM_INPUT_HOOK,
                    input.as_ptr().cast(),
                    input.len() as u32,
                    output.as_mut_ptr().cast(),
                    output.len() as u32,
                    &mut returned,
                    null_mut(),
                )
            };
            if success == 0 {
                return Err(io::Error::last_os_error());
            }
            InputHookRecord::from_reply(output, returned as usize)
        }

        /// Read at most `limit` selectors, retaining an exact stop reason.
        /// The driver can change between requests; this is not a snapshot.
        pub fn read_prefix(&mut self, limit: usize) -> io::Result<HookProbe> {
            if !(1..=MAX_HOOK_RECORDS).contains(&limit) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Hook probe limit must be between 1 and 64",
                ));
            }
            let mut records = Vec::with_capacity(limit);
            for index in 0..limit as u32 {
                match self.read_hook(index) {
                    Ok(record) => records.push(record),
                    Err(error) => {
                        return Ok(HookProbe {
                            records,
                            stop: ProbeStop::QueryFailed { index, error },
                        });
                    }
                }
            }
            Ok(HookProbe {
                records,
                stop: ProbeStop::LimitReached,
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn native_setupapi_structure_matches_windows_abi() {
            assert_eq!(size_of::<Guid>(), 16);
            assert_eq!(
                size_of::<InterfaceData>(),
                if cfg!(target_pointer_width = "64") {
                    32
                } else {
                    28
                }
            );
            assert_eq!(std::mem::offset_of!(InterfaceData, class_guid), 4);
            assert_eq!(std::mem::offset_of!(InterfaceData, flags), 20);
            assert_eq!(std::mem::offset_of!(InterfaceData, reserved), 24);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interface_path(identity: &str) -> String {
        format!(r"\\?\rzcontrol#{identity}#d&2411f3c6&0#{{e3be005d-d130-4910-88ff-09ae02f680e9}}")
    }

    #[test]
    fn mouse_layout_retains_signed_coordinates_and_raw_button_data() {
        let mut raw = [0u8; INPUT_HOOK_BYTES];
        raw[4..8].copy_from_slice(&2u32.to_le_bytes());
        for (offset, value) in [(8, 0x1234u16), (10, 1), (12, 0x400), (14, 0x8000)] {
            raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        raw[16..20].copy_from_slice(&0x87654321u32.to_le_bytes());
        raw[20..24].copy_from_slice(&(-120i32).to_le_bytes());
        raw[24..28].copy_from_slice(&i32::MIN.to_le_bytes());
        raw[28..32].copy_from_slice(&0xabcdef01u32.to_le_bytes());
        let record = InputHookRecord::from_reply(raw, INPUT_HOOK_BYTES).unwrap();
        assert_eq!(record.keyboard(), None);
        assert_eq!(
            record.mouse(),
            Some(MouseSelector {
                unit_id: 0x1234,
                flags: 1,
                button_flags: 0x400,
                button_data: 0x8000,
                raw_buttons: 0x87654321,
                last_x: -120,
                last_y: i32::MIN,
                extra_information: 0xabcdef01
            })
        );
    }

    #[test]
    fn accepts_observed_endpoint_names_and_keeps_product_scope_explicit() {
        let keyboard =
            ControlInterface::from_registered_path(interface_path("VID_1532&PID_025D&MI_00"))
                .unwrap();
        assert_eq!(keyboard.product_id(), 0x025d);
        assert_eq!(keyboard.interface_number(), 0);
        assert!(keyboard.query_supported());

        let unknown =
            ControlInterface::from_registered_path(interface_path("vid_1532&pid_ffff&mi_02"))
                .unwrap();
        assert!(!unknown.query_supported());
    }

    #[test]
    fn rejects_arbitrary_devices_and_malformed_path_fields() {
        for identity in [
            "vid_1234&pid_025d&mi_00",
            "vid_1532&pid_25d&mi_00",
            "vid_1532&pid_zzzz&mi_00",
            "vid_1532&pid_025d&mi_000",
            "vid_1532&pid_025d&mi_00&col01",
        ] {
            assert!(ControlInterface::from_registered_path(interface_path(identity)).is_none());
        }
        assert!(ControlInterface::from_registered_path(r"\\.\PhysicalDrive0".into()).is_none());
        assert!(ControlInterface::from_registered_path(
            interface_path("vid_1532&pid_025d&mi_00").replace("rzcontrol", "hid")
        )
        .is_none());
    }

    #[test]
    fn decodes_keyboard_fields_at_the_observed_offsets() {
        let mut reply = [0_u8; INPUT_HOOK_BYTES];
        reply[0..4].copy_from_slice(&0x44332211_u32.to_le_bytes());
        reply[4..8].copy_from_slice(&1_u32.to_le_bytes());
        reply[8..10].copy_from_slice(&0xabcd_u16.to_le_bytes());
        reply[10..12].copy_from_slice(&0x1d_u16.to_le_bytes());
        reply[12..14].copy_from_slice(&2_u16.to_le_bytes());
        reply[14..16].copy_from_slice(&0x55aa_u16.to_le_bytes());
        reply[16..20].copy_from_slice(&0x87654321_u32.to_le_bytes());
        reply[291] = 0xcc;

        let record = InputHookRecord::from_reply(reply, INPUT_HOOK_BYTES).unwrap();
        assert_eq!(record.routing_field(), 0x44332211);
        assert_eq!(
            record.keyboard(),
            Some(KeyboardSelector {
                unit_id: 0xabcd,
                make_code: 0x1d,
                flags: 2,
                reserved: 0x55aa,
                extra_information: 0x87654321,
            })
        );
        assert_eq!(record.raw()[291], 0xcc);
    }

    #[test]
    fn unknown_kinds_and_incomplete_replies_are_not_keyboard_records() {
        let mut reply = [0_u8; INPUT_HOOK_BYTES];
        reply[4..8].copy_from_slice(&11_u32.to_le_bytes());
        let record = InputHookRecord::from_reply(reply, INPUT_HOOK_BYTES).unwrap();
        assert!(record.keyboard().is_none());
        for returned in [0, 20, INPUT_HOOK_BYTES - 1, INPUT_HOOK_BYTES + 1] {
            assert_eq!(
                InputHookRecord::from_reply(reply, returned)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn query_code_matches_the_inspected_buffered_any_access_operation() {
        assert_eq!(ENUM_INPUT_HOOK >> 16, 0x8888);
        assert_eq!((ENUM_INPUT_HOOK >> 14) & 3, 0);
        assert_eq!((ENUM_INPUT_HOOK >> 2) & 0xfff, 0xc0c);
        assert_eq!(ENUM_INPUT_HOOK & 3, 0);
    }
}
