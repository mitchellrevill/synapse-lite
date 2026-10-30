//! Retained random-range editing. Sampling/playback remains a separate boundary.
use super::*;
use library::random_delay::{self as codec, Range};
const MIN: u16 = 2320;
const MAX: u16 = 2321;

fn row(view: &View) -> Option<&Value> {
    view.draft.as_ref()?.entry.events.get(view.event?)
}
pub(super) fn selected(view: &View) -> bool {
    row(view).is_some_and(|row| row["Type"] == 0 && row["Number"].is_object())
}
fn range(view: &View) -> Option<Range> {
    codec::from_row(row(view)?).ok().flatten()
}
fn entered(view: &View) -> Result<Range, String> {
    let read = |value: &str| {
        value
            .trim()
            .parse::<u32>()
            .map_err(|_| "Enter whole milliseconds from 0 to 5000".to_string())
    };
    Range::new(read(&view.range_min)?, read(&view.range_max)?).map_err(|e| e.to_string())
}
pub(super) fn sync(view: &mut View) {
    let current = range(view);
    view.range_min = current.map(|r| r.min_ms.to_string()).unwrap_or_default();
    view.range_max = current.map(|r| r.max_ms.to_string()).unwrap_or_default();
}
pub(super) fn dirty(view: &View) -> bool {
    range(view).is_some_and(|current| entered(view).ok() != Some(current))
}
pub(super) fn editable(view: &View) -> bool {
    range(view).is_some()
}
pub(super) fn commit(state: &mut State) -> Result<(), String> {
    let Some(current) = range(&state.macros) else {
        return Ok(());
    };
    let next = entered(&state.macros)?;
    if next != current {
        let view = &state.macros;
        let guid = view.draft.as_ref().unwrap().entry.guid.clone();
        let index = view.event.unwrap();
        local(
            state,
            Change::SetRandomDelay {
                guid,
                index,
                min_ms: next.min_ms,
                max_ms: next.max_ms,
            },
        )?;
    }
    Ok(())
}
pub(super) unsafe fn fields(state: &mut State, left: i32, top: i32) -> bool {
    if !selected(&state.macros) {
        return false;
    }
    let readonly = state.read_only_smoke
        || state.restricted_actions
        || state.macros.busy
        || state.macros.playing()
        || state.macros.recording.is_some()
        || !editable(&state.macros);
    for (id, text, x) in [
        (MIN, state.macros.range_min.clone(), left + 10),
        (MAX, state.macros.range_max.clone(), left + 105),
    ] {
        state.retained_fields.remove(&id);
        state.field(id, &text, x, top, 85);
        if let Some(handle) = state.fields.get(&id) {
            SendMessageW(*handle, EM_SETLIMITTEXT, 4, 0);
            SendMessageW(*handle, EM_SETREADONLY, readonly as usize, 0);
        }
    }
    true
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if notification != EN_CHANGE as u16 || !matches!(id, MIN | MAX) || !selected(&state.macros) {
        return false;
    }
    if state.read_only_smoke
        || state.restricted_actions
        || state.macros.busy
        || state.macros.playing()
        || state.macros.recording.is_some()
        || !editable(&state.macros)
    {
        return true;
    }
    let value = state.raw_text(id);
    if id == MIN {
        state.macros.range_min = value;
    } else {
        state.macros.range_max = value;
    }
    unsafe {
        InvalidateRect(state.hwnd, null(), 0);
    }
    true
}
pub(super) fn label(row: &Value) -> Option<String> {
    if row["Type"] != 0 || !row["Number"].is_object() {
        return None;
    }
    let bound = |key: &str| {
        let value = &row["Number"][key];
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
            .chars()
            .take(32)
            .collect::<String>()
    };
    Some(format!(
        "Random delay · {}–{} seconds{}",
        bound("min"),
        bound("max"),
        if codec::from_row(row).ok().flatten().is_none() {
            " · retained (not editable)"
        } else {
            ""
        }
    ))
}
