//! Dedicated passive Raw Input message thread. This avoids GUI paint latency in
//! movement timing. Source matching/registration policy and Speed stay shared.
//! Monotonic Instant ->100ns is an explicit native clock choice, not a claim
//! about the vendor's recovered timestamp clock. No trajectory is retained.
use super::mouse_feedback::{movement, Registration, WindowsRegistry};
use super::mouse_speed::Speed;
use secbird_core::mouse_input_identity::RawMouseBinding;
use std::collections::BTreeSet;
use std::io;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceList, RAWINPUT, RAWINPUTDEVICELIST, RAWINPUTHEADER,
    RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const STOP: u32 = WM_APP + 0x6a1;
const TIMER: usize = 1;
const STARTUP_DRAIN_LIMIT: usize = 4096;
const MAX_INVENTORY: usize = 512;
static WORKER_ALIVE: AtomicBool = AtomicBool::new(false);
static CLASS: OnceLock<Result<(), i32>> = OnceLock::new();

#[derive(Default)]
struct InitialArrivals {
    pending: BTreeSet<usize>,
}
impl InitialArrivals {
    fn from_handles(handles: Vec<usize>) -> io::Result<Self> {
        if handles.len() > MAX_INVENTORY {
            return Err(io::Error::other("Mouse inventory exceeds512 handles"));
        }
        let mut pending = BTreeSet::new();
        for handle in handles {
            if handle == 0 || !pending.insert(handle) {
                return Err(io::Error::other(
                    "Mouse inventory has a null or repeated handle",
                ));
            }
        }
        Ok(Self { pending })
    }
    fn contains(&self, handle: usize) -> bool {
        self.pending.contains(&handle)
    }
    fn ignore_once(&mut self, event: usize, handle: usize) -> bool {
        event == GIDC_ARRIVAL as usize && handle != 0 && self.pending.remove(&handle)
    }
}

fn capture_initial_inventory() -> io::Result<InitialArrivals> {
    for _ in 0..3 {
        let mut count = 0;
        let sized = unsafe {
            GetRawInputDeviceList(
                null_mut(),
                &mut count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            )
        };
        if sized == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if sized != 0 || count as usize > MAX_INVENTORY {
            return Err(io::Error::other(
                "Raw mouse inventory sizing is invalid or exceeds512 devices",
            ));
        }
        if count == 0 {
            return InitialArrivals::from_handles(Vec::new());
        }
        let mut entries: Vec<RAWINPUTDEVICELIST> =
            (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
        let capacity = count;
        let copied = unsafe {
            GetRawInputDeviceList(
                entries.as_mut_ptr(),
                &mut count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            )
        };
        if copied == u32::MAX {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(122) {
                continue;
            }
            return Err(error);
        }
        if copied > capacity || count > capacity {
            continue;
        }
        entries.truncate(copied as usize);
        if entries.iter().any(|entry| entry.dwType > 2) {
            return Err(io::Error::other(
                "Raw mouse inventory returned an unknown device type",
            ));
        }
        return InitialArrivals::from_handles(
            entries
                .into_iter()
                .filter(|entry| entry.dwType == RIM_TYPEMOUSE)
                .map(|entry| entry.hDevice as usize)
                .collect(),
        );
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "Raw mouse inventory changed during all three attempts",
    ))
}

fn owns_window(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) != 0 && pid == GetCurrentProcessId() }
}

fn post_owned(hwnd: HWND, message: u32) -> bool {
    // Detached cleanup must not notify a foreign process that reused an HWND.
    owns_window(hwnd) && unsafe { PostMessageW(hwnd, message, 0, 0) != 0 }
}

fn drain_bounded(mut discard_next: impl FnMut() -> bool, more: impl FnOnce() -> bool) -> bool {
    for _ in 0..STARTUP_DRAIN_LIMIT {
        if !discard_next() {
            return true;
        }
    }
    !more()
}

