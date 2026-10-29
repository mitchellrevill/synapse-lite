//! Explicit saved-composition output; local canvas preview stays independent.
use super::*;
use crate::host_studio::session::{ApplyIntent, Control as StopControl};
use crate::lighting_status::Reader;
use std::time::{Duration, Instant};

pub(crate) enum Request {
    Apply {
        intent: ApplyIntent,
        archive: Arc<Archive>,
        index: usize,
        key: String,
    },
    Stop,
}
#[derive(Default)]
pub(super) struct View {
    pub(super) control: Option<StopControl>,
    pub(super) reader: Option<Reader>,
    pub(super) status: Value,
    pub(super) busy: bool,
    observed_after: Option<Instant>,
    note: String,
}
pub(super) fn work(engine: &mut Engine, request: Request) -> Result<Value, String> {
    match request {
        Request::Apply {
            intent,
            archive,
            index,
            key,
        } => engine.apply_studio_saved(
            intent,
            &key,
            &archive,
            index,
            &assets_dir().join("DeviceManifest_605_0.json"),
            1,
        ),
        Request::Stop => engine.stop_studio(),
    }
    .map_err(|error| error.to_string())
}
unsafe fn queue(state: &mut State, request: Request) {
    let before = (state.pending, state.pending_jobs.len());
    state.queue(Job::StudioOutput(request));
    state.studio.output.busy = before != (state.pending, state.pending_jobs.len());
    if !state.studio.output.busy {
        error(state, "The device output request was not accepted");
    }
}
pub(super) unsafe fn apply(state: &mut State) {
    if state.studio.output.busy {
        return;
    }
    if state.studio.dirty() || state.studio.renaming {
        error(
            state,
            "Save this composition before applying it to a device",
        );
        return;
    }
    let Some(archive) = state.studio.archive.clone() else {
        error(state, "Open a composition first");
        return;
    };
    let targets: Vec<_> = state
        .devices()
        .iter()
        .filter(|device| device["pid"] == "025D")
        .filter_map(|device| device["id"].as_str().map(str::to_owned))
        .collect();
    let [key]: [String; 1] = match targets.try_into() {
        Ok(target) => target,
        Err(_) => {
            error(
                state,
                "Device playback currently needs exactly one connected Ornata V2",
            );
            return;
        }
    };
    let Some(control) = &state.studio.output.control else {
        error(state, "The device worker is still starting");
        return;
    };
    let intent = match control.apply_intent() {
        Ok(intent) => intent,
        Err(fault) => {
            error(state, fault.to_string());
            return;
        }
    };
    state.studio.output.observed_after = Some(Instant::now());
    state.studio.output.note = "Applying saved composition…".into();
    queue(
        state,
        Request::Apply {
            intent,
            archive,
            index: state.studio.index,
            key,
        },
    );
}
pub(super) unsafe fn stop(state: &mut State) {
    let Some(control) = &state.studio.output.control else {
        return;
    };
    if let Err(fault) = control.request_stop() {
        error(state, fault.to_string());
        return;
    }
    // This atomic request precedes queuing/locking. Even a full UI work queue or
    // blocked device syscall cannot prevent cancellation of later packet admission.
    state.studio.output.observed_after = Some(Instant::now());
    state.studio.output.note = "Stopping device playback…".into();
    if !state.studio.output.busy {
        queue(state, Request::Stop);
    }
}
pub(super) unsafe fn receive(state: &mut State, result: Result<Value, String>) {
    state.studio.output.busy = false;
    match result {
        Ok(status) => {
            state.studio.failed = false;
            state.studio.output.observed_after = Some(Instant::now());
            if status["state"] == "applied"
                && !state.studio.playback.active()
                && !state.studio.dirty()
            {
                playback::toggle(state);
            }
            state.studio.output.note = if status["state"] == "stopped" {
                "Device playback stopped; saved Quick lighting can resume"
            } else {
                "Saved composition applied to the device worker"
            }
            .into();
            state.studio.output.status = status;
        }
        Err(fault) => {
            state.studio.output.note = fault.clone();
            error(state, fault);
        }
    }
    state.studio.status = state.studio.output.note.clone();
    redraw(state);
}
pub(super) unsafe fn tick(state: &mut State) {
    if !state.studio.open {
        return;
    }
    let view = &mut state.studio.output;
    let Some(key) = view.status["target"].as_str() else {
        return;
    };
    let Some(snapshot) = view
        .reader
        .as_ref()
        .and_then(|reader| reader.read_device(key))
    else {
        return;
    };
    if view
        .observed_after
        .is_some_and(|after| snapshot.observed_at < after)
    {
        return;
    }
    if snapshot.observed_at.elapsed() > Duration::from_secs(3) {
        view.note = "Device playback status is old; Stop remains available".into();
    } else if let Some(status) = snapshot.value.get("studio") {
        view.status = status.clone();
        view.note = if status["worker_enabled"] == false {
            "Lighting worker stopped".into()
        } else if let Some(error) = status["runtime_error"].as_str() {
            error.to_owned()
        } else if status["state"] == "faulted" {
            "Device playback failed; stop or explicitly apply again".into()
        } else {
            format!(
                "Device playback: {} accepted frames",
                status["accepted_frames"].as_u64().unwrap_or(0)
            )
        };
    } else if !view.busy {
        view.status = json!({"state":"stopped","target":null});
        view.note = "Device playback stopped; saved Quick lighting can resume".into();
    }
    state.studio.status = view.note.clone();
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter) {
    let view = &state.studio;
    button(
        p,
        "APPLY SAVED",
        855,
        87,
        125,
        Control::ApplyDevice,
        view.editable() && !view.dirty() && !view.output.busy && view.output.control.is_some() && view.canvas.can_preview(),
    );
    button(
        p,
        "STOP DEVICE",
        990,
        87,
        120,
        Control::StopDevice,
        view.output.control.is_some(),
    );
}
