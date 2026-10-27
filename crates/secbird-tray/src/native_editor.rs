//! Native editor using source-derived Synapse layout facts and the shared core.
//!
//! Provenance: installed AppEngine 4.0.827 electron/index.css and the locally
//! cached product 605/196 UI styles, recovered 2026-10-27. See the research
//! specification for hashes and byte offsets. This is a partial native port;
//! GDI text, missing SVG hit regions and unimplemented states are not pixel parity.
//! No vendor JavaScript or CSS is embedded or executed here.

use crate::{engine::Engine, server};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::EM_SETLIMITTEXT;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const CLASS: &str = "SynapseLiteNativeEditor";
const RESPONSE_TIMER: usize = 71;
const SMOKE_TIMER: usize = 72;
const BG: u32 = 0x222222;
const PANEL: u32 = 0x111111;
const TEXT: u32 = 0xcccccc;
const MUTED: u32 = 0x999999;
const BORDER: u32 = 0x5d5d5d;
const GREEN: u32 = 0x44d62c;
const DPI_X: u16 = 101;
const DPI_Y: u16 = 102;
const COLOR: u16 = 103;
const BRIGHTNESS: u16 = 104;
const MINUTES: u16 = 105;
const MAPPING: u16 = 106;
const NEW_PROFILE: u16 = 107;
static WINDOW: AtomicUsize = AtomicUsize::new(0);
static OPENING: AtomicBool = AtomicBool::new(false);

#[link(name = "user32")]
extern "system" {
    fn GetDpiForWindow(hwnd: HWND) -> u32;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn rgb(hex: u32) -> u32 {
    ((hex >> 16) & 255) | (hex & 0x00ff00) | ((hex & 255) << 16)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Customize,
    Performance,
    Lighting,
    Power,
    Calibration,
    Advanced,
    Help,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Customize => "CUSTOMIZE",
            Self::Performance => "PERFORMANCE",
            Self::Lighting => "LIGHTING",
            Self::Power => "POWER",
            Self::Calibration => "CALIBRATION",
            Self::Advanced => "ADVANCED",
            Self::Help => "?",
        }
    }

    fn width(self) -> i32 {
        match self {
            Self::Customize => 100,
            Self::Performance | Self::Calibration => 118,
            Self::Lighting | Self::Advanced => 92,
            Self::Power => 76,
            Self::Help => 28,
        }
    }
}

#[derive(Clone)]
enum Action {
    Home,
    Device(String),
    Tab(Tab),
    Refresh,
    ProfileMenu,
    Profile(String),
    CreateProfile,
    Dpi,
    Poll(u32),
    Brightness,
    Color,
    Effect(&'static str),
    Sleep,
    ReadButtons,
    SelectButton(String),
    Assign,
    RestoreButtons,
    Minimize,
    Maximize,
    Close,
}

struct Hit {
    rect: RECT,
    action: Action,
}

enum Job {
    Refresh,
    Rescan,
    Action(&'static str, Value),
    Buttons(Value),
}

struct Reply {
    state: Option<Value>,
    buttons: Option<Value>,
    status: String,
    failed: bool,
}

/// Device access runs on one worker, under the same engine lock as the tray.
/// The UI thread never holds that lock and no browser or HTTP listener is started.
fn worker(engine: Arc<Mutex<Engine>>, jobs: mpsc::Receiver<Job>, replies: mpsc::Sender<Reply>) {
    for job in jobs {
        let result = (|| -> Result<Reply, String> {
            let mut engine = engine
                .lock()
                .map_err(|_| "Device worker is unavailable".to_string())?;
            let mut buttons = None;
            let mut status = "Ready".to_string();
            let state = match job {
                Job::Refresh => server::dispatch(&mut engine, "GET", "/api/state", &[]),
                Job::Rescan => server::dispatch(&mut engine, "POST", "/api/rescan", b"{}"),
                Job::Buttons(body) => {
                    buttons = Some(
                        server::dispatch(
                            &mut engine,
                            "POST",
                            "/api/buttons",
                            &serde_json::to_vec(&body).map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?,
                    );
                    status = "Assignments read from the device".into();
                    server::dispatch(&mut engine, "GET", "/api/state", &[])
                }
                Job::Action(path, body) => {
                    let answer = server::dispatch(
                        &mut engine,
                        "POST",
                        path,
                        &serde_json::to_vec(&body).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    if let Some(failures) = answer
                        .get("failures")
                        .and_then(Value::as_array)
                        .filter(|v| !v.is_empty())
                    {
                        return Err(format!(
                            "Some settings failed: {}",
                            failures
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("; ")
                        ));
                    }
                    if answer.get("buttons").is_some() {
                        buttons = Some(answer);
                    }
                    status = "Settings applied".into();
                    server::dispatch(&mut engine, "GET", "/api/state", &[])
                }
            }
            .map_err(|e| e.to_string())?;
            Ok(Reply {
                state: Some(state),
                buttons,
                status,
                failed: false,
            })
        })();
        let reply = result.unwrap_or_else(|error| Reply {
            state: None,
            buttons: None,
            status: error,
            failed: true,
        });
        if replies.send(reply).is_err() {
            break;
        }
    }
}

struct Fonts {
    small: HFONT,
    normal: HFONT,
    heading: HFONT,
    product: HFONT,
    loaded: Vec<PathBuf>,
}

impl Fonts {
    unsafe fn new(assets: &Path, scale: f32) -> Self {
        let mut loaded = Vec::new();
        for name in [
            "Roboto-Regular.ttf",
            "Roboto-Light.ttf",
            "RazerF5-Regular.ttf",
        ] {
            let path = assets.join(name);
            if AddFontResourceExW(wide(&path.to_string_lossy()).as_ptr(), FR_PRIVATE, null()) > 0 {
                loaded.push(path);
            }
        }
        let make = |family: &str, pixels: i32, weight: u32| {
            CreateFontW(
                -((pixels as f32 * scale).round() as i32),
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide(family).as_ptr(),
            )
        };
        Self {
            small: make("Roboto", 12, 400),
            normal: make("Roboto", 14, 400),
            heading: make("RazerF5", 20, 400),
            product: make("Roboto", 16, 400),
            loaded,
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            for font in [self.small, self.normal, self.heading, self.product] {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
            for path in &self.loaded {
                RemoveFontResourceExW(wide(&path.to_string_lossy()).as_ptr(), FR_PRIVATE, null());
            }
        }
    }
}

struct Bitmap {
    handle: HBITMAP,
    width: i32,
    height: i32,
}

impl Bitmap {
    unsafe fn load(path: &Path) -> Option<Self> {
        let handle = LoadImageW(
            null_mut(),
            wide(&path.to_string_lossy()).as_ptr(),
            IMAGE_BITMAP,
            0,
            0,
            LR_LOADFROMFILE | LR_CREATEDIBSECTION,
        );
        if handle.is_null() {
            return None;
        }
        let mut info: BITMAP = std::mem::zeroed();
        if GetObjectW(
            handle,
            std::mem::size_of::<BITMAP>() as i32,
            (&mut info as *mut BITMAP).cast(),
        ) == 0
        {
            DeleteObject(handle);
            return None;
        }
        Some(Self {
            handle,
            width: info.bmWidth,
            height: info.bmHeight,
        })
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.handle);
        }
    }
}

struct State {
    hwnd: HWND,
    scale: f32,
    width: i32,
    height: i32,
    state: Value,
    selected: Option<String>,
    tab: Tab,
    profile_open: bool,
    selected_button: String,
    buttons: Vec<Value>,
    hits: Vec<Hit>,
    focus: usize,
    fields: HashMap<u16, HWND>,
    jobs: mpsc::Sender<Job>,
    replies: mpsc::Receiver<Reply>,
    pending: usize,
    status: String,
    failed: bool,
    smoke_view: Option<String>,
    smoke_requested: Option<String>,
    smoke_reported: bool,
    assets: PathBuf,
    fonts: Fonts,
    mouse: Option<Bitmap>,
    keyboard: Option<Bitmap>,
    edit_brush: HBRUSH,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.edit_brush);
        }
    }
}

