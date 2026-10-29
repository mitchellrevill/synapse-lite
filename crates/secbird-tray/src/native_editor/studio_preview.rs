//! Local canvas playback. Compilation uses the file worker; no engine/HID call.
use super::*;

pub(super) struct Playback {
    requested: bool,
    pending: bool,
    generation: u64,
    plan: Option<Box<Preview>>,
    snapshot: Option<ReadSnapshot>,
    random_state: u32,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            requested: false,
            pending: false,
            generation: 0,
            plan: None,
            snapshot: None,
            random_state: 1,
        }
    }
}
impl Playback {
    pub(super) fn active(&self) -> bool {
        self.requested
    }
    pub(super) fn clear_frame(&mut self) {
        self.plan = None;
        self.snapshot = None;
    }
}
pub(super) unsafe fn stop(state: &mut State) {
    state.studio.playback.requested = false;
    state.studio.playback.clear_frame();
    // Keep pending true until its response returns. Repeated stop/start cannot
    // accumulate background compilation jobs or retained full JSON snapshots.
    KillTimer(state.hwnd, PREVIEW_TIMER);
}
pub(super) unsafe fn toggle(state: &mut State) {
    if state.studio.playback.requested {
        stop(state);
        state.studio.status = "Local preview stopped".into();
        return;
    }
    if !state.studio.canvas.can_preview() {
        error(
            state,
            "Local preview needs this composition's exact device geometry",
        );
        return;
    }
    state.studio.playback.requested = true;
    if SetTimer(state.hwnd, PREVIEW_TIMER, 40, None) == 0 {
        stop(state);
        error(state, "Could not start local preview timer");
        return;
    }
    tick(state);
}
pub(super) unsafe fn tick(state: &mut State) {
    if !state.studio.open || state.restricted_actions || !state.studio.playback.requested {
        stop(state);
        return;
    }
    if state.studio.busy || state.studio.dialog {
        return;
    }
    let Some(draft) = state.studio.draft.as_ref() else {
        stop(state);
        return;
    };
    let fresh = state
        .studio
        .playback
        .snapshot
        .as_ref()
        .is_some_and(|s| draft.matches_snapshot(s));
    if !fresh {
        if state.studio.playback.pending {
            return;
        }
        let Some(generation) = state.studio.playback.generation.checked_add(1) else {
            stop(state);
            error(state, "Local preview session limit reached");
            return;
        };
        let snapshot = draft.read_snapshot();
        let view = &mut state.studio;
        view.playback.generation = generation;
        view.playback.clear_frame();
        view.playback.pending = true;
        view.status = "Preparing local preview…".into();
        let request = Request::Preview {
            epoch: view.epoch,
            generation,
            snapshot,
            random_state: view.playback.random_state,
        };
        let before = (state.pending, state.pending_jobs.len());
        state.queue(Job::Studio(request));
        if before == (state.pending, state.pending_jobs.len()) {
            state.studio.playback.pending = false;
            stop(state);
            error(state, "Local preview compile was not accepted");
        }
    } else if let Some(plan) = state.studio.playback.plan.as_mut() {
        if let Err(message) = plan.advance() {
            stop(state);
            error(state, message.to_string());
        }
    }
    // Keep native text controls intact while rendering. Rebuilding them every
    // 40ms would discard focus/caret and inflate repaint work.
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn receive(
    state: &mut State,
    epoch: u64,
    generation: u64,
    snapshot: ReadSnapshot,
    result: Result<Box<Preview>, String>,
) {
    if generation != state.studio.playback.generation {
        return;
    }
    state.studio.playback.pending = false;
    if !state.studio.playback.requested
        || !state.studio.open
        || epoch != state.studio.epoch
        || !state
            .studio
            .draft
            .as_ref()
            .is_some_and(|d| d.matches_snapshot(&snapshot))
    {
        return;
    }
    match result {
        Ok(plan) => {
            state.studio.playback.random_state = plan.final_random_state();
            state.studio.playback.plan = Some(plan);
            state.studio.playback.snapshot = Some(snapshot);
            state.studio.status = "Local canvas preview · no device output".into();
            state.studio.failed = false;
        }
        Err(message) => {
            stop(state);
            error(state, message);
        }
    }
    redraw(state);
}
pub(super) fn color(state: &State, device: &str, key: &str) -> Option<u32> {
    let plan = state.studio.playback.plan.as_ref()?;
    if plan.device_id() != device {
        return None;
    }
    let (row, col) = key.split_once(':')?;
    let row = row.parse::<usize>().ok()?;
    let col = col.parse::<usize>().ok()?;
    if row >= 7 || col >= 23 {
        return None;
    }
    let c = plan.frame()[row * 23 + col];
    Some((u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2))
}
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    let p = &state.studio.playback;
    json!({"requested":p.requested,"pending":p.pending,"generation":p.generation,"current":p.snapshot.as_ref().is_some_and(|s|state.studio.draft.as_ref().is_some_and(|d|d.matches_snapshot(s))),"frames":p.plan.as_ref().map_or(0,|p|p.rendered_frames()),"effects":p.plan.as_ref().map_or(0,|p|p.effect_count()),"bytes":p.plan.as_ref().map_or(0,|p|p.storage_bytes()),"frame":p.plan.as_ref().map(|p|p.frame().iter().map(|c|[c.0,c.1,c.2]).collect::<Vec<_>>())})
}
