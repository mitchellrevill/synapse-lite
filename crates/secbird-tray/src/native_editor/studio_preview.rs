//! Local canvas playback. Compilation uses the file worker; no engine/HID call.
use super::*;

pub(super) struct Playback {
    requested: bool,
    synthetic_started: Option<std::time::Instant>,
    pending: bool,
    pending_synthetic: bool,
    generation: u64,
    plan: Option<Box<Preview>>,
    snapshot: Option<ReadSnapshot>,
    random_state: u32,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            requested: false,
            synthetic_started: None,
            pending: false,
            pending_synthetic: false,
            generation: 0,
            plan: None,
            snapshot: None,
            random_state: 1,
        }
    }
}
impl Playback {
    fn settle_pending(&mut self, generation:u64) -> bool {
        if generation!=self.generation {return false;}
        self.pending=false;
        self.pending_synthetic==self.synthetic_started.is_some()
    }
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
    state.studio.playback.synthetic_started = None;
    state.studio.playback.clear_frame();
    // Keep pending true until its response returns. Repeated stop/start cannot
    // accumulate background compilation jobs or retained full JSON snapshots.
    KillTimer(state.hwnd, PREVIEW_TIMER);
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn toggle(state: &mut State) {
    // Explicit preview intent may acknowledge an earlier error. Background
    // compile progress/completion must not acknowledge a newer rejected edit.
    state.studio.failed = false;
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
    state.studio.playback.synthetic_started = (state.studio.effect()==Some("ambient")).then(std::time::Instant::now);
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
    if state.studio.playback.synthetic_started.is_some_and(|start|start.elapsed()>=std::time::Duration::from_secs(10)) {
        stop(state);
        state.studio.status="Synthetic test finished (10s); no desktop capture or device output".into();
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
        view.playback.pending_synthetic = view.playback.synthetic_started.is_some();
        if !view.failed {
            view.status = "Preparing local preview…".into();
        }
        let request = Request::Preview {
            synthetic_ambient: view.playback.synthetic_started.is_some(),
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
    // Settle the old job, but reject a completion from a stopped/restarted mode.
    if !state.studio.playback.settle_pending(generation) {return;}
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
            accept_plan(&mut state.studio, snapshot, plan);
        }
        Err(message) => {
            stop(state);
            error(state, message);
        }
    }
    redraw(state);
}
fn accept_plan(view: &mut View, snapshot: ReadSnapshot, plan: Box<Preview>) {
    view.playback.random_state = plan.final_random_state();
    view.playback.plan = Some(plan);
    view.playback.snapshot = Some(snapshot);
    if !view.failed {
        view.status = if view.playback.synthetic_started.is_some() {"Synthetic Ambient diagnostic - reconstructed projection - 10s - no capture/output"} else {"Local canvas preview - no device output"}.into();
    }
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
    json!({"synthetic_ambient":p.synthetic_started.is_some(),"requested":p.requested,"pending":p.pending,"generation":p.generation,"current":p.snapshot.as_ref().is_some_and(|s|state.studio.draft.as_ref().is_some_and(|d|d.matches_snapshot(s))),"frames":p.plan.as_ref().map_or(0,|p|p.rendered_frames()),"effects":p.plan.as_ref().map_or(0,|p|p.effect_count()),"bytes":p.plan.as_ref().map_or(0,|p|p.storage_bytes()),"frame":p.plan.as_ref().map(|p|p.frame().iter().map(|c|[c.0,c.1,c.2]).collect::<Vec<_>>())})
}

#[cfg(test)]
mod ambient_lifecycle_tests {
    use super::*;
    #[test]
    fn completed_preview_preserves_newer_validation_error_and_exact_draft() {
        let raw = include_str!("../../../secbird-core/tests/fixtures/studio-preview-profile.json");
        let draft = Draft::open(raw).unwrap();
        let snapshot = draft.read_snapshot();
        let plan = Box::new(Preview::from_snapshot(&snapshot, 1).unwrap());
        let mut view = View { draft: Some(draft), failed: true,
            status: "Enter speed0..50, width100..400 and pause0..60000ms".into(), ..View::default() };
        let before = view.draft.as_ref().unwrap().json().to_owned();
        let history = view.draft.as_ref().unwrap().undo_len();
        let error = view.status.clone();
        accept_plan(&mut view, snapshot, plan);
        assert!(view.failed, "background success erased the validation error");
        assert_eq!(view.status, error);
        assert_eq!(view.draft.as_ref().unwrap().json(), before);
        assert_eq!(view.draft.as_ref().unwrap().undo_len(), history);
        assert!(view.playback.snapshot.as_ref().is_some_and(|s| view.draft.as_ref().unwrap().matches_snapshot(s)));
        assert!(view.playback.plan.is_some(), "valid canvas frames should still be accepted");
    }
    #[test]
    fn pending_synthetic_completion_cannot_escape_its_deadline_after_mode_restart() {
        // Stop during compile, then restart an ordinary preview on the same snapshot.
        let mut playback=Playback {generation:7,pending:true,pending_synthetic:true,synthetic_started:None,requested:true,..Default::default()};
        assert!(!playback.settle_pending(7));
        assert!(!playback.pending);
        // New ordinary compile remains pending when an older generation finishes.
        playback.generation=8;playback.pending=true;playback.pending_synthetic=false;
        assert!(!playback.settle_pending(7));assert!(playback.pending);
        assert!(playback.settle_pending(8));assert!(!playback.pending);
        // The reverse mode switch also refuses its old result.
        playback.synthetic_started=Some(std::time::Instant::now());playback.pending=true;
        assert!(!playback.settle_pending(8));
    }
}
