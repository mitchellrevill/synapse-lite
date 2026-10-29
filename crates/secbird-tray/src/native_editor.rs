//! Native editor using source-derived Synapse layout facts and the shared core.
//!
//! Provenance: installed AppEngine 4.0.827 electron/index.css and the locally
//! cached product 605/196 UI styles, recovered 2026-10-27. See the research
//! specification for hashes and byte offsets. This is a partial native port;
//! GDI text, missing SVG hit regions and unimplemented states are not pixel parity.
//! No vendor JavaScript or CSS is embedded or executed here.

use crate::{engine::Engine, server};
use secbird_core::performance::Performance;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    EM_GETSEL, EM_SETLIMITTEXT, EM_SETMARGINS, EM_SETSEL, WM_MOUSELEAVE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, ReleaseCapture, SetCapture, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod assignment_geometry;
mod assignment_fonts;
mod assignment_layout;
mod assignment_model;
mod assignment_view;
mod color_editor_layout;
mod color_editor_view;
mod feedback;
mod dot_background;
#[cfg(test)]
mod interaction_tests;
mod lighting_layout;
mod lighting_view;
mod image_render;
mod host_tabs;
mod linked_games;
mod macro_view;
mod host_tab_model;
mod calibration_view;
mod advanced_view;
mod mouse_speed;
mod mouse_feedback;
mod mouse_input_worker;
mod performance_layout;
mod performance_view;
mod power_layout;
mod power_view;
mod profile_files;
mod profile_modal_backend;
mod profile_modal_files;
mod profile_modal_layout;
mod profile_modal_model;
mod profile_modal_view;
mod runtime_status;
mod source_layout;
mod source_text;
mod viewport;
mod window_frame;
mod studio_view;
mod studio_canvas;
// Live Reactive admission remains closed pending native pulse/input proof.
#[allow(dead_code)]
mod studio_reactive_source;
mod device_artwork;

const CLASS: &str = "SynapseLiteNativeEditor";
const RESPONSE_TIMER: usize = 71;
const ASSIGNMENT_REPEAT: usize = 78;
const SMOKE_TIMER: usize = 72;
const PROFILE_DIALOG: u32 = WM_APP + 91;
const PROFILE_RENAME: u32 = WM_APP + 92;
const PROFILE_MODAL_BROWSE: u32 = WM_APP + 94;
const PROFILE_MODAL_SAVE: u32 = WM_APP + 95;
#[cfg(test)]
const PROFILE_MODAL_TEST_BROWSE: u32 = WM_APP + 96;
#[cfg(test)]
const PROFILE_MODAL_TEST_SAVE: u32 = WM_APP + 97;
#[cfg(test)]
const PROFILE_MODAL_TEST_OPEN: u32 = WM_APP + 98;
#[cfg(test)]
const PROFILE_MODAL_TEST_QUERY: u32 = WM_APP + 99;
#[cfg(test)]
const RUNTIME_TEST_INJECT: u32 = WM_APP + 104;
#[cfg(test)]
const RUNTIME_TEST_QUERY: u32 = WM_APP + 105;
#[cfg(test)]
const LIGHTING_DROPDOWN_TEST: u32 = WM_APP + 109;
#[cfg(test)]
const READ_ONLY_TEST_SET: u32 = WM_APP + 106;
#[cfg(test)]
const STUDIO_TEST: u32 = WM_APP + 115;
#[cfg(test)]
const ASSIGNMENT_TEST_QUERY: u32 = WM_APP + 107;
const FAILURE_DIALOG: u32 = WM_APP + 93;
const FAILURE_TITLE: &str = "Synapse Lite - Action failed";
const PERFORMANCE_REPEAT: usize = 73;
const LIGHTING_REPEAT: usize = 74;
const FAILURE_RETRY: usize = 75;
const CACHED_LIGHTING_TIMER: usize = 76;
const CACHED_FEEDBACK_RETRY: usize = 77;
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
const PROFILE_NAME: u16 = 107;
const COMPAT_NOTICES: u16 = 108;
const PERFORMANCE_INPUT: u16 = 120;
static WINDOW: AtomicUsize = AtomicUsize::new(0);
static OPENING: AtomicBool = AtomicBool::new(false);

/// Tray exit waits for the owned editor's ordinary close/unsaved flow. An
/// acknowledgement from an older window cannot bypass a newly opened editor.
pub(crate) fn defer_process_exit(tray: HWND) -> bool {
    let editor=WINDOW.load(Ordering::Acquire) as HWND;
    if editor.is_null() {return false;}
    unsafe {
        if PostMessageW(editor,studio_view::EXIT_REQUEST,0,tray as isize)==0 {
            return WINDOW.load(Ordering::Acquire)!=0;
        }
    }
    true
}

#[link(name = "user32")]
extern "system" {
    fn GetDpiForWindow(hwnd: HWND) -> u32;
    fn GetDpiForSystem() -> u32;
    fn GetSystemMetricsForDpi(index: i32, dpi: u32) -> i32;
    fn GetWindowDpiAwarenessContext(hwnd: HWND) -> isize;
    fn GetAwarenessFromDpiAwarenessContext(context: isize) -> i32;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn rgb(hex: u32) -> u32 {
    ((hex >> 16) & 255) | (hex & 0x00ff00) | ((hex & 255) << 16)
}

fn dirty_fields(
    same_owner: bool,
    current: &HashMap<u16, String>,
    defaults: &HashMap<u16, String>,
) -> HashMap<u16, String> {
    if !same_owner {
        return HashMap::new();
    }
    current
        .iter()
        .filter(|(id, value)| defaults.get(id).is_some_and(|original| original != *value))
        .map(|(id, value)| (*id, value.clone()))
        .collect()
}

fn available_profile_name(base: &str, existing: &[String], duplicate: bool) -> Option<String> {
    let base = if duplicate {
        base.rsplit_once(" (")
            .filter(|(_, suffix)| {
                suffix
                    .strip_suffix(')')
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            })
            .map(|(prefix, _)| prefix)
            .unwrap_or(base)
    } else {
        base
    };
    if !existing.iter().any(|name| name == base) {
        return Some(base.to_string());
    }
    (1..=100)
        .map(|index| {
            if duplicate {
                format!("{base} ({index})")
            } else {
                format!("{base} {index}")
            }
        })
        .find(|candidate| !existing.contains(candidate))
}

#[cfg(test)]
mod form_tests {
    use super::{available_profile_name, dirty_fields, enqueue_setting, Job};
    use serde_json::json;
    use std::collections::HashMap;
    use std::collections::VecDeque;

    #[test]
    fn rapid_settings_coalesce_without_crossing_an_ordered_operation() {
        let mut queue = VecDeque::new();
        enqueue_setting(
            &mut queue,
            Job::Action(
                "/api/performance",
                json!({"id":"same","expected_profile":"Work","value":400}),
            ),
        )
        .unwrap();
        enqueue_setting(
            &mut queue,
            Job::Action(
                "/api/performance",
                json!({"id":"same","expected_profile":"Work","value":800}),
            ),
        )
        .unwrap();
        assert_eq!(queue.len(), 1);
        assert!(matches!(&queue[0],Job::Action(_,body) if body["value"]==800));
        enqueue_setting(
            &mut queue,
            Job::Action("/api/polling", json!({"id":"same","hz":500})),
        )
        .unwrap();
        enqueue_setting(
            &mut queue,
            Job::Action(
                "/api/performance",
                json!({"id":"same","expected_profile":"Work","value":1200}),
            ),
        )
        .unwrap();
        enqueue_setting(
            &mut queue,
            Job::Action(
                "/api/performance",
                json!({"id":"same","expected_profile":"Other","value":1600}),
            ),
        )
        .unwrap();
        assert_eq!(queue.len(), 4);
        assert!(matches!(&queue[1], Job::Action("/api/polling", _)));
    }

    #[test]
    fn lighting_queue_keeps_preset_and_device_boundaries() {
        let mut queue = VecDeque::new();
        for (device, effect, value) in [
            ("one", 1, 10),
            ("one", 1, 20),
            ("one", 2, 30),
            ("two", 2, 40),
        ] {
            enqueue_setting(
                &mut queue,
                Job::Action(
                    "/api/source-effect",
                    json!({"id":"same","expected_device":device,"expected_profile":"Work",
                    "source_effect_id":effect,"value":value}),
                ),
            )
            .unwrap();
        }
        assert_eq!(queue.len(), 3);
        assert!(matches!(&queue[0],Job::Action(_,body) if body["value"]==20));
        assert!(matches!(&queue[1],Job::Action(_,body) if body["source_effect_id"]==2));
        assert!(matches!(&queue[2],Job::Action(_,body) if body["expected_device"]=="two"));
    }

    #[test]
    fn queue_bound_refuses_work_without_discarding_accepted_operations() {
        let mut queue = VecDeque::new();
        for _ in 0..16 {
            enqueue_setting(&mut queue, Job::Refresh).unwrap();
        }
        assert!(enqueue_setting(&mut queue, Job::Rescan).is_err());
        assert_eq!(queue.len(), 16);
        assert!(queue.iter().all(|job| matches!(job, Job::Refresh)));
    }

    #[test]
    fn resize_retains_edits_but_clean_fields_follow_hardware() {
        let defaults = HashMap::from([(101, "600".into()), (103, "44D62C".into())]);
        let current = HashMap::from([(101, "600".into()), (103, "FF3300".into())]);
        let draft = dirty_fields(true, &current, &defaults);
        assert_eq!(draft.get(&103).map(String::as_str), Some("FF3300"));
        assert!(!draft.contains_key(&101));
        let updated = HashMap::from([(101, "800".into()), (103, "44D62C".into())]);
        assert_eq!(
            draft
                .get(&101)
                .or_else(|| updated.get(&101))
                .map(String::as_str),
            Some("800")
        );
    }

    #[test]
    fn drafts_never_leak_between_devices_profiles_or_tabs() {
        let defaults = HashMap::from([(103, "44D62C".into())]);
        let current = HashMap::from([(103, "FF3300".into())]);
        assert!(dirty_fields(false, &current, &defaults).is_empty());
    }

    #[test]
    fn source_profile_names_fill_the_first_gap_and_strip_copy_suffixes() {
        let names = vec![
            "Default".into(),
            "Default 1".into(),
            "Default (1)".into(),
            "Default (3)".into(),
        ];
        assert_eq!(
            available_profile_name("Default", &names, false).as_deref(),
            Some("Default 2")
        );
        assert_eq!(
            available_profile_name("Default (3)", &names, true).as_deref(),
            Some("Default (2)")
        );
        assert_eq!(
            available_profile_name("Other", &names, true).as_deref(),
            Some("Other")
        );
        assert_eq!(
            available_profile_name("Work (notes)", &names, true).as_deref(),
            Some("Work (notes)")
        );
    }
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
    Studio(studio_view::Control),
    LinkedGames(linked_games::Control),
    MacroLibrary(macro_view::Control),
    Home,
    Device(String),
    HostScroll(i32),
    HostClose(String),
    HostMove(String, usize),
    Calibration(calibration_view::Control),
    Advanced(advanced_view::Control),
    Tab(Tab),
    Refresh,
    ProfileMenu,
    ProfileActions,
    Profile(String),
    CreateProfile,
    DuplicateProfile,
    RenameProfile,
    ProfileModal(profile_modal_model::Control),
    ImportProfile,
    ExportProfile,
    Dpi,
    PerfEnabled,
    PerfSelect(usize),
    PerfIndependent(usize),
    PerfVisible(usize),
    PerfSlider(usize, Axis, performance_layout::Rect),
    PerfDrag(usize),
    PerfStep(usize, Axis, i16),
    Poll(u32),
    Brightness,
    Color,
    Effect(&'static str),
    Sleep,
    PowerSlider(power_view::Kind, power_layout::Rect),
    Lighting(lighting_view::Control),
    Assignment(assignment_view::Control),
    ReadButtons,
    SelectButton(String),
    Assign,
    RestoreButtons,
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy)]
enum PerformanceGesture {
    Slider {
        row: usize,
        axis: Axis,
        rect: performance_layout::Rect,
    },
    Reorder {
        row: usize,
        target: usize,
    },
    Spinner {
        row: usize,
        axis: Axis,
        delta: i16,
    },
}

struct Hit {
    rect: RECT,
    action: Action,
}

enum Job {
    Studio(studio_view::Request),
    LinkedGames(linked_games::Request),
    MacroLibrary(macro_view::Request),
    StudioOutput(studio_view::OutputRequest),
    SensorRead(String, String),
    CachedLighting(runtime_status::Request),
    Profiles(profile_modal_model::Request),
    Refresh,
    Rescan,
    Action(&'static str, Value),
    Buttons(Value),
    ImportFile {
        path: PathBuf,
        target: Option<String>,
    },
    ExportFile {
        path: PathBuf,
        format: profile_files::Format,
        target: String,
    },
}

fn enqueue_setting(queue: &mut VecDeque<Job>, job: Job) -> Result<(), &'static str> {
    if let (Some(Job::Action(previous, a)), Job::Action(next, b)) = (queue.back(), &job) {
        if previous == next
            && [
                "/api/performance",
                "/api/dpi",
                "/api/polling",
                "/api/brightness",
                "/api/sleep",
                "/api/device-sleep",
                "/api/critical-threshold",
                "/api/lighting-settings",
                "/api/lighting-policy",
                "/api/source-custom-colors",
                "/api/source-effect",
            ]
            .contains(previous)
            && a["id"] == b["id"]
            && a["expected_profile"] == b["expected_profile"]
            && a["expected_device"] == b["expected_device"]
            && (next != &"/api/source-effect" || a["source_effect_id"] == b["source_effect_id"])
        {
            *queue.back_mut().unwrap() = job;
            return Ok(());
        }
    }
    if queue.len() >= 16 {
        return Err("The device queue is full; wait for the current operation");
    }
    queue.push_back(job);
    Ok(())
}

struct Reply {
    profile_modal: Option<profile_modal_model::Message>,
    state: Option<Value>,
    buttons: Option<Value>,
    status: String,
    failed: bool,
    notices: Vec<String>,
}

enum WorkerReply {
    StudioControl(crate::host_studio::session::Control, crate::lighting_status::Reader, Value),
    StudioOutput(Result<Value, String>),
    Studio(studio_view::Response),
    LinkedGames(Result<linked_games::Data,String>),
    MacroLibrary(Result<macro_view::Data,String>),
    Sensor((String, String), Result<(Value,mouse_feedback::BindingResult), String>),
    User(Reply),
    CachedLighting(runtime_status::Response),
}

/// Device access runs on one worker, under the same engine lock as the tray.
/// The UI thread never holds that lock and no browser or HTTP listener is started.
fn worker(
    engine: Arc<Mutex<Engine>>,
    jobs: mpsc::Receiver<Job>,
    replies: mpsc::Sender<WorkerReply>,
) {
    // Clone once before processing the initial ordinary snapshot. Subsequent
    // status jobs never contend with a uniform batch for the Engine mutex.
    let status_reader = engine
        .lock()
        .ok()
        .map(|engine| engine.lighting_status_reader());
    if let (Ok(engine), Some(reader)) = (engine.lock(), status_reader.as_ref()) {
        let _ = replies.send(WorkerReply::StudioControl(engine.studio_control(), reader.clone(), engine.studio_status()));
    }
    for job in jobs {
        let job = match job {
            Job::MacroLibrary(request)=>{
                let context=engine.lock().map_err(|_|"Device worker is unavailable".to_string()).map(|engine|engine.is_measurement_read_only());
                let result=context.and_then(|read_only|macro_view::work(request,read_only));
                if replies.send(WorkerReply::MacroLibrary(result)).is_err(){break;}continue;
            }
            Job::LinkedGames(request)=>{
                let context=engine.lock().map_err(|_|"Device worker is unavailable".to_string()).map(|engine|{
                    let rows=engine.devices().iter().map(|device|linked_games::DeviceRow{id:device.key(),name:device.name().trim_end_matches(" (Wired)").trim_end_matches(" (Wireless)").to_string(),pid:device.pid,profiles:vec![]}).collect();
                    (rows,engine.is_measurement_read_only())
                });
                let result=context.and_then(|(devices,read_only)|linked_games::work(request,devices,read_only));
                if replies.send(WorkerReply::LinkedGames(result)).is_err(){break;}continue;
            }
            Job::StudioOutput(request) => {
                let result = engine.lock().map_err(|_| "Device worker is unavailable".to_string())
                    .and_then(|mut engine| studio_view::output_work(&mut engine, request));
                if replies.send(WorkerReply::StudioOutput(result)).is_err() { break; }
                continue;
            }
            Job::Studio(request) => {
                // Drop the device lock before reading/writing composition or bookmark files.
                let read_only=matches!(&request,studio_view::Request::Save{..}|studio_view::Request::Remember{..})
                    && engine.lock().map(|engine|engine.is_measurement_read_only()).unwrap_or(true);
                if replies.send(WorkerReply::Studio(studio_view::work(request,read_only))).is_err() {break;}
                continue;
            }
            Job::SensorRead(id, profile) => {
                let result=engine.lock().map_err(|_|"Device worker is unavailable".to_string())
                    .and_then(|mut engine|{
                        let view=server::mouse_sensor_snapshot(&mut engine,&id,&profile).map_err(|e|e.to_string())?;
                        let binding=if view["dynamic"]["available"]==true&&view["dynamic"]["raw"]["state"]==1 {
                            engine.devices().iter().find(|d|d.key()==id).ok_or_else(||"Selected mouse disconnected".to_string()).and_then(|device|secbird_core::mouse_input_identity::discover(device).map_err(|e|e.to_string()))
                        }else{Err("Live graph is inactive".into())};
                        Ok((view,binding))
                    });
                if replies.send(WorkerReply::Sensor((id,profile),result)).is_err() { break; }
                continue;
            }
            Job::CachedLighting(request) => {
                let response = match &status_reader {
                    Some(reader) => runtime_status::read(reader, request),
                    None => runtime_status::Response {
                        request,
                        payload: runtime_status::Payload::Unavailable,
                    },
                };
                if replies.send(WorkerReply::CachedLighting(response)).is_err() {
                    break;
                }
                continue;
            }
            Job::Profiles(request) => {
                let reply = profile_modal_backend::run(request, &engine);
                let _ = replies.send(WorkerReply::User(reply));
                continue;
            }
            other => other,
        };
        let result = (|| -> Result<Reply, String> {
            if let Job::ImportFile { path, target } = job {
                let bytes = profile_files::read(&path)?;
                let mut engine = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())?;
                let answer =
                    server::import_profile_document(&mut engine, &bytes, target.as_deref())
                        .map_err(|e| e.to_string())?;
                return Ok(Reply {
                    profile_modal: None,
                    state: answer.get("state").cloned(),
                    buttons: None,
                    status: format!(
                        "Imported {} profile(s). Select a profile to apply it.",
                        answer["added"].as_u64().unwrap_or(0)
                    ),
                    failed: false,
                    notices: strings(&answer["warnings"]),
                });
            }
            if let Job::ExportFile {
                path,
                format,
                target,
            } = job
            {
                let report = {
                    let _engine = engine
                        .lock()
                        .map_err(|_| "Device worker is unavailable".to_string())?;
                    server::export_profile_document(match format {
                        profile_files::Format::Native => None,
                        profile_files::Format::Synapse4 => Some(target.as_str()),
                    })
                    .map_err(|e| e.to_string())?
                };
                profile_files::write(&path, &report.bytes)?;
                return Ok(Reply {
                    profile_modal: None,
                    state: None,
                    buttons: None,
                    status: "Profile export saved".into(),
                    failed: false,
                    notices: report.warnings,
                });
            }
            let mut engine = engine
                .lock()
                .map_err(|_| "Device worker is unavailable".to_string())?;
            let mut buttons = None;
            let mut status = "Ready".to_string();
            let mut failed = false;
            let mut notices = Vec::new();
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
                    notices = strings(&answer["warnings"]);
                    if let Some(failures) = answer
                        .get("failures")
                        .and_then(Value::as_array)
                        .filter(|v| !v.is_empty())
                    {
                        status = format!(
                            "Some settings failed: {}",
                            failures
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                        failed = true;
                    }
                    if answer.get("buttons").is_some() {
                        buttons = Some(answer);
                    }
                    if !failed {
                        status = "Settings applied".into();
                    }
                    server::dispatch(&mut engine, "GET", "/api/state", &[])
                }
                Job::CachedLighting(_)
                | Job::LinkedGames(_)
                | Job::MacroLibrary(_)
                | Job::Studio(_)
                | Job::StudioOutput(_)
                | Job::SensorRead(..)
                | Job::Profiles(_)
                | Job::ImportFile { .. }
                | Job::ExportFile { .. } => return Err("File operation was not routed".into()),
            }
            .map_err(|e| e.to_string())?;
            Ok(Reply {
                profile_modal: None,
                state: Some(state),
                buttons,
                status,
                failed,
                notices,
            })
        })();
        let reply = result.unwrap_or_else(|error| Reply {
            profile_modal: None,
            state: None,
            buttons: None,
            status: error,
            failed: true,
            notices: Vec::new(),
        });
        if replies.send(WorkerReply::User(reply)).is_err() {
            break;
        }
    }
}

