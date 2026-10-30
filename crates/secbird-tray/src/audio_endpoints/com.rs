//! Narrow SDK ABI bindings. Vtable order is from Windows SDK26100 mmdeviceapi.h,
//! propsys.h and endpointvolume.h; unused methods cannot be called here.
use super::*;
#[path = "capture.rs"]
mod capture;
pub(super) use capture::capture;
use std::{
    ffi::c_void,
    marker::PhantomData,
    ptr::{null, null_mut, NonNull},
    rc::Rc,
};
use windows_sys::{
    core::{GUID, HRESULT},
    Win32::{
        Media::Audio::{MMDeviceEnumerator, DEVICE_STATE_ACTIVE},
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            StructuredStorage::{PropVariantClear, PropVariantToStringAlloc, PROPVARIANT},
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
        },
        UI::Shell::PropertiesSystem::PROPERTYKEY,
    },
};

const ENUMERATOR: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const VOLUME: GUID = GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);
const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};
const NOT_FOUND: HRESULT = 0x80070490u32 as i32;
const MAX_TEXT: usize = 4096;
#[repr(C)]
struct Unknown {
    query: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}
#[repr(C)]
struct Enumerator {
    base: Unknown,
    enumerate: unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> HRESULT,
    default: unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> HRESULT,
    device: unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
struct Collection {
    base: Unknown,
    count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    item: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
struct Device {
    base: Unknown,
    activate: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const PROPVARIANT,
        *mut *mut c_void,
    ) -> HRESULT,
    properties: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    id: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    state: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
}
#[repr(C)]
struct Properties {
    base: Unknown,
    count: usize,
    at: usize,
    value: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> HRESULT,
}
#[repr(C)]
struct Volume {
    base: Unknown,
    unused_before: [usize; 6],
    scalar: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
    unused_after: [usize; 5],
    mute: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
}

/// Own one counted COM reference. Rc marker forbids transfer across apartments.
struct Com<T> {
    ptr: NonNull<c_void>,
    thread: PhantomData<Rc<T>>,
}
impl<T> Com<T> {
    unsafe fn owned(hr: HRESULT, ptr: *mut c_void, operation: &str) -> Result<Self, String> {
        check(hr, operation)?;
        Ok(Self {
            ptr: NonNull::new(ptr).ok_or_else(|| format!("{operation}: null interface"))?,
            thread: PhantomData,
        })
    }
    fn raw(&self) -> *mut c_void {
        self.ptr.as_ptr()
    }
    unsafe fn table(&self) -> &T {
        &**(self.raw() as *const *const T)
    }
}
impl<T> Drop for Com<T> {
    fn drop(&mut self) {
        unsafe {
            let table = &**(self.raw() as *const *const Unknown);
            (table.release)(self.raw());
        }
    }
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct TaskString(*mut u16);
impl Drop for TaskString {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(self.0.cast());
        }
    }
}
impl TaskString {
    unsafe fn text(&self) -> Result<String, String> {
        if self.0.is_null() {
            return Err("Audio property returned a null string".into());
        }
        for length in 0..MAX_TEXT {
            if *self.0.add(length) == 0 {
                return String::from_utf16(std::slice::from_raw_parts(self.0, length))
                    .map_err(|_| "Audio property contains invalid UTF16".into());
            }
        }
        Err("Audio property exceeds4095 UTF16 units".into())
    }
}
struct Variant(PROPVARIANT);
impl Drop for Variant {
    fn drop(&mut self) {
        unsafe {
            PropVariantClear(&mut self.0);
        }
    }
}
fn check(hr: HRESULT, operation: &str) -> Result<(), String> {
    if hr < 0 {
        Err(format!("{operation}: HRESULT 0x{:08X}", hr as u32))
    } else {
        Ok(())
    }
}
unsafe fn id(device: &Com<Device>) -> Result<String, String> {
    let mut text = TaskString(null_mut());
    check((device.table().id)(device.raw(), &mut text.0), "GetId")?;
    text.text()
}
unsafe fn name(device: &Com<Device>) -> Result<String, String> {
    let mut raw = null_mut();
    let hr = (device.table().properties)(device.raw(), 0, &mut raw); // STGM_READ
    let properties = Com::<Properties>::owned(hr, raw, "OpenPropertyStore")?;
    let mut value = Variant(std::mem::zeroed());
    check(
        (properties.table().value)(properties.raw(), &FRIENDLY_NAME, &mut value.0),
        "Get friendly name",
    )?;
    let mut text = TaskString(null_mut());
    check(
        PropVariantToStringAlloc(&value.0, &mut text.0),
        "Convert friendly name",
    )?;
    text.text()
}
fn scalar(value: f32) -> Result<f32, String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err("Audio volume scalar is outside0..1".into())
    }
}
unsafe fn endpoint(device: &Com<Device>) -> Result<Endpoint, String> {
    let id = id(device)?;
    let mut state = 0;
    check((device.table().state)(device.raw(), &mut state), "GetState")?;
    let mut issues = vec![];
    let name = match name(device) {
        Ok(name) => Some(name),
        Err(error) => {
            issues.push(error);
            None
        }
    };
    let mut result = Endpoint {
        id,
        name,
        state,
        default_roles: vec![],
        volume_scalar: None,
        muted: None,
        issues,
    };
    let mut raw = null_mut();
    let hr = (device.table().activate)(
        device.raw(),
        &VOLUME,
        CLSCTX_INPROC_SERVER,
        null(),
        &mut raw,
    );
    match Com::<Volume>::owned(hr, raw, "Activate endpoint volume") {
        Err(error) => result.issues.push(error),
        Ok(volume) => {
            let mut value = 0.;
            let read = check(
                (volume.table().scalar)(volume.raw(), &mut value),
                "Read volume",
            )
            .and_then(|_| scalar(value));
            match read {
                Ok(value) => result.volume_scalar = Some(value),
                Err(error) => result.issues.push(error),
            }
            let mut muted = 0;
            match check((volume.table().mute)(volume.raw(), &mut muted), "Read mute") {
                Ok(()) => result.muted = Some(muted != 0),
                Err(error) => result.issues.push(error),
            }
        }
    }
    Ok(result)
}
pub(super) fn discover() -> Result<Snapshot, String> {
    unsafe {
        check(
            CoInitializeEx(null(), COINIT_MULTITHREADED as u32),
            "Initialize audio COM apartment",
        )?;
        let _apartment = Apartment;
        let mut raw = null_mut();
        let hr = CoCreateInstance(
            &MMDeviceEnumerator,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &ENUMERATOR,
            &mut raw,
        );
        let enumerator = Com::<Enumerator>::owned(hr, raw, "Create endpoint enumerator")?;
        let mut defaults: [Option<String>; 3] = [None, None, None];
        for (role, default) in defaults.iter_mut().enumerate() {
            let mut raw = null_mut();
            let hr = (enumerator.table().default)(enumerator.raw(), 0, role as i32, &mut raw);
            if hr == NOT_FOUND {
                continue;
            }
            let device = Com::<Device>::owned(hr, raw, "Read default playback device")?;
            *default = Some(id(&device)?);
        }
        let mut raw = null_mut();
        let hr = (enumerator.table().enumerate)(enumerator.raw(), 0, DEVICE_STATE_ACTIVE, &mut raw);
        let collection = Com::<Collection>::owned(hr, raw, "Enumerate playback endpoints")?;
        let mut count = 0;
        check(
            (collection.table().count)(collection.raw(), &mut count),
            "Count playback endpoints",
        )?;
        if count as usize > MAX_ENDPOINTS {
            return Err("Audio endpoint count exceeds64".into());
        }
        let mut endpoints = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut raw = null_mut();
            let hr = (collection.table().item)(collection.raw(), index, &mut raw);
            let device = Com::<Device>::owned(hr, raw, "Read playback endpoint")?;
            endpoints.push(endpoint(&device)?);
        }
        assemble(endpoints, defaults)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_com_releases_exactly_once_and_rejects_null_or_failed_outputs() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        #[repr(C)]
        struct Fake {
            table: *const Unknown,
            releases: *const AtomicUsize,
        }
        unsafe extern "system" fn release(raw: *mut c_void) -> u32 {
            let fake = &*(raw as *const Fake);
            (*fake.releases).fetch_add(1, Ordering::SeqCst);
            0
        }
        unsafe extern "system" fn fake_query(
            _: *mut c_void,
            _: *const GUID,
            _: *mut *mut c_void,
        ) -> HRESULT {
            -1
        }
        let releases = AtomicUsize::new(0);
        let table = Unknown {
            query: fake_query,
            add_ref: 0,
            release,
        };
        let mut fake = Fake {
            table: &table,
            releases: &releases,
        };
        let reference =
            unsafe { Com::<Unknown>::owned(0, (&mut fake as *mut Fake).cast(), "fake") }.unwrap();
        assert_eq!(releases.load(Ordering::SeqCst), 0);
        drop(reference);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
        assert!(unsafe { Com::<Unknown>::owned(-1, null_mut(), "failed") }.is_err());
        assert!(unsafe { Com::<Unknown>::owned(0, null_mut(), "null") }.is_err());
    }
    #[test]
    fn allocated_strings_refuse_bad_utf16_and_unterminated_buffers() {
        use windows_sys::Win32::System::Com::CoTaskMemAlloc;
        unsafe {
            let text = TaskString(CoTaskMemAlloc(MAX_TEXT * 2).cast());
            assert!(!text.0.is_null());
            for i in 0..MAX_TEXT {
                *text.0.add(i) = b'a' as u16;
            }
            assert!(text.text().is_err());
            *text.0 = 0xd800;
            *text.0.add(1) = 0;
            assert!(text.text().is_err());
            *text.0 = b'X' as u16;
            assert_eq!(text.text().unwrap(), "X");
        }
    }
    #[test]
    fn scalar_validation_refuses_nonfinite_and_out_of_range_without_clamping() {
        for value in [-1., 1.01, f32::NAN, f32::INFINITY] {
            assert!(scalar(value).is_err());
        }
        for value in [0., 0.5, 1.] {
            assert_eq!(scalar(value).unwrap(), value);
        }
        assert!(check(1, "S_FALSE").is_ok());
        assert!(check(-1, "failure").is_err());
    }
    #[test]
    fn sdk_vtable_slots_and_property_key_layout_are_explicit() {
        let slot = std::mem::size_of::<usize>();
        let sdk: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/audio-endpoint-sdk-abi.json"
        ))
        .unwrap();
        let offset = |interface: &str, method: &str| {
            sdk["methods"][interface]
                .as_array()
                .unwrap()
                .iter()
                .position(|value| value == method)
                .unwrap()
                * slot
        };
        assert_eq!(
            std::mem::offset_of!(Enumerator, enumerate),
            offset("IMMDeviceEnumerator", "EnumAudioEndpoints")
        );
        assert_eq!(
            std::mem::offset_of!(Enumerator, default),
            offset("IMMDeviceEnumerator", "GetDefaultAudioEndpoint")
        );
        assert_eq!(
            std::mem::offset_of!(Collection, count),
            offset("IMMDeviceCollection", "GetCount")
        );
        assert_eq!(
            std::mem::offset_of!(Collection, item),
            offset("IMMDeviceCollection", "Item")
        );
        assert_eq!(
            std::mem::offset_of!(Device, activate),
            offset("IMMDevice", "Activate")
        );
        assert_eq!(
            std::mem::offset_of!(Device, properties),
            offset("IMMDevice", "OpenPropertyStore")
        );
        assert_eq!(
            std::mem::offset_of!(Device, id),
            offset("IMMDevice", "GetId")
        );
        assert_eq!(
            std::mem::offset_of!(Device, state),
            offset("IMMDevice", "GetState")
        );
        assert_eq!(
            std::mem::offset_of!(Properties, value),
            offset("IPropertyStore", "GetValue")
        );
        assert_eq!(
            std::mem::offset_of!(Volume, scalar),
            offset("IAudioEndpointVolume", "GetMasterVolumeLevelScalar")
        );
        assert_eq!(
            std::mem::offset_of!(Volume, mute),
            offset("IAudioEndpointVolume", "GetMute")
        );
        assert_eq!(std::mem::size_of::<PROPERTYKEY>(), 20);
    }
}
