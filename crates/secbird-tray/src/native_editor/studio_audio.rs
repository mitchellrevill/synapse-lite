//! Audio Meter authoring never starts capture or device output.
use super::*;
use secbird_core::studio::document::{AudioProperty, SelectedAudioValue};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
const BOOST: u16 = 2120;
const DECAY: u16 = 2121;

#[derive(Clone)]
pub enum Control {
    Boost,
    Decay,
    Auto(bool),
    Gradient,
}
#[derive(Default)]
pub(super) struct Properties {
    pub(super) gradient_open: bool,
    boost: String,
    decay: String,
    boost_mixed: bool,
    decay_mixed: bool,
    auto: Option<SelectedAudioValue>,
}
pub(super) fn refresh(view: &mut View) {
    if view.effect() != Some("audio") {
        view.audio = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        view.audio.auto =
            Some(draft.audio_selection_value(id, &selection, AudioProperty::AutoBoost)?);
        for (property, text, mixed) in [
            (
                AudioProperty::Boost,
                &mut view.audio.boost,
                &mut view.audio.boost_mixed,
            ),
            (
                AudioProperty::Decay,
                &mut view.audio.decay,
                &mut view.audio.decay_mixed,
            ),
        ] {
            let value = draft.audio_selection_value(id, &selection, property)?;
            *mixed = value == SelectedAudioValue::Mixed;
            *text = match value {
                SelectedAudioValue::Uniform(Some(value)) => value.to_string(),
                _ => String::new(),
            };
        }
        Ok(())
    })();
    if let Err(error) = result {
        view.status = error.to_string();
        view.failed = true;
    }
}
fn boost_enabled(view: &View) -> bool {
    matches!(
        view.audio.auto,
        Some(SelectedAudioValue::Uniform(None | Some(0.0)))
    )
}
pub(super) fn action(state: &mut State, control: Control) {
    let view = &mut state.studio;
    if view.effect() != Some("audio") {
        return;
    }
    if matches!(control, Control::Gradient) {
        view.audio.gradient_open = !view.audio.gradient_open;
        spectrum::refresh(view);
        return;
    }
    if !view.editable() {
        return;
    }
    let result = (|| -> Result<bool, String> {
        let (property, value) = match control {
            Control::Boost => {
                if !boost_enabled(view) {
                    return Err("Turn off Auto before editing Boost".into());
                }
                (
                    AudioProperty::Boost,
                    view.audio
                        .boost
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "Enter Boost 0.25 to 4 in steps of 0.25")?,
                )
            }
            Control::Decay => (
                AudioProperty::Decay,
                view.audio
                    .decay
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| "Enter Decay 0.1 to 2 seconds in steps of 0.1")?,
            ),
            Control::Auto(value) => (AudioProperty::AutoBoost, if value { 1.0 } else { 0.0 }),
            Control::Gradient => unreachable!(),
        };
        view.draft
            .as_mut()
            .ok_or("Open a composition first")?
            .set_audio_property(
                view.selected.as_deref().ok_or("Select Audio Meter")?,
                &view.canvas.selection(),
                property,
                value,
            )
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status =
                    "Audio Meter updated; save to keep changes. Audio playback is not implemented."
                        .into();
            }
            view.failed = false;
        }
        Err(error) => {
            view.status = error;
            view.failed = true;
        }
    }
    refresh(view);
    spectrum::refresh(view);
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.studio.effect() != Some("audio") || state.studio.audio.gradient_open {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    for (id, text, y, enabled) in [
        (
            BOOST,
            state.studio.audio.boost.clone(),
            278,
            boost_enabled(&state.studio),
        ),
        (DECAY, state.studio.audio.decay.clone(), 425, true),
    ] {
        state.retained_fields.remove(&id);
        state.field(id, &text, right + 10, y, 220);
        if let Some(&field) = state.fields.get(&id) {
            SendMessageW(field, EM_SETLIMITTEXT, 12, 0);
            SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
            EnableWindow(
                field,
                i32::from(enabled && state.studio.editable() && state.studio.canvas.count() > 0),
            );
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !matches!(id, BOOST | DECAY) || notification != EN_CHANGE as u16 {
        return false;
    }
    let value = state.raw_text(id);
    if id == BOOST {
        state.studio.audio.boost = value;
    } else {
        state.studio.audio.decay = value;
    }
    true
}
pub(super) fn enter(state: &State) -> Option<Control> {
    let focus = unsafe { GetFocus() };
    if state.fields.get(&BOOST).copied() == Some(focus) {
        Some(Control::Boost)
    } else if state.fields.get(&DECAY).copied() == Some(focus) {
        Some(Control::Decay)
    } else {
        None
    }
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    if view.audio.gradient_open {
        spectrum::paint(p, view, right);
        return;
    }
    let enabled = view.editable() && view.canvas.count() > 0;
    p.text(
        if view.audio.auto == Some(SelectedAudioValue::Mixed) {
            "BOOST / AUTO · MIXED"
        } else if view.audio.boost_mixed {
            "BOOST · MIXED"
        } else {
            "BOOST 0.25–4 · STEP 0.25"
        },
        right + 10,
        245,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY BOOST",
        right + 10,
        315,
        220,
        super::Control::Audio(Control::Boost),
        enabled && boost_enabled(view),
    );
    for (index, (label, value)) in [("AUTO ON", true), ("AUTO OFF", false)]
        .into_iter()
        .enumerate()
    {
        let x = right + 10 + index as i32 * 115;
        button(
            p,
            label,
            x,
            355,
            105,
            super::Control::Audio(Control::Auto(value)),
            enabled,
        );
        if view.audio.auto
            == Some(SelectedAudioValue::Uniform(Some(if value {
                1.0
            } else {
                0.0
            })))
        {
            p.fill(x, 382, 105, 2, GREEN);
        }
    }
    p.text(
        if view.audio.decay_mixed {
            "DECAY · MIXED"
        } else {
            "DECAY 0.1–2s · STEP 0.1"
        },
        right + 10,
        395,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY DECAY",
        right + 10,
        462,
        220,
        super::Control::Audio(Control::Decay),
        enabled,
    );
    button(
        p,
        "EDIT GRADIENT · LOW / HIGH",
        right + 10,
        502,
        220,
        super::Control::Audio(Control::Gradient),
        true,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        542,
        220,
        super::Control::Erase,
        enabled,
    );
}
