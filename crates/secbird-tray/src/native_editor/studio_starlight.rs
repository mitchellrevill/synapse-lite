//! Native Starlight properties over retained source regional settings.
use super::*;
use secbird_core::studio::document::{
    SelectedStarlightValue, StarlightProperty, STARLIGHT_DURATIONS,
};
const DENSITY: u16 = 2108;
#[derive(Clone)]
pub enum Control {
    Duration(usize),
    Density,
    Random(bool),
    Gradient,
}
#[derive(Default)]
pub(super) struct Properties {
    pub(super) gradient_open: bool,
    density: String,
    duration: Option<SelectedStarlightValue>,
    random: Option<SelectedStarlightValue>,
}
pub(super) fn refresh(view: &mut View) {
    if view.effect() != Some("starlight") {
        view.starlight = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        view.starlight.duration =
            Some(draft.starlight_selection_value(id, &selection, StarlightProperty::Duration)?);
        view.starlight.random =
            Some(draft.starlight_selection_value(id, &selection, StarlightProperty::Random)?);
        view.starlight.density =
            match draft.starlight_selection_value(id, &selection, StarlightProperty::Density)? {
                SelectedStarlightValue::Uniform(Some(value)) => value.to_string(),
                _ => String::new(),
            };
        Ok(())
    })();
    if let Err(error) = result {
        view.status = error.to_string();
        view.failed = true;
    }
}
pub(super) fn gradient_enabled(view: &View) -> bool {
    matches!(
        view.starlight.random,
        Some(SelectedStarlightValue::Uniform(None | Some(0)))
    )
}
pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Gradient) {
        state.studio.starlight.gradient_open = !state.studio.starlight.gradient_open;
        spectrum::refresh(&mut state.studio);
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        if view.effect() != Some("starlight") {
            return Err("Select Starlight first".into());
        }
        let id = view.selected.as_deref().ok_or("Select a layer")?;
        let selection = view.canvas.selection();
        let (property, value) = match control {
            Control::Duration(index) => (
                StarlightProperty::Duration,
                *STARLIGHT_DURATIONS.get(index).ok_or("Unknown duration")?,
            ),
            Control::Density => (
                StarlightProperty::Density,
                view.starlight
                    .density
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| "Enter density1..10")?,
            ),
            Control::Random(value) => (StarlightProperty::Random, u32::from(value)),
            Control::Gradient => unreachable!(),
        };
        view.draft
            .as_mut()
            .ok_or("Open a composition first")?
            .set_starlight_property(id, &selection, property, value)
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status = "Starlight updated; save to keep changes".into();
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
    if state.studio.effect() != Some("starlight") || state.studio.starlight.gradient_open {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    let text = state.studio.starlight.density.clone();
    state.retained_fields.remove(&DENSITY);
    state.field(DENSITY, &text, right + 10, 360, 220);
    if let Some(&field) = state.fields.get(&DENSITY) {
        SendMessageW(field, EM_SETLIMITTEXT, 2, 0);
        SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !state.studio.open || state.studio.effect()!=Some("starlight") {return false;}
    if id != DENSITY || notification != EN_CHANGE as u16 {
        return false;
    }
    state.studio.starlight.density = state.raw_text(id);
    true
}
pub(super) fn has_focus(state: &State) -> bool {
    state
        .fields
        .get(&DENSITY)
        .is_some_and(|&field| unsafe { field == GetFocus() })
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    if view.starlight.gradient_open {
        spectrum::paint(p, view, right);
        return;
    }
    let enabled = view.editable() && view.canvas.count() > 0;
    let props = &view.starlight;
    p.text(
        if props.duration == Some(SelectedStarlightValue::Mixed) {
            "DURATION · MIXED"
        } else {
            "DURATION"
        },
        right + 10,
        245,
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
            278,
            70,
            super::Control::Starlight(Control::Duration(index)),
            enabled,
        );
        if props.duration
            == Some(SelectedStarlightValue::Uniform(Some(
                STARLIGHT_DURATIONS[index],
            )))
        {
            p.fill(x, 305, 70, 2, GREEN);
        }
    }
    p.text(
        "DENSITY 1–10",
        right + 10,
        330,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY DENSITY",
        right + 10,
        400,
        220,
        super::Control::Starlight(Control::Density),
        enabled,
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
            445,
            105,
            super::Control::Starlight(Control::Random(value)),
            enabled,
        );
        if props.random == Some(SelectedStarlightValue::Uniform(Some(u32::from(value))))
            || (!value && props.random == Some(SelectedStarlightValue::Uniform(None)))
        {
            p.fill(x, 472, 105, 2, GREEN);
        }
    }
    button(
        p,
        "EDIT GRADIENT",
        right + 10,
        490,
        220,
        super::Control::Starlight(Control::Gradient),
        true,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        530,
        220,
        super::Control::Erase,
        enabled,
    );
}