struct Fonts {
    calibration_welcome: HFONT,
    source_text: source_text::Renderer,
    tab: HFONT,
    tiny: HFONT,
    small: HFONT,
    normal: HFONT,
    heading: HFONT,
    product: HFONT,
    widget_heading: HFONT,
    input: HFONT,
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
            source_text: source_text::Renderer::new(assets),
            calibration_welcome: make("RazerF5",26,400),
            tab: make("Roboto", 12, 300),
            tiny: make("Roboto", 10, 400),
            small: make("Roboto", 12, 400),
            normal: make("Roboto", 14, 400),
            heading: make("RazerF5", 20, 400),
            product: make("Roboto", 16, 400),
            widget_heading: make("RazerF5", 16, 400),
            input: make("Arial", 14, 400),
            loaded,
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            for font in [
                self.calibration_welcome,
                self.tab,
                self.tiny,
                self.small,
                self.normal,
                self.heading,
                self.product,
                self.widget_heading,
                self.input,
            ] {
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
    alpha: bool,
    smooth: *mut windows_sys::Win32::Graphics::GdiPlus::GpBitmap,
}

impl Bitmap {
    unsafe fn load(path: &Path) -> Option<Self> {
        let alpha_path = path.with_extension("bgra");
        if alpha_path.exists() {
            return Self::load_alpha(&alpha_path);
        }
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
            alpha: false,
            smooth: null_mut(),
        })
    }

    // Provisioned files contain top-down, premultiplied BGRA pixels. Keeping
    // alpha separate from LoadImage avoids its legacy BMP alpha conversion.
    unsafe fn load_alpha(path: &Path) -> Option<Self> {
        use std::io::Read;
        let mut file = std::fs::File::open(path).ok()?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header).ok()?;
        if &header[..8] != b"SLBGRA1\0" {
            return None;
        }
        let width = u32::from_le_bytes(header[8..12].try_into().ok()?);
        let height = u32::from_le_bytes(header[12..16].try_into().ok()?);
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return None;
        }
        let length = width as usize * height as usize * 4;
        if file.metadata().ok()?.len() != (16 + length) as u64 {
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width as i32;
        info.bmiHeader.biHeight = -(height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = null_mut();
        let handle = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
        if handle.is_null() {
            return None;
        }
        let mut bitmap = Self {
            handle,
            width: width as i32,
            height: height as i32,
            alpha: true,
            smooth: null_mut(),
        };
        let buffer = std::slice::from_raw_parts_mut(pixels.cast::<u8>(), length);
        file.read_exact(buffer).ok()?;
        // Refuse straight-alpha or malformed generated data rather than
        // producing bright fringes around semitransparent edges.
        if buffer
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|c| *c > p[3]))
        {
            return None;
        }
        bitmap.smooth = image_render::from_pixels(width as i32, height as i32, buffer.as_ptr());
        if bitmap.smooth.is_null() {
            return None;
        }
        Some(bitmap)
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            image_render::dispose(self.smooth);
            DeleteObject(self.handle);
        }
    }
}

struct State {
    linked_games:linked_games::View,
    macros:macro_view::View,
    studio: studio_view::View,
    exit_tray: Option<HWND>,
    calibration: calibration_view::View,
    advanced: advanced_view::View,
    mouse_feedback: mouse_feedback::Observer,
    host_tabs: RefCell<host_tabs::View>,
    customize_background: RefCell<dot_background::Cache>,
    hwnd: HWND,
    scale: f32,
    width: i32,
    height: i32,
    state: Value,
    selected: Option<String>,
    tab: Tab,
    profile_open: bool,
    profile_offset: usize,
    profile_actions: bool,
    profile_modal: Option<profile_modal_model::View>,
    profile_export: Option<profile_modal_model::ExportPlan>,
    renaming: Option<String>,
    notices: Vec<String>,
    selected_button: String,
    assignment: assignment_model::View,
    assignment_next: Option<Box<Action>>,
    buttons: Vec<Value>,
    hits: Vec<Hit>,
    focus: usize,
    fields: HashMap<u16, HWND>,
    field_defaults: HashMap<u16, String>,
    field_owner: Option<(String, String, String)>,
    retained_fields: HashMap<u16, String>,
    performance_input_text: HashMap<u16, String>,
    jobs: mpsc::Sender<Job>,
    replies: mpsc::Receiver<WorkerReply>,
    runtime_observer: runtime_status::Observer,
    lighting_probe: bool,
    probe_kind: Option<crate::runtime_probe::ProbeKind>,
    read_only_smoke: bool,
    restricted_actions: bool,
    lighting_runtime_observed: Option<std::time::Instant>,
    runtime_feedback_waiting: bool,
    pending: usize,
    pending_jobs: VecDeque<Job>,
    closing: bool,
    failures: feedback::Failures,
    performance: Option<Performance>,
    performance_owner: Option<(String, String)>,
    performance_scroll: f64,
    performance_hover: Option<usize>,
    performance_hover_hit: Option<usize>,
    performance_gesture: Option<PerformanceGesture>,
    power_gesture: Option<power_view::Gesture>,
    lighting: lighting_view::View,
    lighting_capture: bool,
    status: String,
    failed: bool,
    smoke_view: Option<String>,
    smoke_requested: Option<String>,
    smoke_reported: bool,
    smoke_overlay_started: bool,
    assets: PathBuf,
    fonts: Fonts,
    mouse: Option<Bitmap>,
    device_artwork: RefCell<device_artwork::View>,
    keyboard: Option<Bitmap>,
    keyboard_customize: Option<Bitmap>,
    keyboard_layout: Option<source_layout::SourceLayout>,
    icons: HashMap<&'static str, Bitmap>,
    edit_brush: HBRUSH,
    dim_edit_brush: HBRUSH,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.edit_brush);
            DeleteObject(self.dim_edit_brush);
        }
    }
}

impl State {
    fn source_viewport(&self) -> bool {
        self.studio.open || self.device()
            .and_then(|d| d["pid"].as_str())
            .is_some_and(|pid| viewport::covered(pid, self.tab.title()))
    }
    fn content_bottom_css(&self) -> f64 {
        viewport::bottom(
            f64::from(self.height) / f64::from(self.scale),
            self.source_viewport(),
        )
    }
    fn content_bottom_px(&self) -> i32 {
        if self.source_viewport() {
            self.height
        } else {
            self.height - self.px(35)
        }
    }
    fn lighting_point(&self, point: POINT) -> lighting_view::Point {
        lighting_view::Point {
            x: f64::from(point.x) / f64::from(self.scale),
            y: f64::from(point.y) / f64::from(self.scale),
        }
    }

    unsafe fn lighting_update(&mut self, update: lighting_view::Update) -> bool {
        match update.capture {
            lighting_view::Capture::None => {}
            lighting_view::Capture::Start => {
                self.lighting_capture = true;
                SetCapture(self.hwnd);
                SetTimer(self.hwnd, LIGHTING_REPEAT, 300, None);
            }
            lighting_view::Capture::Release => {
                self.lighting_capture = false;
                KillTimer(self.hwnd, LIGHTING_REPEAT);
                ReleaseCapture();
            }
        }
        for command in update.commands {
            match command {
                lighting_view::Command::CommitIntent {
                    owner,
                    settings,
                    scope,
                } => {
                    let path = match scope {
                        lighting_view::Scope::Brightness => "/api/lighting-settings",
                        lighting_view::Scope::Policy => "/api/lighting-policy",
                    };
                    self.queue(Job::Action(path,json!({"id":owner.device,"expected_device":owner.device,"expected_profile":owner.profile,"lighting_settings":settings})));
                }
                lighting_view::Command::ApplyEffect {
                    owner,
                    source_id,
                    setting,
                } => {
                    self.queue(Job::Action("/api/source-effect",json!({"id":owner.device,"expected_device":owner.device,"expected_profile":owner.profile,"source_effect_id":source_id,"source_effect_setting":setting})));
                }
                lighting_view::Command::PersistCustomColors { owner, slots } => {
                    self.queue(Job::Action("/api/source-custom-colors",json!({"id":owner.device,"expected_device":owner.device,"expected_profile":owner.profile,"custom_colors":slots})));
                }
                lighting_view::Command::Eyedropper => {
                    self.error("The source eyedropper is not implemented yet")
                }
            }
        }
        if update.rebuild_fields {
            self.rebuild_fields();
        }
        if update.handled {
            InvalidateRect(self.hwnd, null(), 0);
        }
        update.handled
    }
    unsafe fn sync_performance(&mut self, force: bool) {
        let device = self.device().cloned();
        let owner = device.as_ref().map(|d| {
            (
                d["id"].as_str().unwrap_or_default().to_string(),
                d["active"].as_str().unwrap_or_default().to_string(),
            )
        });
        let same_owner = owner == self.performance_owner;
        let editing = self.fields.iter().any(|(id, control)| {
            (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10).contains(id) && *control == GetFocus()
        });
        let queued = self
            .pending_jobs
            .iter()
            .any(|job| matches!(job, Job::Action("/api/performance", _)));
        if !force && same_owner && (self.performance_gesture.is_some() || queued || editing) {
            return;
        }
        if !same_owner {
            self.hits.clear();
            self.performance_scroll = 0.0;
            self.performance_hover = None;
            self.performance_gesture = None;
            KillTimer(self.hwnd, PERFORMANCE_REPEAT);
            ReleaseCapture();
        }
        self.performance_owner = owner;
        let performance = device.as_ref().and_then(|d| {
            if !matches!(d["pid"].as_str(), Some("00C4" | "00C5")) {
                return None;
            }
            if d["profile"]["performance"].is_object() {
                let intent: Performance =
                    serde_json::from_value(d["profile"]["performance"].clone()).ok()?;
                return intent.validate().ok().map(|_| intent);
            }
            let stages = if d["profile"]["dpi_stages"].is_object() {
                d["profile"]["dpi_stages"].clone()
            } else {
                json!({"active":d["stages"]["active"],"stages":d["stages"]["list"]})
            };
            let stages = serde_json::from_value(stages).ok()?;
            Performance::from_hardware(&stages).ok()
        });
        if self.performance != performance {
            self.hits.clear();
        }
        self.performance = performance;
    }

    unsafe fn commit_performance(&mut self) {
        if let (Some(intent), Some((device, profile))) =
            (&self.performance, &self.performance_owner)
        {
            self.queue(Job::Action(
                "/api/performance",
                json!({"id":device,"expected_profile":profile,"performance":intent}),
            ));
        }
    }

