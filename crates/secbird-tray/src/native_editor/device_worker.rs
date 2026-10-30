//! Serial background jobs for the native editor.
//!
//! Owns queue coalescing, request/reply routing and device access. Requests are
//! processed in channel order. Keep lock scopes intact: composition file work
//! and cached lighting status intentionally run outside the Engine lock.
use super::{
    assignment_backend, assignment_model, chroma_view, linked_games, macro_view, mouse_feedback,
    onboard_view, profile_files, profile_modal_backend, profile_modal_model, runtime_status,
    settings_view, shortcuts, strings, studio_view, ASSIGNMENT_SAVED_STATUS,
};
use crate::{engine::Engine, server};
use serde_json::Value;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};

pub(super) enum Job {
    DeviceInformation(String),
    LocalAssignment(Box<assignment_model::Draft>),
    Onboard(onboard_view::Request),
    Chroma(chroma_view::Request),
    Settings(Option<Value>),
    Studio(studio_view::Request),
    LinkedGames(linked_games::Request),
    Shortcuts(shortcuts::Request),
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

pub(super) fn enqueue_setting(queue: &mut VecDeque<Job>, job: Job) -> Result<(), &'static str> {
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

pub(super) struct Reply {
    pub(super) profile_modal: Option<profile_modal_model::Message>,
    pub(super) state: Option<Value>,
    pub(super) buttons: Option<Value>,
    pub(super) status: String,
    pub(super) failed: bool,
    pub(super) notices: Vec<String>,
}

pub(super) enum WorkerReply {
    DeviceInformation(String, String),
    Onboard(onboard_view::Request, Result<Value, String>),
    Chroma(Result<chroma_view::Response, String>),
    Settings(Result<Value, String>),
    StudioControl(
        crate::host_studio::session::Control,
        crate::lighting_status::Reader,
        Value,
    ),
    StudioOutput(Result<Value, String>),
    Studio(studio_view::Response),
    LinkedGames(Result<linked_games::Data, String>),
    Shortcuts(Result<shortcuts::Data, String>),
    MacroLibrary(Result<macro_view::Data, String>),
    Sensor(
        (String, String),
        Result<(Value, mouse_feedback::BindingResult), String>,
    ),
    User(Reply),
    CachedLighting(runtime_status::Response),
}

/// Device access runs on one worker, under the same engine lock as the tray.
/// The UI thread never holds that lock and no browser or HTTP listener is started.
pub(super) fn worker(
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
        let _ = replies.send(WorkerReply::StudioControl(
            engine.studio_control(),
            reader.clone(),
            engine.studio_status(),
        ));
    }
    for job in jobs {
        let job = match job {
            Job::DeviceInformation(id) => {
                let text = match engine.lock() {
                    Ok(engine) => engine
                        .devices()
                        .iter()
                        .find(|dev| dev.key() == id)
                        .map(secbird_core::device_information::diagnostic_text)
                        .unwrap_or_else(|| {
                            "Device disconnected. Rescan devices and try again.".into()
                        }),
                    Err(_) => "Device worker unavailable; try refreshing.".into(),
                };
                if replies
                    .send(WorkerReply::DeviceInformation(id, text))
                    .is_err()
                {
                    break;
                }
                continue;
            }
            Job::Onboard(request) => {
                let result = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .and_then(|mut engine| onboard_view::work(&mut engine, &request));
                if replies.send(WorkerReply::Onboard(request, result)).is_err() {
                    break;
                }
                continue;
            }
            Job::Chroma(request) => {
                let result = engine
                    .lock()
                    .map_err(|_| "Device worker unavailable".to_string())
                    .and_then(|mut engine| chroma_view::work(&mut engine, request));
                if replies.send(WorkerReply::Chroma(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::Shortcuts(request) => {
                let context = engine
                    .lock()
                    .map_err(|_| "Device engine is busy".to_string())
                    .map(|engine| engine.is_measurement_read_only());
                let result = context.and_then(|read_only| shortcuts::work(request, read_only));
                if replies.send(WorkerReply::Shortcuts(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::MacroLibrary(request) => {
                let context = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .map(|engine| engine.is_measurement_read_only());
                let result = context.and_then(|read_only| macro_view::work(request, read_only));
                if replies.send(WorkerReply::MacroLibrary(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::LinkedGames(request) => {
                let context = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .map(|engine| {
                        let rows = engine
                            .devices()
                            .iter()
                            .map(|device| linked_games::DeviceRow {
                                id: device.key(),
                                name: device
                                    .name()
                                    .trim_end_matches(" (Wired)")
                                    .trim_end_matches(" (Wireless)")
                                    .to_string(),
                                pid: device.pid,
                                profiles: vec![],
                            })
                            .collect();
                        (rows, engine.is_measurement_read_only())
                    });
                let result = context.and_then(|(devices, read_only)| {
                    linked_games::work(request, devices, read_only)
                });
                if replies.send(WorkerReply::LinkedGames(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::StudioOutput(request) => {
                let result = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .and_then(|mut engine| studio_view::output_work(&mut engine, request));
                if replies.send(WorkerReply::StudioOutput(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::Settings(body) => {
                let result = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .and_then(|mut engine| {
                        if let Some(value) = body.as_ref().filter(|value| {
                            value["key"].as_str().is_some_and(|k| k.starts_with("store_"))
                        }) {
                            return settings_view::store_work(&engine, value);
                        }
                        if let Some(value) = body.as_ref().filter(|value| value["key"] == "lighting_control") {
                            if engine.is_measurement_read_only() {
                                return Err("Read-only mode refuses lighting ownership changes".into());
                            }
                            let snapshot = server::dispatch(&mut engine, "GET", "/api/state", &[])
                                .map_err(|e| e.to_string())?;
                            if !snapshot["devices"].as_array().is_some_and(|devices| {
                                devices.iter().any(|device| {
                                    device["id"] == value["id"] && device["active"] == value["expected_profile"]
                                })
                            }) {
                                return Err("Device or profile changed; refresh before changing lighting control".into());
                            }
                            server::dispatch(
                                &mut engine, "POST", "/api/lighting-control",
                                &serde_json::to_vec(value).map_err(|e| e.to_string())?,
                            ).map_err(|e| e.to_string())?;
                            return Ok(server::settings_json(&engine));
                        }
                        let (method, bytes) = match body {
                            Some(value) => ("POST", serde_json::to_vec(&value).map_err(|e| e.to_string())?),
                            None => ("GET", vec![]),
                        };
                        server::dispatch(&mut engine, method, "/api/settings", &bytes).map_err(|e| e.to_string())
                    });
                if replies.send(WorkerReply::Settings(result)).is_err() {
                    break;
                }
                continue;
            }
            Job::Studio(request) => {
                // Drop the device lock before reading/writing composition or bookmark files.
                let read_only = matches!(
                    &request,
                    studio_view::Request::Save { .. }
                        | studio_view::Request::Export { .. }
                        | studio_view::Request::Remember { .. }
                ) && engine
                    .lock()
                    .map(|engine| engine.is_measurement_read_only())
                    .unwrap_or(true);
                if replies
                    .send(WorkerReply::Studio(studio_view::work(request, read_only)))
                    .is_err()
                {
                    break;
                }
                continue;
            }
            Job::SensorRead(id, profile) => {
                let result = engine
                    .lock()
                    .map_err(|_| "Device worker is unavailable".to_string())
                    .and_then(|mut engine| {
                        let view = server::mouse_sensor_snapshot(&mut engine, &id, &profile)
                            .map_err(|e| e.to_string())?;
                        let binding = if view["dynamic"]["available"] == true
                            && view["dynamic"]["raw"]["state"] == 1
                        {
                            engine
                                .devices()
                                .iter()
                                .find(|d| d.key() == id)
                                .ok_or_else(|| "Selected mouse disconnected".to_string())
                                .and_then(|device| {
                                    secbird_core::mouse_input_identity::discover(device)
                                        .map_err(|e| e.to_string())
                                })
                        } else {
                            Err("Live graph is inactive".into())
                        };
                        Ok((view, binding))
                    });
                if replies
                    .send(WorkerReply::Sensor((id, profile), result))
                    .is_err()
                {
                    break;
                }
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
                Job::LocalAssignment(draft) => {
                    let state = assignment_backend::save(&mut engine, &draft)?;
                    status = ASSIGNMENT_SAVED_STATUS.into();
                    Ok(state)
                }
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
                    if !failed {
                        status = answer
                            .get("status")
                            .and_then(Value::as_str)
                            .unwrap_or("Settings applied")
                            .into();
                    }
                    if answer.get("buttons").is_some() {
                        buttons = Some(answer);
                    }
                    if !failed && path == "/api/sensitivity-draft" {
                        status =
                            "Sensitivity draft saved locally; device assignment unchanged".into();
                    }
                    server::dispatch(&mut engine, "GET", "/api/state", &[])
                }
                Job::Onboard(_)
                | Job::Chroma(_)
                | Job::CachedLighting(_)
                | Job::LinkedGames(_)
                | Job::Shortcuts(_)
                | Job::MacroLibrary(_)
                | Job::Studio(_)
                | Job::StudioOutput(_)
                | Job::SensorRead(..)
                | Job::Profiles(_)
                | Job::ImportFile { .. }
                | Job::DeviceInformation(_)
                | Job::Settings(_)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