impl State {
    /// Record the actually presented read-only view, without screenshots or serials.
    /// A requested view is not considered covered merely because the process ran.
    unsafe fn record_smoke_frame(&mut self, presented: bool) {
        if self.smoke_reported || self.smoke_view.is_some() || self.pending != 0 {
            return;
        }
        let Some(requested) = &self.smoke_requested else {
            return;
        };
        let Some(path) = std::env::var_os("SYNAPSE_LITE_SMOKE_REPORT") else {
            return;
        };
        let kind = self.device().and_then(|d| d["kind"].as_str());
        let matched = match requested.as_str() {
            "dashboard" => self.selected.is_none(),
            "mouse-performance" => kind == Some("mouse") && self.tab == Tab::Performance,
            "keyboard-lighting" => kind == Some("keyboard") && self.tab == Tab::Lighting,
            _ => false,
        };
        let report = json!({
            "requested": requested,
            "matched": matched,
            "presented": presented,
            "visible": IsWindowVisible(self.hwnd) != 0,
            "width": self.width,
            "height": self.height,
            "scale": self.scale,
            "device_kind": kind,
            "tab": self.tab.title(),
            "detected_devices": self.devices().len(),
            "loaded_source_fonts": self.fonts.loaded.len(),
            "mouse_artwork": self.mouse.is_some(),
            "keyboard_artwork": self.keyboard.is_some(),
            "failed": self.failed,
            "fidelity": "partial source-derived native port; pixel identity unproven"
        });
        match serde_json::to_vec_pretty(&report)
            .ok()
            .and_then(|bytes| std::fs::write(path, bytes).ok())
        {
            Some(()) => self.smoke_reported = true,
            None => secbird_core::log::append("Could not write native smoke frame evidence"),
        }
    }

