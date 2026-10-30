//! WASAPI shared render-loopback only. Every acquired packet and started stream
//! is released on its owning apartment thread; no raw audio leaves this module.
use super::*;
use crate::audio_capture::{Choice, Config, Report};
use secbird_core::audio_analysis::{Analyzer, Format, MeterFrame, MAX_PACKET_BYTES};
use std::time::{Duration, Instant};
use windows_sys::Win32::Media::Audio::{AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_S_BUFFER_EMPTY};
const AUDIO_CLIENT: GUID = GUID::from_u128(0x1cb9ad4c_dbfa_4c32_b178_c2f568a703b2);
const CAPTURE_CLIENT: GUID = GUID::from_u128(0xc8adbd64_e71e_48a0_a4de_185c395cd317);
const ENDPOINT: GUID = GUID::from_u128(0x1be09788_6894_4089_8586_9a2a6c265ac5);
#[repr(C)]
struct Endpoint {
    base: Unknown,
    flow: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
}
#[repr(C)]
struct AudioClient {
    base: Unknown,
    initialize: unsafe extern "system" fn(
        *mut c_void,
        i32,
        u32,
        i64,
        i64,
        *const u8,
        *const GUID,
    ) -> HRESULT,
    buffer_size: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    unused: [usize; 3],
    mix_format: unsafe extern "system" fn(*mut c_void, *mut *mut u8) -> HRESULT,
    period: usize,
    start: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    stop: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    reset: usize,
    event: usize,
    service: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
struct CaptureClient {
    base: Unknown,
    buffer: unsafe extern "system" fn(
        *mut c_void,
        *mut *mut u8,
        *mut u32,
        *mut u32,
        *mut u64,
        *mut u64,
    ) -> HRESULT,
    release: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    next: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
}
struct MixFormat(*mut u8);
impl Drop for MixFormat {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(self.0.cast());
        }
    }
}
struct Stop<'a> {
    client: &'a Com<AudioClient>,
    attempted: bool,
}
impl Stop<'_> {
    unsafe fn finish(&mut self) -> Result<(), String> {
        if self.attempted {
            return Err("Loopback stop was already attempted".into());
        }
        self.attempted = true;
        check(
            (self.client.table().stop)(self.client.raw()),
            "Stop loopback stream",
        )
    }
}
impl Drop for Stop<'_> {
    fn drop(&mut self) {
        if !self.attempted {
            unsafe {
                let _ = self.finish();
            }
        }
    }
}
struct Packet<'a> {
    client: &'a Com<CaptureClient>,
    frames: u32,
    released: bool,
}
impl Packet<'_> {
    unsafe fn release(&mut self) -> Result<(), String> {
        if self.released {
            return Err("Loopback packet release was already attempted".into());
        }
        self.released = true;
        check(
            (self.client.table().release)(self.client.raw(), self.frames),
            "Release loopback packet",
        )
    }
}
impl Drop for Packet<'_> {
    fn drop(&mut self) {
        if !self.released {
            unsafe {
                let _ = self.release();
            }
        }
    }
}
fn combine<T>(result: Result<T, String>, cleanup: Result<(), String>) -> Result<T, String> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup also failed: {cleanup}")),
    }
}
fn role(role: Role) -> i32 {
    match role {
        Role::Console => 0,
        Role::Multimedia => 1,
        Role::Communications => 2,
    }
}
unsafe fn default_device(
    enumerator: &Com<Enumerator>,
    selected: Role,
) -> Result<Com<Device>, String> {
    let mut raw = null_mut();
    let hr = (enumerator.table().default)(enumerator.raw(), 0, role(selected), &mut raw);
    Com::owned(hr, raw, "Get default playback capture source")
}
pub(crate) fn capture(
    config: &Config,
    cancel: &AtomicBool,
    mut sink: impl FnMut(MeterFrame),
) -> Result<Report, String> {
    unsafe {
        if cancel.load(Ordering::Acquire) {
            return Err("Playback capture canceled before setup".into());
        }
        check(
            CoInitializeEx(null(), COINIT_MULTITHREADED as u32),
            "Initialize playback apartment",
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
        let enumerator = Com::<Enumerator>::owned(hr, raw, "Create playback enumerator")?;
        let device = match &config.choice {
            Choice::Default(selected) => default_device(&enumerator, *selected)?,
            Choice::Exact(selected) => {
                if selected.is_empty()
                    || selected.contains('\0')
                    || selected.encode_utf16().count() > 4095
                {
                    return Err("Invalid exact playback endpoint".into());
                }
                let wide: Vec<_> = selected.encode_utf16().chain(Some(0)).collect();
                let mut raw = null_mut();
                let hr = (enumerator.table().device)(enumerator.raw(), wide.as_ptr(), &mut raw);
                Com::<Device>::owned(hr, raw, "Get exact playback endpoint")?
            }
        };
        let endpoint_id = id(&device)?;
        if let Choice::Exact(selected) = &config.choice {
            if selected != &endpoint_id {
                return Err("Playback endpoint identity changed".into());
            }
        }
        let mut raw = null_mut();
        let hr = (device.table().base.query)(device.raw(), &ENDPOINT, &mut raw);
        let endpoint = Com::<Endpoint>::owned(hr, raw, "Query endpoint data flow")?;
        let mut flow = -1;
        check(
            (endpoint.table().flow)(endpoint.raw(), &mut flow),
            "Read endpoint data flow",
        )?;
        if flow != 0 {
            return Err("Loopback refuses non-render endpoints".into());
        }
        let mut state = 0;
        check(
            (device.table().state)(device.raw(), &mut state),
            "Read playback state",
        )?;
        if state != DEVICE_STATE_ACTIVE {
            return Err("Playback endpoint is not active".into());
        }
        let mut raw = null_mut();
        let hr = (device.table().activate)(
            device.raw(),
            &AUDIO_CLIENT,
            CLSCTX_INPROC_SERVER,
            null(),
            &mut raw,
        );
        let client = Com::<AudioClient>::owned(hr, raw, "Activate loopback client")?;
        let mut mix = MixFormat(null_mut());
        check(
            (client.table().mix_format)(client.raw(), &mut mix.0),
            "Get playback mix format",
        )?;
        if mix.0.is_null() {
            return Err("Null playback mix format".into());
        }
        let header = std::slice::from_raw_parts(mix.0, 18);
        let tag = u16::from_le_bytes(header[..2].try_into().unwrap());
        let extra = u16::from_le_bytes(header[16..18].try_into().unwrap());
        if extra > 4096 || (tag == 0xfffe && extra < 22) {
            return Err("Invalid playback format extension".into());
        }
        let format = Format::from_wave_format(std::slice::from_raw_parts(
            mix.0,
            if tag == 0xfffe { 40 } else { 18 },
        ))
        .map_err(|error| error.to_string())?;
        check(
            (client.table().initialize)(
                client.raw(),
                0,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                1_000_000,
                0,
                mix.0,
                null(),
            ),
            "Initialize shared loopback",
        )?;
        let mut buffer_frames = 0;
        check(
            (client.table().buffer_size)(client.raw(), &mut buffer_frames),
            "Read loopback buffer size",
        )?;
        if buffer_frames == 0
            || (buffer_frames as usize)
                .checked_mul(usize::from(format.block_align()))
                .is_none_or(|bytes| bytes > MAX_PACKET_BYTES)
        {
            return Err("Playback buffer exceeds bounded capture geometry".into());
        }
        let mut raw = null_mut();
        let hr = (client.table().service)(client.raw(), &CAPTURE_CLIENT, &mut raw);
        let capture = Com::<CaptureClient>::owned(hr, raw, "Get loopback capture service")?;
        let mut analyzer = Analyzer::for_meter(format);
        let mut report = Report {
            endpoint_id,
            format,
            native_buffer_frames: buffer_frames,
            packets: 0,
            sample_frames: 0,
            silent_frames: 0,
            analysis_frames: 0,
            discontinuities: 0,
            timestamp_errors: 0,
            stop_succeeded: false,
            analysis_storage_bytes: analyzer.storage_bytes(),
            packet_storage_bytes: 0,
        };
        if cancel.load(Ordering::Acquire) {
            return Err("Playback capture canceled before stream start".into());
        }
        check(
            (client.table().start)(client.raw()),
            "Start loopback stream",
        )?;
        let mut stop = Stop {
            client: &client,
            attempted: false,
        };
        let started = Instant::now();
        let mut last_default_check = Instant::now();
        let mut packet_bytes = Vec::new();
        let result = (|| -> Result<(), String> {
            while !cancel.load(Ordering::Acquire)
                && config
                    .duration
                    .is_none_or(|duration| started.elapsed() < duration)
            {
                if let Choice::Default(selected) = config.choice {
                    if last_default_check.elapsed() >= Duration::from_millis(500) {
                        if id(&default_device(&enumerator, selected)?)? != report.endpoint_id {
                            return Err("Default playback source changed; capture stopped instead of rerouting".into());
                        }
                        last_default_check = Instant::now();
                    }
                }
                let mut drained = 0;
                loop {
                    if cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let mut available = 0;
                    check(
                        (capture.table().next)(capture.raw(), &mut available),
                        "Get next loopback packet",
                    )?;
                    if available == 0 {
                        break;
                    }
                    if drained >= 128 {
                        return Err("Loopback packet backlog exceeds bounded drain".into());
                    }
                    drained += 1;
                    let mut data = null_mut();
                    let mut frames = 0;
                    let mut flags = 0;
                    let hr = (capture.table().buffer)(
                        capture.raw(),
                        &mut data,
                        &mut frames,
                        &mut flags,
                        null_mut(),
                        null_mut(),
                    );
                    check(hr, "Get loopback packet")?;
                    if hr == AUDCLNT_S_BUFFER_EMPTY || frames == 0 {
                        break;
                    }
                    let mut packet = Packet {
                        client: &capture,
                        frames,
                        released: false,
                    };
                    let consumed = (|| -> Result<(), String> {
                        if frames > buffer_frames || flags & !7 != 0 {
                            return Err("Invalid loopback packet size or flags".into());
                        }
                        let length = frames as usize * usize::from(format.block_align());
                        if length > MAX_PACKET_BYTES {
                            return Err("Loopback packet exceeds byte limit".into());
                        }
                        let silent = flags & 2 != 0;
                        if !silent && data.is_null() {
                            return Err("Null nonsilent playback packet".into());
                        }
                        packet_bytes.clear();
                        if !silent {
                            if length > packet_bytes.capacity() {
                                packet_bytes
                                    .try_reserve_exact(length)
                                    .map_err(|error| error.to_string())?;
                            }
                            packet_bytes
                                .extend_from_slice(std::slice::from_raw_parts(data, length));
                        }
                        Ok(())
                    })();
                    // Release the OS buffer before FFT work or caller publication.
                    combine(consumed, packet.release())?;
                    report.packet_storage_bytes =
                        report.packet_storage_bytes.max(packet_bytes.capacity());
                    report.packets += 1;
                    report.sample_frames += u64::from(frames);
                    if flags & 2 != 0 {
                        report.silent_frames += u64::from(frames);
                    }
                    if flags & 1 != 0 {
                        report.discontinuities += 1;
                        analyzer.reset();
                    }
                    if flags & 4 != 0 {
                        report.timestamp_errors += 1;
                    }
                    if cancel.load(Ordering::Acquire) {
                        break;
                    }
                    analyzer
                        .push(&packet_bytes, frames, flags & 2 != 0, |frame| {
                            report.analysis_frames += 1;
                            if !cancel.load(Ordering::Acquire) {
                                sink(frame);
                            }
                        })
                        .map_err(|error| error.to_string())?;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        })();
        combine(result, stop.finish())?;
        report.stop_succeeded = true;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_packet_release_is_attempted_once_and_preserves_the_processing_error() {
        use std::sync::atomic::AtomicUsize;
        #[repr(C)]
        struct Fake {
            table: *const CaptureClient,
            attempts: AtomicUsize,
        }
        unsafe extern "system" fn query(
            _: *mut c_void,
            _: *const GUID,
            _: *mut *mut c_void,
        ) -> HRESULT {
            -1
        }
        unsafe extern "system" fn release_ref(_: *mut c_void) -> u32 {
            0
        }
        unsafe extern "system" fn buffer(
            _: *mut c_void,
            _: *mut *mut u8,
            _: *mut u32,
            _: *mut u32,
            _: *mut u64,
            _: *mut u64,
        ) -> HRESULT {
            -1
        }
        unsafe extern "system" fn release(raw: *mut c_void, frames: u32) -> HRESULT {
            let fake = &*(raw as *const Fake);
            if frames != 480 {
                return -2;
            }
            fake.attempts.fetch_add(1, Ordering::SeqCst);
            -1
        }
        unsafe extern "system" fn next(_: *mut c_void, _: *mut u32) -> HRESULT {
            -1
        }
        let table = CaptureClient {
            base: Unknown {
                query,
                add_ref: 0,
                release: release_ref,
            },
            buffer,
            release,
            next,
        };
        let mut fake = Fake {
            table: &table,
            attempts: AtomicUsize::new(0),
        };
        let client = unsafe {
            Com::<CaptureClient>::owned(0, (&mut fake as *mut Fake).cast(), "fake packet owner")
        }
        .unwrap();
        let mut packet = Packet {
            client: &client,
            frames: 480,
            released: false,
        };
        let failure =
            combine::<()>(Err("invalid PCM".into()), unsafe { packet.release() }).unwrap_err();
        assert!(failure.contains("invalid PCM") && failure.contains("cleanup also failed"));
        assert!(unsafe { packet.release() }.is_err());
        drop(packet);
        assert_eq!(fake.attempts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn capture_slots_match_independent_windows_sdk_declarations() {
        let sdk: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/audio-endpoint-sdk-abi.json"
        ))
        .unwrap();
        let slot = std::mem::size_of::<usize>();
        let offset = |interface: &str, method: &str| {
            sdk["methods"][interface]
                .as_array()
                .unwrap()
                .iter()
                .position(|name| name == method)
                .unwrap()
                * slot
        };
        for (actual, interface, method) in [
            (
                std::mem::offset_of!(Enumerator, device),
                "IMMDeviceEnumerator",
                "GetDevice",
            ),
            (
                std::mem::offset_of!(Endpoint, flow),
                "IMMEndpoint",
                "GetDataFlow",
            ),
            (
                std::mem::offset_of!(AudioClient, initialize),
                "IAudioClient",
                "Initialize",
            ),
            (
                std::mem::offset_of!(AudioClient, buffer_size),
                "IAudioClient",
                "GetBufferSize",
            ),
            (
                std::mem::offset_of!(AudioClient, mix_format),
                "IAudioClient",
                "GetMixFormat",
            ),
            (
                std::mem::offset_of!(AudioClient, start),
                "IAudioClient",
                "Start",
            ),
            (
                std::mem::offset_of!(AudioClient, stop),
                "IAudioClient",
                "Stop",
            ),
            (
                std::mem::offset_of!(AudioClient, service),
                "IAudioClient",
                "GetService",
            ),
            (
                std::mem::offset_of!(CaptureClient, buffer),
                "IAudioCaptureClient",
                "GetBuffer",
            ),
            (
                std::mem::offset_of!(CaptureClient, release),
                "IAudioCaptureClient",
                "ReleaseBuffer",
            ),
            (
                std::mem::offset_of!(CaptureClient, next),
                "IAudioCaptureClient",
                "GetNextPacketSize",
            ),
        ] {
            assert_eq!(actual, offset(interface, method), "{interface}.{method}");
        }
    }
}
