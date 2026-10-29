//! Native Reactive settings; no input capture or lighting output is started.
use super::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use secbird_core::studio::document::{ReactiveProperty, SelectedReactiveValue, REACTIVE_DURATIONS};
const COLOR: u16 = 2109;
#[derive(Clone)]
pub enum Control {
    Duration(usize),
    Random(bool),
    Color(bool),
}
#[derive(Default)]
pub(super) struct Properties {
    color: String,
    selected_color: Option<SelectedReactiveValue>,
    duration: Option<SelectedReactiveValue>,
    random: Option<SelectedReactiveValue>,
}
pub(super) fn refresh(view: &mut View) {
    if view.effect() != Some("reactive") {
        view.reactive = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        view.reactive.duration =
            Some(draft.reactive_selection_value(id, &selection, ReactiveProperty::Duration)?);
        view.reactive.random =
            Some(draft.reactive_selection_value(id, &selection, ReactiveProperty::Random)?);
        let value = draft.reactive_selection_value(id, &selection, ReactiveProperty::Color)?;
        view.reactive.color = match value {
            SelectedReactiveValue::Uniform(Some(value)) => format!("{value:06X}"),
            _ => String::new(),
        };
        view.reactive.selected_color = Some(value);
        Ok(())
    })();
    if let Err(error) = result {
        view.status = error.to_string();
        view.failed = true;
    }
}
fn color_enabled(view: &View) -> bool {
    matches!(
        view.reactive.random,
        Some(SelectedReactiveValue::Uniform(None | Some(0)))
    )
}
pub(super) fn action(state: &mut State, control: Control) {
    let view = &mut state.studio;
    if !view.editable() || view.effect() != Some("reactive") {
        return;
    }
    let result = (|| -> Result<bool, String> {
        let id = view.selected.as_deref().ok_or("Select Reactive first")?;
        let selection = view.canvas.selection();
        let (property, value) = match control {
            Control::Duration(index) => (
                ReactiveProperty::Duration,
                Some(*REACTIVE_DURATIONS.get(index).ok_or("Unknown duration")?),
            ),
            Control::Random(value) => (ReactiveProperty::Random, Some(u32::from(value))),
            Control::Color(clear) => {
                if !color_enabled(view) {
                    return Err("Turn off Random Colour first".into());
                }
                let value = if clear {
                    None
                } else {
                    let text = view.reactive.color.trim().trim_start_matches('#');
                    if text.len() != 6 {
                        return Err("Enter six hexadecimal colour digits".into());
                    }
                    Some(
                        u32::from_str_radix(text, 16)
                            .map_err(|_| "Enter six hexadecimal colour digits")?,
                    )
                };
                (ReactiveProperty::Color, value)
            }
        };
        view.draft
            .as_mut()
            .ok_or("Open a composition first")?
            .set_reactive_property(id, &selection, property, value)
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status="Reactive updated; save to keep changes. Live key-triggered playback is not implemented.".into();
            }
            view.failed = false;
        }
        Err(error) => {
            view.status = error;
            view.failed = true;
        }
    }
    refresh(view);
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.studio.effect() != Some("reactive") {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    let text = state.studio.reactive.color.clone();
    state.retained_fields.remove(&COLOR);
    state.field(COLOR, &text, right + 10, 278, 220);
    if let Some(&field) = state.fields.get(&COLOR) {
        SendMessageW(field, EM_SETLIMITTEXT, 7, 0);
        SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
        EnableWindow(
            field,
            i32::from(
                state.studio.editable()
                    && state.studio.canvas.count() > 0
                    && color_enabled(&state.studio),
            ),
        );
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if id != COLOR || notification != EN_CHANGE as u16 {
        return false;
    }
    state.studio.reactive.color = state.raw_text(id);
    true
}
pub(super) fn has_focus(state: &State) -> bool {
    state
        .fields
        .get(&COLOR)
        .is_some_and(|&field| unsafe { field == GetFocus() })
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    let enabled = view.editable() && view.canvas.count() > 0;
    let props = &view.reactive;
    p.text(
        if props.selected_color == Some(SelectedReactiveValue::Mixed) {
            "COLOUR · MIXED"
        } else {
            "COLOUR"
        },
        right + 10,
        245,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    if let Some(SelectedReactiveValue::Uniform(Some(color))) = props.selected_color {
        if color <= 0xffffff {
            p.fill(right + 10, 310, 220, 3, color);
        }
    }
    button(
        p,
        "APPLY COLOUR",
        right + 10,
        325,
        130,
        super::Control::Reactive(Control::Color(false)),
        enabled && color_enabled(view),
    );
    button(
        p,
        "NONE",
        right + 150,
        325,
        80,
        super::Control::Reactive(Control::Color(true)),
        enabled && color_enabled(view),
    );
    for (index, (label, value)) in [("RANDOM ON", true), ("RANDOM OFF", false)]
        .into_iter()
        .enumerate()
    {
        let x = right + 10 + index as i32 * 115;
        button(
            p,
            label,
            x,
            370,
            105,
            super::Control::Reactive(Control::Random(value)),
            enabled,
        );
        if props.random == Some(SelectedReactiveValue::Uniform(Some(u32::from(value))))
            || (!value && props.random == Some(SelectedReactiveValue::Uniform(None)))
        {
            p.fill(x, 397, 105, 2, GREEN);
        }
    }
    p.text(
        if props.duration == Some(SelectedReactiveValue::Mixed) {
            "DURATION · MIXED"
        } else {
            "DURATION"
        },
        right + 10,
        415,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for (index, label) in ["SLOW", "NORMAL", "FAST"].into_iter().enumerate() {
        let x = right + 10 + index as i32 * 76;
        button(
            p,
            label,
            x,
            448,
            70,
            super::Control::Reactive(Control::Duration(index)),
            enabled,
        );
        if props.duration
            == Some(SelectedReactiveValue::Uniform(Some(
                REACTIVE_DURATIONS[index],
            )))
        {
            p.fill(x, 475, 70, 2, GREEN);
        }
    }
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        500,
        220,
        super::Control::Erase,
        enabled,
    );
}