unsafe fn discard_input_copy(hwnd: HWND) -> bool {
    let mut message: MSG = std::mem::zeroed();
    if PeekMessageW(&mut message, hwnd, WM_INPUT, WM_INPUT, PM_REMOVE) == 0 {
        return false;
    }
    // Only this observer's queued copies are removed. DefWindowProc performs
    // foreground cleanup; legacy/application input is never suppressed.
    DefWindowProcW(
        message.hwnd,
        message.message,
        message.wParam,
        message.lParam,
    );
    true
}

unsafe fn input_copy_waiting(hwnd: HWND) -> bool {
    let mut message: MSG = std::mem::zeroed();
    PeekMessageW(&mut message, hwnd, WM_INPUT, WM_INPUT, PM_NOREMOVE) != 0
}

#[derive(Clone, Debug, Default)]
pub(super) struct Sample {
    pub(super) active: bool,
    pub(super) registered: bool,
    pub(super) speed: f64,
    pub(super) accepted_records: u64,
    pub(super) error: Option<String>,
    pub(super) device_changed: bool,
}

#[derive(Default)]
struct Latest {
    sample: Sample,
    pending: bool,
}
#[derive(Default)]
struct Mailbox(Mutex<Latest>);
impl Mailbox {
    fn publish(
        &self,
        mut sample: Sample,
        notify: bool,
        cancelled: &AtomicBool,
        post: impl FnOnce() -> bool,
    ) {
        let mut latest = self.0.lock().unwrap_or_else(|error| error.into_inner());
        // Cancellation wins even if input was already being decoded. The check
        // is inside the same lock as the UI's synchronous inactive publication.
        if cancelled.load(Ordering::Acquire) {
            sample.active = false;
            sample.speed = 0.;
        }
        latest.sample = sample;
        if notify && !latest.pending {
            latest.pending = post();
        }
    }
    fn snapshot(&self) -> Sample {
        let mut latest = self.0.lock().unwrap_or_else(|error| error.into_inner());
        latest.pending = false;
        latest.sample.clone()
    }
    fn cancel(&self) {
        let mut latest = self.0.lock().unwrap_or_else(|error| error.into_inner());
        latest.sample.active = false;
        latest.sample.speed = 0.;
    }
}

struct Alive;
impl Drop for Alive {
    fn drop(&mut self) {
        WORKER_ALIVE.store(false, Ordering::Release);
    }
}

