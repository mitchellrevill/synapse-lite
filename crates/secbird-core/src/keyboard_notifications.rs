//! Selected Ornata filter notifications. No selectors, redirection or input send.
//!
//! Opening explicitly subscribes this file handle to passive notifications. This
//! is not a read-only-runtime API and is never started automatically by the app.
use crate::{driver_control, rzcontrol_packet::KeyboardInput, sensor_rotation, Device};
use serde::Serialize;
use std::{
    ffi::c_void,
    io,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const RECEIVE: u32 = 0x8888_3018;
const NOTIFY: u32 = 0x8888_3038;
const RECORD_BYTES: usize = 304;
const QUEUE_RECORDS: usize = 128;
static LIVE: AtomicBool = AtomicBool::new(false);
type Handle = *mut c_void;

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(
        path: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16)
        -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn DeviceIoControl(
        handle: Handle,
        code: u32,
        input: *const c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn GetOverlappedResult(
        handle: Handle,
        overlapped: *mut Overlapped,
        returned: *mut u32,
        wait: i32,
    ) -> i32;
    fn CancelIoEx(handle: Handle, overlapped: *mut Overlapped) -> i32;
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
struct OwnedHandle(Handle);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Only metadata from the exact present device can construct this binding.
/// No Debug/serialization/path getter is provided for private device identities.
#[derive(Clone)]
pub struct Target {
    hid: String,
    control: String,
    container: [u8; 16],
}
impl Target {
    pub fn discover(device: &Device) -> io::Result<Self> {
        if device.pid != 0x025d || device.interface != 2 {
            return Err(invalid("Ornata configuration interface2 required"));
        }
        let hid = device
            .path
            .to_str()
            .map_err(|_| invalid("Invalid HID identity"))?;
        if sensor_rotation::hid_identity(hid)? != (0x025d, 2) {
            return Err(invalid("Selected HID identity mismatch"));
        }
        let container = sensor_rotation::interface_container_id(hid)?;
        let mut matches = Vec::new();
        for interface in driver_control::enumerate_control_interfaces()? {
            if interface.product_id() == 0x025d
                && interface.interface_number() == 0
                && sensor_rotation::interface_container_id(interface.path())? == container
            {
                matches.push(interface);
            }
        }
        if matches.len() != 1 {
            return Err(invalid(
                "Missing or ambiguous physical Ornata control endpoint",
            ));
        }
        Ok(Self {
            hid: hid.into(),
            control: matches.remove(0).path().into(),
            container,
        })
    }
    fn verify(&self) -> io::Result<()> {
        if sensor_rotation::hid_identity(&self.hid)? != (0x025d, 2)
            || sensor_rotation::interface_container_id(&self.hid)? != self.container
            || sensor_rotation::interface_container_id(&self.control)? != self.container
        {
            return Err(invalid("Ornata association changed"));
        }
        let present = driver_control::enumerate_control_interfaces()?;
        if present
            .iter()
            .filter(|item| {
                item.product_id() == 0x025d
                    && item.interface_number() == 0
                    && item.path() == self.control
            })
            .count()
            != 1
        {
            return Err(invalid("Ornata control endpoint is no longer present"));
        }
        Ok(())
    }
    fn open(&self) -> io::Result<OwnedHandle> {
        self.verify()?;
        if self.control.contains('\0') || self.control.encode_utf16().count() > 32767 {
            return Err(invalid("Invalid control path"));
        }
        let path: Vec<_> = self.control.encode_utf16().chain(Some(0)).collect();
        let handle =
            unsafe { CreateFileW(path.as_ptr(), 0, 3, null(), 3, 0x4000_0000, null_mut()) };
        if handle == (-1isize as Handle) {
            return Err(io::Error::last_os_error());
        }
        let handle = OwnedHandle(handle);
        // Revalidate after open as well; no subscription precedes this check.
        self.verify()?;
        Ok(handle)
    }
}

// There is no arbitrary IOCTL entry point. Interception and output operations
// cannot be expressed through this passive transport.
#[derive(Clone, Copy)]
enum Operation {
    Receive,
    Notifications(bool),
}
impl Operation {
    fn parameters(self) -> (u32, Vec<u8>, usize) {
        match self {
            Self::Receive => (RECEIVE, Vec::new(), RECORD_BYTES),
            Self::Notifications(enabled) => (NOTIFY, u32::from(enabled).to_le_bytes().to_vec(), 0),
        }
    }
}

/// Heap-stable OVERLAPPED and buffers remain owned until terminal completion.
/// Drop may block only its dedicated worker while draining a broken driver.
struct Request<'a> {
    driver: &'a OwnedHandle,
    event: OwnedHandle,
    overlapped: Box<Overlapped>,
    input: Vec<u8>,
    output: Vec<u8>,
    pending: bool,
    returned: usize,
}
impl<'a> Request<'a> {
    fn start(driver: &'a OwnedHandle, operation: Operation) -> io::Result<Self> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let (code, input, bytes) = operation.parameters();
        let mut request = Self {
            driver,
            event: OwnedHandle(event),
            overlapped: Box::new(Overlapped {
                internal: 0,
                internal_high: 0,
                offset: 0,
                offset_high: 0,
                event,
            }),
            input,
            output: vec![0; bytes],
            pending: false,
            returned: 0,
        };
        let mut returned = 0;
        let ok = unsafe {
            DeviceIoControl(
                driver.0,
                code,
                if request.input.is_empty() {
                    null()
                } else {
                    request.input.as_ptr().cast()
                },
                request.input.len() as u32,
                if request.output.is_empty() {
                    null_mut()
                } else {
                    request.output.as_mut_ptr().cast()
                },
                bytes as u32,
                &mut returned,
                (&mut *request.overlapped as *mut Overlapped).cast(),
            )
        };
        if ok != 0 {
            request.returned = returned as usize;
        } else {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(997) {
                return Err(error);
            }
            request.pending = true;
        }
        Ok(request)
    }
    fn poll(&mut self, milliseconds: u32) -> io::Result<bool> {
        if !self.pending {
            return self.check_size().map(|_| true);
        }
        let wait = unsafe { WaitForSingleObject(self.event.0, milliseconds) };
        if wait == 258 {
            return Ok(false);
        }
        if wait != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut bytes = 0;
        let ok =
            unsafe { GetOverlappedResult(self.driver.0, &mut *self.overlapped, &mut bytes, 0) };
        if ok == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(996) {
                return Ok(false);
            }
            self.pending = false;
            return Err(error);
        }
        self.pending = false;
        self.returned = bytes as usize;
        self.check_size().map(|_| true)
    }
    fn check_size(&self) -> io::Result<()> {
        if self.returned > self.output.len() {
            Err(invalid("Notification output exceeds its owned buffer"))
        } else {
            Ok(())
        }
    }
    fn cancel_and_drain(&mut self) {
        if self.pending {
            unsafe {
                CancelIoEx(self.driver.0, &mut *self.overlapped);
            }
            // Cancellation is a request, not completion. Keep both buffers and
            // the handle alive even if a faulty driver never acknowledges it.
            while self.pending {
                match self.poll(100) {
                    Ok(_) => {}
                    Err(_) if !self.pending => {}
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        }
    }
    fn bytes(&self) -> &[u8] {
        &self.output[..self.returned]
    }
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.cancel_and_drain();
    }
}

fn control(driver: &OwnedHandle, enabled: bool, stop: Option<&AtomicBool>) -> io::Result<()> {
    let mut request = Request::start(driver, Operation::Notifications(enabled))?;
    let start = Instant::now();
    loop {
        if request.poll(25)? {
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(2)
            || stop.is_some_and(|stop| stop.load(Ordering::Acquire))
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Notification control cancelled or timed out",
            ));
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn decode(bytes: &[u8]) -> io::Result<Option<KeyboardInput>> {
    if bytes.len() != RECORD_BYTES {
        return Err(invalid("Driver notification must be304 bytes"));
    }
    // Only the independently recovered ordinary keyboard notification. Preserve
    // unknown masks/kinds as counts; do not reinterpret them as key events.
    if u32_at(bytes, 8) != 4 || !matches!(u32_at(bytes, 16), 1 | 0x0b) {
        return Ok(None);
    }
    Ok(Some(KeyboardInput {
        unit_id: u16_at(bytes, 20),
        make_code: u16_at(bytes, 22),
        flags: u16_at(bytes, 24),
        reserved: u16_at(bytes, 26),
        extra_information: u32_at(bytes, 28),
    }))
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub association_verified: bool,
    /// Enable request completed successfully; not a GET/readback or proof that
    /// physical input reaches this endpoint.
    pub subscription_enabled: bool,
    pub subscription_disabled: bool,
    pub received_records: u64,
    pub keyboard_records: u64,
    pub other_records: u64,
    pub cancel_requested: bool,
    pub pending_drained: bool,
    pub finished: bool,
    pub error: Option<String>,
}
fn update(state: &Mutex<Snapshot>, f: impl FnOnce(&mut Snapshot)) {
    if let Ok(mut state) = state.lock() {
        f(&mut state);
    }
}
struct Admission;
impl Drop for Admission {
    fn drop(&mut self) {
        LIVE.store(false, Ordering::Release);
    }
}

/// Explicit, bounded passive stream. At most one worker per process; a detached
/// draining worker keeps admission closed until its actual completion.
pub struct Session {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<Snapshot>>,
    events: mpsc::Receiver<KeyboardInput>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Session {
    pub fn start(target: Target) -> io::Result<Self> {
        if LIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Keyboard notification worker is still live",
            ));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let (tx, events) = mpsc::sync_channel(QUEUE_RECORDS);
        let worker_stop = stop.clone();
        let worker_state = state.clone();
        let worker = thread::Builder::new()
            .name("ornata-notifications".into())
            .spawn(move || {
                let _admission = Admission;
                run(target, worker_stop, worker_state, tx);
            });
        match worker {
            Ok(worker) => Ok(Self {
                stop,
                state,
                events,
                worker: Some(worker),
            }),
            Err(error) => {
                LIVE.store(false, Ordering::Release);
                Err(error)
            }
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_else(|_| Snapshot {
                error: Some("Notification state unavailable".into()),
                ..Default::default()
            })
    }
    pub fn try_next(&self) -> Option<KeyboardInput> {
        self.events.try_recv().ok()
    }
    pub fn stop(&mut self) -> bool {
        self.stop.store(true, Ordering::Release);
        let start = Instant::now();
        while self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
            && start.elapsed() < Duration::from_millis(300)
        {
            thread::sleep(Duration::from_millis(2));
        }
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
        self.snapshot().finished
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    target: Target,
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<Snapshot>>,
    tx: mpsc::SyncSender<KeyboardInput>,
) {
    let result = (|| -> io::Result<()> {
        let driver = target.open()?;
        update(&state, |state| state.association_verified = true);
        // Even failed enable acknowledgement can have applied the subscription.
        let result = (|| -> io::Result<()> {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            control(&driver, true, Some(&stop))?;
            update(&state, |state| state.subscription_enabled = true);
            while !stop.load(Ordering::Acquire) {
                let mut request = Request::start(&driver, Operation::Receive)?;
                loop {
                    if stop.load(Ordering::Acquire) {
                        update(&state, |state| state.cancel_requested = true);
                        request.cancel_and_drain();
                        break;
                    }
                    if request.poll(25)? {
                        let event = decode(request.bytes())?;
                        update(&state, |state| {
                            state.received_records += 1;
                            if event.is_some() {
                                state.keyboard_records += 1;
                            } else {
                                state.other_records += 1;
                            }
                        });
                        if let Some(event) = event {
                            tx.try_send(event).map_err(|_| {
                                io::Error::new(
                                    io::ErrorKind::Other,
                                    "Notification consumer is absent or too slow",
                                )
                            })?;
                        }
                        break;
                    }
                }
            }
            Ok(())
        })();
        // Receive and enable requests were drained before this point. Disable
        // this file's subscription only; no global hook collection is touched.
        let disabled = control(&driver, false, None);
        update(&state, |state| {
            state.pending_drained = true;
            state.subscription_disabled = disabled.is_ok();
        });
        result.and(disabled)
    })();
    update(&state, |state| {
        state.finished = true;
        if let Err(error) = result {
            state.error = Some(format!(
                "notification I/O failed ({:?}, OS {:?})",
                error.kind(),
                error.raw_os_error()
            ));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passive_operations_cannot_encode_interception_or_input_submission() {
        assert_eq!(Operation::Receive.parameters(), (RECEIVE, vec![], 304));
        assert_eq!(
            Operation::Notifications(true).parameters(),
            (NOTIFY, vec![1, 0, 0, 0], 0)
        );
        assert_eq!(
            Operation::Notifications(false).parameters(),
            (NOTIFY, vec![0, 0, 0, 0], 0)
        );
    }
    #[test]
    fn exact_keyboard_layout_and_opaque_unknown_kinds() {
        let mut bytes = [0; 304];
        bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&11u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&7u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&0x1eu16.to_le_bytes());
        bytes[24..26].copy_from_slice(&3u16.to_le_bytes());
        bytes[28..32].copy_from_slice(&0x12345678u32.to_le_bytes());
        let event = decode(&bytes).unwrap().unwrap();
        assert_eq!(
            (
                event.unit_id,
                event.make_code,
                event.flags,
                event.extra_information
            ),
            (7, 0x1e, 3, 0x12345678)
        );
        // The normal role2 keyboard callback uses1; the separately recovered
        // helper uses0xB. Both copy the same12-byte KEYBOARD_INPUT_DATA.
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(decode(&bytes).unwrap(), Some(event));
        for length in [0, 12, 32, 303] {
            assert!(decode(&bytes[..length]).is_err());
        }
        for mask in [0u32, 2, 6, 8, u32::MAX] {
            bytes[8..12].copy_from_slice(&mask.to_le_bytes());
            assert!(decode(&bytes).unwrap().is_none());
        }
        bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&12u32.to_le_bytes());
        assert!(decode(&bytes).unwrap().is_none());
    }
    #[test]
    fn native_overlapped_layout_matches_windows_abi() {
        assert_eq!(
            std::mem::size_of::<Overlapped>(),
            if cfg!(target_pointer_width = "64") {
                32
            } else {
                20
            }
        );
        assert_eq!(
            std::mem::offset_of!(Overlapped, event),
            if cfg!(target_pointer_width = "64") {
                24
            } else {
                16
            }
        );
    }
}
