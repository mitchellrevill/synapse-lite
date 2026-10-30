//! Breathing-only playback authoring. Live input retains the shared admission gate.
use super::*;
use secbird_core::studio::document::{BreathingEdit, SelectedBreathing};

#[derive(Clone)]
pub enum Control {
    Page,
    Edit(BreathingEdit),
    Repeat(i64),
}
#[derive(Default)]
pub(super) struct Properties {
    layer: Option<String>,
    last_count: Option<i64>,
}
pub(super) fn refresh(view: &mut View) {
    if view.breathing.layer != view.selected {
        view.breathing = Properties {
            layer: view.selected.clone(),
            last_count: None,
        };
    }
}
pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Page) {
        state.studio.breathing_playback = !state.studio.breathing_playback;
        state.studio.breathing_gradient = false;
        selection_changed(state);
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        let id = view.selected.as_deref().ok_or("Select a Breathing layer")?;
        let selection = view.canvas.selection();
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let count = view.breathing.last_count.unwrap_or(1);
        let edit = match control {
            Control::Edit(BreathingEdit::Start(start)) => BreathingEdit::Playback {
                start: Some(start),
                end: if start == "random" { "never" } else { "after" },
                cycles: if start == "random" { -1 } else { count },
            },
            Control::Edit(BreathingEdit::End(end)) => BreathingEdit::Playback {
                start: None,
                end,
                cycles: if end == "after" { count } else { -1 },
            },
            Control::Edit(edit) => edit,
            Control::Repeat(delta) => {
                let SelectedBreathing::Uniform(settings) = draft
                    .breathing_selection_settings(id, &selection)
                    .map_err(|e| e.to_string())?
                else {
                    return Err("Select matching playback settings before changing repeats".into());
                };
                BreathingEdit::Cycles(if settings.cycles <= 0 {
                    1
                } else {
                    settings.cycles.saturating_add(delta).clamp(1, 100)
                })
            }
            Control::Page => unreachable!(),
        };
        let changed = draft
            .edit_breathing(id, &selection, edit)
            .map_err(|e| e.to_string())?;
        if let BreathingEdit::Cycles(count) = edit {
            if count > 0 {
                view.breathing.last_count = Some(count);
            }
        }
        Ok(changed)
    })();
    match result {
        Ok(_) => {
            view.status = "Breathing settings updated · save to keep changes".into();
            view.failed = false;
        }
        Err(message) => {
            view.status = message;
            view.failed = true;
        }
    }
    selection_changed(state);
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    let selected = view.draft.as_ref().and_then(|d| {
        d.breathing_selection_settings(view.selected.as_deref()?, &view.canvas.selection())
            .ok()
    });
    let settings = match &selected {
        Some(SelectedBreathing::Uniform(s)) => Some(s),
        _ => None,
    };
    let enabled = view.editable() && view.canvas.count() > 0;
    let mut choice = |label: &str, y: i32, col: i32, active: bool, edit: BreathingEdit| {
        let x = right + 10 + col * 114;
        button(
            p,
            label,
            x,
            y,
            106,
            super::Control::Breathing(Control::Edit(edit)),
            enabled,
        );
        if active {
            p.fill(x, y + 27, 106, 2, GREEN);
        }
    };
    choice(
        "FIXED",
        278,
        0,
        settings.is_some_and(|s| !s.random),
        BreathingEdit::Random(false),
    );
    choice(
        "RANDOM",
        278,
        1,
        settings.is_some_and(|s| s.random),
        BreathingEdit::Random(true),
    );
    choice(
        "RANDOM TIMES",
        347,
        0,
        settings.is_some_and(|s| s.start == "random"),
        BreathingEdit::Start("random"),
    );
    choice(
        "ON PRESS",
        347,
        1,
        settings.is_some_and(|s| s.start == "onPress"),
        BreathingEdit::Start("onPress"),
    );
    choice(
        "SELECTED KEYS",
        381,
        0,
        settings.is_some_and(|s| s.start == "onSelectedKeys"),
        BreathingEdit::Start("onSelectedKeys"),
    );
    for (index, (label, mode)) in [("NEVER", "never"), ("AFTER", "after"), ("TOGGLE", "toggle")]
        .into_iter()
        .enumerate()
    {
        let x = right + 10 + index as i32 * 76;
        let available =
            enabled && (mode != "toggle" || settings.is_some_and(|s| s.start != "random"));
        button(
            p,
            label,
            x,
            450,
            68,
            super::Control::Breathing(Control::Edit(BreathingEdit::End(mode))),
            available,
        );
        if settings.is_some_and(|s| s.end == mode) {
            p.fill(x, 477, 68, 2, GREEN);
        }
    }
    for (label, y) in [("COLOUR MODE", 246), ("START", 316), ("END", 419)] {
        p.text(label, right + 10, y, 230, 25, p.fonts.small, TEXT, DT_LEFT);
    }
    let repeats = match settings {
        Some(s) if s.cycles > 0 => format!("REPEAT · {}", s.cycles),
        Some(_) => "REPEAT · FOREVER".into(),
        None => "REPEAT · MIXED / SELECT LEDS".into(),
    };
    p.text(
        &repeats,
        right + 10,
        488,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for (label, x, width, control) in [
        (
            "FOREVER",
            right + 10,
            102,
            Control::Edit(BreathingEdit::Cycles(-1)),
        ),
        ("−", right + 120, 48, Control::Repeat(-1)),
        ("+", right + 176, 48, Control::Repeat(1)),
    ] {
        button(
            p,
            label,
            x,
            518,
            width,
            super::Control::Breathing(control),
            enabled,
        );
    }
    button(
        p,
        "RESET TO TWO COLOURS",
        right + 10,
        556,
        220,
        super::Control::Breathing(Control::Edit(BreathingEdit::ResetColors)),
        enabled,
    );
}
