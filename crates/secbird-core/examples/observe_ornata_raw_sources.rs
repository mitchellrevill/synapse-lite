//! Explicit passive per-interface event attribution. No selectors or key output.
//! Usage: <fresh-report.json> --observe-passive-gated <1..10>
#[cfg(windows)]
mod probe {
    use secbird_core::{
        discover,
        hidapi::HidApi,
        keyboard_input_identity::{self, RawKeyboardSet},
        keyboard_notifications::{Session, Target},
        Db,
    };
    use serde_json::{json, Value};
    use std::{
        collections::BTreeSet,
        io,
        mem::{size_of, zeroed},
        path::Path,
        ptr::{null, null_mut},
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::*, WindowsAndMessaging::*},
    };
    fn invalid(text: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, text)
    }
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn registrations() -> io::Result<Vec<RAWINPUTDEVICE>> {
        for _ in 0..3 {
            let mut count = 0;
            let result = unsafe {
                GetRegisteredRawInputDevices(
                    null_mut(),
                    &mut count,
                    size_of::<RAWINPUTDEVICE>() as u32,
                )
            };
            if result == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            if result != 0 || count > 512 {
                return Err(invalid("Invalid registered input count"));
            }
            if count == 0 {
                return Ok(vec![]);
            }
            let mut rows = vec![unsafe { zeroed() }; count as usize];
            let result = unsafe {
                GetRegisteredRawInputDevices(
                    rows.as_mut_ptr(),
                    &mut count,
                    size_of::<RAWINPUTDEVICE>() as u32,
                )
            };
            if result == u32::MAX {
                if io::Error::last_os_error().raw_os_error() == Some(122) {
                    continue;
                }
                return Err(io::Error::last_os_error());
            }
            if result as usize > rows.len() {
                return Err(invalid("Input registration grew past capacity"));
            }
            rows.truncate(result as usize);
            return Ok(rows);
        }
        Err(invalid("Input registrations did not stabilize"))
    }
    fn keyboard_registration(row: &RAWINPUTDEVICE) -> bool {
        row.usUsagePage == 1 && matches!(row.usUsage, 0 | 6)
    }
    struct Registration {
        window: HWND,
        installed: bool,
    }
    impl Registration {
        fn open(window: HWND) -> io::Result<Self> {
            if registrations()?.iter().any(keyboard_registration) {
                return Err(invalid("Existing process keyboard registration preserved"));
            }
            let row = RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: 0x2100,
                hwndTarget: window,
            };
            if unsafe { RegisterRawInputDevices(&row, 1, size_of::<RAWINPUTDEVICE>() as u32) } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let registration = Self {
                window,
                installed: true,
            };
            if !registration.owned()? {
                return Err(invalid("Keyboard registration ownership/readback mismatch"));
            }
            Ok(registration)
        }
        fn owned(&self) -> io::Result<bool> {
            let rows = registrations()?;
            let mut rows = rows.iter().filter(|row| keyboard_registration(row));
            let Some(row) = rows.next() else {
                return Ok(false);
            };
            Ok(rows.next().is_none()
                && row.usUsage == 6
                && row.hwndTarget == self.window
                && matches!(row.dwFlags, 0x100 | 0x2100))
        }
        fn release(&mut self) -> io::Result<()> {
            if !self.installed {
                return Ok(());
            }
            if !self.owned()? {
                return Err(invalid(
                    "Keyboard ownership changed; foreign registration preserved",
                ));
            }
            let row = RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: null_mut(),
            };
            if unsafe { RegisterRawInputDevices(&row, 1, size_of::<RAWINPUTDEVICE>() as u32) } == 0
            {
                return Err(io::Error::last_os_error());
            }
            if registrations()?.iter().any(keyboard_registration) {
                return Err(invalid("Keyboard registration removal unverified"));
            }
            self.installed = false;
            Ok(())
        }
    }
    impl Drop for Registration {
        fn drop(&mut self) {
            let _ = self.release();
        }
    }
    struct Context {
        bindings: RawKeyboardSet,
        arrivals: BTreeSet<usize>,
        validated: [bool; 2],
        counting: bool,
        makes: [u64; 2],
        breaks: [u64; 2],
        other_headers: u64,
        error: Option<String>,
    }
    fn selected_packet_valid(first: &RAWINPUTHEADER, packet: &RAWINPUT, copied: u32) -> bool {
        let expected = size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>();
        !first.hDevice.is_null()
            && first.dwType == RIM_TYPEKEYBOARD
            && first.dwSize as usize == expected
            && copied as usize == expected
            && packet.header.dwSize as usize == expected
            && packet.header.dwType == RIM_TYPEKEYBOARD
            && packet.header.hDevice == first.hDevice
    }
    impl Context {
        fn input(&mut self, handle: isize) -> io::Result<()> {
            let mut header: RAWINPUTHEADER = unsafe { zeroed() };
            let mut bytes = size_of::<RAWINPUTHEADER>() as u32;
            let copied = unsafe {
                GetRawInputData(
                    handle as _,
                    RID_HEADER,
                    (&mut header as *mut RAWINPUTHEADER).cast(),
                    &mut bytes,
                    size_of::<RAWINPUTHEADER>() as u32,
                )
            };
            if copied != size_of::<RAWINPUTHEADER>() as u32 || header.dwType != RIM_TYPEKEYBOARD {
                return Err(invalid("Malformed raw keyboard header"));
            }
            let Some(binding) = self
                .bindings
                .bindings()
                .iter()
                .find(|binding| binding.handle() == header.hDevice as usize)
            else {
                if self.counting {
                    self.other_headers += 1;
                }
                return Ok(());
            };
            let index = binding.interface() as usize;
            if !self.validated[index] {
                if !binding.validate()? {
                    return Err(invalid("Raw keyboard identity changed"));
                }
                self.validated[index] = true;
            }
            if !self.counting {
                return Ok(());
            }
            let mut raw: RAWINPUT = unsafe { zeroed() };
            let mut bytes = size_of::<RAWINPUT>() as u32;
            let copied = unsafe {
                GetRawInputData(
                    handle as _,
                    RID_INPUT,
                    (&mut raw as *mut RAWINPUT).cast(),
                    &mut bytes,
                    size_of::<RAWINPUTHEADER>() as u32,
                )
            };
            if !selected_packet_valid(&header, &raw, copied) {
                return Err(invalid("Malformed selected keyboard record"));
            }
            if unsafe { raw.data.keyboard.Flags } & 1 == 0 {
                self.makes[index] += 1;
            } else {
                self.breaks[index] += 1;
            }
            Ok(())
        }
        fn device_change(&mut self, event: usize, handle: usize) -> io::Result<()> {
            if event == GIDC_ARRIVAL as usize && self.arrivals.remove(&handle) {
                for binding in self.bindings.bindings() {
                    if binding.handle() == handle && !binding.validate()? {
                        return Err(invalid("Initial keyboard arrival changed identity"));
                    }
                }
                return Ok(());
            }
            Err(invalid("Keyboard inventory changed; discard observation"))
        }
    }
    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let context = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Context;
        if !context.is_null() {
            let context = &mut *context;
            if context.error.is_none() {
                let result = match message {
                    WM_INPUT => context.input(l),
                    WM_INPUT_DEVICE_CHANGE => context.device_change(w, l as usize),
                    _ => Ok(()),
                };
                if result.is_err() {
                    context.error = Some("Raw keyboard observation invalidated".into());
                }
            }
        }
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        }
        DefWindowProcW(window, message, w, l)
    }
    struct Window {
        handle: HWND,
        class: Vec<u16>,
        instance: *mut std::ffi::c_void,
    }
    impl Window {
        fn open(context: &mut Context) -> io::Result<Self> {
            let class = wide(&format!("OrnataRawSource{}", std::process::id()));
            let instance = unsafe { GetModuleHandleW(null()) };
            let mut definition: WNDCLASSW = unsafe { zeroed() };
            definition.lpfnWndProc = Some(window_proc);
            definition.hInstance = instance;
            definition.lpszClassName = class.as_ptr();
            if unsafe { RegisterClassW(&definition) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let handle = unsafe {
                CreateWindowExW(
                    0,
                    class.as_ptr(),
                    class.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    instance,
                    (context as *mut Context).cast(),
                )
            };
            if handle.is_null() {
                unsafe {
                    UnregisterClassW(class.as_ptr(), instance);
                };
                return Err(io::Error::last_os_error());
            }
            Ok(Self {
                handle,
                class,
                instance,
            })
        }
    }
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.handle);
                UnregisterClassW(self.class.as_ptr(), self.instance);
            }
        }
    }
    fn pump() -> io::Result<()> {
        for _ in 0..4096 {
            let mut message: MSG = unsafe { zeroed() };
            if unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } == 0 {
                return Ok(());
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            };
        }
        Err(invalid(
            "Raw input message backlog exceeded observation limit",
        ))
    }
    pub fn sample(seconds: u64, ready: &Path, signal: &Path) -> io::Result<Value> {
        let api = HidApi::new().map_err(|_| invalid("HID discovery failed"))?;
        let db = Db::builtin().map_err(|_| invalid("Device database failed"))?;
        let mut found: Vec<_> = discover(&api, &db)
            .into_iter()
            .filter(|device| device.pid == 0x025d && device.interface == 2)
            .collect();
        if found.len() != 1 {
            return Err(invalid("Selected Ornata missing or ambiguous"));
        }
        let mut device = found.remove(0);
        device.enforce_query_only();
        let bindings = keyboard_input_identity::discover(&device)?;
        for binding in bindings.bindings() {
            if !binding.validate()? {
                return Err(invalid("Keyboard binding changed before registration"));
            }
        }
        let arrivals = bindings.initial_handles().iter().copied().collect();
        let mut context = Box::new(Context {
            bindings,
            arrivals,
            validated: [false; 2],
            counting: false,
            makes: [0; 2],
            breaks: [0; 2],
            other_headers: 0,
            error: None,
        });
        let window = Window::open(&mut context)?;
        let mut registration = Registration::open(window.handle)?;
        let mut native = Session::start(Target::discover(&device)?)?;
        let mut vendor_sample_records = None;
        let result = (|| -> io::Result<()> {
            let setup = Instant::now();
            while !native.snapshot().subscription_enabled
                && !native.snapshot().finished
                && setup.elapsed() < Duration::from_secs(4)
            {
                pump()?;
                thread::sleep(Duration::from_millis(2));
            }
            pump()?;
            if !native.snapshot().subscription_enabled
                || native.snapshot().finished
                || context.error.is_some()
                || !registration.owned()?
            {
                return Err(invalid("Observation setup did not become ready"));
            }
            use std::io::Write;
            let mut marker = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(ready)?;
            marker
                .write_all(b"{\"notification_stream_ready\":true,\"raw_keyboard_ready\":true}\n")?;
            marker.sync_all()?;
            let waiting = Instant::now();
            while !signal.exists() {
                pump()?;
                while native.try_next().is_some() {}
                if waiting.elapsed() > Duration::from_secs(30) || context.error.is_some() {
                    return Err(invalid("Sample gate expired or device changed"));
                }
                thread::sleep(Duration::from_millis(2));
            }
            pump()?;
            while native.try_next().is_some() {}
            context.counting = true;
            let vendor_start = native.snapshot().received_records;
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(seconds) {
                pump()?;
                while native.try_next().is_some() {}
                if context.error.is_some() || native.snapshot().finished || !registration.owned()? {
                    return Err(invalid("Input observation lost its owner or device"));
                }
                thread::sleep(Duration::from_millis(2));
            }
            context.counting = false;
            vendor_sample_records = Some(
                native
                    .snapshot()
                    .received_records
                    .saturating_sub(vendor_start),
            );
            for binding in context.bindings.bindings() {
                if !binding.validate()? {
                    return Err(invalid("Keyboard identity changed by sample end"));
                }
            }
            Ok(())
        })();
        context.counting = false;
        let native_stopped = native.stop();
        let snapshot = native.snapshot();
        let raw_removed = registration.release().is_ok();
        drop(registration);
        drop(window);
        Ok(
            json!({"scope":"explicit passive selected-Ornata per-interface raw counts plus vendor notifications",
            "snapshot":snapshot,"vendor_sample_records":vendor_sample_records,"stop_completed":native_stopped,"raw_registration_removed":raw_removed,
            "raw_observation_valid":result.is_ok()&&context.error.is_none(),"sample_gated":true,
            "raw_endpoints":[{"mi":"00","make_records":context.makes[0],"break_records":context.breaks[0]},
                {"mi":"01","make_records":context.makes[1],"break_records":context.breaks[1]}],
            "other_keyboard_headers":context.other_headers,"selected_raw_records":context.makes.iter().sum::<u64>()+context.breaks.iter().sum::<u64>(),
            "key_identities_or_text_emitted":false,"foreign_key_payloads_read":false,"paths_or_serials_emitted":false,
            "selector_mutations":0,"redirection_changes":0,"input_submissions":0,"hid_sets":0}),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn packet_identity_and_complete_keyboard_size_are_required_before_union_read() {
            let size = (size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>()) as u32;
            let first = RAWINPUTHEADER {
                dwType: RIM_TYPEKEYBOARD,
                dwSize: size,
                hDevice: 1usize as _,
                wParam: 0,
            };
            let mut packet: RAWINPUT = unsafe { zeroed() };
            packet.header = first;
            assert!(selected_packet_valid(&first, &packet, size));
            for copied in [0, size - 1, size + 1, u32::MAX] {
                assert!(!selected_packet_valid(&first, &packet, copied));
            }
            packet.header.hDevice = 2usize as _;
            assert!(!selected_packet_valid(&first, &packet, size));
            packet.header = first;
            packet.header.dwType = RIM_TYPEMOUSE;
            assert!(!selected_packet_valid(&first, &packet, size));
            packet.header = first;
            packet.header.dwSize = size - 1;
            assert!(!selected_packet_valid(&first, &packet, size));
        }
        #[test]
        fn keyboard_class_conflicts_include_page_wide_registrations() {
            let row = |page, usage| RAWINPUTDEVICE {
                usUsagePage: page,
                usUsage: usage,
                dwFlags: 0,
                hwndTarget: null_mut(),
            };
            assert!(keyboard_registration(&row(1, 6)));
            assert!(keyboard_registration(&row(1, 0)));
            assert!(!keyboard_registration(&row(1, 2)));
            assert!(!keyboard_registration(&row(12, 1)));
        }
    }
}
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 4 && args[0] == "--gated-child" {
        let seconds = args[1].parse::<u64>()?;
        if !(1..=10).contains(&seconds) {
            return Err("Duration1..10 required".into());
        }
        let report = probe::sample(
            seconds,
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        )
        .unwrap_or_else(
            |_| serde_json::json!({"status":"failed","stage":"raw_source_observation"}),
        );
        println!("{report}");
        return Ok(());
    }
    if args.len() != 3 || args[1] != "--observe-passive-gated" {
        return Err("Use <fresh-report.json> --observe-passive-gated <1..10>".into());
    }
    let seconds = args[2].parse::<u64>()?;
    let path = std::path::Path::new(&args[0]);
    let ready = path.with_extension("ready.json");
    let signal = path.with_extension("gate");
    if !(1..=10).contains(&seconds) || path.exists() || ready.exists() || signal.exists() {
        return Err("Fresh report/gates and duration1..10 required".into());
    }
    let report = probe::sample(seconds, &ready, &signal)?;
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    output.write_all(&serde_json::to_vec_pretty(&report)?)?;
    output.sync_all()?;
    println!("{report}");
    if report["raw_observation_valid"] != true
        || report["raw_registration_removed"] != true
        || report["stop_completed"] != true
    {
        return Err("Observation/cleanup did not pass".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The source-attribution probe requires Windows");
}
