use super::*;
use crate::host_macro_worker as worker;
pub(in crate::native_editor) const TIMER: usize = 0x4d41;

pub(super) unsafe fn start(state: &mut State) -> Result<(), String> {
    if !active(state)
        || state.macros.busy
        || state.macros.playing()
        || state.macros.recording.is_some()
        || state.read_only_smoke
        || state.restricted_actions
    {
        return Err("Macro playback is unavailable in this editor state".into());
    }
    if state.macros.dirty() || state.macros.form.is_some() {
        return Err("Save or discard the macro draft before playing".into());
    }
    let entry = &state.macros.draft.as_ref().ok_or("Select a macro")?.entry;
    let request = worker::Request {
        path: secbird_core::config_dir().join("profiles.json"),
        guid: entry.guid.clone(),
        revision: entry.revision.clone(),
        read_only: false,
    };
    //Install observation before admission, so a timer failure cannot orphan a run.
    if SetTimer(state.hwnd, TIMER, 40, None) == 0 {
        return Err("Could not monitor macro playback".into());
    }
    match worker::start(request) {
        Ok(session) => {
            state.macros.playback = Some(session);
            state.macros.playback_report = None;
            state.macros.playback_snapshot = None;
            state.macros.playback_next = None;
            state.macros.status = "Preparing macro…".into();
            state.macros.failed = false;
            state.macros.picker = false;
            state.rebuild_fields();
            Ok(())
        }
        Err(error) => {
            KillTimer(state.hwnd, TIMER);
            Err(error)
        }
    }
}
pub(super) unsafe fn stop(state: &mut State) {
    if let Some(session) = &state.macros.playback {
        session.stop();
        state.macros.status = "Stopping… Waiting for playback to finish.".into();
        state.macros.failed = false;
        InvalidateRect(state.hwnd, null(), 0);
    }
}
pub(in crate::native_editor) unsafe fn transition(state: &mut State, next: &Action) -> bool {
    if !state.macros.playing() {
        return false;
    }
    remember_transition(&mut state.macros.playback_next, next);
    stop(state);
    true
}
fn remember_transition(pending: &mut Option<Box<Action>>, next: &Action) {
    // A requested window/tray exit must survive clicks while Stop drains.
    // Navigation may change destination until that terminal intent is queued.
    if !matches!(pending.as_deref(), Some(Action::Close)) {
        *pending = Some(Box::new(next.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_survives_navigation_while_macro_drains() {
        let mut pending = None;
        remember_transition(&mut pending, &Action::Device("first".into()));
        remember_transition(&mut pending, &Action::Home);
        assert!(matches!(pending.as_deref(), Some(Action::Home)));
        remember_transition(&mut pending, &Action::Close);
        for next in [
            Action::Home,
            Action::Refresh,
            Action::Device("second".into()),
            Action::Close,
        ] {
            remember_transition(&mut pending, &next);
            assert!(
                matches!(pending.as_deref(), Some(Action::Close)),
                "Closing must survive later queued navigation"
            );
        }
    }
}
pub(in crate::native_editor) unsafe fn block_close(state: &mut State) -> bool {
    transition(state, &Action::Close)
}
pub(in crate::native_editor) unsafe fn destroy(state: &mut State) {
    if let Some(session) = &state.macros.playback {
        session.stop();
    }
    KillTimer(state.hwnd, TIMER);
}
pub(in crate::native_editor) unsafe fn tick(state: &mut State) {
    let Some(session) = &state.macros.playback else {
        KillTimer(state.hwnd, TIMER);
        return;
    };
    if state.read_only_smoke || state.restricted_actions {
        session.stop();
    }
    match worker::poll(session) {
        worker::Poll::Pending => return,
        worker::Poll::Running(snapshot) => {
            //Long delays need observation, not a full window repaint at25fps.
            if state.macros.playback_snapshot.as_ref() == Some(&snapshot) {
                return;
            }
            state.macros.status = if snapshot.stopping {
                "Stopping… Waiting for the current launch to return.".into()
            } else if snapshot.total == 0 {
                "Preparing macro…".into()
            } else {
                format!(
                    "Playing… {} {} accepted.",
                    snapshot.launches,
                    if snapshot.launches == 1 {
                        "launch"
                    } else {
                        "launches"
                    }
                )
            };
            state.macros.playback_snapshot = Some(snapshot);
        }
        result => {
            let report = match result {
                worker::Poll::Finished(report) => report,
                _ => worker::Report {
                    outcome: worker::Outcome::Failed(
                        "The playback session is unavailable; a launch may already have happened."
                            .into(),
                    ),
                    launches: 0,
                    completed: 0,
                    uncertain: true,
                },
            };
            state.macros.status = match &report.outcome {
                worker::Outcome::Finished => format!(
                    "Playback finished. {} {} accepted.",
                    report.launches,
                    if report.launches == 1 {
                        "launch"
                    } else {
                        "launches"
                    }
                ),
                worker::Outcome::Stopped => format!(
                    "Playback stopped. {} {} had already been accepted.",
                    report.launches,
                    if report.launches == 1 {
                        "launch"
                    } else {
                        "launches"
                    }
                ),
                worker::Outcome::Failed(error) => {
                    if report.launches == 0 {
                        error.clone()
                    } else {
                        format!(
                            "{error} {} earlier {} had already been accepted.",
                            report.launches,
                            if report.launches == 1 {
                                "launch"
                            } else {
                                "launches"
                            }
                        )
                    }
                }
            };
            state.macros.failed = matches!(report.outcome, worker::Outcome::Failed(_));
            state.macros.playback_report = Some(report);
            state.macros.playback_snapshot = None;
            state.macros.playback = None;
            KillTimer(state.hwnd, TIMER);
            state.rebuild_fields();
            let next = state.macros.playback_next.take();
            if let Some(next) = next {
                state.action(*next);
            }
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) fn diagnostics(view: &View) -> Value {
    json!({"active":view.playing(),"snapshot":view.playback_snapshot.as_ref().map(|s|json!({"total":s.total,"completed":s.completed,"source_row":s.source_row,"launches":s.launches,"stopping":s.stopping})),
        "report":view.playback_report.as_ref().map(|r|json!({"outcome":match r.outcome{worker::Outcome::Finished=>"finished",worker::Outcome::Stopped=>"stopped",worker::Outcome::Failed(_)=>"failed"},"launches":r.launches,"completed":r.completed,"uncertain":r.uncertain})),"transition_pending":view.playback_next.is_some()})
}
