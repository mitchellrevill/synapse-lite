//! Interim native Spectrum property controls over the source regional contract.
use super::*;
use secbird_core::studio::document::{
    GradientStop, SelectedDuration, SelectedGradient, StopEdit, SPECTRUM_DURATIONS,
};

#[derive(Clone)]
pub enum Control {
    Duration(usize),
    Preset(usize),
    Stop(i32),
    ApplyStop(bool),
    Add,
    Delete,
}
#[derive(Default)]
pub(super) struct Properties {
    duration: Option<SelectedDuration>,
    gradient: Option<SelectedGradient>,
    index: usize,
    position: String,
    color: String,
}
impl Properties {
    fn stops(&self) -> Option<&[GradientStop]> {
        match &self.gradient {
            Some(SelectedGradient::Uniform(Some(stops))) => Some(stops),
            _ => None,
        }
    }
    fn field_values(&mut self) {
        let Some(stops) = self.stops() else {
            self.position.clear();
            self.color.clear();
            self.index = 0;
            return;
        };
        if stops.is_empty() {
            self.position.clear();
            self.color.clear();
            self.index = 0;
            return;
        }
        let index = self.index.min(stops.len() - 1);
        let stop = stops[index].clone();
        self.index = index;
        self.position = stop.position.to_string();
        self.color = stop.color.map(|c| format!("{c:06X}")).unwrap_or_default();
    }
}
pub(super) fn refresh(view: &mut View) {
    if !matches!(view.effect(), Some("spectrum" | "wave")) {
        view.spectrum = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let wave = view.effect() == Some("wave");
    if !wave {
        match draft.spectrum_selection_duration(id, &selection) {
            Ok(value) => view.spectrum.duration = Some(value),
            Err(error) => {
                view.spectrum.duration = None;
                view.status = error.to_string();
                view.failed = true;
            }
        }
    } else {
        view.spectrum.duration = None;
    }
    match if wave {
        draft.wave_selection_gradient(id, &selection)
    } else {
        draft.spectrum_selection_gradient(id, &selection)
    } {
        Ok(value) => view.spectrum.gradient = Some(value),
        Err(error) => {
            view.spectrum.gradient = None;
            view.status = error.to_string();
            view.failed = true;
        }
    }
    view.spectrum.field_values();
}

// Recovered module6257.be data; different from Wave's seven-stop palette.
fn preset(index: usize) -> Option<Vec<GradientStop>> {
    let rows: &[(u8, u32)] = match index {
        0 => &[
            (0, 0xff18ff),
            (18, 0xff1010),
            (28, 0xff8000),
            (35, 0xefff00),
            (42, 0xdfff00),
            (49, 0x00ff00),
            (52, 0x00ffff),
            (64, 0x1858ff),
            (77, 0x1858ff),
            (100, 0xff18ff),
        ],
        1 => &[
            (0, 0x00ff00),
            (16, 0x001400),
            (32, 0x00ff00),
            (48, 0x001400),
            (64, 0x00ff00),
            (80, 0x001400),
            (100, 0x00ff00),
        ],
        2 => &[
            (0, 0x00ffff),
            (27, 0xffffff),
            (39, 0x00ffff),
            (61, 0x0078ff),
            (73, 0xffffff),
            (100, 0x00ffff),
        ],
        3 => &[
            (0, 0xff18ff),
            (25, 0x1858ff),
            (50, 0x00ffff),
            (75, 0x1858ff),
            (100, 0xff18ff),
        ],
        4 => &[(0, 0xff0000), (50, 0xefff00), (100, 0xff0000)],
        _ => return None,
    };
    Some(
        rows.iter()
            .map(|&(position, color)| GradientStop {
                position,
                color: Some(color),
            })
            .collect(),
    )
}
fn entered_stop(properties: &Properties, none: bool) -> Result<(u8, Option<u32>), String> {
    let position = properties
        .position
        .trim()
        .parse::<u8>()
        .map_err(|_| "Enter a position from 0 to 100 percent")?;
    let text = properties.color.trim().trim_start_matches('#');
    let color = if none {
        None
    } else {
        if text.len() != 6 {
            return Err("Enter a six-digit RGB colour or choose None".into());
        }
        Some(u32::from_str_radix(text, 16).map_err(|_| "Enter a six-digit RGB colour")?)
    };
    Ok((position, color))
}
pub(super) fn action(state: &mut State, control: Control) {
    if let Control::Stop(delta) = control {
        let properties = &mut state.studio.spectrum;
        let count = properties.stops().map_or(0, <[GradientStop]>::len);
        properties.index =
            (properties.index as i32 + delta).clamp(0, count.saturating_sub(1) as i32) as usize;
        properties.field_values();
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        if !matches!(view.effect(), Some("spectrum" | "wave")) {
            return Err("Select Spectrum or Wave first".into());
        }
        let wave = view.effect() == Some("wave");
        let id = view.selected.as_deref().ok_or("Select a layer")?;
        let selection = view.canvas.selection();
        let properties = &view.spectrum;
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let mut edit_stop = |edit| {
            if wave {
                draft.edit_wave_stop(id, &selection, edit)
            } else {
                draft.edit_spectrum_stop(id, &selection, edit)
            }
        };
        let result = match control {
            Control::Duration(_) if wave => return Err("Wave has no Spectrum duration".into()),
            Control::Duration(index) => draft.set_spectrum_duration(
                id,
                &selection,
                *SPECTRUM_DURATIONS.get(index).ok_or("Unknown duration")?,
            ),
            Control::Preset(index) if wave => draft.set_wave_preset(id, &selection, index),
            Control::Preset(index) => draft.set_spectrum_gradient(
                id,
                &selection,
                &preset(index).ok_or("Unknown preset")?,
                false,
            ),
            Control::ApplyStop(none) => {
                let (position, color) = entered_stop(properties, none)?;
                edit_stop(StopEdit::Set {
                    index: properties.index,
                    position,
                    color,
                })
            }
            Control::Add => {
                let (position, color) =
                    entered_stop(properties, properties.color.trim().is_empty())?;
                edit_stop(StopEdit::Insert { position, color })
            }
            Control::Delete => edit_stop(StopEdit::Delete {
                index: properties.index,
            }),
            Control::Stop(_) => unreachable!(),
        };
        result.map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status = "Gradient updated · save to keep changes".into();
            }
            view.failed = false;
        }
        Err(message) => {
            view.status = message;
            view.failed = true;
        }
    }
    refresh(view);
}
pub(super) unsafe fn fields(state: &mut State) {
    if !(state.studio.effect() == Some("spectrum")
        || (state.studio.effect() == Some("wave") && state.studio.wave.gradient_open))
        || state.studio.spectrum.stops().is_none_or(|s| s.is_empty())
    {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    for (id, text, x, y, width, limit) in [
        (
            POSITION,
            state.studio.spectrum.position.clone(),
            right + 10,
            440,
            65,
            3,
        ),
        (
            STOP_COLOR,
            state.studio.spectrum.color.clone(),
            right + 90,
            440,
            130,
            7,
        ),
    ] {
        state.retained_fields.remove(&id);
        state.field(id, &text, x, y, width);
        if let Some(&edit) = state.fields.get(&id) {
            SendMessageW(edit, EM_SETLIMITTEXT, limit, 0);
            SendMessageW(edit, WM_SETFONT, state.fonts.normal as usize, 1);
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if notification != EN_CHANGE as u16 || !matches!(id, POSITION | STOP_COLOR) {
        return false;
    }
    let text = state.raw_text(id);
    if id == POSITION {
        state.studio.spectrum.position = text;
    } else {
        state.studio.spectrum.color = text;
    }
    true
}
pub(super) fn has_focus(state: &State) -> bool {
    let focus = unsafe { GetFocus() };
    !focus.is_null()
        && [POSITION, STOP_COLOR]
            .iter()
            .any(|id| state.fields.get(id).copied() == Some(focus))
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    let enabled = view.editable() && view.canvas.count() > 0;
    let properties = &view.spectrum;
    if view.effect() == Some("wave") {
        button(
            p,
            "BACK TO MOTION",
            right + 10,
            278,
            220,
            super::Control::Wave(wave::Control::Gradient),
            true,
        );
    } else {
        let text = match properties.duration {
            Some(SelectedDuration::Mixed) => "DURATION · MIXED",
            _ => "DURATION",
        };
        p.text(text, right + 10, 246, 230, 28, p.fonts.small, TEXT, DT_LEFT);
        for (index, label) in ["SLOW", "MEDIUM", "FAST"].iter().enumerate() {
            button(
                p,
                label,
                right + 10 + index as i32 * 76,
                278,
                70,
                super::Control::Spectrum(Control::Duration(index)),
                enabled && properties.duration.is_some(),
            );
            if properties.duration == Some(SelectedDuration::Uniform(SPECTRUM_DURATIONS[index])) {
                p.fill(right + 10 + index as i32 * 76, 305, 70, 2, GREEN);
            }
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
            super::Control::Spectrum(Control::Preset(index)),
            enabled,
        );
    }
    let count = properties.stops().map_or(0, <[GradientStop]>::len);
    let text = match &properties.gradient {
        Some(SelectedGradient::Mixed) => "MIXED · CHOOSE A PRESET".into(),
        _ if count > 0 => format!("STOP {} OF {} · % / RGB", properties.index + 1, count),
        _ => "CHOOSE A GRADIENT".into(),
    };
    p.text(
        &text,
        right + 10,
        378,
        230,
        28,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "PREVIOUS",
        right + 10,
        409,
        105,
        super::Control::Spectrum(Control::Stop(-1)),
        count > 0 && properties.index > 0,
    );
    button(
        p,
        "NEXT",
        right + 125,
        409,
        105,
        super::Control::Spectrum(Control::Stop(1)),
        properties.index + 1 < count,
    );
    button(
        p,
        "APPLY STOP",
        right + 10,
        480,
        130,
        super::Control::Spectrum(Control::ApplyStop(false)),
        enabled && count > 0,
    );
    button(
        p,
        "NONE",
        right + 150,
        480,
        80,
        super::Control::Spectrum(Control::ApplyStop(true)),
        enabled && count > 0,
    );
    button(
        p,
        "ADD STOP",
        right + 10,
        516,
        105,
        super::Control::Spectrum(Control::Add),
        enabled && count > 0 && count < 10,
    );
    button(
        p,
        "DELETE",
        right + 125,
        516,
        105,
        super::Control::Spectrum(Control::Delete),
        enabled && count > 2,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        552,
        220,
        super::Control::Erase,
        enabled,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_spectrum_presets_match_independently_extracted_source_data() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio_spectrum_presets.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["source_sha256"],
            "109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19"
        );
        let expected = fixture["presets"].as_array().unwrap();
        assert_eq!(expected.len(), 5);
        for (index, expected) in expected.iter().enumerate() {
            assert_eq!(
                serde_json::to_value(preset(index).unwrap()).unwrap(),
                *expected
            );
        }
        assert!(preset(5).is_none());
    }
}
