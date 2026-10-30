//! Native Wave authoring over the source regional parameter contract.
use super::*;
use secbird_core::studio::document::{SelectedWaveValue, WaveProperty};
pub(super) const SPEED: u16 = 2105;
pub(super) const WIDTH: u16 = 2106;
pub(super) const PAUSE: u16 = 2107;
#[derive(Clone)]
pub enum Control {
    Direction(u32),
    Preset(usize),
    Motion,
    Gradient,
}
#[derive(Default)]
pub(super) struct Properties {
    pub(super) gradient_open: bool,
    values: [String; 3],
    angle: Option<SelectedWaveValue>,
    mixed: [bool; 3],
}
pub(super) fn refresh(view: &mut View) {
    let gradient_open = view.wave.gradient_open && view.effect() == Some("wave");
    view.wave = Properties {
        gradient_open,
        ..Properties::default()
    };
    if view.effect() != Some("wave") {
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        view.wave.angle = Some(draft.wave_selection_value(id, &selection, WaveProperty::Angle)?);
        for (index, property) in [
            WaveProperty::Speed,
            WaveProperty::Width,
            WaveProperty::Pause,
        ]
        .into_iter()
        .enumerate()
        {
            let value = draft.wave_selection_value(id, &selection, property)?;
            view.wave.mixed[index] = value == SelectedWaveValue::Mixed;
            view.wave.values[index] = match value {
                SelectedWaveValue::Uniform(Some(value)) => value.to_string(),
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
pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Gradient) {
        state.studio.wave.gradient_open = !state.studio.wave.gradient_open;
        spectrum::refresh(&mut state.studio);
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        if view.effect() != Some("wave") {
            return Err("Select Wave first".into());
        }
        let id = view.selected.as_deref().ok_or("Select a Wave layer")?;
        let selection = view.canvas.selection();
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let result = match control {
            Control::Gradient => unreachable!(),
            Control::Direction(angle) => {
                draft.set_wave_property(id, &selection, WaveProperty::Angle, angle)
            }
            Control::Preset(index) => draft.set_wave_preset(id, &selection, index),
            Control::Motion => {
                let mut values = [0; 3];
                for (index, text) in view.wave.values.iter().enumerate() {
                    values[index] = text
                        .trim()
                        .parse::<u32>()
                        .map_err(|_| "Enter speed0..50, width100..400 and pause0..60000ms")?;
                }
                draft.set_wave_motion(id, &selection, values[0], values[1], values[2])
            }
        };
        result.map_err(|error| error.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status = "Wave updated for selected LEDs; save to keep changes".into();
            }
            view.failed = false;
            refresh(view);
        }
        Err(error) => {
            view.status = error;
            view.failed = true;
        }
    }
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.studio.effect() != Some("wave") || state.studio.wave.gradient_open {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    for (index, id) in [SPEED, WIDTH, PAUSE].into_iter().enumerate() {
        let text = state.studio.wave.values[index].clone();
        state.retained_fields.remove(&id);
        state.field(id, &text, right + 10 + index as i32 * 77, 427, 68);
        if let Some(&field) = state.fields.get(&id) {
            SendMessageW(field, EM_SETLIMITTEXT, 5, 0);
            SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !state.studio.open || state.studio.effect()!=Some("wave") {return false;}
    let Some(index) = [SPEED, WIDTH, PAUSE].iter().position(|&field| field == id) else {
        return false;
    };
    if notification != EN_CHANGE as u16 {
        return false;
    }
    state.studio.wave.values[index] = state.raw_text(id);
    true
}
pub(super) fn has_focus(state: &State) -> bool {
    let focus = unsafe { GetFocus() };
    [SPEED, WIDTH, PAUSE]
        .iter()
        .any(|id| state.fields.get(id) == Some(&focus))
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    if view.wave.gradient_open {
        spectrum::paint(p, view, right);
        return;
    }
    let enabled = view.editable() && view.canvas.count() > 0;
    p.text(
        "DIRECTION",
        right + 10,
        245,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for (index, (label, angle)) in [("UP", 0), ("RIGHT", 90), ("DOWN", 180), ("LEFT", 270)]
        .into_iter()
        .enumerate()
    {
        let x = right + 10 + index as i32 * 58;
        button(
            p,
            label,
            x,
            277,
            53,
            super::Control::Wave(Control::Direction(angle)),
            enabled,
        );
        if view.wave.angle == Some(SelectedWaveValue::Uniform(Some(angle))) {
            p.fill(x, 304, 53, 2, GREEN);
        }
    }
    p.text(
        "GRADIENT PRESETS",
        right + 10,
        316,
        230,
        28,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for index in 0..5 {
        button(
            p,
            &(index + 1).to_string(),
            right + 10 + index as i32 * 46,
            347,
            40,
            super::Control::Wave(Control::Preset(index)),
            enabled,
        );
    }
    for (index, label) in ["SPEED", "WIDTH %", "PAUSE MS"].into_iter().enumerate() {
        p.text(
            label,
            right + 10 + index as i32 * 77,
            398,
            74,
            25,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        if view.wave.mixed[index] {
            p.text(
                "MIXED",
                right + 10 + index as i32 * 77,
                456,
                74,
                22,
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
        }
    }
    button(
        p,
        "APPLY MOTION",
        right + 10,
        487,
        220,
        super::Control::Wave(Control::Motion),
        enabled,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        527,
        220,
        super::Control::Erase,
        enabled,
    );
    button(
        p,
        "EDIT GRADIENT",
        right + 10,
        567,
        220,
        super::Control::Wave(Control::Gradient),
        true,
    );
}