pub(super) struct Session {
    cancelled: Arc<AtomicBool>,
    window: Arc<AtomicUsize>,
    mailbox: Arc<Mailbox>,
    thread: Option<JoinHandle<()>>,
}
impl Session {
    /// Starts asynchronously. Initial snapshot is inactive; actual registration
    /// and full metadata validation happen on the dedicated worker, not the UI.
    pub(super) fn start(
        binding: RawMouseBinding,
        notify_hwnd: HWND,
        notify_message: u32,
    ) -> io::Result<Self> {
        if notify_hwnd.is_null()
            || !(WM_APP..=0xbfff).contains(&notify_message)
            || !owns_window(notify_hwnd)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Missing feedback notification window/message",
            ));
        }
        if WORKER_ALIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Previous mouse observer has not finished cleanup",
            ));
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let window = Arc::new(AtomicUsize::new(0));
        let mailbox = Arc::new(Mailbox::default());
        let worker_cancelled = cancelled.clone();
        let worker_window = window.clone();
        let worker_mailbox = mailbox.clone();
        let notify = notify_hwnd as usize;
        let spawned = thread::Builder::new()
            .name("mouse-feedback".into())
            .spawn(move || {
                let _alive = Alive;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        binding,
                        worker_cancelled.clone(),
                        worker_window,
                        worker_mailbox.clone(),
                        notify,
                        notify_message,
                    );
                }));
                if result.is_err() {
                    let mut sample = worker_mailbox.snapshot();
                    sample.active = false;
                    sample.speed = 0.;
                    sample.error = Some("Mouse observer stopped after an internal failure".into());
                    worker_mailbox.publish(sample, true, &worker_cancelled, || {
                        post_owned(notify as HWND, notify_message)
                    });
                }
            });
        match spawned {
            Ok(thread) => Ok(Self {
                cancelled,
                window,
                mailbox,
                thread: Some(thread),
            }),
            Err(error) => {
                WORKER_ALIVE.store(false, Ordering::Release);
                Err(error)
            }
        }
    }
    pub(super) fn snapshot(&self) -> Sample {
        self.mailbox.snapshot()
    }
    /// Cancellation is immediate for displayed state. Never blocks the UI on an
    /// unbounded join: after100ms ownership is detached, with global admission
    /// closed until that worker actually completes cleanup.
    pub(super) fn stop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.mailbox.cancel();
        let hwnd = self.window.load(Ordering::Acquire);
        if hwnd != 0 {
            post_owned(hwnd as HWND, STOP);
        }
        let Some(worker) = self.thread.take() else {
            return;
        };
        let until = Instant::now() + Duration::from_millis(100);
        while !worker.is_finished() && Instant::now() < until {
            thread::sleep(Duration::from_millis(1));
        }
        if worker.is_finished() {
            let _ = worker.join();
        }
        // Otherwise dropping JoinHandle detaches; Alive remains worker-owned.
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Context {
    binding: Option<RawMouseBinding>,
    initial_arrivals: InitialArrivals,
    first_record_validated: bool,
    cancelled: Arc<AtomicBool>,
    window: Arc<AtomicUsize>,
    mailbox: Arc<Mailbox>,
    notify: usize,
    notify_message: u32,
    registration: Registration,
    epoch: Instant,
    speed: Speed,
    sample: Sample,
    timer: bool,
}
impl Context {
    fn device_change(&mut self, event: usize, handle: usize) {
        if self.cancelled.load(Ordering::Acquire) {
            return;
        }
        // Observed Windows behavior: INPUTSINK|DEVNOTIFY registration queues an
        // ARRIVAL for existing inventory. Admit only one arrival per pre-captured
        // handle, without a timing grace period. Selected identity is rechecked;
        // removal, unknown/new handles and repeated arrivals always invalidate.
        if self.initial_arrivals.ignore_once(event, handle) {
            if let Some(binding) = self.binding.as_ref() {
                if handle == binding.handle()
                    && !matches!(binding.matches_live_handle(handle), Ok(true))
                {
                    self.fail(
                        "Selected mouse changed during its initial arrival".into(),
                        true,
                    );
                } else if handle == binding.handle() {
                    // If this initial metadata notification arrived after some
                    // movement, require the first-record validation/drain again
                    // so its lookup latency cannot become a speed spike.
                    self.first_record_validated = false;
                    self.speed.clear();
                    self.epoch = Instant::now();
                    self.publish(true);
                }
            }
            return;
        }
        self.fail("Mouse device list changed; refresh Advanced".into(), true);
    }
    fn now(&self) -> u64 {
        (self.epoch.elapsed().as_nanos() / 100).min(u128::from(u64::MAX - 1)) as u64 + 1
    }
    fn hwnd(&self) -> HWND {
        self.window.load(Ordering::Acquire) as HWND
    }
    fn publish(&mut self, notify: bool) {
        self.sample.registered = self.registration.is_owned();
        self.sample.speed = if self.sample.active {
            self.speed.value()
        } else {
            0.
        };
        let target = self.notify;
        let message = self.notify_message;
        self.mailbox
            .publish(self.sample.clone(), notify, &self.cancelled, || {
                post_owned(target as HWND, message)
            });
    }
    fn fail(&mut self, error: String, device_changed: bool) {
        self.cancelled.store(true, Ordering::Release);
        self.binding = None;
        self.sample.active = false;
        self.sample.device_changed |= device_changed;
        self.sample.error = Some(error.chars().take(512).collect());
        self.speed.clear();
        self.publish(true);
        unsafe { PostQuitMessage(0) };
    }
    fn arm_timer(&mut self) {
        if self.speed.needs_tick() && !self.timer {
            if unsafe { SetTimer(self.hwnd(), TIMER, 16, None) } == 0 {
                self.fail("Live graph timer could not start".into(), false);
            } else {
                self.timer = true;
            }
        } else if !self.speed.needs_tick() && self.timer {
            unsafe { KillTimer(self.hwnd(), TIMER) };
            self.timer = false;
        }
    }
    fn input(&mut self, handle: LPARAM) {
        if self.cancelled.load(Ordering::Acquire) || !self.sample.active {
            return;
        }
        let Some(binding) = self.binding.as_ref() else {
            return;
        };
        let mut input: RAWINPUT = unsafe { std::mem::zeroed() };
        let mut bytes = size_of::<RAWINPUT>() as u32;
        let got = unsafe {
            GetRawInputData(
                handle as *mut _,
                RID_INPUT,
                (&mut input as *mut RAWINPUT).cast(),
                &mut bytes,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if got == u32::MAX
            || got != size_of::<RAWINPUT>() as u32
            || bytes != size_of::<RAWINPUT>() as u32
        {
            return;
        }
        let Some((x, y)) = movement(&input, binding.handle()) else {
            return;
        };
        if !self.first_record_validated {
            if !matches!(binding.matches_live_handle(binding.handle()), Ok(true)) {
                self.fail(
                    "Selected mouse identity changed before feedback began".into(),
                    false,
                );
                return;
            }
            if !matches!(self.registration.owns_live(&mut WindowsRegistry), Ok(true)) {
                self.fail(
                    "Mouse feedback registration ownership changed".into(),
                    false,
                );
                return;
            }
            self.first_record_validated = true;
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            // Metadata lookup may have delayed the queue. Discard the startup
            // record and at most4096 queued observer copies, then establish a
            // fresh ingestion-clock epoch. This intentionally avoids startup
            // backlog spikes; scheduler/native-clock parity remains unproved.
            let hwnd = self.hwnd();
            let cancelled = &self.cancelled;
            let drained = drain_bounded(
                || !cancelled.load(Ordering::Acquire) && unsafe { discard_input_copy(hwnd) },
                || unsafe { input_copy_waiting(hwnd) },
            );
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            if !drained {
                self.fail(
                    "Mouse feedback startup backlog exceeded4096 records; refresh Advanced".into(),
                    false,
                );
                return;
            }
            self.speed.clear();
            self.epoch = Instant::now();
            return;
        }
        let now = self.now();
        self.sample.accepted_records = self.sample.accepted_records.saturating_add(1);
        let changed = self.speed.push(x, y, now);
        self.publish(changed);
        self.arm_timer();
    }
    fn tick(&mut self) {
        if self.cancelled.load(Ordering::Acquire) {
            unsafe { PostQuitMessage(0) };
            return;
        }
        if !matches!(self.registration.owns_live(&mut WindowsRegistry), Ok(true)) {
            self.fail(
                "Mouse feedback registration ownership changed".into(),
                false,
            );
            return;
        }
        let now = self.now();
        let changed = self.speed.tick(now);
        self.publish(changed);
        self.arm_timer();
    }
    fn cleanup(&mut self) {
        self.sample.active = false;
        self.speed.clear();
        self.binding = None;
        let hwnd = self.hwnd();
        if self.timer && !hwnd.is_null() {
            unsafe { KillTimer(hwnd, TIMER) };
            self.timer = false;
        }
        if let Err(error) = self.registration.stop(&mut WindowsRegistry) {
            let prior = self.sample.error.take().unwrap_or_default();
            self.sample.error = Some(
                format!("{prior} Mouse feedback cleanup: {error}")
                    .chars()
                    .take(512)
                    .collect(),
            );
        }
        self.publish(true);
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn class_name() -> Vec<u16> {
    "SecbirdPassiveMouseWorkerV1"
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
fn register_class() -> io::Result<()> {
    let result = CLASS.get_or_init(|| unsafe {
        let name = class_name();
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(window_proc);
        class.hInstance = GetModuleHandleW(null());
        class.lpszClassName = name.as_ptr();
        if RegisterClassW(&class) == 0 {
            Err(io::Error::last_os_error().raw_os_error().unwrap_or(1))
        } else {
            Ok(())
        }
    });
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(io::Error::from_raw_os_error(*error)),
    }
}

fn run(
    binding: RawMouseBinding,
    cancelled: Arc<AtomicBool>,
    window: Arc<AtomicUsize>,
    mailbox: Arc<Mailbox>,
    notify: usize,
    notify_message: u32,
) {
    let mut context = Box::new(Context {
        binding: Some(binding),
        initial_arrivals: InitialArrivals::default(),
        first_record_validated: false,
        cancelled,
        window,
        mailbox,
        notify,
        notify_message,
        registration: Registration::default(),
        epoch: Instant::now(),
        speed: Speed::default(),
        sample: Sample::default(),
        timer: false,
    });
    if context.cancelled.load(Ordering::Acquire) {
        return;
    }
    if let Err(error) = register_class() {
        context.fail(format!("Mouse feedback window class: {error}"), false);
        return;
    }
    let name = class_name();
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            GetModuleHandleW(null()),
            (&mut *context as *mut Context).cast(),
        )
    };
    if hwnd.is_null() {
        context.fail("Mouse feedback window could not start".into(), false);
        return;
    }
    context.window.store(hwnd as usize, Ordering::Release);
    if !context.cancelled.load(Ordering::Acquire) {
        if !matches!(
            context
                .binding
                .as_ref()
                .map(|binding| binding.matches_live_handle(binding.handle())),
            Some(Ok(true))
        ) {
            context.fail("Selected mouse's live input identity changed".into(), false);
        } else if !context.cancelled.load(Ordering::Acquire) {
            match capture_initial_inventory() {
                Ok(inventory)
                    if context
                        .binding
                        .as_ref()
                        .is_some_and(|binding| inventory.contains(binding.handle())) =>
                {
                    context.initial_arrivals = inventory;
                    if !context.cancelled.load(Ordering::Acquire) {
                        match context
                            .registration
                            .start(&mut WindowsRegistry, hwnd as usize)
                        {
                            Ok(()) => {
                                context.sample.active = true;
                                context.epoch = Instant::now();
                                context.publish(true);
                            }
                            Err(error) => {
                                context.fail(format!("Live graph unavailable: {error}"), false)
                            }
                        }
                    }
                }
                Ok(_) => context.fail(
                    "Selected mouse left the initial input inventory".into(),
                    true,
                ),
                Err(error) => {
                    context.fail(format!("Live graph inventory unavailable: {error}"), false)
                }
            }
        }
    }
    if !context.cancelled.load(Ordering::Acquire) {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            loop {
                let result = GetMessageW(&mut message, null_mut(), 0, 0);
                if result == 0 {
                    break;
                }
                if result < 0 {
                    context.fail("Mouse feedback message loop failed".into(), false);
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
                if context.cancelled.load(Ordering::Acquire) {
                    break;
                }
            }
        }
    }
    context.cleanup();
    unsafe { DestroyWindow(hwnd) };
    context.window.store(0, Ordering::Release);
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return 1;
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
    if message == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = &mut *pointer;
        match message {
            WM_INPUT => {
                context.input(lparam);
                None
            } // DefWindowProc releases foreground raw-input resources.
            WM_INPUT_DEVICE_CHANGE => {
                context.device_change(wparam, lparam as usize);
                Some(0)
            }
            WM_TIMER if wparam == TIMER => {
                context.tick();
                Some(0)
            }
            STOP | WM_CLOSE => {
                context.cancelled.store(true, Ordering::Release);
                context.sample.active = false;
                context.speed.clear();
                context.publish(true);
                PostQuitMessage(0);
                Some(0)
            }
            _ => None,
        }
    }));
    match result {
        Ok(Some(value)) => value,
        Ok(None) => DefWindowProcW(hwnd, message, wparam, lparam),
        Err(_) => {
            (&mut *pointer).fail("Mouse observer callback failed".into(), false);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "passive physical wired mouse observation for5s; no SET or injected movement; run alone"]
    fn physical_mouse_feedback_observation_and_cleanup() {
        use super::super::mouse_feedback::Registry;
        use secbird_core::{hidapi::HidApi, Db, Device};
        let api = HidApi::new().unwrap();
        let db = Db::builtin().unwrap();
        let candidates: Vec<_> = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == 0x1532
                    && d.product_id() == 196
                    && d.interface_number() == 0
                    && d.usage_page() == 1
                    && d.usage() == 2
            })
            .collect();
        assert_eq!(
            candidates.len(),
            1,
            "Needs one wired00C4 standard mouse collection"
        );
        let info = candidates[0];
        let mut device = Device::open(
            Box::new(api.open_path(info.path()).unwrap()),
            196,
            0,
            info.path().to_owned(),
            &[31],
            db.get(196).cloned(),
        )
        .expect("Mouse serial GET handshake");
        device.enforce_query_only();
        let binding = secbird_core::mouse_input_identity::discover(&device).unwrap();
        assert!(binding.matches_live_handle(binding.handle()).unwrap());
        let before = WindowsRegistry.mouse().unwrap();
        assert!(
            before.is_none(),
            "Run alone; another observer owns process registration"
        );
        register_class().unwrap();
        let name = class_name();
        let notify = unsafe {
            CreateWindowExW(
                0,
                name.as_ptr(),
                null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        assert!(!notify.is_null());
        let mut session = Session::start(binding, notify, WM_APP + 109).unwrap();
        let began = Instant::now();
        let mut registered = false;
        let mut peak = 0f64;
        let mut accepted = 0;
        while began.elapsed() < Duration::from_secs(5) {
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, notify, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            let sample = session.snapshot();
            assert!(sample.error.is_none(), "Observer error: {:?}", sample.error);
            registered |= sample.active && sample.registered;
            peak = peak.max(sample.speed);
            accepted = accepted.max(sample.accepted_records);
            thread::sleep(Duration::from_millis(10));
        }
        session.stop();
        let stopped = session.snapshot();
        assert!(!stopped.active);
        assert_eq!(stopped.speed, 0.);
        let deadline = Instant::now() + Duration::from_secs(2);
        while WORKER_ALIVE.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!WORKER_ALIVE.load(Ordering::Acquire));
        let after = WindowsRegistry.mouse().unwrap();
        assert_eq!(after, before);
        assert!(registered, "Observer did not register");
        unsafe {
            DestroyWindow(notify);
        }
        let path = std::env::temp_dir().join(format!(
            "physical-mouse-feedback-{}.json",
            std::process::id()
        ));
        let value = serde_json::json!({"pid":"00C4","query_only_device":device.is_query_only(),"registration_flags":0x2100,"registration_verified":registered,"accepted_records":accepted,"movement_observed":accepted>0,"peak_reported_count_per_ms":peak,"stop_speed_zero":true,"registration_restored":after==before,"worker_ended":true,"set_count":0,"injected_movement":false,"paths_or_serials_emitted":false});
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        eprintln!("Passive feedback physical report: {}", path.display());
    }
    use std::cell::Cell;
    #[test]
    fn mailbox_coalesces_and_snapshot_rearms_without_trajectories() {
        let mailbox = Mailbox::default();
        let cancel = AtomicBool::new(false);
        let posts = Cell::new(0);
        let post = || {
            posts.set(posts.get() + 1);
            true
        };
        mailbox.publish(
            Sample {
                active: true,
                speed: 1.,
                ..Default::default()
            },
            true,
            &cancel,
            post,
        );
        mailbox.publish(
            Sample {
                active: true,
                speed: 2.,
                accepted_records: 10,
                ..Default::default()
            },
            true,
            &cancel,
            post,
        );
        assert_eq!(posts.get(), 1);
        let latest = mailbox.snapshot();
        assert_eq!(latest.speed, 2.);
        assert_eq!(latest.accepted_records, 10);
        mailbox.publish(latest, true, &cancel, post);
        assert_eq!(posts.get(), 2);
    }
    #[test]
    fn cancellation_wins_over_already_decoded_input_and_post_failure_retries() {
        let mailbox = Mailbox::default();
        let cancel = AtomicBool::new(false);
        mailbox.publish(Sample::default(), true, &cancel, || false);
        let posts = Cell::new(0);
        mailbox.publish(Sample::default(), true, &cancel, || {
            posts.set(1);
            true
        });
        assert_eq!(posts.get(), 1);
        cancel.store(true, Ordering::Release);
        mailbox.cancel();
        mailbox.publish(
            Sample {
                active: true,
                registered: true,
                speed: 99.,
                ..Default::default()
            },
            false,
            &cancel,
            || panic!("silent update posted"),
        );
        let sample = mailbox.snapshot();
        assert!(!sample.active);
        assert_eq!(sample.speed, 0.);
        assert!(sample.registered);
    }
    #[test]
    fn shared_decoder_rejects_unrelated_absolute_zero_and_invalid_size() {
        unsafe {
            let mut input: RAWINPUT = std::mem::zeroed();
            input.header.dwType = 0;
            input.header.dwSize = size_of::<RAWINPUT>() as u32;
            input.header.hDevice = 7usize as _;
            input.data.mouse.lLastX = 3;
            input.data.mouse.lLastY = -4;
            assert_eq!(movement(&input, 7), Some((3, -4)));
            assert_eq!(movement(&input, 8), None);
            assert_eq!(movement(&input, 0), None);
            input.data.mouse.usFlags = 1;
            assert_eq!(movement(&input, 7), None);
            input.data.mouse.usFlags = 0;
            input.header.dwSize -= 1;
            assert_eq!(movement(&input, 7), None);
            input.header.dwSize += 1;
            input.header.dwType = 1;
            assert_eq!(movement(&input, 7), None);
        }
    }
    #[test]
    fn single_worker_admission_is_not_reopened_by_session_cancellation() {
        let alive = AtomicBool::new(false);
        let cancelled = AtomicBool::new(false);
        assert!(alive
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok());
        cancelled.store(true, Ordering::Release);
        assert!(alive
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
        alive.store(false, Ordering::Release);
        assert!(alive
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok());
    }
    #[test]
    fn startup_drain_is_bounded_and_refuses_remaining_backlog() {
        let removed = Cell::new(0);
        assert!(drain_bounded(
            || {
                let count = removed.get();
                if count == 3 {
                    return false;
                }
                removed.set(count + 1);
                true
            },
            || panic!("short queue queried overflow")
        ));
        assert_eq!(removed.get(), 3);
        removed.set(0);
        assert!(!drain_bounded(
            || {
                removed.set(removed.get() + 1);
                true
            },
            || true
        ));
        assert_eq!(removed.get(), 4096);
        assert!(drain_bounded(|| true, || false));
    }
    #[test]
    fn known_initial_arrival_is_admitted_once_and_new_or_repeated_arrivals_invalidate() {
        let mut inventory = InitialArrivals::from_handles(vec![7, 8]).unwrap();
        assert!(inventory.ignore_once(GIDC_ARRIVAL as usize, 7));
        assert!(!inventory.ignore_once(GIDC_ARRIVAL as usize, 7));
        assert!(!inventory.ignore_once(GIDC_ARRIVAL as usize, 9));
        assert!(inventory.ignore_once(GIDC_ARRIVAL as usize, 8));
    }
    #[test]
    fn removals_and_unknown_notifications_never_receive_a_grace_period() {
        let mut inventory = InitialArrivals::from_handles(vec![7]).unwrap();
        assert!(!inventory.ignore_once(GIDC_REMOVAL as usize, 7));
        assert!(!inventory.ignore_once(0, 7));
        assert!(!inventory.ignore_once(GIDC_ARRIVAL as usize, 0));
    }
    #[test]
    fn startup_inventory_bounds_nulls_and_duplicate_handles_are_checked() {
        assert!(InitialArrivals::from_handles((1..=512).collect()).is_ok());
        assert!(InitialArrivals::from_handles((1..=513).collect()).is_err());
        assert!(InitialArrivals::from_handles(vec![0]).is_err());
        assert!(InitialArrivals::from_handles(vec![7, 7]).is_err());
        assert!(!InitialArrivals::default().ignore_once(GIDC_ARRIVAL as usize, 7));
    }
}