    unsafe fn edit_performance(&mut self, row: usize, axis: Axis, value: u16, commit: bool) {
        if !commit
            && self
                .performance
                .as_ref()
                .and_then(|p| p.stages.get(row))
                .is_some_and(|r| {
                    if axis == Axis::X {
                        r.x == value
                    } else {
                        r.y == value
                    }
                })
        {
            return;
        }
        let result = self
            .performance
            .as_mut()
            .ok_or("Performance data is unavailable".to_string())
            .and_then(|p| {
                let result = match axis {
                    Axis::X => p.set_x(row, value),
                    Axis::Y => p.set_y(row, value),
                };
                result.map_err(|e| e.to_string())?;
                if commit && p.stages[row].visible {
                    p.active = row as u8 + 1;
                }
                Ok(())
            });
        if let Err(error) = result {
            self.error(&error);
            return;
        }
        if let Some(p) = &self.performance {
            for (axis, value) in [(Axis::X, p.stages[row].x), (Axis::Y, p.stages[row].y)] {
                let id = PERFORMANCE_INPUT + row as u16 * 2 + u16::from(axis == Axis::Y);
                if let Some(control) = self.fields.get(&id) {
                    let text = value.to_string();
                    SetWindowTextW(*control, wide(&text).as_ptr());
                    self.field_defaults.insert(id, text.clone());
                    self.performance_input_text.insert(id, text);
                }
            }
        }
        if commit {
            self.commit_performance();
        }
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn commit_performance_input(&mut self, id: u16) {
        let Some(index) = id.checked_sub(PERFORMANCE_INPUT).filter(|v| *v < 10) else {
            return;
        };
        let axis = if index % 2 == 0 { Axis::X } else { Axis::Y };
        let value = performance_layout::normalize_input_value(self.text(id).parse::<i64>().ok());
        self.edit_performance(index as usize / 2, axis, value as u16, true);
    }

    unsafe fn step_performance(&mut self, row: usize, axis: Axis, delta: i16) {
        let id = PERFORMANCE_INPUT + row as u16 * 2 + u16::from(axis == Axis::Y);
        let current = if self.fields.contains_key(&id) {
            // Empty/minus are valid transient text states, not a request to
            // restore the previous model value before stepping.
            self.text(id).parse::<i64>().ok()
        } else {
            self.performance
                .as_ref()
                .and_then(|p| p.stages.get(row))
                .map(|r| i64::from(if axis == Axis::X { r.x } else { r.y }))
        };
        let value = performance_layout::step_input_value(current, delta);
        let editing = self
            .fields
            .get(&id)
            .is_some_and(|control| *control == GetFocus());
        self.edit_performance(row, axis, value as u16, !editing);
    }

    unsafe fn performance_pointer(&mut self, point: POINT) {
        let old_hover = self.performance_hover;
        let old_hit = self.performance_hover_hit;
        self.performance_hover_hit = self
            .hits
            .iter()
            .rposition(|hit| PtInRect(&hit.rect, point) != 0);
        let logical_x = f64::from(point.x) / f64::from(self.scale);
        let logical_y = f64::from(point.y) / f64::from(self.scale);
        if let Some((geometry, indices)) = performance_view::layout(self) {
            self.performance_hover = geometry
                .stages
                .iter()
                .zip(&indices)
                .find(|(g, _)| {
                    logical_x >= g.row.x
                        && logical_x < g.row.x + g.row.width
                        && logical_y >= g.row.y
                        && logical_y < g.row.y + g.row.height
                })
                .map(|(_, row)| *row);
        }
        if logical_y < 128.0 || logical_y >= self.content_bottom_css() {
            self.performance_hover = None;
        }
        match self.performance_gesture {
            Some(PerformanceGesture::Slider { row, axis, rect }) => {
                // Native range controls reserve half the 12px thumb at each end.
                let percent =
                    ((logical_x - rect.x - 6.0) / (rect.width - 12.0) * 100.0).clamp(0.0, 100.0);
                if let Some(value) = performance_layout::percent_to_dpi(percent) {
                    self.edit_performance(row, axis, value as u16, false);
                }
            }
            Some(PerformanceGesture::Reorder { row, .. }) => {
                if let Some(target) = self.performance_hover {
                    self.performance_gesture = Some(PerformanceGesture::Reorder { row, target });
                }
            }
            _ => {}
        }
        if old_hover != self.performance_hover
            || old_hit != self.performance_hover_hit
            || self.performance_gesture.is_some()
        {
            InvalidateRect(self.hwnd, null(), 0);
        }
    }

    unsafe fn finish_performance_gesture(&mut self) {
        let gesture = self.performance_gesture.take();
        KillTimer(self.hwnd, PERFORMANCE_REPEAT);
        ReleaseCapture();
        match gesture {
            Some(PerformanceGesture::Slider { row, axis, .. }) => {
                if let Some(value) = self
                    .performance
                    .as_ref()
                    .and_then(|p| p.stages.get(row))
                    .map(|r| if axis == Axis::X { r.x } else { r.y })
                {
                    self.edit_performance(row, axis, value, true);
                }
            }
            Some(PerformanceGesture::Reorder { row, target }) => {
                if row != target && self.performance_hover == Some(target) {
                    let result = self.performance.as_mut().map(|p| p.move_row(row, target));
                    if let Some(Err(error)) = result {
                        self.error(&error.to_string());
                    } else {
                        self.commit_performance();
                        self.rebuild_fields();
                    }
                }
            }
            _ => {}
        }
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn performance_field(&mut self, id: u16, value: u16, rect: performance_layout::Rect) {
        let value = value.to_string();
        self.field_defaults.insert(id, value.clone());
        let shown = self.retained_fields.remove(&id).unwrap_or(value);
        self.performance_input_text.insert(id, shown.clone());
        let px = |v: f64| (v * f64::from(self.scale)).round() as i32;
        let control = CreateWindowExW(
            0,
            wide("EDIT").as_ptr(),
            wide(&shown).as_ptr(),
            WS_VISIBLE | WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            px(rect.x + 7.0),
            px(rect.y + 1.0),
            px(55.0),
            px(24.0),
            self.hwnd,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        if !control.is_null() {
            SendMessageW(control, WM_SETFONT, self.fonts.input as usize, 1);
            SendMessageW(control, EM_SETLIMITTEXT, 6, 0);
            SendMessageW(control, EM_SETMARGINS, 3, 0);
            let top = (self.px(128) - px(rect.y + 1.0)).clamp(0, px(24.0));
            let bottom = (self.content_bottom_px() - px(rect.y + 1.0)).clamp(0, px(24.0));
            let region = CreateRectRgn(0, top.min(bottom), px(41.0), bottom);
            if !region.is_null() && SetWindowRgn(control, region, 1) == 0 {
                DeleteObject(region);
            }
            self.fields.insert(id, control);
        }
    }

    unsafe fn create_profile(&mut self, base: &str, duplicate: bool) {
        self.profile_actions = false;
        let names = self
            .device()
            .map(|d| strings(&d["profiles"]))
            .unwrap_or_default();
        let Some(name) = available_profile_name(base, &names, duplicate) else {
            self.error("No unused profile name is available");
            return;
        };
        self.queue(Job::Action("/api/profile", json!({"id":self.selected,"action":if duplicate {"duplicate"} else {"add"},"name":name})));
    }

    unsafe fn finish_rename(&mut self, cancel: bool) {
        let Some(previous) = self.renaming.take() else {
            return;
        };
        let name = self.text(PROFILE_NAME);
        if !cancel && !name.is_empty() && name != previous {
            self.queue(Job::Action(
                "/api/profile",
                json!({"id":self.selected,"action":"rename","previous_name":previous,"name":name}),
            ));
        }
        self.retained_fields.remove(&PROFILE_NAME);
        self.rebuild_fields();
        InvalidateRect(self.hwnd, null(), 0);
    }

    /// Open only read-only presentation paths once, after the real device reply.
    /// Export previews use the normal worker; no file path/commit is generated.
    unsafe fn prepare_smoke_overlay(&mut self) {
        if self.smoke_overlay_started || self.smoke_reported {
            return;
        }
        let Some(requested) = self.smoke_requested.clone() else {
            return;
        };
        if !matches!(
            requested.as_str(),
            "keyboard-lighting-color-editor"
                | "keyboard-profile-import"
                | "keyboard-profile-export"
                | "keyboard-profile-cloud"
        ) {
            return;
        }
        self.smoke_overlay_started = true;
        if !self
            .device()
            .is_some_and(|d| d["pid"] == "025D" && d["active"].as_str().is_some())
        {
            return;
        }
        match requested.as_str() {
            "keyboard-lighting-color-editor" => {
                lighting_view::open_read_only_smoke_editor(self);
            }
            "keyboard-profile-export" => self.open_profile_modal(profile_modal_model::Mode::Export),
            "keyboard-profile-import" | "keyboard-profile-cloud" => {
                self.open_profile_modal(profile_modal_model::Mode::Import);
                if requested == "keyboard-profile-cloud" {
                    self.profile_modal_action(profile_modal_model::Control::Cloud(true));
                }
            }
            _ => {}
        }
    }

    fn profile_smoke_metadata(&self) -> Value {
        let Some(v) = self.profile_modal.as_ref() else {
            return json!({"open":false,"ready":false});
        };
        let owner_matches = self.device().is_some_and(|d| {
            d["id"].as_str() == Some(v.owner.device.as_str())
                && d["active"].as_str() == Some(v.owner.profile.as_str())
        });
        let rows = v.data.as_ref().map(|d| d.rows.len()).unwrap_or(0);
        let data_ready = v.mode == profile_modal_model::Mode::Import || v.data.is_some();
        json!({"open":true,"mode":if v.mode==profile_modal_model::Mode::Import{"import"}else{"export"},
            "cloud":v.cloud,"loading":v.loading,"has_error":!v.error.is_empty(),"warning":v.warning,
            "owner_matches_current_snapshot":owner_matches,"profile_rows":rows,
            "selected_profiles":v.choices.iter().filter(|c|c.selected).count(),
            "macro_rows":v.data.as_ref().map(|d|d.rows.iter().map(|r|r.macros.len()).sum::<usize>()).unwrap_or(0),
            "selected_macro_references":v.choices.iter().map(|c|c.macros.len()).sum::<usize>(),
            "has_browsed_file":!v.filename.is_empty(),"commit_enabled":v.enabled(),
            "ready":owner_matches&&!v.loading&&v.error.is_empty()&&data_ready})
    }

    /// Record the actually presented read-only view, without screenshots or serials.
    /// A requested view is not considered covered merely because the process ran.
    unsafe fn record_smoke_frame(&mut self, presented: bool) {
        if (self.smoke_reported && !self.lighting_probe)
            || self.smoke_view.is_some()
            || self.pending != 0
        {
            return;
        }
        let Some(requested) = &self.smoke_requested else {
            return;
        };
        let Some(path) = std::env::var_os("SYNAPSE_LITE_SMOKE_REPORT") else {
            return;
        };
        if self.profile_modal.as_ref().is_some_and(|v| v.loading) {
            return;
        }
        let kind = self.device().and_then(|d| d["kind"].as_str());
        let color_editor = lighting_view::smoke_metadata(self);
        let profile_modal = self.profile_smoke_metadata();
        let modal_icons = [
            "profile-close",
            "profile-folder",
            "profile-macro",
            "profile-cone",
            "profile-warning",
        ]
        .iter()
        .filter(|name| self.icons.contains_key(**name))
        .count();
        let matched = match requested.as_str() {
            "dashboard" => self.selected.is_none(),
            "mouse-performance-hidden" => {
                kind == Some("mouse")
                    && self.tab == Tab::Performance
                    && performance_view::layout(self).is_some()
                    && self
                        .performance
                        .as_ref()
                        .is_some_and(|p| p.visible_count() < p.stages.len())
            }
            "mouse-performance-independent" => {
                kind == Some("mouse")
                    && self.tab == Tab::Performance
                    && performance_view::layout(self).is_some()
                    && self
                        .performance
                        .as_ref()
                        .is_some_and(|p| p.stages.iter().any(|r| r.independent))
            }
            "mouse-performance" | "mouse-performance-polling" => {
                kind == Some("mouse")
                    && self.tab == Tab::Performance
                    && performance_view::layout(self).is_some()
            }
            "keyboard-lighting" => {
                kind == Some("keyboard")
                    && self.tab == Tab::Lighting
                    && self.device().is_some_and(|d| {
                        d["pid"] == "025D"
                            && d["lighting_readback"]["available"] == true
                            && d["lighting_readback"]["selectors_match"] == true
                    })
                    && self.lighting.owner.is_some()
                    && self.lighting.intent.brightness.is_some()
                    && self
                        .icons
                        .keys()
                        .filter(|name| name.starts_with("lighting-"))
                        .count()
                        == 17
                    && self.fonts.loaded.len() == 3
            }
            "keyboard-lighting-color-editor" => {
                kind == Some("keyboard")
                    && self.tab == Tab::Lighting
                    && color_editor["ready"] == true
                    && self.fonts.loaded.len() == 3
                    && self
                        .icons
                        .keys()
                        .filter(|name| name.starts_with("lighting-"))
                        .count()
                        == 17
            }
            "keyboard-profile-import" | "keyboard-profile-export" | "keyboard-profile-cloud" => {
                let expected_mode = if requested == "keyboard-profile-export" {
                    "export"
                } else {
                    "import"
                };
                kind == Some("keyboard")
                    && self.device().is_some_and(|d| d["pid"] == "025D")
                    && profile_modal["ready"] == true
                    && profile_modal["mode"] == expected_mode
                    && profile_modal["cloud"] == json!(requested == "keyboard-profile-cloud")
                    && profile_modal["has_browsed_file"] == false
                    && (requested != "keyboard-profile-export"
                        || profile_modal["selected_profiles"]
                            .as_u64()
                            .is_some_and(|n| n > 0))
                    && modal_icons == 5
                    && self.icons.contains_key("expand")
                    && self.fonts.loaded.len() == 3
            }
            "mouse-power" => {
                kind == Some("mouse")
                    && power_view::layout(self).is_some()
                    && self.device().is_some_and(|d| {
                        d["device_sleep"]["available"] == true
                            && d["critical_threshold"]["available"] == true
                    })
            }
            "keyboard-customize" => {
                kind == Some("keyboard")
                    && self.tab == Tab::Customize
                    && self.keyboard_layout.is_some()
                    && self.keyboard_customize.is_some()
            }
            _ => false,
        };
        let active_runtime = self.lighting_probe.then(|| {
            let age = self
                .lighting_runtime_observed
                .map(|time| time.elapsed().as_millis().min(u128::from(u64::MAX)) as u64);
            crate::runtime_probe::fresh_readiness_for(
                self.probe_kind.unwrap_or(crate::runtime_probe::ProbeKind::Uniform),
                lighting_view::runtime_snapshot(&self.lighting),
                self.device()
                    .and_then(|d| d["active"].as_str())
                    .unwrap_or(""),
                age,
            )
        });
        let matched = matched
            && presented
            && IsWindowVisible(self.hwnd) != 0
            && active_runtime.as_ref().is_none_or(|v| v["ready"] == true);
        let mut report = json!({
            "requested": requested,
            "matched": matched,
            "presented": presented,
            "visible": IsWindowVisible(self.hwnd) != 0,
            "width": self.width,
            "height": self.height,
            "scale": self.scale,
            "dpi_awareness": GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(self.hwnd)),
            "device_kind": kind,
            "tab": self.tab.title(),
            "detected_devices": self.devices().len(),
            "loaded_source_fonts": self.fonts.loaded.len(),
            "mouse_artwork": self.mouse.is_some(),
            "device_dashboard_artwork": self.device_artwork.borrow().snapshot(),
            "keyboard_artwork": self.keyboard.is_some(),
            "keyboard_customize_artwork": self.keyboard_customize.is_some(),
            "keyboard_source_inputs": self.keyboard_layout.as_ref().map(|layout| layout.keys.len()),
            "keyboard_named_source_inputs": self.keyboard_layout.as_ref().map(|layout| layout.keys.iter().filter(|key| !key.button_key.is_empty()).count()),
            "keyboard_source_assignment_options": self.keyboard_layout.as_ref().map(|layout| layout.keys.iter().map(|key| key.function_list.len()).sum::<usize>()),
            "source_performance_view": performance_view::layout(self).is_some(),
            "source_power_view": power_view::layout(self).is_some(),
            "source_lighting_view": self.tab == Tab::Lighting && self.lighting.owner.is_some(),
            "lighting_brightness_readback": self.device().map(|d|d["lighting_readback"].clone()),
            "lighting_intent_origin": self.device().map(|d|d["lighting_settings_origin"].clone()),
            "lighting_source_icons": self.icons.keys().filter(|name|name.starts_with("lighting-")).count(),
            "lighting_overlay":color_editor,
            "profile_overlay":profile_modal,
            "profile_source_icons":modal_icons,
            "smoke_overlay_started":self.smoke_overlay_started,
            "power_sleep_available": self.device().map(|d|d["device_sleep"]["available"].clone()),
            "power_threshold_available": self.device().map(|d|d["critical_threshold"]["available"].clone()),
            "performance_configured_rows": self.performance.as_ref().map(|p|p.stages.len()),
            "performance_visible_rows": self.performance.as_ref().map(|p|p.visible_count()),
            "performance_independent_rows": self.performance.as_ref().map(|p|p.stages.iter().filter(|r|r.independent).count()),
            "performance_inputs": self.fields.keys().filter(|id|(PERFORMANCE_INPUT..PERFORMANCE_INPUT+10).contains(id)).count(),
            "performance_scroll_css": self.performance_scroll,
            "presentation_only_override": matches!(requested.as_str(),"mouse-performance-independent"|"mouse-performance-hidden") || color_editor["presentation_only_effect_override"]==true,
            "failed": self.failed,
            "source_viewport":self.source_viewport(),
            "reserved_footer_css":if self.source_viewport(){0}else{35},
            "fidelity": "partial source-derived native port; pixel identity unproven"
        });
        report["source_text_fallback"] = json!(self.fonts.source_text.diagnostic());
        report["customize_background_cached"] = json!(self.customize_background.borrow().ready());
        report["source_text_cached_layouts"] = json!(self.fonts.source_text.metrics().cached_layouts);
        report["active_native_runtime"] = active_runtime.unwrap_or(Value::Null);
        report["uniform_worker_disabled"] = json!(!self.lighting_probe);
        report["intrinsic_read_only_measurement"] = json!(
            self.read_only_smoke
                && self.state["measurement_read_only"] == true
                && self.state["query_only_device_handles"] == true
        );
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
        self.submit(job, true);
    }
    unsafe fn submit(&mut self, job: Job, user_operation: bool) {
        // Measurement presentations reject device/file mutations. A read-only
        // profile-export preview remains allowed solely to render its dialog.
        let presentation_preview = self.read_only_smoke
            && matches!(
                &job,
                Job::Profiles(profile_modal_model::Request::PreviewExport { .. })
            );
        if self.restricted_actions && !matches!(&job, Job::Refresh | Job::SensorRead(..) | Job::LinkedGames(linked_games::Request::Read) | Job::MacroLibrary(macro_view::Request::Read) | Job::Studio(studio_view::Request::Restore{..})) && !presentation_preview {
            return;
        }
        if self.pending > 0 {
            if let Err(error) = enqueue_setting(&mut self.pending_jobs, job) {
                self.error(error);
            }
            InvalidateRect(self.hwnd, null(), 0);
            return;
        }
        if user_operation && !matches!(&job, Job::Refresh | Job::Rescan) {
            self.failures.new_operation();
        }
        let sensor_read=matches!(&job,Job::SensorRead(..));
        if self.jobs.send(job).is_ok() {
            self.pending += 1;
            if !sensor_read { self.status = "Working…".into();self.failed = false; }
            SetTimer(self.hwnd, RESPONSE_TIMER, 40, None);
            InvalidateRect(self.hwnd, null(), 0);
        } else {
            self.error("The native device worker is unavailable; this operation was not accepted");
            InvalidateRect(self.hwnd, null(), 0);
        }
    }

    fn runtime_binding(&self) -> Option<runtime_status::Binding> {
        if self.tab != Tab::Lighting {
            return None;
        }
        let device = self.device()?;
        if device["pid"] != "025D" {
            return None;
        }
        let owner = self.lighting.owner.as_ref()?;
        if device["id"].as_str() != Some(owner.device.as_str())
            || device["active"].as_str() != Some(owner.profile.as_str())
        {
            return None;
        }
        Some(runtime_status::Binding {
            device: owner.device.clone(),
            profile: owner.profile.clone(),
        })
    }
    fn runtime_feedback_busy(&self) -> bool {
        self.lighting_capture
            || lighting_view::runtime_feedback_busy(&self.lighting)
            || self.profile_modal.is_some()
    }
    unsafe fn flush_runtime_feedback(&mut self) {
        if self.runtime_feedback_waiting
            && !self.runtime_feedback_busy()
            && IsWindowVisible(self.hwnd) != 0
        {
            self.runtime_feedback_waiting = false;
            if self.failures.outstanding() {
                PostMessageW(self.hwnd, FAILURE_DIALOG, 1, 0);
            }
        }
    }
    unsafe fn request_cached_lighting(&mut self) {
        self.flush_runtime_feedback();
        if self.closing
            || self.pending != 0
            || !self.pending_jobs.is_empty()
            || IsWindowVisible(self.hwnd) == 0
            || IsIconic(self.hwnd) != 0
        {
            return;
        }
        let Some(binding) = self.runtime_binding() else {
            return;
        };
        let Some(request) = self
            .runtime_observer
            .begin(binding, std::time::Instant::now())
        else {
            return;
        };
        if self.jobs.send(Job::CachedLighting(request.clone())).is_ok() {
            SetTimer(self.hwnd, RESPONSE_TIMER, 40, None);
        } else {
            self.collect_cached_lighting(runtime_status::Response {
                request,
                payload: runtime_status::Payload::Unavailable,
            });
        }
    }
    unsafe fn collect_cached_lighting(&mut self, response: runtime_status::Response) {
        let binding = self.runtime_binding();
        let change = self.runtime_observer.finish(response, binding.as_ref());
        let mut changed = false;
        if let Some(runtime) = change.runtime {
            self.lighting_runtime_observed = change.observed_at;
            changed = lighting_view::update_runtime_only(&mut self.lighting, runtime.clone());
            if let Some(binding) = &binding {
                if let Some(devices) = self.state["devices"].as_array_mut() {
                    if let Some(device) = devices
                        .iter_mut()
                        .find(|d| d["id"].as_str() == Some(binding.device.as_str()))
                    {
                        device["lighting_runtime"] = runtime;
                    }
                }
            }
        }
        // A read-only reply never touches pending user counts, status/failed,
        // notices, intent or controls. Background failures have separate dedup.
        for fault in change.faults {
            if self.lighting_probe {
                self.failed = true;
                self.status = "Active Lighting probe runtime failed".into();
                continue;
            }
            if self.failures.record(&fault) {
                self.runtime_feedback_waiting = true;
            }
        }
        self.flush_runtime_feedback();
        if changed {
            InvalidateRect(self.hwnd, null(), 0);
        }
    }

    unsafe fn collect(&mut self) {
        while let Ok(message) = self.replies.try_recv() {
            let reply = match message {
                WorkerReply::LinkedGames(result)=>{self.pending=self.pending.saturating_sub(1);linked_games::receive(self,result);continue;}
                WorkerReply::MacroLibrary(result)=>{self.pending=self.pending.saturating_sub(1);macro_view::receive(self,result);continue;}
                WorkerReply::StudioControl(control, reader, status) => {
                    studio_view::output_control(self, control, reader, status);
                    continue;
                }
                WorkerReply::StudioOutput(result) => {
                    self.pending=self.pending.saturating_sub(1);
                    studio_view::output_receive(self, result);
                    continue;
                }
                WorkerReply::Studio(response) => {
                    self.pending=self.pending.saturating_sub(1);
                    studio_view::receive(self,response);
                    continue;
                }
                WorkerReply::Sensor(owner,result) => {
                    self.pending=self.pending.saturating_sub(1);
                    match result {
                        Ok((view,binding))=>{calibration_view::receive(self,owner.clone(),Ok(view));mouse_feedback::receive(self,owner,binding);}
                        Err(error)=>{calibration_view::receive(self,owner,Err(error));mouse_feedback::stop(self);}
                    }
                    continue;
                }
                WorkerReply::User(reply) => reply,
                WorkerReply::CachedLighting(response) => {
                    self.collect_cached_lighting(response);
                    continue;
                }
            };
            self.pending = self.pending.saturating_sub(1);
            if let Some(message) = reply.profile_modal {
                if let Some(view) = &mut self.profile_modal {
                    view.apply(message);
                }
            }
            if let Some(state) = reply.state {
                self.state = state;
                self.lighting_runtime_observed = None;
            }
            // Smoke-only selection follows the real read-only device snapshot.
            if let Some(view) = self.smoke_view.take() {
                if view=="studio" {self.studio.open=true;self.selected=None;}
                let target = match view.as_str() {
                    "mouse-performance"
                    | "mouse-performance-polling"
                    | "mouse-performance-independent"
                    | "mouse-performance-hidden" => Some(("mouse", Tab::Performance)),
                    "keyboard-lighting" | "keyboard-lighting-color-editor" => {
                        Some(("keyboard", Tab::Lighting))
                    }
                    "keyboard-profile-import"
                    | "keyboard-profile-export"
                    | "keyboard-profile-cloud" => Some(("keyboard", Tab::Customize)),
                    "mouse-power" => Some(("mouse", Tab::Power)),
                    "mouse-calibration" => Some(("mouse", Tab::Calibration)),
                    "mouse-advanced" => Some(("mouse", Tab::Advanced)),
                    "keyboard-customize" => Some(("keyboard", Tab::Customize)),
                    _ => None,
                };
                if let Some((kind, tab)) = target {
                    if let Some(id) = self
                        .devices()
                        .iter()
                        .find(|d| {
                            d["kind"].as_str() == Some(kind)
                                && (!matches!(
                                    view.as_str(),
                                    "keyboard-lighting-color-editor"
                                        | "keyboard-profile-import"
                                        | "keyboard-profile-export"
                                        | "keyboard-profile-cloud"
                                ) || d["pid"] == "025D")
                        })
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
            if reply.failed {
                self.error(&reply.status);
            } else {
                self.status = reply.status;
                self.failed = false;
            }
            if !reply.notices.is_empty() {
                self.status = format!(
                    "{} — {} compatibility notices (see Help)",
                    self.status,
                    reply.notices.len()
                );
                self.notices = reply.notices;
            }
            if self.selected.is_some() && self.device().is_none() && !linked_games::active(self) && !macro_view::active(self) {
                self.selected = None;
                self.buttons.clear();
            }
            if self.profile_modal.as_ref().is_some_and(|view| {
                self.device().is_none_or(|d| {
                    d["id"].as_str() != Some(view.owner.device.as_str())
                        || d["active"].as_str() != Some(view.owner.profile.as_str())
                })
            }) {
                self.profile_modal = None;
                ReleaseCapture();
                self.error("The selected device/profile changed; reopen the profile dialog");
            }
            assignment_view::sync(self);
            calibration_view::request(self,false);
            self.sync_performance(false);
            if let Some(device) = self.device().cloned() {
                let preserve=self.pending_jobs.iter().any(|job|matches!(job,Job::Action(path,_) if path.starts_with("/api/lighting-")||path.starts_with("/api/source-")));
                let owner = self.lighting.owner.clone();
                lighting_view::sync(&mut self.lighting, &device, preserve);
                if owner != self.lighting.owner && self.lighting_capture {
                    self.lighting_capture = false;
                    KillTimer(self.hwnd, LIGHTING_REPEAT);
                    ReleaseCapture();
                }
            } else {
                self.lighting = lighting_view::View::default();
            }
            if !self.smoke_reported {
                if self.smoke_requested.as_deref() == Some("mouse-performance-independent") {
                    if let Some(p) = &mut self.performance {
                        p.stages[usize::from(p.active - 1)].independent = true;
                    }
                }
                if self.smoke_requested.as_deref() == Some("mouse-performance-hidden") {
                    if let Some(p) = &mut self.performance {
                        if p.stages.len() > 2 {
                            let row = (usize::from(p.active - 1) + 1) % p.stages.len();
                            p.set_visible(row, false).ok();
                        }
                    }
                }
                if self.smoke_requested.as_deref() == Some("mouse-performance-polling") {
                    if let Some((g, _)) = performance_view::layout(self) {
                        self.performance_scroll = viewport::card_scroll_max(
                            g.polling_widget.y + g.polling_widget.height,
                            self.content_bottom_css(),
                        );
                    }
                }
            }
            self.prepare_smoke_overlay();
            self.rebuild_fields();
            InvalidateRect(self.hwnd, null(), 0);
        }
        if self.pending == 0 {
            if let Some(job) = self.pending_jobs.pop_front() {
                self.submit(job, false);
            } else {
                if !self.runtime_observer.pending() {
                    KillTimer(self.hwnd, RESPONSE_TIMER);
                }
                if self.closing && !self.failures.outstanding() {
                    PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
                }
            }
        }
    }

    fn text(&self, id: u16) -> String {
        self.raw_text(id).trim().to_string()
    }

    fn raw_text(&self, id: u16) -> String {
        let Some(&hwnd) = self.fields.get(&id) else {
            return String::new();
        };
        unsafe {
            let length = GetWindowTextLengthW(hwnd).clamp(0, 4096) as usize;
            let mut text = vec![0u16; length + 1];
            let read = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
            String::from_utf16_lossy(&text[..read.max(0) as usize])
        }
    }

    unsafe fn field(&mut self, id: u16, value: &str, x: i32, y: i32, width: i32) {
        self.field_defaults.insert(id, value.to_string());
        let shown = self
            .retained_fields
            .remove(&id)
            .unwrap_or_else(|| value.to_string());
        let hwnd = CreateWindowExW(
            0,
            wide("EDIT").as_ptr(),
            wide(&shown).as_ptr(),
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
        let owner = self.device().map(|device| {
            (
                device["id"].as_str().unwrap_or_default().to_string(),
                device["active"].as_str().unwrap_or_default().to_string(),
                self.tab.title().to_string(),
            )
        });
        let current: HashMap<_, _> = self
            .fields
            .keys()
            .map(|id| (*id, self.raw_text(*id)))
            .collect();
        self.retained_fields =
            dirty_fields(owner == self.field_owner, &current, &self.field_defaults);
        self.field_owner = owner;
        // The child owns dialog drafts; generic retained fields must not leak
        // an old color editor's text into a different custom slot.
        self.retained_fields
            .retain(|id, _| !(lighting_view::HEX..=lighting_view::BOOST).contains(id));
        self.field_defaults.clear();
        self.performance_input_text.clear();
        let focused = GetFocus();
        let focus = self
            .fields
            .iter()
            .find(|(_, hwnd)| **hwnd == focused)
            .map(|(id, hwnd)| {
                let mut start = 0u32;
                let mut end = 0u32;
                SendMessageW(
                    *hwnd,
                    EM_GETSEL,
                    (&mut start as *mut u32) as usize,
                    (&mut end as *mut u32) as isize,
                );
                (*id, start, end)
            });
        for (_, hwnd) in self.fields.drain() {
            DestroyWindow(hwnd);
        }
        if self.profile_modal.is_some() {
            self.focus = 0;
            return;
        }
        if linked_games::active(self) {linked_games::fields(self);return;}
        if macro_view::active(self) {macro_view::fields(self);return;}
        if self.studio.open {
            studio_view::rebuild_fields(self);
            return;
        }
        let Some(device) = self.device().cloned() else {
            return;
        };
        let center = ((self.width as f32 / self.scale) as i32 / 2).max(420);
        let tab = self.tab;
        match tab {
            Tab::Performance if self.performance.is_some() => {
                if let Some((geometry, indices)) = performance_view::layout(self) {
                    for (g, row) in geometry.stages.iter().zip(indices) {
                        let values = self.performance.as_ref().unwrap().stages[row].clone();
                        self.performance_field(
                            PERFORMANCE_INPUT + row as u16 * 2,
                            values.x,
                            g.x_input,
                        );
                        if let Some(rect) = g.y_input {
                            self.performance_field(
                                PERFORMANCE_INPUT + row as u16 * 2 + 1,
                                values.y,
                                rect,
                            );
                        }
                    }
                }
            }
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
            Tab::Lighting if lighting_view::rebuild_fields(self) => {}
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
            Tab::Power if device["sleep"].is_object() && power_view::layout(self).is_none() => {
                self.field(
                    MINUTES,
                    &device["sleep"]["minutes"].as_u64().unwrap_or(5).to_string(),
                    center - 310,
                    255,
                    110,
                );
            }
            Tab::Customize if assignment_view::rebuild_fields(self) => {}
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
        if let Some(name) = self.renaming.clone() {
            self.field(PROFILE_NAME, &name, 46, 90, 230);
            if let Some(hwnd) = self.fields.get(&PROFILE_NAME) {
                SendMessageW(*hwnd, EM_SETLIMITTEXT, 32, 0);
            }
        }
        if self.tab == Tab::Help && !self.notices.is_empty() {
            let text = self.notices.join("\r\n\r\n");
            let hwnd = CreateWindowExW(
                0,
                wide("EDIT").as_ptr(),
                wide(&text).as_ptr(),
                WS_VISIBLE
                    | WS_CHILD
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_READONLY as u32,
                self.px(center - 310),
                self.px(292),
                self.px(620),
                self.px(220),
                self.hwnd,
                COMPAT_NOTICES as usize as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            if !hwnd.is_null() {
                SendMessageW(hwnd, WM_SETFONT, self.fonts.normal as usize, 1);
                self.fields.insert(COMPAT_NOTICES, hwnd);
            }
        }
        if let Some((id, start, end)) = focus {
            if let Some(hwnd) = self.fields.get(&id) {
                SetFocus(*hwnd);
                SendMessageW(*hwnd, EM_SETSEL, start as usize, end as isize);
            }
        }
    }

    unsafe fn action(&mut self, action: Action) {
        if matches!(action, Action::Studio(studio_view::Control::StopDevice)) {
            studio_view::action(self, studio_view::Control::StopDevice);
            return;
        }
        if matches!(&action, Action::Device(id) if self.selected.as_ref() == Some(id)) {
            return;
        }
        let leaving_macro=matches!(&action,Action::Home|Action::Device(_)|Action::Refresh|Action::Close|Action::Studio(studio_view::Control::Enter))||matches!(&action,Action::HostClose(id) if id==macro_view::ID);
        if leaving_macro&&!macro_view::can_leave(self){return;}
        if self.restricted_actions
            && !matches!(&action, Action::Close | Action::Minimize | Action::Maximize)
        {
            return;
        }
        let stays_on_page = matches!(&action, Action::HostMove(..))
            || matches!(&action, Action::HostClose(id) if self.selected.as_ref() != Some(id));
        if !stays_on_page && !matches!(
            &action,
            Action::Assignment(_) | Action::Minimize | Action::Maximize | Action::HostScroll(_)
        ) && self.assignment.draft.is_some()
        {
            if self
                .assignment
                .request(assignment_model::Transition::External)
                .is_none()
            {
                self.assignment_next = Some(Box::new(action));
                self.rebuild_fields();
                InvalidateRect(self.hwnd, null(), 0);
                return;
            }
            self.assignment.cancel();
        }
        if assignment_view::covered(self)
            && matches!(
                &action,
                Action::ReadButtons | Action::Assign | Action::RestoreButtons
            )
        {
            return;
        }
        match action {
            Action::LinkedGames(control)=>linked_games::action(self,control),
            Action::MacroLibrary(control)=>macro_view::action(self,control),
            Action::Studio(control) => studio_view::action(self,control),
            Action::Calibration(control) => calibration_view::action(self,control),
            Action::Advanced(control) => advanced_view::action(self,control),
            Action::HostClose(id) => host_tabs::close(self, &id),
            Action::HostMove(id, target) => host_tabs::move_to(self, &id, target),
            Action::HostScroll(direction) => host_tabs::scroll(self, direction),
            Action::Assignment(control) => assignment_view::action(self, control),
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
                self.studio.canvas.cancel();ReleaseCapture();
                self.studio.open=false;
                host_tabs::remember(self);
                self.renaming = None;
                self.profile_actions = false;
                self.selected = None;
                self.profile_open = false;
                self.buttons.clear();
                self.sync_performance(true);
                self.rebuild_fields();
            }
            Action::Device(id) => {
                if (id==linked_games::ID||id==macro_view::ID) && self.studio.open && studio_view::block_close(self){return;}
                self.studio.canvas.cancel();ReleaseCapture();
                self.studio.open=false;
                if !host_tabs::open(self, &id) { return; }
                host_tabs::remember(self);
                let page = self.host_tabs.borrow().pages.get(&id).copied().unwrap_or(Tab::Customize);
                self.renaming = None;
                self.profile_actions = false;
                self.selected = Some(id);
                if linked_games::active(self){linked_games::request(self,linked_games::Request::Read);}
                if macro_view::active(self){macro_view::request(self,macro_view::Request::Read);}
                self.tab = page;
                self.profile_open = false;
                self.buttons.clear();
                self.sync_performance(true);
                self.rebuild_fields();
            }
            Action::Tab(tab) => {
                self.renaming = None;
                self.profile_actions = false;
                self.tab = tab;
                self.profile_open = false;
                self.performance_scroll = 0.0;
                self.performance_gesture = None;
                KillTimer(self.hwnd, PERFORMANCE_REPEAT);
                ReleaseCapture();
                self.sync_performance(true);
                self.rebuild_fields();
                calibration_view::request(self,true);
            }
            Action::Refresh if linked_games::active(self)=>linked_games::request(self,linked_games::Request::Read),
            Action::Refresh if macro_view::active(self)=>macro_view::request(self,macro_view::Request::Read),
            Action::Refresh => {mouse_feedback::stop(self);self.calibration.owner=None;self.queue(Job::Rescan);}
            Action::ProfileMenu => {
                self.profile_open = !self.profile_open;
                self.profile_offset = 0;
                self.profile_actions = false;
            }
            Action::ProfileActions => {
                self.profile_actions = !self.profile_actions;
                self.profile_open = false;
            }
            Action::Profile(name) => {
                self.profile_open = false;
                self.queue(Job::Action(
                    "/api/profile",
                    json!({"id":self.selected,"action":"use","name":name}),
                ));
            }
            Action::CreateProfile => {
                let base = std::env::var("COMPUTERNAME")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{s}-Default"))
                    .unwrap_or_else(|| "Default".into());
                self.create_profile(&base, false);
            }
            Action::DuplicateProfile => {
                let active = self
                    .device()
                    .and_then(|d| d["active"].as_str())
                    .unwrap_or("Default")
                    .to_string();
                self.create_profile(&active, true);
            }
            Action::RenameProfile => {
                self.profile_actions = false;
                self.renaming = self
                    .device()
                    .and_then(|d| d["active"].as_str())
                    .map(str::to_string);
                self.rebuild_fields();
                if let Some(hwnd) = self.fields.get(&PROFILE_NAME) {
                    SetFocus(*hwnd);
                    SendMessageW(*hwnd, EM_SETSEL, 0, -1);
                }
            }
            Action::ProfileModal(control) => self.profile_modal_action(control),
            Action::ImportProfile | Action::ExportProfile => {
                self.profile_actions = false;
                if self
                    .device()
                    .is_some_and(|d| matches!(d["pid"].as_str(), Some("025D" | "00C4" | "00C5")))
                {
                    self.open_profile_modal(if matches!(action, Action::ImportProfile) {
                        profile_modal_model::Mode::Import
                    } else {
                        profile_modal_model::Mode::Export
                    });
                } else {
                    PostMessageW(
                        self.hwnd,
                        PROFILE_DIALOG,
                        usize::from(matches!(action, Action::ExportProfile)),
                        0,
                    );
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
            Action::PerfEnabled => {
                if let Some(p) = &mut self.performance {
                    p.enabled = !p.enabled;
                }
                self.commit_performance();
                self.rebuild_fields();
            }
            Action::PerfSelect(row) => {
                if let Some(p) = &mut self.performance {
                    if p.active != row as u8 + 1 && p.stages.get(row).is_some_and(|r| r.visible) {
                        p.active = row as u8 + 1;
                        self.commit_performance();
                    }
                }
            }
            Action::PerfIndependent(row) => {
                let result = self.performance.as_mut().and_then(|p| {
                    let next = !p.stages.get(row)?.independent;
                    Some(p.set_independent(row, next))
                });
                match result {
                    Some(Ok(())) => {
                        self.commit_performance();
                        self.rebuild_fields();
                    }
                    Some(Err(error)) => self.error(&error.to_string()),
                    None => self.error("The stage changed; refresh before editing"),
                }
            }
            Action::PerfVisible(row) => {
                let result = self.performance.as_mut().and_then(|p| {
                    let next = !p.stages.get(row)?.visible;
                    Some(p.set_visible(row, next))
                });
                match result {
                    Some(Ok(())) => {
                        self.commit_performance();
                        self.rebuild_fields();
                    }
                    Some(Err(error)) => self.error(&error.to_string()),
                    None => self.error("The stage changed; refresh before editing"),
                }
            }
            Action::PerfSlider(row, axis, rect) => {
                self.performance_gesture = Some(PerformanceGesture::Slider { row, axis, rect });
                SetCapture(self.hwnd);
            }
            Action::PerfDrag(row) => {
                self.performance_gesture = Some(PerformanceGesture::Reorder { row, target: row });
                SetCapture(self.hwnd);
            }
            Action::PerfStep(row, axis, delta) => {
                self.performance_gesture = Some(PerformanceGesture::Spinner { row, axis, delta });
                SetCapture(self.hwnd);
                self.step_performance(row, axis, delta);
                SetTimer(self.hwnd, PERFORMANCE_REPEAT, 300, None);
            }
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
            Action::PowerSlider(..) => {}
            Action::Lighting(control) => {
                let update = lighting_view::action(self, control);
                self.lighting_update(update);
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
        for (id, control) in &self.fields {
            if *id != PROFILE_NAME {
                ShowWindow(
                    *control,
                    if self.profile_open || self.profile_actions {
                        SW_HIDE
                    } else {
                        SW_SHOW
                    },
                );
            }
        }
        mouse_feedback::reconcile(self);
        InvalidateRect(self.hwnd, null(), 0);
    }

    fn zone(&self) -> &str {
        self.device()
            .and_then(|d| d["leds"].as_array())
            .and_then(|v| v.first())
            .and_then(|l| l["name"].as_str())
            .unwrap_or("backlight")
    }

    unsafe fn open_profile_modal(&mut self, mode: profile_modal_model::Mode) {
        let Some(d) = self.device() else {
            self.error("Select a device first");
            return;
        };
        let owner = profile_modal_model::Owner {
            device: d["id"].as_str().unwrap_or_default().into(),
            profile: d["active"].as_str().unwrap_or("Default").into(),
        };
        let name = d["name"].as_str().unwrap_or("Razer device").to_string();
        let view = profile_modal_model::View::new(owner.clone(), mode, name);
        let token = view.token;
        let update = lighting_view::cancel(self);
        self.lighting_update(update);
        self.performance_gesture = None;
        self.power_gesture = None;
        KillTimer(self.hwnd, PERFORMANCE_REPEAT);
        ReleaseCapture();
        self.profile_modal = Some(view);
        self.profile_open = false;
        self.profile_actions = false;
        self.renaming = None;
        self.focus = 0;
        self.rebuild_fields();
        if mode == profile_modal_model::Mode::Export {
            self.queue(Job::Profiles(profile_modal_model::Request::PreviewExport {
                token,
                owner,
            }));
        }
    }
    unsafe fn profile_preview_path(
        &mut self,
        token: u64,
        owner: profile_modal_model::Owner,
        path: PathBuf,
    ) {
        if let Some(view) = self
            .profile_modal
            .as_mut()
            .filter(|v| v.token == token && v.owner == owner)
        {
            view.filename = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            view.loading = true;
            view.error.clear();
            view.data = None;
            view.choices.clear();
            self.queue(Job::Profiles(profile_modal_model::Request::PreviewImport {
                token,
                owner,
                path,
            }));
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    unsafe fn profile_modal_action(&mut self, control: profile_modal_model::Control) {
        use profile_modal_model::{Control, Mode, Request};
        match control {
            Control::Close => {
                self.profile_modal = None;
                ReleaseCapture();
                self.rebuild_fields();
            }
            Control::Browse => {
                PostMessageW(self.hwnd, PROFILE_MODAL_BROWSE, 0, 0);
            }
            Control::Commit => {
                let Some(view) = self.profile_modal.take() else {
                    return;
                };
                if !view.enabled() {
                    self.profile_modal = Some(view);
                    return;
                }
                let Some(data) = view.data else { return };
                if view.mode == Mode::Import {
                    self.queue(Job::Profiles(Request::Import {
                        owner: view.owner,
                        data,
                        choices: view.choices,
                    }));
                } else {
                    self.profile_export = Some(profile_modal_model::ExportPlan {
                        owner: view.owner,
                        data,
                        choices: view.choices,
                    });
                    #[cfg(not(test))]
                    PostMessageW(self.hwnd, PROFILE_MODAL_SAVE, 0, 0);
                    #[cfg(test)]
                    if !self.smoke_requested.is_some() {
                        PostMessageW(self.hwnd, PROFILE_MODAL_SAVE, 0, 0);
                    }
                }
                self.rebuild_fields();
            }
            other => {
                if let Some(view) = &mut self.profile_modal {
                    view.toggle(&other);
                }
            }
        }
        InvalidateRect(self.hwnd, null(), 0);
    }
    fn error(&mut self, message: &str) {
        self.status = message.into();
        self.failed = true;
        if self.lighting_probe {
            return;
        }
        if self.failures.record(message) {
            unsafe {
                PostMessageW(self.hwnd, FAILURE_DIALOG, 0, 0);
            }
        }
    }
}

struct Painter<'a> {
    dc: HDC,
    scale: f32,
    fonts: &'a Fonts,
    hits: Vec<Hit>,
}

impl Painter<'_> {
    unsafe fn source_text(
        &self,
        text: &str,
        role: source_text::Role,
        source: source_text::SourceBox,
        bounds: RECT,
        color: u32,
    ) -> bool {
        if text.is_empty() || bounds.right <= bounds.left || bounds.bottom <= bounds.top {
            return false;
        }
        self.fonts.source_text.draw(
            self.dc,
            text,
            role,
            source_text::Clip {
                x: bounds.left,
                y: bounds.top,
                width: (bounds.right - bounds.left) as u32,
                height: (bounds.bottom - bounds.top) as u32,
            },
            source,
            self.scale,
            rgb(color),
        )
    }
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
        if image.alpha {
            image_render::draw(
                self.dc,
                image.smooth,
                self.rect(x + (width - w) / 2, y + (height - h) / 2, w, h),
                image.width,
                image.height,
            );
            return;
        }
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
    host_tabs::paint(state, &mut p, width);
    for (index, (label, action)) in [
        ("—", Action::Minimize),
        ("□", Action::Maximize),
        ("×", Action::Close),
    ]
    .into_iter()
    .enumerate()
    {
        let x = width - 144 + index as i32 * 48;
        let icon = match action {
            Action::Minimize => "minimize",
            Action::Maximize => {
                if IsZoomed(state.hwnd) != 0 {
                    "restore"
                } else {
                    "maximize"
                }
            }
            _ => "close",
        };
        if let Some(image) = state.icons.get(icon) {
            // Minimize/maximize SVGs include their 48x32 transparent cell;
            // close/restore contain only the roughly 13px glyph itself.
            if matches!(icon, "close" | "restore") {
                p.image(Some(image), x + 17, 15, 13, 13);
            } else {
                p.image(Some(image), x, 5, 48, 32);
            }
        } else {
            p.text(label, x, 0, 48, 42, p.fonts.product, MUTED, DT_CENTER);
        }
        p.hits.push(Hit {
            rect: p.rect(x, 0, 48, 42),
            action,
        });
    }
    p.image(state.icons.get("back"), 10, 51, 20, 20);
    p.hits.push(Hit {
        rect: p.rect(0, 42, 40, 38),
        action: Action::Home,
    });
    p.image(state.icons.get("refresh"), 50, 51, 20, 20);
    p.hits.push(Hit {
        rect: p.rect(40, 42, 40, 38),
        action: Action::Refresh,
    });
    let device = state.device().cloned();
    let title = if macro_view::active(state) {"MACRO"} else if linked_games::active(state) {"LINKED GAMES"} else if state.studio.open { "RAZER CHROMA STUDIO" } else { device
        .as_ref()
        .and_then(|d| d["name"].as_str())
        .unwrap_or("RAZER SYNAPSE") };
    let title = title.to_uppercase();
    if !p.source_text(
        &title,
        source_text::Role::ToolbarTitle,
        source_text::SourceBox {
            left: (width / 4) as f32,
            top: 42.0,
            width: (width / 2) as f32,
        },
        p.rect(width / 4, 42, width / 2, 38),
        MUTED,
    ) {
        p.text(
            &title,
            width / 4,
            42,
            width / 2,
            38,
            p.fonts.normal,
            MUTED,
            DT_CENTER,
        );
    }
    p.line(0, 79, width, 0);
    if macro_view::active(state) {macro_view::paint(state,&mut p,width,height);
    } else if linked_games::active(state) {linked_games::paint(state,&mut p,width,height);
    } else if state.studio.open {
        studio_view::paint(state,&mut p,width,height);
    } else if let Some(device) = device {
        let center = (width / 2).max(420);
        let active = device["active"].as_str().unwrap_or("Default");
        if state.renaming.is_none() {
            p.line(
                46,
                90,
                230,
                if state.profile_open { GREEN } else { 0x515151 },
            );
            p.line(
                46,
                116,
                230,
                if state.profile_open { GREEN } else { 0x515151 },
            );
            p.fill(
                46,
                90,
                1,
                27,
                if state.profile_open { GREEN } else { 0x515151 },
            );
            p.fill(
                275,
                90,
                1,
                27,
                if state.profile_open { GREEN } else { 0x515151 },
            );
            p.text(active, 51, 90, 199, 27, p.fonts.normal, TEXT, DT_LEFT);
            p.image(state.icons.get("expand"), 259, 98, 10, 10);
            p.hits.push(Hit {
                rect: p.rect(46, 90, 230, 27),
                action: Action::ProfileMenu,
            });
        }
        p.image(state.icons.get("profile-default"), 10, 90, 26, 26);
        p.image(state.icons.get("more"), 289, 93, 20, 20);
        p.hits.push(Hit {
            rect: p.rect(286, 90, 26, 26),
            action: Action::ProfileActions,
        });
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
            Tab::Customize if assignment_view::paint_customize(state, &mut p) => {}
            Tab::Customize => {
                let keyboard = device["kind"].as_str() == Some("keyboard");
                let exact_keyboard = keyboard
                    && device["pid"] == "025D"
                    && state.keyboard_layout.is_some()
                    && state.keyboard_customize.is_some();
                let image = if exact_keyboard {
                    state.keyboard_customize.as_ref()
                } else if device["pid"] == "025D" {
                    state.keyboard.as_ref()
                } else if matches!(device["pid"].as_str(), Some("00C4" | "00C5")) {
                    state.mouse.as_ref()
                } else {
                    None
                };
                let image_width = if exact_keyboard {
                    730
                } else if keyboard {
                    690
                } else {
                    300
                };
                p.image(
                    image,
                    center - image_width / 2,
                    if exact_keyboard { 154 } else { 176 },
                    image_width,
                    if exact_keyboard {
                        340
                    } else if keyboard {
                        387
                    } else {
                        340
                    },
                );
                if exact_keyboard {
                    if let Some(layout) = &state.keyboard_layout {
                        let viewport = source_layout::Viewport {
                            left: f64::from(p.px(center - 365)),
                            top: f64::from(p.px(154)),
                            width: f64::from(p.px(730)),
                            height: f64::from(p.px(387)),
                        };
                        if let Some(key) = layout.keys.iter().find(|key| {
                            key.firmware_id
                                .and_then(|id| u8::try_from(id).ok())
                                .is_some_and(|id| {
                                    secbird_core::buttons::Input::Key(id).name()
                                        == state.selected_button
                                })
                        }) {
                            if layout.bounds(&key.input_id, viewport).is_some()
                                && layout.trace_key_path(dc, &key.input_id, viewport)
                            {
                                let pen = CreatePen(PS_SOLID, p.px(1), rgb(GREEN));
                                let previous = SelectObject(dc, pen);
                                StrokePath(dc);
                                SelectObject(dc, previous);
                                DeleteObject(pen);
                            }
                        }
                    }
                }
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
                let saved = SaveDC(p.dc);
                IntersectClipRect(p.dc, 0, p.px(128), state.width, state.content_bottom_px());
                let previous_hits = p.hits.len();
                let source_view = performance_view::paint(state, &mut p, &device);
                for hit in p.hits.iter_mut().skip(previous_hits) {
                    hit.rect.top = hit.rect.top.max((128.0 * state.scale).round() as i32);
                    hit.rect.bottom = hit.rect.bottom.min(state.content_bottom_px());
                }
                let mut index = 0usize;
                p.hits.retain(|hit| {
                    let keep = index < previous_hits || hit.rect.bottom > hit.rect.top;
                    index += 1;
                    keep
                });
                RestoreDC(p.dc, saved);
                if !source_view {
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
            }
            Tab::Lighting => {
                let saved = SaveDC(p.dc);
                IntersectClipRect(p.dc, 0, p.px(128), state.width, state.content_bottom_px());
                let source = lighting_view::paint(state, &mut p, &device);
                RestoreDC(p.dc, saved);
                if !source {
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
            }
            Tab::Power => {
                if !power_view::paint(state, &mut p, &device) {
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
            }
            Tab::Calibration => calibration_view::paint(state,&mut p),
            Tab::Advanced => advanced_view::paint(state,&mut p),
            Tab::Help => {}
        }
        if matches!(state.tab, Tab::Help) {
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
            let profiles = strings(&device["profiles"]);
            let visible = profiles.len().saturating_sub(state.profile_offset).min(7);
            p.fill(46, 117, 230, visible as i32 * 25 + 2, 0);
            p.line(46, 117, 230, BORDER);
            for (index, name) in profiles
                .iter()
                .skip(state.profile_offset)
                .take(visible)
                .enumerate()
            {
                let y = 118 + index as i32 * 25;
                p.text(
                    name,
                    51,
                    y,
                    220,
                    25,
                    p.fonts.normal,
                    if name == active { GREEN } else { TEXT },
                    DT_LEFT,
                );
                p.hits.push(Hit {
                    rect: p.rect(47, y, 228, 25),
                    action: Action::Profile(name.clone()),
                });
            }
            p.line(46, 118 + visible as i32 * 25, 230, BORDER);
        }
        if state.profile_actions {
            let rows = [
                ("Add", Action::CreateProfile),
                ("Import", Action::ImportProfile),
                ("Rename", Action::RenameProfile),
                ("Duplicate", Action::DuplicateProfile),
                ("Export", Action::ExportProfile),
            ];
            p.fill(285, 116, 157, 145, 0);
            p.line(285, 116, 157, BORDER);
            p.line(285, 260, 157, BORDER);
            p.fill(285, 116, 1, 145, BORDER);
            p.fill(441, 116, 1, 145, BORDER);
            let mut y = 117;
            for (index, (label, action)) in rows.into_iter().enumerate() {
                if index == 2 {
                    p.line(286, y + 4, 155, BORDER);
                    y += 9;
                }
                p.text(label, 292, y, 143, 27, p.fonts.normal, TEXT, DT_LEFT);
                p.hits.push(Hit {
                    rect: p.rect(286, y, 155, 27),
                    action,
                });
                y += 27;
            }
        }
    } else {
        p.text("DEVICES", 30, 104, 300, 32, p.fonts.heading, GREEN, DT_LEFT);
        p.button("LINKED GAMES",width-440,104,195,Action::LinkedGames(linked_games::Control::Open),false);
        p.button("MACRO",width-640,104,180,Action::MacroLibrary(macro_view::Control::Open),false);
        p.button("CHROMA STUDIO",width-225,104,195,Action::Studio(studio_view::Control::Enter),false);
        for (index, device) in state.devices().iter().enumerate() {
            let x = 30 + (index as i32 % 4) * 285;
            let y = 158 + (index as i32 / 4) * 280;
            p.fill(x, y, 260, 250, PANEL);
            // Only visible cards demand image residency. Off-screen devices
            // remain in the catalogue without loading their photographs.
            if x < width && y < height && y + 250 > 128 {
                if !state.device_artwork.borrow_mut().draw(&p, device, x + 15, y + 20, 230, 160) {
                    let default_edition = device.get("edition_id")
                        .is_none_or(|id|id.is_null() || id.as_u64() == Some(0));
                    let image = if default_edition { match device["pid"].as_str() {
                            Some("025D") => state.keyboard.as_ref(),
                            Some("00C4" | "00C5") => state.mouse.as_ref(),
                            _ => None,
                        } } else { None };
                    p.image(image, x + 15, y + 20, 230, 160);
                }
            }
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
    if !state.source_viewport() && !linked_games::active(state) && !macro_view::active(state) {
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
    }
    assignment_view::paint_popup(state, &mut p);
    profile_modal_view::paint(state, &mut p);
    if p.fonts.source_text.diagnostic().is_some() {
        p.fill(0, height - 24, width, 24, BG);
        p.text(
            "Source typography unavailable; using fallback text",
            20,
            height - 24,
            width - 40,
            24,
            p.fonts.tiny,
            MUTED,
            DT_LEFT,
        );
    }
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
        // The entire window is our client surface. DefWindowProc otherwise
        // paints a light sizing frame when this custom frame loses focus.
        WM_NCPAINT => return 0,
        WM_NCACTIVATE => return DefWindowProcW(hwnd, message, wparam, -1),
        WM_NCCALCSIZE => {
            window_frame::calculate(hwnd, wparam, lparam);
            return 0;
        }
        WM_GETMINMAXINFO => {
            if lparam != 0 {
                window_frame::limits(hwnd, &mut *(lparam as *mut MINMAXINFO));
            }
            return 0;
        }
        WM_NCHITTEST => {
            if let Some(hit) = window_frame::border_hit(hwnd, lparam) {
                return hit;
            }
            let mut point = POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
            if !pointer.is_null() {
                if let Ok(state) = (*pointer).try_borrow() {
                    let over_hit = state.hits.iter().any(|hit| PtInRect(&hit.rect, point) != 0);
                    let host_height =
                        (42.0 * GetDpiForWindow(hwnd).max(96) as f64 / 96.0).round() as i32;
                    if point.y >= 0 && point.y < host_height && !over_hit {
                        return HTCAPTION as LRESULT;
                    }
                }
            }
            return HTCLIENT as LRESULT;
        }
        WM_WINDOWPOSCHANGED => {
            // DefWindowProc synchronously delivers WM_SIZE/WM_MOVE. Forward
            // before borrowing State so the new layout is actually retained.
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        WM_CLOSE => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
            if !pointer.is_null() {
                if let Ok(mut state) = (*pointer).try_borrow_mut() {
                    if !macro_view::can_leave(&mut state){return 0;}
                    if studio_view::block_close(&mut state) {return 0;}
                    mouse_feedback::stop(&mut state);
                    if state.assignment.draft.as_ref().is_some_and(|d| d.dirty) {
                        state.assignment.pending = Some(assignment_model::Transition::External);
                        state.assignment_next = Some(Box::new(Action::Close));
                        state.rebuild_fields();
                        InvalidateRect(hwnd, null(), 0);
                        return 0;
                    }
                    let focused = GetFocus();
                    let input = state
                        .fields
                        .iter()
                        .find(|(_, control)| **control == focused)
                        .map(|(id, _)| *id);
                    if let Some(id) = input.filter(|id| {
                        !state.closing && (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10).contains(id)
                    }) {
                        state.commit_performance_input(id);
                    }
                    if state.failures.outstanding() {
                        state.closing = true;
                        PostMessageW(hwnd, FAILURE_DIALOG, 0, 0);
                        return 0;
                    }
                    if state.pending > 0 || !state.pending_jobs.is_empty() {
                        state.closing = true;
                        ShowWindow(hwnd, SW_HIDE);
                        SetTimer(hwnd, RESPONSE_TIMER, 40, None);
                        return 0;
                    }
                }
            }
            DestroyWindow(hwnd);
            return 0;
        }
        WM_DESTROY => {
            WINDOW.store(0, Ordering::Release);
            let pointer=GetWindowLongPtrW(hwnd,GWLP_USERDATA) as *mut RefCell<State>;
            if !pointer.is_null(){if let Ok(mut state)=(*pointer).try_borrow_mut(){
                mouse_feedback::stop(&mut state);
                if let Some(tray)=state.exit_tray.take(){PostMessageW(tray,WM_CLOSE,1,hwnd as isize);}
            }}
            KillTimer(hwnd, RESPONSE_TIMER);
            KillTimer(hwnd, SMOKE_TIMER);
            KillTimer(hwnd, CACHED_LIGHTING_TIMER);
            KillTimer(hwnd, studio_view::PREVIEW_TIMER);
            KillTimer(hwnd, CACHED_FEEDBACK_RETRY);
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
    if message==studio_view::DIALOG {studio_view::dialog(hwnd,pointer,wparam);return 0;}
    #[cfg(test)]
    if message==STUDIO_TEST {
        if let Ok(mut state)=(*pointer).try_borrow_mut() {
            match wparam {
                0=>state.action(Action::Studio(studio_view::Control::Enter)),
                1=>{let value=&*(lparam as *const Value);studio_view::read_path(&mut state,PathBuf::from(value.as_str().unwrap()));},
                2=>{*(lparam as *mut Value)=studio_view::diagnostics(&state);},
                3=>{let value=&*(lparam as *const Value);studio_view::save_path(&mut state,PathBuf::from(value.as_str().unwrap()));},
                4=>state.studio.test_decision=Some(lparam as i32),
                5=>studio_view::reset_test(&mut state),
                6=>{let value=&*(lparam as *const Value);return studio_view::capture_frame(&mut state,Path::new(value.as_str().unwrap())) as isize;},
                7=>studio_view::output_test_dirty_apply(&mut state),
                8=>{let value=&*(lparam as *const Value);studio_view::add_preset_test(&mut state,(value[0].as_u64().unwrap() as u16,value[1].as_u64().unwrap() as u16,value[2].as_u64().unwrap() as u16));},
                9=>{EndMenu();},
                _=>{},
            }
        }
        return 0;
    }
    #[cfg(test)]
    if message==macro_view::TEST {
        if let Ok(mut state)=(*pointer).try_borrow_mut(){match wparam {
            1=>state.action(Action::MacroLibrary(macro_view::Control::Open)),
            2=>*(lparam as *mut Value)=macro_view::diagnostics(&state),
            3=>state.action(Action::Home),
            4=>{let value=&*(lparam as *const Value);if macro_view::can_leave(&mut state){macro_view::request(&mut state,macro_view::Request::Import(PathBuf::from(value.as_str().unwrap())));}},
            5=>state.action(Action::MacroLibrary(macro_view::Control::Discard)),
            6=>{
                use secbird_core::profile_compat::macro_library::input_events::{Input,Gesture};
                let value=&*(lparam as *const Value);if let Some(target)=value["macro"].as_str(){state.action(Action::MacroLibrary(macro_view::Control::ChooseNested(target.into())));}else{let input=if let Some(key)=value["key"].as_str(){Input::Keyboard(key.into())}else{Input::Mouse(value["mouse"].as_u64().unwrap() as u8)};state.action(Action::MacroLibrary(macro_view::Control::ChooseInput(input)));}
                let gesture=match value["gesture"].as_str(){Some("down")=>Gesture::Down,Some("up")=>Gesture::Up,_=>Gesture::Tap};state.action(Action::MacroLibrary(macro_view::Control::ChooseGesture(gesture)));
            },
            7=>{let value=&*(lparam as *const Value);state.action(Action::MacroLibrary(macro_view::Control::Select(value.as_str().unwrap().into())));},
            8=>{*(lparam as *mut Value)=match macro_view::menu_targets(&state){Ok(targets)=>json!(targets.into_iter().map(|target|json!({"guid":target.guid,"name":target.name,"disabled":target.reason.is_some()})).collect::<Vec<_>>()),Err(error)=>json!({"error":error})};},
            _=>{},
        }}return 0;
    }
    #[cfg(test)]
    if message==linked_games::TEST {
        if let Ok(mut state)=(*pointer).try_borrow_mut(){match wparam {
            1=>state.action(Action::LinkedGames(linked_games::Control::Open)),
            2=>{let value=&*(lparam as *const Value);if let Some(path)=value.as_str(){linked_games::request(&mut state,linked_games::Request::Add(path.into()));}},
            3=>*(lparam as *mut Value)=linked_games::diagnostics(&state),
            4=>*(lparam as *mut Value)=host_tabs::diagnostics(&state),
            _=>{},
        }}return 0;
    }
    if message==linked_games::BROWSE {
        let allowed=(*pointer).try_borrow().is_ok_and(|state|linked_games::active(&state)&&!state.read_only_smoke&&!state.restricted_actions&&!state.linked_games.busy);
        if allowed {let result=linked_games::browse(hwnd);if let Ok(mut state)=(*pointer).try_borrow_mut(){match result {Ok(Some(path))=>linked_games::request(&mut state,linked_games::Request::Add(path)),Ok(None)=>{},Err(error)=>state.error(&error)}InvalidateRect(hwnd,null(),0);}}
        return 0;
    }
    if message==macro_view::BROWSE {
        let allowed=(*pointer).try_borrow().is_ok_and(|state|macro_view::active(&state)&&!state.read_only_smoke&&!state.restricted_actions&&!state.macros.busy);
        if allowed {let result=macro_view::browse(hwnd);if let Ok(mut state)=(*pointer).try_borrow_mut(){if macro_view::can_leave(&mut state){match result {Ok(Some(path))=>macro_view::request(&mut state,macro_view::Request::Import(path)),Ok(None)=>{},Err(error)=>state.error(&error)}}InvalidateRect(hwnd,null(),0);}}
        return 0;
    }
    if message==macro_view::INPUT_MENU {
        let allowed=(*pointer).try_borrow().is_ok_and(|state|macro_view::active(&state)&&!state.read_only_smoke&&!state.restricted_actions&&!state.macros.busy);
        if allowed {let targets=(*pointer).try_borrow().map_err(|_|"Macro editor is busy".to_string()).and_then(|state|if wparam==0{macro_view::menu_targets(&state)}else{Ok(vec![])});let result=targets.and_then(|targets|{if let Ok(mut state)=(*pointer).try_borrow_mut(){state.macros.popup=true;}macro_view::choose_input(hwnd,wparam!=0,&targets)});if let Ok(mut state)=(*pointer).try_borrow_mut(){state.macros.popup=false;match result {Ok(Some(control))=>state.action(Action::MacroLibrary(control)),Ok(None)=>{},Err(error)=>state.error(&error)}}}
        return 0;
    }
    if message==studio_view::EXIT_REQUEST {
        let tray=lparam as HWND;
        let mut process=0;
        GetWindowThreadProcessId(tray,&mut process);
        if process==std::process::id() {
            if let Ok(mut state)=(*pointer).try_borrow_mut(){state.exit_tray=Some(tray);}
            ShowWindow(hwnd,SW_SHOW);SetForegroundWindow(hwnd);PostMessageW(hwnd,WM_CLOSE,0,0);
        }
        return 0;
    }
    if message==mouse_feedback::NOTIFY {
        if let Ok(mut state)=(*pointer).try_borrow_mut(){mouse_feedback::notification(&mut state);}
        else{PostMessageW(hwnd,mouse_feedback::NOTIFY,0,0);}
        return 0;
    }
    if message == FAILURE_DIALOG {
        let message = match (*pointer).try_borrow_mut() {
            Ok(mut state) => {
                if wparam == 1 && !state.closing && state.runtime_feedback_busy() {
                    state.runtime_feedback_waiting = true;
                    return 0;
                }
                state.failures.begin()
            }
            Err(_) => {
                // A different native modal loop may still hold State. Retry at a
                // bounded cadence; never flood that loop with immediate reposts.
                SetTimer(
                    hwnd,
                    if wparam == 1 {
                        CACHED_FEEDBACK_RETRY
                    } else {
                        FAILURE_RETRY
                    },
                    50,
                    None,
                );
                return 0;
            }
        };
        if let Some(message) = message {
            // Native acknowledged error UI is a port adaptation, not a recovered stock toast.
            MessageBoxW(
                hwnd,
                wide(&message).as_ptr(),
                wide(FAILURE_TITLE).as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            // The modal loop can close the owner. Re-read userdata rather than reuse its pointer.
            let current = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
            if !current.is_null() {
                if let Ok(mut state) = (*current).try_borrow_mut() {
                    state.failures.acknowledge();
                    if state.failures.outstanding() {
                        PostMessageW(hwnd, FAILURE_DIALOG, 0, 0);
                    } else if state.closing && state.pending == 0 && state.pending_jobs.is_empty() {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
            }
        }
        return 0;
    }
    if message == PROFILE_MODAL_BROWSE {
        let pending = (*pointer).try_borrow().ok().and_then(|state| {
            state
                .profile_modal
                .as_ref()
                .map(|view| (view.token, view.owner.clone()))
        });
        if let Some((token, owner)) = pending {
            let result = profile_modal_files::browse(hwnd);
            if let Ok(mut state) = (*pointer).try_borrow_mut() {
                match result {
                    Ok(Some(path)) => state.profile_preview_path(token, owner, path),
                    Ok(None) => {}
                    Err(error) => state.error(&error),
                }
            }
        }
        return 0;
    }
    if message == PROFILE_MODAL_SAVE {
        let plan = (*pointer)
            .try_borrow_mut()
            .ok()
            .and_then(|mut state| state.profile_export.take());
        if let Some(plan) = plan {
            let result = profile_modal_files::save(hwnd);
            if let Ok(mut state) = (*pointer).try_borrow_mut() {
                match result {
                    Ok(Some((path, format))) => {
                        state.queue(Job::Profiles(profile_modal_model::Request::Export {
                            plan,
                            path,
                            format,
                        }))
                    }
                    Ok(None) => {}
                    Err(error) => state.error(&error),
                }
            }
        }
        return 0;
    }
    #[cfg(test)]
    if message == PROFILE_MODAL_TEST_BROWSE {
        let path = (*(lparam as *const PathBuf)).clone();
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            if let Some(v) = &state.profile_modal {
                let (token, owner) = (v.token, v.owner.clone());
                state.profile_preview_path(token, owner, path);
            }
        }
        return 0;
    }
    #[cfg(test)]
    if message == PROFILE_MODAL_TEST_SAVE {
        let path = (*(lparam as *const PathBuf)).clone();
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            if let Some(plan) = state.profile_export.take() {
                let format = if wparam == 2 {
                    profile_files::Format::Native
                } else {
                    profile_files::Format::Synapse4
                };
                state.queue(Job::Profiles(profile_modal_model::Request::Export {
                    plan,
                    path,
                    format,
                }));
            }
        }
        return 0;
    }
    #[cfg(test)]
    if message == PROFILE_MODAL_TEST_OPEN {
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            state.open_profile_modal(if wparam == 1 {
                profile_modal_model::Mode::Export
            } else {
                profile_modal_model::Mode::Import
            });
        }
        return 0;
    }
    #[cfg(test)]
    if message == PROFILE_MODAL_TEST_QUERY {
        if let Ok(state) = (*pointer).try_borrow() {
            let value = if let Some(v) = &state.profile_modal {
                json!({"open":true,"loading":v.loading,"enabled":v.enabled(),"native":v.native(),"cloud":v.cloud,"warning":v.warning,"error":v.error,"scroll":v.scroll,"mode":if v.mode==profile_modal_model::Mode::Import{"import"}else{"export"},"rows":v.data.as_ref().map(|d|d.rows.iter().zip(&v.choices).map(|(r,c)|json!({"name":r.name,"device":r.device_key,"selected":c.selected,"macros":r.macros.iter().map(|m|json!({"guid":m.guid,"selected":c.macros.contains(&m.guid),"mark":v.macro_marks.get(d.rows.iter().position(|other|std::ptr::eq(other,r)).unwrap()).is_some_and(|marks|marks.contains(&m.guid))})).collect::<Vec<_>>()})).collect::<Vec<_>>()).unwrap_or_default()})
            } else {
                json!({"open":false,"export_plan":state.profile_export.is_some()})
            };
            *(lparam as *mut Value) = value;
        }
        return 0;
    }
    #[cfg(test)]
    if message == ASSIGNMENT_TEST_QUERY {
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            if wparam == 2 {
                *(lparam as *mut Value) = assignment_view::font_diagnostics(&state);
                return 0;
            }
            if wparam == 3 {
                *(lparam as *mut Value) = json!({"width":state.width,"height":state.height,"scale":state.scale,"hit_count":state.hits.len()});
                return 0;
            }
            if wparam == 1 {
                state.assignment.cancel();
                state.assignment_next = None;
                state.rebuild_fields();
            }
            let draft = state.assignment.draft.as_ref();
            let g = assignment_view::shell(&state);
            *(lparam as *mut Value) = json!({"view":assignment_view::diagnostics(&state),"open":draft.is_some(),"input":draft.map(|d|&d.input.input_id),"category":draft.and_then(|d|d.category).map(|c|c.key()),"dirty":draft.is_some_and(|d|d.dirty),"prompt":state.assignment.pending.is_some(),"speed":draft.map(|d|d.speed),"source_original":draft.map(|d|&d.context.original_profile),"popup":g.map(|g|json!({"x":g.popup.x,"y":g.popup.y,"height":g.popup.height})),"pending":state.pending});
        }
        return 0;
    }
    #[cfg(test)]
    if message == READ_ONLY_TEST_SET {
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            state.read_only_smoke = wparam != 0;
            state.restricted_actions = state.read_only_smoke || state.lighting_probe;
        }
        return 0;
    }
    #[cfg(test)]
    if message == RUNTIME_TEST_INJECT {
        if wparam == 2 {
            SetFocus(GetDlgItem(hwnd, i32::from(lighting_view::HEX)));
        }
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            if let Some(binding) = state.runtime_binding() {
                if wparam == 1 {
                    state.failures.new_operation();
                }
                let value = (*(lparam as *const Value)).clone();
                let response = state.runtime_observer.test_response(binding, value);
                state.collect_cached_lighting(response);
            }
        }
        return 0;
    }
    #[cfg(test)]
    if message == LIGHTING_DROPDOWN_TEST {
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            *(lparam as *mut Value) = lighting_view::test_dropdown(&mut state, wparam == 1);
        }
        return 0;
    }
    #[cfg(test)]
    if message == RUNTIME_TEST_QUERY {
        if wparam == 8 {
            // Atomically hover and click the actual close hit target. Keep the
            // user's cursor outside this simulated test window untouched.
            let tab = (*pointer).borrow().hits.iter().find(|hit|
                hit.rect.top > 0 && hit.rect.top < 60 &&
                matches!(&hit.action,Action::Device(id) if Some(id)==(*pointer).borrow().selected.as_ref()))
                .map(|hit|hit.rect);
            if let Some(tab) = tab {
                let point = (((tab.left+tab.right)/2) as u32 | (((tab.top+tab.bottom)/2) as u32)<<16) as isize;
                SendMessageW(hwnd,WM_MOUSEMOVE,0,point);
                RedrawWindow(hwnd,null(),null_mut(),RDW_INVALIDATE|RDW_UPDATENOW);
                let close = (*pointer).borrow().hits.iter().find(|hit|matches!(hit.action,Action::HostClose(_))).map(|hit|hit.rect);
                if let Some(close) = close {
                    let point = (((close.left+close.right)/2) as u32 | (((close.top+close.bottom)/2) as u32)<<16) as isize;
                    SendMessageW(hwnd,WM_MOUSEMOVE,0,point);
                    SendMessageW(hwnd,WM_LBUTTONDOWN,1,point);
                    SendMessageW(hwnd,WM_LBUTTONUP,0,point);
                }
            }
            SendMessageW(hwnd,RUNTIME_TEST_QUERY,0,lparam);
            return 0;
        }
        if wparam == 7 {
            // Simulated modifier facts exercise the production shortcut mapper
            // without injecting Ctrl/Shift into the user's keyboard state.
            let request = (*(lparam as *const Value)).clone();
            let action = host_tabs::shortcut_action(&(*pointer).borrow(),
                request["key"].as_u64().unwrap() as usize, true,
                request["shift"].as_bool().unwrap_or(false));
            if let Some(Some(action)) = action { (*pointer).borrow_mut().action(action); }
            SendMessageW(hwnd, RUNTIME_TEST_QUERY, 0, lparam);
            return 0;
        }
        if wparam == 6 {
            // One UI-thread batch tests transient hover/press state without
            // moving the user's cursor. Otherwise TrackMouseEvent legitimately
            // queues WM_MOUSELEAVE between a controller's separate messages.
            let point = &*(lparam as *const Value);
            let x = point["x"].as_u64().unwrap() as u32;
            let y = point["y"].as_u64().unwrap() as u32;
            let packed = (x | y << 16) as isize;
            SendMessageW(hwnd, WM_MOUSEMOVE, 0, packed);
            SendMessageW(hwnd, WM_LBUTTONDOWN, 1, packed);
            SendMessageW(hwnd, RUNTIME_TEST_QUERY, 0, lparam);
            return 0;
        }
        if let Ok(state) = (*pointer).try_borrow() {
            *(lparam as *mut Value) = json!({
                "capture":state.lighting_capture,
                "busy":state.runtime_feedback_busy(),
                "deferred":state.runtime_feedback_waiting,
                "failures":state.failures.outstanding(),
                "pending":state.pending,
                "brightness":state.lighting.intent.brightness.as_ref().map(|b|b.value),
                "status":state.status,"failed":state.failed,
                "host_tabs":host_tabs::diagnostics(&state),"calibration":calibration_view::diagnostics(&state),"advanced":advanced_view::diagnostics(&state),"mouse_feedback":mouse_feedback::diagnostics(&state)
            });
        }
        return 0;
    }
    if message == PROFILE_DIALOG {
        // Common dialogs pump this window's messages. Release the state borrow
        // before opening one so paints, device replies and resize remain safe.
        let target = (*pointer).borrow().selected.clone();
        let job = if wparam == 0 {
            profile_files::choose_import(hwnd)
                .map(|path| path.map(|path| Job::ImportFile { path, target }))
        } else {
            profile_files::choose_export(hwnd).and_then(|selection| match (selection, target) {
                (Some((path, format)), Some(target)) => Ok(Some(Job::ExportFile {
                    path,
                    format,
                    target,
                })),
                (None, _) => Ok(None),
                _ => Err("Select a connected device before exporting".into()),
            })
        };
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            match job {
                Ok(Some(job)) => state.queue(job),
                Ok(None) => {}
                Err(error) => state.error(&error),
            }
        }
        return 0;
    }
    let Ok(mut state) = (*pointer).try_borrow_mut() else {
        if message == WM_SIZE {
            // ShowWindow/SetWindowPos can also run inside an editor action.
            // Reconcile after that borrow ends; WM_SIZE reads the current
            // client rect so a deferred older notification cannot undo a resize.
            PostMessageW(hwnd, WM_SIZE, wparam, 0);
            return 0;
        }
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    if message==WM_COMMAND && (macro_view::command(&mut state,(wparam&0xffff)as u16,((wparam>>16)&0xffff)as u16) || linked_games::command(&mut state,(wparam&0xffff)as u16,((wparam>>16)&0xffff)as u16) || studio_view::command(&mut state,(wparam&0xffff)as u16,((wparam>>16)&0xffff)as u16)) {return 0;}
    if message==WM_MOUSEWHEEL && studio_view::wheel(&mut state,((wparam>>16)as u16 as i16)as i32,
        POINT{x:(lparam as u32&0xffff)as i16 as i32,y:((lparam as u32>>16)&0xffff)as i16 as i32}) {return 0;}
    if studio_view::layer_pointer(&mut state,message,lparam) {return 0;}
    if studio_canvas::Canvas::pointer(&mut state,message,lparam) {return 0;}
    if state.restricted_actions
        && matches!(
            message,
            WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_RBUTTONUP | WM_MOUSEWHEEL | WM_COMMAND | WM_KEYDOWN
        )
    {
        return 0;
    }
    if state.profile_modal.is_none()
        && matches!(message, WM_MOUSEMOVE | WM_MOUSELEAVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_CANCELMODE)
    {
        host_tabs::pointer(&state, message, lparam);
    }
    if state.profile_modal.is_some() {
        match message {
            WM_LBUTTONDOWN => {
                SetFocus(hwnd);
                let x = f64::from((lparam as u32 & 0xffff) as i16) / f64::from(state.scale);
                let y = f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale);
                let g = profile_modal_view::geometry(&state, state.profile_modal.as_ref().unwrap());
                let v = state.profile_modal.as_mut().unwrap();
                if !v.cloud {
                    if let Some(bar) = profile_modal_layout::scrollbar(&g, &v.expanded(), v.scroll)
                    {
                        if bar.contains(x, y) {
                            if bar.thumb_contains(y) {
                                v.scroll_grab = Some(y - bar.thumb.y);
                                SetCapture(hwnd);
                            } else {
                                v.scroll = bar.page(v.scroll, y);
                            }
                            InvalidateRect(hwnd, null(), 0);
                        }
                    }
                }
                return 0;
            }
            WM_LBUTTONUP => {
                if state
                    .profile_modal
                    .as_mut()
                    .unwrap()
                    .scroll_grab
                    .take()
                    .is_some()
                {
                    ReleaseCapture();
                    InvalidateRect(hwnd, null(), 0);
                    return 0;
                }
                let point = POINT {
                    x: (lparam as u32 & 0xffff) as i16 as i32,
                    y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                };
                let action = state
                    .hits
                    .iter()
                    .rev()
                    .find(|h| PtInRect(&h.rect, point) != 0)
                    .map(|h| h.action.clone());
                if let Some(Action::ProfileModal(control)) = action {
                    state.profile_modal_action(control);
                }
                return 0;
            }
            WM_MOUSEWHEEL => {
                let modal_width = f64::from(state.width) / f64::from(state.scale);
                if let Some(v) = &mut state.profile_modal {
                    let g = profile_modal_layout::geometry(
                        modal_width,
                        v.mode == profile_modal_model::Mode::Import,
                        v.native(),
                        v.warning,
                    );
                    let (_, max) = profile_modal_layout::rows(&g, &v.expanded(), 0.0);
                    v.scroll = (v.scroll
                        - if (wparam >> 16) as u16 as i16 > 0 {
                            40.0
                        } else {
                            -40.0
                        })
                    .clamp(0.0, max);
                }
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
            WM_MOUSEMOVE => {
                let y = f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale);
                let g = profile_modal_view::geometry(&state, state.profile_modal.as_ref().unwrap());
                let v = state.profile_modal.as_mut().unwrap();
                if let Some(grab) = v.scroll_grab {
                    if let Some(bar) = profile_modal_layout::scrollbar(&g, &v.expanded(), v.scroll)
                    {
                        v.scroll = bar.drag(y, grab);
                    }
                }
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
            WM_CANCELMODE | WM_CAPTURECHANGED => {
                state.profile_modal.as_mut().unwrap().scroll_grab = None;
                if message == WM_CANCELMODE {
                    ReleaseCapture();
                }
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
            WM_KEYDOWN if wparam == 27 => return 0,
            WM_RBUTTONUP => return 0,
            _ => {}
        }
    }
    match message {
        WM_COMMAND if (wparam & 0xffff) as u16 == assignment_view::SPEED => {
            let code = (wparam >> 16) as u32;
            if code == EN_CHANGE || code == EN_KILLFOCUS {
                assignment_view::change(&mut state, code == EN_KILLFOCUS);
            }
            0
        }
        WM_COMMAND
            if (lighting_view::HEX..=lighting_view::BOOST)
                .contains(&((wparam & 0xffff) as u16)) =>
        {
            let event = match (wparam >> 16) as u32 {
                EN_SETFOCUS => Some(lighting_view::InputEvent::Focus),
                EN_CHANGE => Some(lighting_view::InputEvent::Change),
                EN_KILLFOCUS => Some(lighting_view::InputEvent::Blur),
                _ => None,
            };
            if let Some(event) = event {
                let update = lighting_view::input(&mut state, (wparam & 0xffff) as u16, event);
                state.lighting_update(update);
            }
            0
        }
        WM_COMMAND
            if (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10)
                .contains(&((wparam & 0xffff) as u16))
                && (wparam >> 16) as u32 == EN_CHANGE =>
        {
            let id = (wparam & 0xffff) as u16;
            let text = state.raw_text(id);
            let digits = text.strip_prefix('-').unwrap_or(&text);
            if text.is_empty()
                || text == "-"
                || (!digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()))
            {
                state.performance_input_text.insert(id, text);
            } else if let (Some(previous), Some(control)) =
                (state.performance_input_text.get(&id), state.fields.get(&id))
            {
                SetWindowTextW(*control, wide(previous).as_ptr());
                SendMessageW(*control, EM_SETSEL, previous.len(), previous.len() as isize);
            }
            0
        }
        WM_COMMAND
            if (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10)
                .contains(&((wparam & 0xffff) as u16))
                && (wparam >> 16) as u32 == EN_KILLFOCUS =>
        {
            if !state.closing {
                state.commit_performance_input((wparam & 0xffff) as u16);
            }
            0
        }
        PROFILE_RENAME => {
            state.finish_rename(wparam != 0);
            0
        }
        WM_COMMAND
            if (wparam & 0xffff) as u16 == PROFILE_NAME
                && (wparam >> 16) as u32 == EN_KILLFOCUS =>
        {
            if state.renaming.is_some() {
                PostMessageW(hwnd, PROFILE_RENAME, 0, 0);
            }
            0
        }
        WM_SIZE => {
            let update = lighting_view::cancel(&mut state);
            state.lighting_update(update);
            if state.power_gesture.take().is_some() {
                ReleaseCapture();
            }
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            state.width = client.right;
            state.height = client.bottom;
            state.hits.clear();
            state.rebuild_fields();
            mouse_feedback::reconcile(&mut state);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_SHOWWINDOW => {if wparam==0{mouse_feedback::stop(&mut state);}else{mouse_feedback::reconcile(&mut state);}0}
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            mouse_feedback::reconcile(&mut state);
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
        WM_SETCURSOR if state.tab == Tab::Performance && state.performance.is_some() => {
            let mut point: POINT = std::mem::zeroed();
            if GetCursorPos(&mut point) != 0 {
                ScreenToClient(hwnd, &mut point);
                state.performance_pointer(point);
            }
            drop(state);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_TIMER if wparam == ASSIGNMENT_REPEAT => {
            assignment_view::repeat(&mut state);
            0
        }
        WM_MOUSEMOVE if assignment_view::covered(&state) => {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            assignment_view::pointer(
                &mut state,
                POINT {
                    x: (lparam as u32 & 0xffff) as i16 as i32,
                    y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                },
            );
            0
        }
        WM_MOUSEWHEEL if {
            let mut point = POINT { x: (lparam as u32 & 0xffff) as i16 as i32, y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32 };
            ScreenToClient(hwnd, &mut point);
            host_tabs::contains(&state, point)
        } => {
            host_tabs::wheel(&state, ((wparam >> 16) as u16 as i16) as i32);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_MOUSEWHEEL if assignment_view::covered(&state) => {
            assignment_view::wheel(&mut state, ((wparam >> 16) as u16 as i16) as f64 / 120.0);
            0
        }
        WM_MOUSEMOVE if advanced_view::capturing(&state) => {
            advanced_view::pointer(&mut state,POINT{x:(lparam as u32 & 0xffff) as i16 as i32,y:((lparam as u32>>16)&0xffff) as i16 as i32});0
        }
        WM_MOUSEMOVE if state.tab==Tab::Advanced=>{advanced_view::hover(&mut state,POINT{x:(lparam as u32&0xffff) as i16 as i32,y:((lparam as u32>>16)&0xffff) as i16 as i32});0}
        WM_MOUSEMOVE if state.lighting_capture => {
            let point = state.lighting_point(POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            });
            let update = lighting_view::pointer(&mut state, point);
            state.lighting_update(update);
            0
        }
        WM_MOUSEMOVE
            if state.tab == Tab::Lighting && state.device().is_some_and(|d| d["pid"] == "025D") =>
        {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_MOUSEMOVE if state.tab == Tab::Performance && state.performance.is_some() => {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            state.performance_pointer(POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            });
            0
        }
        WM_MOUSEMOVE if state.power_gesture.is_some() => {
            power_view::pointer(
                &mut state,
                POINT {
                    x: (lparam as u32 & 0xffff) as i16 as i32,
                    y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                },
            );
            0
        }
        WM_MOUSELEAVE => {
            state.assignment.hover = None;
            if let Some(d) = &mut state.assignment.draft {
                if d.rail_expanded {
                    d.rail_expanded = false;
                    state.rebuild_fields();
                }
            }
            state.performance_hover = None;
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_CANCELMODE => {
            advanced_view::cancel(&mut state);
            assignment_view::release(&mut state);
            let update = lighting_view::cancel(&mut state);
            state.lighting_update(update);
            // DefWindowProc can synchronously send WM_CAPTURECHANGED while
            // this state is borrowed. Cancel explicitly before releasing it.
            state.power_gesture = None;
            if state.performance_gesture.take().is_some() {
                KillTimer(hwnd, PERFORMANCE_REPEAT);
                state.sync_performance(true);
                state.rebuild_fields();
            }
            ReleaseCapture();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_CAPTURECHANGED
            if state.assignment.speed_drag
                || state.assignment.spinner.is_some()
                || state.assignment.rail_grab.is_some()
                || state.assignment.drawer_grab.is_some()
                || state.assignment.form_grab.is_some()
                || state.assignment.options_grab.is_some() =>
        {
            assignment_view::release(&mut state);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_CAPTURECHANGED if advanced_view::capturing(&state) => {
            advanced_view::cancel(&mut state);InvalidateRect(hwnd,null(),0);0
        }
        WM_CAPTURECHANGED if state.lighting_capture => {
            let update = lighting_view::cancel(&mut state);
            state.lighting_update(update);
            state.lighting_capture = false;
            KillTimer(hwnd, LIGHTING_REPEAT);
            0
        }
        WM_CAPTURECHANGED if state.power_gesture.is_some() => {
            state.power_gesture = None;
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_CAPTURECHANGED if state.performance_gesture.is_some() => {
            state.performance_gesture = None;
            KillTimer(hwnd, PERFORMANCE_REPEAT);
            state.sync_performance(true);
            state.rebuild_fields();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_MBUTTONDOWN if state.profile_modal.is_none() => {
            let point = POINT { x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32 };
            let id = state.hits.iter().rev().find(|hit| point.y < state.px(42) && PtInRect(&hit.rect, point) != 0)
                .and_then(|hit|match &hit.action { Action::Device(id) | Action::HostClose(id) => Some(id.clone()), _ => None });
            if let Some(id) = id { state.action(Action::HostClose(id)); }
            0
        }
        WM_LBUTTONDOWN => {
            let point = POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            };
            advanced_view::outside(&mut state,point);
            let logical = state.lighting_point(point);
            let update = lighting_view::outside(&mut state, logical);
            state.lighting_update(update);
            let action = state
                .hits
                .iter()
                .rev()
                .find(|hit| PtInRect(&hit.rect, point) != 0)
                .map(|hit| hit.action.clone());
            if !matches!(action, Some(Action::PerfStep(..))) {
                let focused = GetFocus();
                if let Some(id) = state
                    .fields
                    .iter()
                    .find(|(_, control)| **control == focused)
                    .map(|(id, _)| *id)
                {
                    if (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10).contains(&id) {
                        state.commit_performance_input(id);
                    }
                    if id == PROFILE_NAME {
                        PostMessageW(hwnd, PROFILE_RENAME, 0, 0);
                    }
                    if (lighting_view::HEX..=lighting_view::BOOST).contains(&id) {
                        let update =
                            lighting_view::input(&mut state, id, lighting_view::InputEvent::Blur);
                        state.lighting_update(update);
                    }
                }
                SetFocus(hwnd);
            }
            if let Some(
                action @ (Action::PerfSlider(..) | Action::PerfDrag(..) | Action::PerfStep(..)),
            ) = action
            {
                state.action(action);
                state.performance_pointer(point);
            } else if let Some(Action::Lighting(control)) = action {
                let update = lighting_view::begin(&mut state, control, logical);
                state.lighting_update(update);
            } else if let Some(Action::Assignment(assignment_view::Control::Step(delta))) = action {
                assignment_view::begin_step(&mut state, delta);
            } else if let Some(Action::Assignment(
                control @ (assignment_view::Control::Slider(_)
                | assignment_view::Control::RailThumb(_)
                | assignment_view::Control::DrawerThumb(_)
                | assignment_view::Control::FormThumb(_)
                | assignment_view::Control::OptionsThumb(_)),
            )) = action
            {
                assignment_view::pointer(&mut state, point);
                assignment_view::action(&mut state, control);
            } else if let Some(Action::Advanced(control @ (advanced_view::Control::Point(_) | advanced_view::Control::RotationSlider)))=action {
                advanced_view::action(&mut state,control);
                if matches!(control,advanced_view::Control::RotationSlider){advanced_view::pointer(&mut state,point);}
            } else if let Some(Action::PowerSlider(kind, shape)) = action {
                power_view::begin(&mut state, kind, shape);
                power_view::pointer(&mut state, point);
            }
            0
        }
        WM_MOUSEWHEEL if linked_games::active(&state) => {linked_games::wheel(&mut state,(wparam>>16) as u16 as i16);0}
        WM_MOUSEWHEEL if macro_view::active(&state) => {macro_view::wheel(&mut state,(wparam>>16) as u16 as i16);0}
        WM_MOUSEWHEEL if state.profile_open => {
            let count = state
                .device()
                .map(|d| strings(&d["profiles"]).len())
                .unwrap_or(0);
            if ((wparam >> 16) as u16 as i16) < 0 {
                state.profile_offset = (state.profile_offset + 1).min(count.saturating_sub(7));
            } else {
                state.profile_offset = state.profile_offset.saturating_sub(1);
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_MOUSEWHEEL
            if state.tab == Tab::Lighting && state.device().is_some_and(|d| d["pid"] == "025D") =>
        {
            let update = lighting_view::scroll(&mut state, (wparam >> 16) as u16 as i16);
            state.lighting_update(update);
            0
        }
        WM_MOUSEWHEEL if state.tab==Tab::Advanced => {
            advanced_view::wheel(&mut state,(wparam>>16) as u16 as i16,wparam&0x0008!=0);0
        }
        WM_MOUSEWHEEL if state.tab == Tab::Performance && state.performance.is_some() => {
            if let Some((g, _)) = performance_view::layout(&state) {
                let bottom = (g.sensitivity_widget.y + g.sensitivity_widget.height)
                    .max(g.polling_widget.y + g.polling_widget.height)
                    + state.performance_scroll;
                let viewport = state.content_bottom_css();
                let delta = f64::from((wparam >> 16) as u16 as i16) / 120.0 * 60.0;
                state.performance_scroll = (state.performance_scroll - delta)
                    .clamp(0.0, viewport::card_scroll_max(bottom, viewport));
                state.performance_hover = None;
                state.rebuild_fields();
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            if advanced_view::capturing(&state){advanced_view::pointer(&mut state,POINT{x:(lparam as u32 & 0xffff) as i16 as i32,y:((lparam as u32>>16)&0xffff) as i16 as i32});advanced_view::finish(&mut state);return 0;}
            if assignment_view::release(&mut state) {
                return 0;
            }
            if state.lighting_capture {
                let logical = state.lighting_point(POINT {
                    x: (lparam as u32 & 0xffff) as i16 as i32,
                    y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                });
                let update = lighting_view::pointer(&mut state, logical);
                state.lighting_update(update);
                let update = lighting_view::finish(&mut state);
                state.lighting_update(update);
                return 0;
            }
            if state.power_gesture.is_some() {
                power_view::pointer(
                    &mut state,
                    POINT {
                        x: (lparam as u32 & 0xffff) as i16 as i32,
                        y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                    },
                );
                power_view::finish(&mut state);
                return 0;
            }
            if state.performance_gesture.is_some() {
                let point = POINT {
                    x: (lparam as u32 & 0xffff) as i16 as i32,
                    y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
                };
                state.performance_pointer(point);
                state.finish_performance_gesture();
                return 0;
            }
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
            } else {
                if assignment_view::open_at(&mut state, point) {
                    InvalidateRect(hwnd, null(), 0);
                    return 0;
                }
                let center = ((state.width as f32 / state.scale).round() as i32 / 2).max(420);
                let selection = if state.tab == Tab::Customize
                    && !state.profile_open
                    && !state.profile_actions
                    && state.keyboard_customize.is_some()
                    && state.device().is_some_and(|d| d["pid"] == "025D")
                {
                    state
                        .keyboard_layout
                        .as_ref()
                        .and_then(|layout| {
                            layout.hit_test(
                                source_layout::Point {
                                    x: f64::from(point.x),
                                    y: f64::from(point.y),
                                },
                                source_layout::Viewport {
                                    left: f64::from(state.px(center - 365)),
                                    top: f64::from(state.px(154)),
                                    width: f64::from(state.px(730)),
                                    height: f64::from(state.px(387)),
                                },
                            )
                        })
                        .map(|key| {
                            (
                                key.firmware_id.and_then(|id| u8::try_from(id).ok()),
                                key.label.clone(),
                            )
                        })
                } else {
                    None
                };
                if let Some((id, label)) = selection {
                    if let Some(id) = id {
                        state.selected_button = secbird_core::buttons::Input::Key(id).name();
                        if state.buttons.is_empty() {
                            let target = state.id();
                            state.queue(Job::Buttons(target));
                        }
                        state.rebuild_fields();
                    } else {
                        state.error(&format!("{label}: the firmware mapping is not verified"));
                    }
                }
                state.profile_open = false;
                state.profile_actions = false;
                for control in state.fields.values() {
                    ShowWindow(*control, SW_SHOW);
                }
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            if state.tab==Tab::Advanced&&advanced_view::template_key(&mut state,wparam){return 0;}
            if wparam == 27 && state.assignment.draft.is_some() {
                let control = if state.assignment.pending.is_some() {
                    assignment_view::Control::Dismiss
                } else {
                    assignment_view::Control::Cancel
                };
                assignment_view::action(&mut state, control);
                return 0;
            }
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
                    if state.profile_open || state.profile_actions {
                        state.profile_open = false;
                        state.profile_actions = false;
                        for control in state.fields.values() {
                            ShowWindow(*control, SW_SHOW);
                        }
                        InvalidateRect(hwnd, null(), 0);
                    } else {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                }
                _ => {}
            }
            0
        }
        WM_KEYUP
            if matches!(wparam, 13 | 32)
                && matches!(
                    state.performance_gesture,
                    Some(PerformanceGesture::Spinner { .. })
                ) =>
        {
            state.finish_performance_gesture();
            0
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let row = state
                .fields
                .iter()
                .find(|(_, control)| **control == lparam as HWND)
                .map(|(id, _)| *id)
                .and_then(|id| id.checked_sub(PERFORMANCE_INPUT))
                .filter(|n| *n < 10)
                .map(|n| n as usize / 2);
            if let Some(row) = row.filter(|row| {
                state
                    .performance
                    .as_ref()
                    .and_then(|p| p.stages.get(*row))
                    .is_some_and(|r| !r.visible)
            }) {
                let hover = state.performance_hover == Some(row);
                SetTextColor(wparam as HDC, rgb(if hover { 0x555555 } else { 0x494949 }));
                SetBkColor(wparam as HDC, rgb(if hover { 0x1d1d1d } else { PANEL }));
                return (if hover {
                    state.dim_edit_brush
                } else {
                    state.edit_brush
                }) as isize;
            }

            SetTextColor(wparam as HDC, rgb(TEXT));
            SetBkColor(wparam as HDC, rgb(PANEL));
            state.edit_brush as isize
        }
        WM_TIMER if wparam == FAILURE_RETRY => {
            KillTimer(hwnd, FAILURE_RETRY);
            PostMessageW(hwnd, FAILURE_DIALOG, 0, 0);
            0
        }
        WM_TIMER if wparam == CACHED_FEEDBACK_RETRY => {
            KillTimer(hwnd, CACHED_FEEDBACK_RETRY);
            PostMessageW(hwnd, FAILURE_DIALOG, 1, 0);
            0
        }
        WM_TIMER if wparam == CACHED_LIGHTING_TIMER => {
            state.request_cached_lighting();
            studio_view::output_tick(&mut state);
            0
        }
        WM_TIMER if wparam == studio_view::PREVIEW_TIMER => {
            studio_view::preview_tick(&mut state);
            0
        }
        WM_TIMER if wparam == RESPONSE_TIMER => {
            state.collect();
            0
        }
        WM_RBUTTONUP if state.tab == Tab::Lighting => {
            let logical = state.lighting_point(POINT {
                x: (lparam as u32 & 0xffff) as i16 as i32,
                y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
            });
            let update = lighting_view::context(&mut state, logical);
            state.lighting_update(update);
            0
        }
        WM_TIMER if wparam == LIGHTING_REPEAT => {
            let update = lighting_view::tick(&mut state);
            state.lighting_update(update);
            0
        }
        WM_TIMER if wparam == PERFORMANCE_REPEAT => {
            if let Some(PerformanceGesture::Spinner { row, axis, delta }) =
                state.performance_gesture
            {
                state.step_performance(row, axis, delta);
            }
            0
        }
        WM_TIMER if wparam == SMOKE_TIMER => {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
            0
        }
        WM_DPICHANGED => {
            state.scale = (wparam as u32 & 0xffff) as f32 / 96.0;
            state.fonts = Fonts::new(&state.assets, state.scale);
            let rect = *(lparam as *const RECT);
            // Applying the new monitor's bounds synchronously sends resize
            // and paint callbacks. Let those callbacks update the editor.
            drop(state);
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
            if let Ok(mut state) = (*pointer).try_borrow_mut() {
                state.width = client.right;
                state.height = client.bottom;
                state.hits.clear();
                state.rebuild_fields();
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        _ => {
            // Caption dragging and border resizing enter a nested Windows
            // message loop here. Keeping State borrowed would reject every
            // nested paint, DPI, timer and resize callback until it exits.
            drop(state);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }
}

pub(crate) fn assets_dir() -> PathBuf {
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
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        let policy = crate::runtime_probe::Policy::from_args(&arguments);
        let lighting_probe = policy.active_probe().is_some();
        let smoke = close_after.is_some()
            || lighting_probe
            || std::env::args().any(|arg| arg == "--runtime-smoke");
        let smoke_requested = smoke.then(|| {
            if lighting_probe {
                crate::runtime_probe::presentation_for(
                    policy.active_probe().unwrap(),
                    &std::env::var("SYNAPSE_LITE_SMOKE_VIEW")
                        .unwrap_or_else(|_| "keyboard-lighting".into()),
                )
                .unwrap_or("keyboard-lighting")
                .to_string()
            } else {
                std::env::var("SYNAPSE_LITE_SMOKE_VIEW").unwrap_or_else(|_| "dashboard".into())
            }
        });
        let assets = assets_dir();
        let _image_runtime = image_render::Runtime::start()?;
        // The manifest enables PerMonitorV2. Initial outer dimensions are DIP,
        // like Electron's source window defaults, rather than physical pixels.
        let initial_scale = GetDpiForSystem().max(96) as f32 / 96.0;
        let (jobs_tx, jobs_rx) = mpsc::channel();
        let (replies_tx, replies_rx) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("synapse-device-worker".into())
            .spawn(move || worker(engine, jobs_rx, replies_tx))
            .map_err(|e| e.to_string())?;
        let mut state = Box::new(RefCell::new(State {
            linked_games:linked_games::View::default(),
            macros:macro_view::View::default(),
            studio: studio_view::View::default(),
            exit_tray: None,
            calibration: calibration_view::View::default(),
            advanced: advanced_view::View::default(),
            mouse_feedback: mouse_feedback::Observer::default(),
            host_tabs: RefCell::new(host_tabs::View::load()),
            customize_background: RefCell::new(dot_background::Cache::default()),
            hwnd: null_mut(),
            scale: initial_scale,
            width: 1280,
            height: 720,
            state: json!({"devices":[]}),
            selected: None,
            tab: Tab::Customize,
            profile_open: false,
            profile_offset: 0,
            profile_actions: false,
            profile_modal: None,
            profile_export: None,
            renaming: None,
            notices: Vec::new(),
            selected_button: String::new(),
            assignment: assignment_model::View::default(),
            assignment_next: None,
            buttons: Vec::new(),
            hits: Vec::new(),
            focus: 0,
            fields: HashMap::new(),
            field_defaults: HashMap::new(),
            field_owner: None,
            retained_fields: HashMap::new(),
            performance_input_text: HashMap::new(),
            jobs: jobs_tx,
            replies: replies_rx,
            runtime_observer: runtime_status::Observer::default(),
            lighting_probe,
            probe_kind: policy.active_probe(),
            read_only_smoke: policy.read_only(),
            restricted_actions: policy.restricted_actions(),
            lighting_runtime_observed: None,
            runtime_feedback_waiting: false,
            pending: 0,
            pending_jobs: VecDeque::new(),
            closing: false,
            failures: feedback::Failures::default(),
            performance: None,
            performance_owner: None,
            performance_scroll: 0.0,
            performance_hover: None,
            performance_hover_hit: None,
            performance_gesture: None,
            power_gesture: None,
            lighting: lighting_view::View::default(),
            lighting_capture: false,
            status: "Reading connected devices…".into(),
            failed: false,
            smoke_view: smoke_requested.clone(),
            smoke_requested,
            smoke_reported: false,
            smoke_overlay_started: false,
            fonts: Fonts::new(&assets, initial_scale),
            mouse: Bitmap::load(&assets.join("mouse.bmp")),
            device_artwork: RefCell::new(device_artwork::View::load(assets.join("device-pack"))),
            keyboard: Bitmap::load(&assets.join("keyboard.bmp")),
            keyboard_customize: Bitmap::load(&assets.join("keyboard-customize.bmp")),
            keyboard_layout: source_layout::SourceLayout::load(
                &assets.join("keyboard-layout.json"),
            )
            .ok()
            .filter(|layout| {
                layout.product_id == 605
                    && layout.edition_id == 0
                    && layout.layout_id == 6
                    && layout.view_box == [0.0, 0.0, 730.0, 387.0]
                    && !layout.source_sha256.is_empty()
            }),
            icons: [
                "host-synapse",
                "host-mouse",
                "host-keyboard",
                "host-linked-games",
                "host-game-placeholder",
                "host-scroll-left",
                "host-scroll-right",
                "host-scroll-left-hover",
                "host-scroll-left-active",
                "host-scroll-left-disabled",
                "host-scroll-right-hover",
                "host-scroll-right-active",
                "host-scroll-right-disabled",
                "close-tab",
                "close-tab-hover",
                "close-tab-pressed",
                "minimize",
                "maximize",
                "restore",
                "close",
                "back",
                "refresh",
                "rotation-mouse",
                "expand",
                "profile-default",
                "profile-close",
                "profile-folder",
                "profile-macro",
                "profile-cone",
                "profile-warning",
                "more",
                "dpi-xy",
                "dpi-xy-active",
                "dpi-xy-disabled",
                "dpi-drag",
                "dpi-stepper-up",
                "dpi-stepper-down",
                "dpi-stage-1-default",
                "dpi-stage-1-active",
                "dpi-stage-2-default",
                "dpi-stage-2-active",
                "dpi-stage-3-default",
                "dpi-stage-3-active",
                "dpi-stage-4-default",
                "dpi-stage-4-active",
                "dpi-stage-5-default",
                "dpi-stage-5-active",
                "lighting-ccw-active",
                "lighting-ccw",
                "lighting-cw-active",
                "lighting-cw",
                "lighting-eyedropper-hover",
                "lighting-eyedropper",
                "lighting-in-active",
                "lighting-in",
                "lighting-left-active",
                "lighting-left",
                "lighting-no-color",
                "lighting-out-active",
                "lighting-out",
                "lighting-plus",
                "lighting-right-active",
                "lighting-right",
                "lighting-sync",
                "assignment-dialog-close",
                "assignment-help",
                "assignment-mouse-cycle",
                "assignment-close",
                "assignment-panel-active",
                "assignment-panel",
                "assignment-panel-close",
                "assignment-DEFAULT-normal",
                "assignment-DEFAULT-hover",
                "assignment-DEFAULT-active",
                "assignment-AI_LAUNCHER-normal",
                "assignment-AI_LAUNCHER-hover",
                "assignment-AI_LAUNCHER-active",
                "assignment-KEYBOARD_FUNCTION-normal",
                "assignment-KEYBOARD_FUNCTION-hover",
                "assignment-KEYBOARD_FUNCTION-active",
                "assignment-MOUSE_FUNCTION-normal",
                "assignment-MOUSE_FUNCTION-hover",
                "assignment-MOUSE_FUNCTION-active",
                "assignment-SENSITIVITY-normal",
                "assignment-SENSITIVITY-hover",
                "assignment-SENSITIVITY-active",
                "assignment-MACRO-normal",
                "assignment-MACRO-hover",
                "assignment-MACRO-active",
                "assignment-INTERDEVICE-normal",
                "assignment-INTERDEVICE-hover",
                "assignment-INTERDEVICE-active",
                "assignment-SWITCH_PROFILE-normal",
                "assignment-SWITCH_PROFILE-hover",
                "assignment-SWITCH_PROFILE-active",
                "assignment-SWITCH_LIGHTING-normal",
                "assignment-SWITCH_LIGHTING-hover",
                "assignment-SWITCH_LIGHTING-active",
                "assignment-DEVICE_BRIGHTNESS-normal",
                "assignment-DEVICE_BRIGHTNESS-hover",
                "assignment-DEVICE_BRIGHTNESS-active",
                "assignment-RAZER_HYPERSHIFT-normal",
                "assignment-RAZER_HYPERSHIFT-hover",
                "assignment-RAZER_HYPERSHIFT-active",
                "assignment-LAUNCH_PROGRAM-normal",
                "assignment-LAUNCH_PROGRAM-hover",
                "assignment-LAUNCH_PROGRAM-active",
                "assignment-MULTIMEDIA-normal",
                "assignment-MULTIMEDIA-hover",
                "assignment-MULTIMEDIA-active",
                "assignment-WINDOWS_SHORTCUT-normal",
                "assignment-WINDOWS_SHORTCUT-hover",
                "assignment-WINDOWS_SHORTCUT-active",
                "assignment-TEXT_FUNCTION-normal",
                "assignment-TEXT_FUNCTION-hover",
                "assignment-TEXT_FUNCTION-active",
                "assignment-DISABLE-normal",
                "assignment-DISABLE-hover",
                "assignment-DISABLE-active",
                "assignment-DIAL_MULTI_FUNCTION-normal",
                "assignment-DIAL_MULTI_FUNCTION-hover",
                "assignment-DIAL_MULTI_FUNCTION-active",
            ]
            .into_iter()
            .filter_map(|name| {
                Bitmap::load(&assets.join(format!("icon-{name}.bmp"))).map(|image| (name, image))
            })
            .collect(),
            assets,
            edit_brush: CreateSolidBrush(rgb(PANEL)),
            dim_edit_brush: CreateSolidBrush(rgb(0x1d1d1d)),
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
            (1280.0 * initial_scale).round() as i32,
            (720.0 * initial_scale).round() as i32,
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
        window_frame::suppress_system_border(hwnd);
        {
            let mut state = state.borrow_mut();
            let actual_scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            if state.scale != actual_scale {
                state.scale = actual_scale;
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
        SetTimer(hwnd, CACHED_LIGHTING_TIMER, 1000, None);
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
            if message.message == WM_KEYDOWN && GetKeyState(18) >= 0 {
                let studio_action=studio_view::key(&state.borrow(),message.wParam,GetKeyState(17)<0,matches!(GetDlgCtrlID(message.hwnd)as u16,studio_view::NAME|studio_view::COLOR));
                if let Some(control)=studio_action {state.borrow_mut().action(Action::Studio(control));continue;}
                let action = host_tabs::shortcut_action(&state.borrow(), message.wParam,
                    GetKeyState(17) < 0, GetKeyState(16) < 0);
                if let Some(action) = action {
                    if let Some(action) = action { state.borrow_mut().action(action); }
                    InvalidateRect(hwnd, null(), 0);
                    continue;
                }
            }
            if message.message == WM_KEYDOWN && matches!(message.wParam, 13 | 27) {
                let rename_edit = state.borrow().fields.get(&PROFILE_NAME).copied();
                if rename_edit == Some(message.hwnd) {
                    PostMessageW(hwnd, PROFILE_RENAME, usize::from(message.wParam == 27), 0);
                    continue;
                }
            }
            let control_id = GetDlgCtrlID(message.hwnd) as u16;
            if control_id == assignment_view::SPEED {
                if message.message == WM_KEYDOWN && matches!(message.wParam, 13 | 27) {
                    SetFocus(hwnd);
                    continue;
                }
                let step = if message.message == WM_KEYDOWN && matches!(message.wParam, 38 | 40) {
                    Some(if message.wParam == 38 { 1 } else { -1 })
                } else if message.message == WM_MOUSEWHEEL {
                    Some(if ((message.wParam >> 16) as u16 as i16) > 0 {
                        1
                    } else {
                        -1
                    })
                } else {
                    None
                };
                if let Some(step) = step {
                    let mut current = state.borrow_mut();
                    if !current.restricted_actions {
                        assignment_view::action(&mut current, assignment_view::Control::Step(step));
                    }
                    continue;
                }
            }
            if (lighting_view::HEX..=lighting_view::BOOST).contains(&control_id)
                && message.message == WM_KEYDOWN
                && matches!(message.wParam, 13 | 27)
            {
                let mut current = state.borrow_mut();
                let event = if message.wParam == 13 {
                    lighting_view::InputEvent::Enter
                } else {
                    lighting_view::InputEvent::Escape
                };
                let update = lighting_view::input(&mut current, control_id, event);
                current.lighting_update(update);
                SetFocus(hwnd);
                continue;
            }
            if control_id == lighting_view::BOOST
                && ((message.message == WM_KEYDOWN && matches!(message.wParam, 38 | 40))
                    || message.message == WM_MOUSEWHEEL)
            {
                let mut current = state.borrow_mut();
                let update = lighting_view::action(
                    &mut current,
                    lighting_view::Control::BoostStep(if message.message == WM_MOUSEWHEEL {
                        if ((message.wParam >> 16) as u16 as i16) > 0 {
                            1
                        } else {
                            -1
                        }
                    } else if message.wParam == 38 {
                        1
                    } else {
                        -1
                    }),
                );
                current.lighting_update(update);
                continue;
            }
            if (PERFORMANCE_INPUT..PERFORMANCE_INPUT + 10).contains(&control_id) {
                if message.message == WM_KEYDOWN && matches!(message.wParam, 13 | 27) {
                    SetFocus(hwnd);
                    continue;
                }
                let step = if message.message == WM_KEYDOWN && matches!(message.wParam, 38 | 40) {
                    Some(if message.wParam == 38 { 50 } else { -50 })
                } else if message.message == WM_MOUSEWHEEL {
                    Some(if ((message.wParam >> 16) as u16 as i16) > 0 {
                        50
                    } else {
                        -50
                    })
                } else {
                    None
                };
                if let Some(delta) = step {
                    let index = control_id - PERFORMANCE_INPUT;
                    state.borrow_mut().step_performance(
                        index as usize / 2,
                        if index % 2 == 0 { Axis::X } else { Axis::Y },
                        delta,
                    );
                    continue;
                }
            }
            if IsDialogMessageW(hwnd, &mut message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
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