    fn px(&self, n: i32) -> i32 {
        (n as f32 * self.scale).round() as i32
    }
    fn devices(&self) -> &[Value] {
        self.state["devices"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    fn device(&self) -> Option<&Value> {
        self.devices()
            .iter()
            .find(|d| d["id"].as_str() == self.selected.as_deref())
    }
    fn id(&self) -> Value {
        json!({ "id": self.selected.as_deref().unwrap_or("") })
    }

    unsafe fn queue(&mut self, job: Job) {
        // A duplicate click must not build an unbounded queue of device writes.
        if self.pending > 0 {
            return;
        }
        if self.jobs.send(job).is_ok() {
            self.pending += 1;
            self.status = "Working…".into();
            self.failed = false;
            SetTimer(self.hwnd, RESPONSE_TIMER, 40, None);
            InvalidateRect(self.hwnd, null(), 0);
        }
    }

    unsafe fn collect(&mut self) {
        while let Ok(reply) = self.replies.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            if let Some(state) = reply.state {
                self.state = state;
            }
            // Smoke-only selection follows the real read-only device snapshot.
            if let Some(view) = self.smoke_view.take() {
                let target = match view.as_str() {
                    "mouse-performance" => Some(("mouse", Tab::Performance)),
                    "keyboard-lighting" => Some(("keyboard", Tab::Lighting)),
                    _ => None,
                };
                if let Some((kind, tab)) = target {
                    if let Some(id) = self
                        .devices()
                        .iter()
                        .find(|d| d["kind"].as_str() == Some(kind))
                        .and_then(|d| d["id"].as_str())
                        .map(str::to_string)
                    {
                        self.selected = Some(id);
                        self.tab = tab;
                    }
                }
            }
            if let Some(buttons) = reply.buttons {
                self.buttons = buttons["buttons"].as_array().cloned().unwrap_or_default();
            }
            self.status = reply.status;
            self.failed = reply.failed;
            if self.selected.is_some() && self.device().is_none() {
                self.selected = None;
                self.buttons.clear();
            }
            self.rebuild_fields();
            InvalidateRect(self.hwnd, null(), 0);
        }
        if self.pending == 0 {
            KillTimer(self.hwnd, RESPONSE_TIMER);
        }
    }

    fn text(&self, id: u16) -> String {
        let Some(&hwnd) = self.fields.get(&id) else {
            return String::new();
        };
        unsafe {
            let length = GetWindowTextLengthW(hwnd).clamp(0, 4096) as usize;
            let mut text = vec![0u16; length + 1];
            let read = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
            String::from_utf16_lossy(&text[..read.max(0) as usize])
                .trim()
                .to_string()
        }
    }

    unsafe fn field(&mut self, id: u16, value: &str, x: i32, y: i32, width: i32) {
        let hwnd = CreateWindowExW(
            0,
            wide("EDIT").as_ptr(),
            wide(value).as_ptr(),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            self.px(x),
            self.px(y),
            self.px(width),
            self.px(27),
            self.hwnd,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        if !hwnd.is_null() {
            SendMessageW(hwnd, WM_SETFONT, self.fonts.normal as usize, 1);
            SendMessageW(hwnd, EM_SETLIMITTEXT, 128, 0);
            self.fields.insert(id, hwnd);
        }
    }

    unsafe fn rebuild_fields(&mut self) {
        for (_, hwnd) in self.fields.drain() {
            DestroyWindow(hwnd);
        }
        let Some(device) = self.device().cloned() else {
            return;
        };
        let center = ((self.width as f32 / self.scale) as i32 / 2).max(420);
        match self.tab {
            Tab::Performance if device["dpi"].is_object() => {
                self.field(
                    DPI_X,
                    &device["dpi"]["x"].to_string(),
                    center - 310,
                    254,
                    110,
                );
                self.field(
                    DPI_Y,
                    &device["dpi"]["y"].to_string(),
                    center - 170,
                    254,
                    110,
                );
            }
            Tab::Lighting if device["leds"].as_array().is_some_and(|v| !v.is_empty()) => {
                let zone = device["leds"][0]["name"].as_str().unwrap_or("backlight");
                let saved = &device["profile"]["lighting"][zone];
                let brightness = saved["brightness"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let color = saved["color"].as_str().unwrap_or("");
                self.field(BRIGHTNESS, &brightness, center - 310, 250, 110);
                self.field(COLOR, color, center + 70, 250, 130);
            }
            Tab::Power if device["sleep"].is_object() => {
                self.field(
                    MINUTES,
                    &device["sleep"]["minutes"].as_u64().unwrap_or(5).to_string(),
                    center - 310,
                    255,
                    110,
                );
            }
            Tab::Customize => {
                if !self.buttons.is_empty() {
                    let current = self
                        .buttons
                        .iter()
                        .find(|b| b["name"].as_str() == Some(&self.selected_button))
                        .and_then(|b| b["normal"].as_str())
                        .unwrap_or("default")
                        .to_string();
                    self.field(MAPPING, &current, center + 240, 292, 200);
                }
            }
            _ => {}
        }
        self.field(NEW_PROFILE, "New profile", 30, 130, 165);
    }

    unsafe fn action(&mut self, action: Action) {
        match action {
            Action::Close => {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
            Action::Minimize => {
                PostMessageW(self.hwnd, WM_SYSCOMMAND, SC_MINIMIZE as usize, 0);
            }
            Action::Maximize => {
                PostMessageW(
                    self.hwnd,
                    WM_SYSCOMMAND,
                    if IsZoomed(self.hwnd) != 0 {
                        SC_RESTORE as usize
                    } else {
                        SC_MAXIMIZE as usize
                    },
                    0,
                );
            }
            Action::Home => {
                self.selected = None;
                self.profile_open = false;
                self.buttons.clear();
                self.rebuild_fields();
            }
            Action::Device(id) => {
                self.selected = Some(id);
                self.tab = Tab::Customize;
                self.profile_open = false;
                self.buttons.clear();
                self.rebuild_fields();
            }
            Action::Tab(tab) => {
                self.tab = tab;
                self.profile_open = false;
                self.rebuild_fields();
            }
            Action::Refresh => self.queue(Job::Rescan),
            Action::ProfileMenu => self.profile_open = !self.profile_open,
            Action::Profile(name) => {
                self.profile_open = false;
                self.queue(Job::Action(
                    "/api/profile",
                    json!({"id":self.selected,"action":"use","name":name}),
                ));
            }
            Action::CreateProfile => {
                let name = self.text(NEW_PROFILE);
                if name.is_empty() {
                    self.error("Enter a profile name");
                } else {
                    self.queue(Job::Action(
                        "/api/profile",
                        json!({"id":self.selected,"action":"new","name":name}),
                    ));
                }
            }
            Action::Dpi => {
                let values = (
                    self.text(DPI_X).parse::<u16>(),
                    self.text(DPI_Y).parse::<u16>(),
                );
                if let (Ok(x), Ok(y)) = values {
                    self.queue(Job::Action(
                        "/api/dpi",
                        json!({"id":self.selected,"x":x,"y":y}),
                    ));
                } else {
                    self.error("DPI must be a positive whole number for each axis");
                }
            }
            Action::Poll(hz) => self.queue(Job::Action(
                "/api/polling",
                json!({"id":self.selected,"hz":hz}),
            )),
            Action::Brightness => {
                if let Some(value) = self
                    .text(BRIGHTNESS)
                    .parse::<u8>()
                    .ok()
                    .filter(|v| *v <= 100)
                {
                    self.queue(Job::Action(
                        "/api/brightness",
                        json!({"id":self.selected,"zone":self.zone(),"value":value}),
                    ));
                } else {
                    self.error("Brightness must be between 0 and 100");
                }
            }
            Action::Color => self.queue(Job::Action(
                "/api/color",
                json!({"id":self.selected,"zone":self.zone(),"color":self.text(COLOR)}),
            )),
            Action::Effect(effect) => self.queue(Job::Action(
                "/api/effect",
                json!({"id":self.selected,"zone":self.zone(),"effect":effect}),
            )),
            Action::Sleep => {
                if let Some(minutes) = self.text(MINUTES).parse::<u16>().ok().filter(|v| *v > 0) {
                    self.queue(Job::Action(
                        "/api/sleep",
                        json!({"id":self.selected,"minutes":minutes}),
                    ));
                } else {
                    self.error("Enter a sleep interval in whole minutes");
                }
            }
            Action::ReadButtons => self.queue(Job::Buttons(self.id())),
            Action::SelectButton(name) => {
                self.selected_button = name;
                self.rebuild_fields();
            }
            Action::Assign => {
                if self.selected_button.is_empty() {
                    self.error("Select an assignment first");
                } else {
                    self.queue(Job::Action("/api/button", json!({"id":self.selected,"button":self.selected_button,"layer":"normal","assign":self.text(MAPPING)})));
                }
            }
            Action::RestoreButtons => self.queue(Job::Action("/api/buttons-restore", self.id())),
        }
        self.focus = 0;
        InvalidateRect(self.hwnd, null(), 0);
    }

    fn zone(&self) -> &str {
        self.device()
            .and_then(|d| d["leds"].as_array())
            .and_then(|v| v.first())
            .and_then(|l| l["name"].as_str())
            .unwrap_or("backlight")
    }

    fn error(&mut self, message: &str) {
        self.status = message.into();
        self.failed = true;
    }
}

struct Painter<'a> {
    dc: HDC,
    scale: f32,
    fonts: &'a Fonts,
    hits: Vec<Hit>,
}

impl Painter<'_> {
    fn px(&self, value: i32) -> i32 {
        (value as f32 * self.scale).round() as i32
    }
    fn rect(&self, x: i32, y: i32, width: i32, height: i32) -> RECT {
        RECT {
            left: self.px(x),
            top: self.px(y),
            right: self.px(x + width),
            bottom: self.px(y + height),
        }
    }
    unsafe fn fill(&self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        let brush = CreateSolidBrush(rgb(color));
        FillRect(self.dc, &self.rect(x, y, width, height), brush);
        DeleteObject(brush);
    }
    unsafe fn text(
        &self,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        font: HFONT,
        color: u32,
        align: u32,
    ) {
        let previous = SelectObject(self.dc, font);
        SetBkMode(self.dc, TRANSPARENT as i32);
        SetTextColor(self.dc, rgb(color));
        let mut rect = self.rect(x, y, width, height);
        DrawTextW(
            self.dc,
            wide(text).as_ptr(),
            -1,
            &mut rect,
            align | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(self.dc, previous);
    }
    unsafe fn line(&self, x: i32, y: i32, width: i32, color: u32) {
        self.fill(x, y, width, 1, color);
    }
    unsafe fn button(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        width: i32,
        action: Action,
        active: bool,
    ) {
        let rect = self.rect(x, y, width, 28);
        let fill = CreateSolidBrush(rgb(if active { GREEN } else { PANEL }));
        let pen = CreatePen(
            PS_SOLID,
            self.px(1),
            rgb(if active { GREEN } else { BORDER }),
        );
        let old_brush = SelectObject(self.dc, fill);
        let old_pen = SelectObject(self.dc, pen);
        RoundRect(
            self.dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            self.px(6),
            self.px(6),
        );
        SelectObject(self.dc, old_brush);
        SelectObject(self.dc, old_pen);
        DeleteObject(fill);
        DeleteObject(pen);
        self.text(
            text,
            x + 5,
            y,
            width - 10,
            28,
            self.fonts.small,
            if active { PANEL } else { TEXT },
            DT_CENTER,
        );
        self.hits.push(Hit { rect, action });
    }
    unsafe fn tab(&mut self, tab: Tab, x: i32, y: i32, active: bool) {
        let label = tab.title();
        let width = tab.width();
        if active {
            let brush = CreateSolidBrush(rgb(GREEN));
            let old = SelectObject(self.dc, brush);
            let pen = CreatePen(PS_SOLID, 1, rgb(GREEN));
            let old_pen = SelectObject(self.dc, pen);
            let rect = self.rect(x, y, width, 28);
            RoundRect(
                self.dc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                self.px(28),
                self.px(28),
            );
            SelectObject(self.dc, old);
            SelectObject(self.dc, old_pen);
            DeleteObject(brush);
            DeleteObject(pen);
        }
        self.text(
            label,
            x,
            y,
            width,
            28,
            self.fonts.small,
            if active { PANEL } else { MUTED },
            DT_CENTER,
        );
        self.hits.push(Hit {
            rect: self.rect(x, y, width, 28),
            action: Action::Tab(tab),
        });
    }
    unsafe fn image(&self, image: Option<&Bitmap>, x: i32, y: i32, width: i32, height: i32) {
        let Some(image) = image else { return };
        let ratio = (width as f32 / image.width as f32).min(height as f32 / image.height as f32);
        let w = (image.width as f32 * ratio).round() as i32;
        let h = (image.height as f32 * ratio).round() as i32;
        let dc = CreateCompatibleDC(self.dc);
        let old = SelectObject(dc, image.handle);
        SetStretchBltMode(self.dc, HALFTONE as i32);
        SetBrushOrgEx(self.dc, 0, 0, null_mut());
        StretchBlt(
            self.dc,
            self.px(x + (width - w) / 2),
            self.px(y + (height - h) / 2),
            self.px(w),
            self.px(h),
            dc,
            0,
            0,
            image.width,
            image.height,
            SRCCOPY,
        );
        SelectObject(dc, old);
        DeleteDC(dc);
    }
}

unsafe fn paint(state: &mut State, dc: HDC) {
    let width = (state.width as f32 / state.scale).round() as i32;
    let height = (state.height as f32 / state.scale).round() as i32;
    let mut p = Painter {
        dc,
        scale: state.scale,
        fonts: &state.fonts,
        hits: Vec::new(),
    };
    p.fill(0, 0, width, height, BG);
    p.fill(0, 0, width, 42, 0);
    p.fill(8, 8, 178, 34, if state.selected.is_none() { BG } else { 0 });
    p.text(
        "SYNAPSE",
        25,
        8,
        145,
        34,
        p.fonts.small,
        if state.selected.is_none() {
            TEXT
        } else {
            MUTED
        },
        DT_LEFT,
    );
    p.hits.push(Hit {
        rect: p.rect(8, 0, 178, 42),
        action: Action::Home,
    });
    for (index, device) in state.devices().iter().enumerate() {
        let x = 190 + index as i32 * 218;
        if x + 218 > width - 150 {
            break;
        }
        let id = device["id"].as_str().unwrap_or("");
        if Some(id) == state.selected.as_deref() {
            p.fill(x, 8, 214, 34, BG);
            p.line(x, 8, 214, GREEN);
        }
        p.text(
            &device["name"]
                .as_str()
                .unwrap_or("RAZER DEVICE")
                .to_uppercase(),
            x + 12,
            9,
            190,
            32,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        p.hits.push(Hit {
            rect: p.rect(x, 0, 214, 42),
            action: Action::Device(id.to_string()),
        });
    }
    for (index, (label, action)) in [
        ("—", Action::Minimize),
        ("□", Action::Maximize),
        ("×", Action::Close),
    ]
    .into_iter()
    .enumerate()
    {
        let x = width - 144 + index as i32 * 48;
        p.text(label, x, 0, 48, 42, p.fonts.product, MUTED, DT_CENTER);
        p.hits.push(Hit {
            rect: p.rect(x, 0, 48, 42),
            action,
        });
    }
    p.text("‹", 0, 42, 40, 38, p.fonts.heading, MUTED, DT_CENTER);
    p.hits.push(Hit {
        rect: p.rect(0, 42, 40, 38),
        action: Action::Home,
    });
    p.text("↻", 40, 42, 40, 38, p.fonts.heading, MUTED, DT_CENTER);
    p.hits.push(Hit {
        rect: p.rect(40, 42, 40, 38),
        action: Action::Refresh,
    });
    let device = state.device().cloned();
    let title = device
        .as_ref()
        .and_then(|d| d["name"].as_str())
        .unwrap_or("RAZER SYNAPSE");
    p.text(
        &title.to_uppercase(),
        width / 4,
        42,
        width / 2,
        38,
        p.fonts.normal,
        MUTED,
        DT_CENTER,
    );
    p.line(0, 79, width, 0);
    if let Some(device) = device {
        let center = (width / 2).max(420);
        p.text("PROFILE", 24, 88, 64, 28, p.fonts.small, TEXT, DT_LEFT);
        let active = device["active"].as_str().unwrap_or("Default");
        p.button(active, 90, 88, 190, Action::ProfileMenu, false);
        let tabs: &[Tab] = match device["kind"].as_str() {
            Some("keyboard") => &[Tab::Customize, Tab::Lighting],
            Some("mouse") => &[
                Tab::Customize,
                Tab::Performance,
                Tab::Power,
                Tab::Calibration,
                Tab::Advanced,
            ],
            _ => &[Tab::Customize, Tab::Performance, Tab::Lighting, Tab::Power],
        };
        let total_width: i32 = tabs.iter().map(|tab| tab.width() + 20).sum();
        let mut tab_x = center - total_width / 2;
        for &tab in tabs {
            p.tab(tab, tab_x, 88, state.tab == tab);
            tab_x += tab.width() + 20;
        }
        p.tab(Tab::Help, width - 58, 88, state.tab == Tab::Help);
        p.fill(0, 126, width, 2, 0);
        p.button("+", 205, 130, 35, Action::CreateProfile, false);
        if let Some(battery) = device["battery"].as_u64() {
            p.text(
                &format!("{battery}%"),
                width - 134,
                88,
                64,
                28,
                p.fonts.small,
                MUTED,
                DT_RIGHT,
            );
        }
        match state.tab {
            Tab::Customize => {
                let keyboard = device["kind"].as_str() == Some("keyboard");
                let image = if keyboard {
                    state.keyboard.as_ref()
                } else {
                    state.mouse.as_ref()
                };
                let image_width = if keyboard { 690 } else { 300 };
                p.image(
                    image,
                    center - image_width / 2,
                    176,
                    image_width,
                    if keyboard { 387 } else { 340 },
                );
                if image.is_none() {
                    p.text(
                        "Product artwork is unavailable",
                        center - 200,
                        290,
                        400,
                        40,
                        p.fonts.normal,
                        MUTED,
                        DT_CENTER,
                    );
                }
                p.text(
                    "BUTTON ASSIGNMENT",
                    30,
                    190,
                    260,
                    30,
                    p.fonts.heading,
                    GREEN,
                    DT_LEFT,
                );
                p.button("READ ASSIGNMENTS", 30, 235, 185, Action::ReadButtons, false);
                if !state.buttons.is_empty() {
                    for (index, button) in state.buttons.iter().take(9).enumerate() {
                        let name = button["name"].as_str().unwrap_or("");
                        let label = button["label"].as_str().unwrap_or(name);
                        p.button(
                            label,
                            30,
                            278 + index as i32 * 32,
                            185,
                            Action::SelectButton(name.to_string()),
                            name == state.selected_button,
                        );
                    }
                    p.text(
                        "ASSIGNMENT",
                        center + 240,
                        246,
                        210,
                        30,
                        p.fonts.heading,
                        GREEN,
                        DT_LEFT,
                    );
                    p.button("SAVE", center + 240, 334, 95, Action::Assign, true);
                    p.button(
                        "RESTORE ALL",
                        center + 240,
                        378,
                        150,
                        Action::RestoreButtons,
                        false,
                    );
                }
            }
            Tab::Performance => {
                p.text(
                    "SENSITIVITY",
                    center - 310,
                    187,
                    300,
                    30,
                    p.fonts.heading,
                    GREEN,
                    DT_LEFT,
                );
                if let Some(dpi) = device["dpi"].as_object() {
                    p.text(
                        "X AXIS",
                        center - 310,
                        225,
                        110,
                        22,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                    p.text(
                        "Y AXIS",
                        center - 170,
                        225,
                        110,
                        22,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                    p.button("APPLY", center - 310, 304, 115, Action::Dpi, true);
                    p.text(
                        &format!(
                            "{}–{} DPI",
                            dpi.get("min").unwrap_or(&Value::Null),
                            dpi.get("max").unwrap_or(&Value::Null)
                        ),
                        center - 310,
                        347,
                        270,
                        24,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                } else {
                    p.text(
                        "Sensitivity is unavailable for this device",
                        center - 310,
                        232,
                        340,
                        28,
                        p.fonts.normal,
                        MUTED,
                        DT_LEFT,
                    );
                }
                p.text(
                    "POLLING RATE",
                    center + 70,
                    187,
                    300,
                    30,
                    p.fonts.heading,
                    GREEN,
                    DT_LEFT,
                );
                if let Some(options) = device["polling"]["options"].as_array() {
                    let active = device["polling"]["hz"].as_u64().unwrap_or(0);
                    for (index, option) in
                        options.iter().filter_map(Value::as_u64).take(5).enumerate()
                    {
                        p.button(
                            &format!("{option} HZ"),
                            center + 70,
                            237 + index as i32 * 42,
                            150,
                            Action::Poll(option as u32),
                            active == option,
                        );
                    }
                } else {
                    p.text(
                        "Polling controls unavailable",
                        center + 70,
                        234,
                        300,
                        28,
                        p.fonts.normal,
                        MUTED,
                        DT_LEFT,
                    );
                }
            }
            Tab::Lighting => {
                p.text(
                    "BRIGHTNESS",
                    center - 310,
                    187,
                    300,
                    30,
                    p.fonts.heading,
                    GREEN,
                    DT_LEFT,
                );
                if device["leds"].as_array().is_some_and(|v| !v.is_empty()) {
                    p.text(
                        "PERCENT",
                        center - 310,
                        222,
                        130,
                        22,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                    p.button("APPLY", center - 310, 298, 115, Action::Brightness, true);
                    p.text(
                        "QUICK EFFECTS",
                        center + 70,
                        187,
                        300,
                        30,
                        p.fonts.heading,
                        GREEN,
                        DT_LEFT,
                    );
                    p.text(
                        "STATIC COLOR (HEX)",
                        center + 70,
                        222,
                        210,
                        22,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                    p.button("APPLY COLOR", center + 70, 298, 150, Action::Color, true);
                    p.button(
                        "SPECTRUM CYCLING",
                        center + 70,
                        352,
                        185,
                        Action::Effect("spectrum"),
                        false,
                    );
                    p.button(
                        "BREATHING",
                        center + 70,
                        394,
                        185,
                        Action::Effect("breathing:44D62C"),
                        false,
                    );
                } else {
                    p.text(
                        "This device has no supported lighting zones",
                        center - 310,
                        237,
                        620,
                        30,
                        p.fonts.normal,
                        MUTED,
                        DT_LEFT,
                    );
                }
            }
            Tab::Power => {
                p.text(
                    "WIRELESS POWER SAVING",
                    center - 310,
                    187,
                    620,
                    30,
                    p.fonts.heading,
                    GREEN,
                    DT_LEFT,
                );
                if device["sleep"].is_object() {
                    p.text(
                        "ENTER SLEEP MODE AFTER (MINUTES)",
                        center - 310,
                        224,
                        370,
                        25,
                        p.fonts.small,
                        MUTED,
                        DT_LEFT,
                    );
                    p.button("APPLY", center - 310, 304, 115, Action::Sleep, true);
                } else {
                    p.text(
                        "Power controls are unavailable for this connection",
                        center - 310,
                        235,
                        620,
                        30,
                        p.fonts.normal,
                        MUTED,
                        DT_LEFT,
                    );
                }
                if let Some(battery) = device["battery"].as_u64() {
                    p.text(
                        &format!("BATTERY  {battery}%"),
                        center + 70,
                        240,
                        250,
                        38,
                        p.fonts.product,
                        TEXT,
                        DT_LEFT,
                    );
                }
            }
            Tab::Calibration | Tab::Advanced | Tab::Help => {}
        }
        if matches!(state.tab, Tab::Calibration | Tab::Advanced | Tab::Help) {
            p.text(
                if state.tab == Tab::Help {
                    "DEVICE INFORMATION"
                } else {
                    state.tab.title()
                },
                center - 310,
                187,
                620,
                30,
                p.fonts.heading,
                GREEN,
                DT_LEFT,
            );
            let description = if state.tab == Tab::Help {
                format!(
                    "{}  •  Product ID {}",
                    device["name"].as_str().unwrap_or("Unknown device"),
                    device["pid"].as_str().unwrap_or("Unknown")
                )
            } else {
                "These device controls are not available in this build".to_string()
            };
            p.text(
                &description,
                center - 310,
                237,
                650,
                30,
                p.fonts.normal,
                MUTED,
                DT_LEFT,
            );
        }
        if state.profile_open {
            let profiles = device["profiles"].as_array().cloned().unwrap_or_default();
            p.fill(
                90,
                117,
                190,
                (profiles.len().min(10) as i32) * 31 + 8,
                PANEL,
            );
            for (index, name) in profiles
                .iter()
                .filter_map(Value::as_str)
                .take(10)
                .enumerate()
            {
                p.button(
                    name,
                    94,
                    121 + index as i32 * 31,
                    182,
                    Action::Profile(name.to_string()),
                    name == active,
                );
            }
        }
    } else {
        p.text("DEVICES", 30, 104, 300, 32, p.fonts.heading, GREEN, DT_LEFT);
        for (index, device) in state.devices().iter().enumerate() {
            let x = 30 + (index as i32 % 4) * 285;
            let y = 158 + (index as i32 / 4) * 280;
            p.fill(x, y, 260, 250, PANEL);
            let image = if device["kind"].as_str() == Some("keyboard") {
                state.keyboard.as_ref()
            } else {
                state.mouse.as_ref()
            };
            p.image(image, x + 15, y + 20, 230, 160);
            p.text(
                device["name"].as_str().unwrap_or("Razer device"),
                x + 12,
                y + 192,
                236,
                40,
                p.fonts.normal,
                TEXT,
                DT_CENTER,
            );
            p.hits.push(Hit {
                rect: p.rect(x, y, 260, 250),
                action: Action::Device(device["id"].as_str().unwrap_or("").into()),
            });
        }
        if state.devices().is_empty() {
            p.text(
                if state.pending > 0 {
                    "Reading connected devices…"
                } else {
                    "No controllable Razer devices found"
                },
                30,
                160,
                width - 60,
                40,
                p.fonts.normal,
                MUTED,
                DT_LEFT,
            );
        }
        if let Some(unsupported) = state.state["unsupported"].as_array() {
            for (index, device) in unsupported.iter().take(4).enumerate() {
                p.text(
                    &format!(
                        "{} — {}",
                        device["name"].as_str().unwrap_or("Razer device"),
                        device["reason"].as_str().unwrap_or("Unavailable")
                    ),
                    30,
                    480 + index as i32 * 26,
                    width - 60,
                    26,
                    p.fonts.small,
                    MUTED,
                    DT_LEFT,
                );
            }
        }
    }
    p.line(0, height - 35, width, 0);
    p.text(
        &state.status,
        20,
        height - 33,
        width - 40,
        30,
        p.fonts.small,
        if state.failed { 0xfd4949 } else { MUTED },
        DT_LEFT,
    );
    if state.focus > 0 {
        if let Some(hit) = p.hits.get(state.focus - 1) {
            DrawFocusRect(dc, &hit.rect);
        }
    }
    state.hits = p.hits;
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        (*(create.lpCreateParams as *mut RefCell<State>))
            .borrow_mut()
            .hwnd = hwnd;
    }
    // Win32 calls can synchronously reenter a window procedure. Never create
    // overlapping mutable references to its state during those callbacks.
    match message {
        WM_CLOSE => {
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            WINDOW.store(0, Ordering::Release);
            KillTimer(hwnd, RESPONSE_TIMER);
            KillTimer(hwnd, SMOKE_TIMER);
            PostQuitMessage(0);
            return 0;
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        _ => {}
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let Ok(mut state) = (*pointer).try_borrow_mut() else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    match message {
        WM_SIZE => {
            state.width = (lparam as u32 & 0xffff) as i32;
            state.height = ((lparam as u32 >> 16) & 0xffff) as i32;
            state.rebuild_fields();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam as *mut MINMAXINFO);
            info.ptMinTrackSize = POINT {
                x: state.px(1000),
                y: state.px(650),
            };
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let buffer = CreateCompatibleDC(dc);
            let bitmap = CreateCompatibleBitmap(dc, state.width.max(1), state.height.max(1));
            let previous = SelectObject(buffer, bitmap);
            paint(&mut state, buffer);
            let presented = BitBlt(dc, 0, 0, state.width, state.height, buffer, 0, 0, SRCCOPY) != 0;
            SelectObject(buffer, previous);
            DeleteObject(bitmap);
            DeleteDC(buffer);
            EndPaint(hwnd, &ps);
            state.record_smoke_frame(presented);
            0
        }
        WM_LBUTTONUP => {
            let point = POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            };
            let action = state
                .hits
                .iter()
                .rev()
                .find(|hit| PtInRect(&hit.rect, point) != 0)
                .map(|hit| hit.action.clone());
            if let Some(action) = action {
                state.action(action);
            }
            0
        }
        WM_KEYDOWN => {
            match wparam as u32 {
                9 => {
                    state.focus = state.focus % state.hits.len().max(1) + 1;
                    InvalidateRect(hwnd, null(), 0);
                }
                13 | 32 => {
                    if let Some(action) = state
                        .hits
                        .get(state.focus.saturating_sub(1))
                        .map(|h| h.action.clone())
                    {
                        state.action(action);
                    }
                }
                27 => {
                    if state.profile_open {
                        state.profile_open = false;
                        InvalidateRect(hwnd, null(), 0);
                    } else {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
                _ => {}
            }
            0
        }
        WM_CTLCOLOREDIT => {
            SetTextColor(wparam as HDC, rgb(TEXT));
            SetBkColor(wparam as HDC, rgb(PANEL));
            state.edit_brush as isize
        }
        WM_TIMER if wparam == RESPONSE_TIMER => {
            state.collect();
            0
        }
        WM_TIMER if wparam == SMOKE_TIMER => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_DPICHANGED => {
            state.scale = (wparam as u32 & 0xffff) as f32 / 96.0;
            state.fonts = Fonts::new(&state.assets, state.scale);
            let rect = &*(lparam as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            state.width = client.right;
            state.height = client.bottom;
            state.rebuild_fields();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_NCHITTEST => {
            let result = DefWindowProcW(hwnd, message, wparam, lparam);
            if result != HTCLIENT as isize {
                return result;
            }
            let mut point = POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            let over_hit = state.hits.iter().any(|h| PtInRect(&h.rect, point) != 0);
            if point.y < state.px(42) && !over_hit {
                HTCAPTION as isize
            } else {
                result
            }
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn assets_dir() -> PathBuf {
    std::env::var_os("SYNAPSE_LITE_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("assets")))
                .unwrap_or_else(|| PathBuf::from("assets"))
        })
}

fn run_window(
    engine: Arc<Mutex<Engine>>,
    close_after: Option<Duration>,
    ready: Option<mpsc::SyncSender<Result<(), String>>>,
) -> Result<(), String> {
    unsafe {
        let smoke = close_after.is_some() || std::env::args().any(|arg| arg == "--runtime-smoke");
        let smoke_requested = smoke.then(|| {
            std::env::var("SYNAPSE_LITE_SMOKE_VIEW").unwrap_or_else(|_| "dashboard".into())
        });
        let assets = assets_dir();
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let (replies_tx, replies_rx) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("synapse-device-worker".into())
            .spawn(move || worker(engine, jobs_rx, replies_tx))
            .map_err(|e| e.to_string())?;
        let mut state = Box::new(RefCell::new(State {
            hwnd: null_mut(),
            scale: 1.0,
            width: 1280,
            height: 720,
            state: json!({"devices":[]}),
            selected: None,
            tab: Tab::Customize,
            profile_open: false,
            selected_button: String::new(),
            buttons: Vec::new(),
            hits: Vec::new(),
            focus: 0,
            fields: HashMap::new(),
            jobs: jobs_tx,
            replies: replies_rx,
            pending: 0,
            status: "Reading connected devices…".into(),
            failed: false,
            smoke_view: smoke_requested.clone(),
            smoke_requested,
            smoke_reported: false,
            fonts: Fonts::new(&assets, 1.0),
            mouse: Bitmap::load(&assets.join("mouse.bmp")),
            keyboard: Bitmap::load(&assets.join("keyboard.bmp")),
            assets,
            edit_brush: CreateSolidBrush(rgb(PANEL)),
        }));
        let instance = GetModuleHandleW(null());
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Synapse Lite").as_ptr(),
            WS_POPUP
                | WS_THICKFRAME
                | WS_MINIMIZEBOX
                | WS_MAXIMIZEBOX
                | WS_SYSMENU
                | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            720,
            null_mut(),
            null_mut(),
            instance,
            (&mut *state as *mut RefCell<State>).cast(),
        );
        if hwnd.is_null() {
            let error = format!(
                "Could not create native editor (Windows error {})",
                GetLastError()
            );
            if let Some(ready) = ready {
                let _ = ready.send(Err(error.clone()));
            }
            return Err(error);
        }
        {
            let mut state = state.borrow_mut();
            state.scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            if state.scale != 1.0 {
                state.fonts = Fonts::new(&state.assets, state.scale);
            }
        }
        WINDOW.store(hwnd as usize, Ordering::Release);
        ShowWindow(hwnd, SW_SHOW);
        // The first ShowWindow can honor a launcher's SW_HIDE startup hint.
        // Smoke explicitly requests a rendered editor, so make it visible.
        if smoke {
            ShowWindow(hwnd, SW_SHOW);
        }
        UpdateWindow(hwnd);
        state.borrow_mut().queue(Job::Refresh);
        if let Some(duration) = close_after {
            SetTimer(
                hwnd,
                SMOKE_TIMER,
                duration.as_millis().clamp(1, u32::MAX as u128) as u32,
                None,
            );
        }
        if let Some(ready) = ready {
            let _ = ready.send(Ok(()));
        }
        let mut message: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        drop(state);
        // Channel closure lets the worker finish its current bounded operation.
        let _ = worker.join();
        Ok(())
    }
}

/// Opens one native editor on its own UI thread, or raises the existing editor.
pub fn open(engine: Arc<Mutex<Engine>>) -> Result<(), String> {
    let hwnd = WINDOW.load(Ordering::Acquire) as HWND;
    if !hwnd.is_null() {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
        return Ok(());
    }
    if OPENING.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("synapse-native-editor".into())
        .spawn(move || {
            let result = run_window(engine, None, Some(ready_tx));
            OPENING.store(false, Ordering::Release);
            if let Err(error) = result {
                secbird_core::log::append(&format!("native editor: {error}"));
            }
        });
    if let Err(error) = thread {
        OPENING.store(false, Ordering::Release);
        return Err(error.to_string());
    }
    ready_rx
        .recv()
        .map_err(|_| "Native editor failed to start".to_string())?
}

/// Runs the same native UI on this thread; an optional timeout enables smoke and
/// memory measurements without installing autostart or changing device settings.
pub fn run_standalone(
    engine: Arc<Mutex<Engine>>,
    close_after: Option<Duration>,
) -> Result<(), String> {
    run_window(engine, close_after, None)
}
