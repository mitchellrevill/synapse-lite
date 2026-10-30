//! Shared native gradient workflow over each effect's regional contract.
#[path = "studio_gradient_color.rs"]
mod color_input;
use super::*;
use color_input::ColorMode;
use secbird_core::studio::document::{
    GradientStop, SelectedDuration, SelectedGradient, StopEdit, SPECTRUM_DURATIONS,
};

#[derive(Clone)]
pub enum Control {
    Duration(usize),
    CancelDrag,
    ColorMode(ColorMode),
    Move(u8),
    Color(Option<u32>),
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
    mode: ColorMode,
    drag: Option<u8>,
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
        self.color = stop.color.map(|c| self.mode.format(c)).unwrap_or_default();
    }
}
pub(super) fn refresh(view: &mut View) {
    if !matches!(
        view.effect(),
        Some("spectrum" | "wave" | "starlight" | "breathing" | "audio")
    ) {
        view.spectrum = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let wave = view.effect() == Some("wave");
    let star = view.effect() == Some("starlight");
    let breath = view.effect() == Some("breathing");
    let audio = view.effect() == Some("audio");
    if !wave && !star && !breath && !audio {
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
    } else if breath {
        draft.breathing_selection_gradient(id, &selection)
    } else if audio {
        draft.audio_selection_gradient(id, &selection)
    } else if star {
        draft.starlight_selection_gradient(id, &selection)
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
    if position > 100 {
        return Err("Enter a position from 0 to 100 percent".into());
    }
    let color = if none {
        None
    } else {
        Some(properties.mode.parse(&properties.color)?)
    };
    Ok((position, color))
}
pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::CancelDrag) {
        return;
    }
    if let Control::ColorMode(mode) = control {
        // Switching representations is view state and never quantizes stored RGB.
        let properties = &mut state.studio.spectrum;
        if properties.mode != mode {
            if properties.color.trim().is_empty() {
                properties.mode = mode;
            } else {
                match properties.mode.parse(&properties.color) {
                    Ok(rgb) => {
                        properties.color = mode.format(rgb);
                        properties.mode = mode;
                    }
                    Err(message) => {
                        state.studio.status = message;
                        state.studio.failed = true;
                    }
                }
            }
        }
        return;
    }
    if let Control::Stop(delta) = control {
        let properties = &mut state.studio.spectrum;
        let count = properties.stops().map_or(0, <[GradientStop]>::len);
        properties.index =
            (properties.index as i32 + delta).clamp(0, count.saturating_sub(1) as i32) as usize;
        properties.field_values();
        return;
    }
    if state.read_only_smoke
        || state.restricted_actions
        || !state.studio.editable()
        || (state.studio.effect() == Some("starlight")
            && !starlight::gradient_enabled(&state.studio))
    {
        return;
    }
    let view = &mut state.studio;
    let mut next_index = None;
    let result = (|| -> Result<bool, String> {
        if !matches!(
            view.effect(),
            Some("spectrum" | "wave" | "starlight" | "breathing" | "audio")
        ) {
            return Err("Select a gradient effect first".into());
        }
        let wave = view.effect() == Some("wave");
        let star = view.effect() == Some("starlight");
        let breath = view.effect() == Some("breathing");
        let audio = view.effect() == Some("audio");
        let id = view.selected.as_deref().ok_or("Select a layer")?;
        let selection = view.canvas.selection();
        let properties = &view.spectrum;
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let mut edit_stop = |edit| {
            if wave {
                draft.edit_wave_stop(id, &selection, edit)
            } else if breath {
                draft.edit_breathing_stop(id, &selection, edit)
            } else if audio {
                draft.edit_audio_stop(id, &selection, edit)
            } else if star {
                draft.edit_starlight_stop(id, &selection, edit)
            } else {
                draft.edit_spectrum_stop(id, &selection, edit)
            }
        };
        let result = match control {
            Control::Duration(_) if wave || star || breath || audio => {
                return Err("Use this effect's own duration controls".into())
            }
            Control::Duration(index) => draft.set_spectrum_duration(
                id,
                &selection,
                *SPECTRUM_DURATIONS.get(index).ok_or("Unknown duration")?,
            ),
            Control::Preset(index) if audio => draft.set_audio_preset(id, &selection, index),
            Control::Preset(index) if star => draft.set_starlight_preset(id, &selection, index),
            Control::Preset(_) if breath => {
                return Err(
                    "Breathing uses its own two-colour stops or an imported gradient".into(),
                )
            }
            Control::Preset(index) if wave => draft.set_wave_preset(id, &selection, index),
            Control::Preset(index) => draft.set_spectrum_gradient(
                id,
                &selection,
                &preset(index).ok_or("Unknown preset")?,
                false,
            ),
            Control::ApplyStop(none) => {
                let (position, color) = entered_stop(properties, none)?;
                next_index = properties
                    .stops()
                    .map(|stops| moved_index(stops, properties.index, position));
                edit_stop(StopEdit::Set {
                    index: properties.index,
                    position,
                    color,
                })
            }
            Control::Add => {
                let (position, color) =
                    entered_stop(properties, properties.color.trim().is_empty())?;
                next_index = properties
                    .stops()
                    .map(|stops| stops.iter().filter(|s| s.position <= position).count());
                edit_stop(StopEdit::Insert { position, color })
            }
            Control::Color(color) => {
                let position = properties
                    .stops()
                    .and_then(|s| s.get(properties.index))
                    .ok_or("Select a stop first")?
                    .position;
                edit_stop(StopEdit::Set {
                    index: properties.index,
                    position,
                    color,
                })
            }
            Control::Move(position) => {
                next_index = properties
                    .stops()
                    .map(|stops| moved_index(stops, properties.index, position));
                edit_stop(StopEdit::Move {
                    index: properties.index,
                    position,
                })
            }
            Control::Delete => edit_stop(StopEdit::Delete {
                index: properties.index,
            }),
            Control::Stop(_) | Control::ColorMode(_) | Control::CancelDrag => unreachable!(),
        };
        result.map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if let Some(index) = next_index {
                view.spectrum.index = index;
            }
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
    if !visible(&state.studio) || state.studio.spectrum.stops().is_none_or(|s| s.is_empty()) {
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
            32,
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
    let enabled = view.editable()
        && view.canvas.count() > 0
        && (view.effect() != Some("starlight") || starlight::gradient_enabled(view));
    let properties = &view.spectrum;
    if view.effect() == Some("breathing") {
        button(
            p,
            "BACK TO COLOURS",
            right + 10,
            278,
            220,
            super::Control::BreathingGradient,
            true,
        );
    } else if view.effect() == Some("audio") {
        button(
            p,
            "BACK TO LEVELS",
            right + 10,
            278,
            220,
            super::Control::Audio(audio::Control::Gradient),
            true,
        );
    } else if view.effect() == Some("starlight") {
        button(
            p,
            "BACK TO PROPERTIES",
            right + 10,
            278,
            220,
            super::Control::Starlight(starlight::Control::Gradient),
            true,
        );
    } else if view.effect() == Some("wave") {
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
        if view.is_breathing() {
            "CUSTOM GRADIENT"
        } else {
            "GRADIENT PRESETS"
        },
        right + 10,
        316,
        230,
        28,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for index in 0..if view.is_breathing() {
        0
    } else if view.effect() == Some("audio") {
        4
    } else {
        5
    } {
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
        Some(SelectedGradient::Mixed) => {
            if view.is_breathing() {
                "MIXED · SELECT MATCHING LEDS".into()
            } else {
                "MIXED · CHOOSE A PRESET".into()
            }
        }
        _ if count > 0 => format!("STOP {} OF {} · % / {}", properties.index + 1, count, properties.mode.label()),
        _ => "CHOOSE A GRADIENT".into(),
    };
    p.text(
        &text,
        right + 10,
        462,
        230,
        18,
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
        enabled
            && count > 0
            && count
                < if matches!(view.effect(), Some("spectrum" | "breathing")) {
                    10
                } else {
                    7
                },
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
    // Default colour tiles recovered from shared Studio picker606 (x).
    for (i, color) in [
        None,
        Some(0xff0000),
        Some(0xff8000),
        Some(0xffff00),
        Some(0x00ff00),
        Some(0x00ffff),
        Some(0x0000ff),
        Some(0xff00ff),
    ]
    .into_iter()
    .enumerate()
    {
        let x = right + 10 + i as i32 * 28;
        button(
            p,
            if color.is_none() { "X" } else { "" },
            x,
            588,
            24,
            super::Control::Spectrum(Control::Color(color)),
            enabled && count > 0,
        );
        if let Some(color) = color {
            p.fill(x + 3, 592, 18, 20, color);
        }
    }
    for (i, mode) in [ColorMode::Hex, ColorMode::Rgb, ColorMode::Hsv]
        .into_iter()
        .enumerate()
    {
        button(
            p,
            mode.label(),
            right + 10 + i as i32 * 76,
            552,
            70,
            super::Control::Spectrum(Control::ColorMode(mode)),
            count > 0,
        );
        if properties.mode == mode {
            p.fill(right + 10 + i as i32 * 76, 579, 70, 2, GREEN);
        }
    }
    if let Some(stops) = properties.stops() {
        paint_strip(p, properties, stops, right + 15);
    }
}

pub(super) fn visible(view: &View) -> bool {
    view.open
        && (view.effect() == Some("spectrum")
            || (view.effect() == Some("wave") && view.wave.gradient_open)
            || (view.effect() == Some("audio") && view.audio.gradient_open)
            || (view.effect() == Some("starlight") && view.starlight.gradient_open)
            || (view.effect() == Some("breathing") && view.breathing_gradient))
}
fn moved_index(stops: &[GradientStop], index: usize, position: u8) -> usize {
    stops
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            *i != index && (s.position < position || (s.position == position && *i < index))
        })
        .count()
}
unsafe fn paint_strip(p: &mut Painter, properties: &Properties, stops: &[GradientStop], left: i32) {
    // Checkerboard denotes None, distinct from black. This is an authoring
    // swatch, not a replacement for effect-specific preview interpolation.
    for x in 0..210 {
        let percent = x as f64 / 209.0 * 100.0;
        let before = stops
            .iter()
            .rfind(|s| s.position as f64 <= percent)
            .or(stops.first());
        let after = stops
            .iter()
            .find(|s| s.position as f64 > percent)
            .or(stops.last());
        let color = before.zip(after).and_then(|(a, b)| {
            let (a_color, b_color) = (a.color?, b.color?);
            let fraction = if a.position == b.position {
                0.0
            } else {
                ((percent - a.position as f64) / (b.position as f64 - a.position as f64))
                    .clamp(0.0, 1.0)
            };
            let channel = |shift: u32| {
                let a = ((a_color >> shift) & 255u32) as f64;
                let b = ((b_color >> shift) & 255u32) as f64;
                (a + (b - a) * fraction).round() as u32
            };
            Some(channel(16) << 16 | channel(8) << 8 | channel(0))
        });
        for y in 0..2 {
            let shade = color.unwrap_or(if (x / 6 + y) % 2 == 0 {
                0x666666
            } else {
                0x333333
            });
            p.fill(left + x, 378 + y * 8, 1, 8, shade);
        }
    }
    for (index, stop) in stops.iter().enumerate() {
        let position = if index == properties.index {
            properties.drag.unwrap_or(stop.position)
        } else {
            stop.position
        };
        let x = left + (position as f64 * 209.0 / 100.0).round() as i32;
        p.fill(
            x - 4,
            394,
            9,
            10,
            if index == properties.index {
                GREEN
            } else {
                TEXT
            },
        );
        p.fill(x - 2, 396, 5, 6, 0);
    }
}
pub(super) fn dragging(view: &View) -> bool {
    view.spectrum.drag.is_some()
}
pub(super) unsafe fn cancel_drag(state: &mut State) {
    if state.studio.spectrum.drag.take().is_some() {
        ReleaseCapture();
        InvalidateRect(state.hwnd, null(), 0);
    }
}
/// A gesture changes only view state until release: one completed drag equals
/// one regional edit/history entry; capture loss leaves the document untouched.
pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if matches!(message, WM_CANCELMODE | WM_CAPTURECHANGED) {
        if state.studio.spectrum.drag.take().is_some() {
            if message == WM_CANCELMODE {
                ReleaseCapture();
            }
            InvalidateRect(state.hwnd, null(), 0);
            return true;
        }
        return false;
    }
    if !visible(&state.studio)
        || state.studio.layer_menu
        || state.studio.dropdown
        || state.studio.output.busy
        || state.read_only_smoke
        || state.restricted_actions
        || !state.studio.editable()
        || state.studio.canvas.count() == 0
        || (state.studio.effect() == Some("starlight")
            && !starlight::gradient_enabled(&state.studio))
    {
        return false;
    }
    let x = (lparam as u32 & 0xffff) as i16 as f64 / state.scale as f64;
    let y = ((lparam as u32 >> 16) & 0xffff) as i16 as f64 / state.scale as f64;
    let left = state.width as f64 / state.scale as f64 - 235.0;
    let position = ((x - left) / 209.0 * 100.0).round().clamp(0.0, 100.0) as u8;
    match message {
        WM_LBUTTONDOWN
            if (374.0..=406.0).contains(&y) && (left - 5.0..=left + 214.0).contains(&x) =>
        {
            let Some(stops) = state.studio.spectrum.stops() else {
                return false;
            };
            let Some((index, stop)) = stops
                .iter()
                .enumerate()
                .min_by_key(|(_, s)| s.position.abs_diff(position))
            else {
                return false;
            };
            let old_position = stop.position;
            state.studio.spectrum.index = index;
            state.studio.spectrum.field_values();
            state.studio.spectrum.drag = Some(old_position);
            SetCapture(state.hwnd);
            state.rebuild_fields();
        }
        WM_MOUSEMOVE if state.studio.spectrum.drag.is_some() => {
            state.studio.spectrum.drag = Some(position);
        }
        WM_LBUTTONUP if state.studio.spectrum.drag.take().is_some() => {
            ReleaseCapture();
            if state
                .studio
                .spectrum
                .stops()
                .and_then(|s| s.get(state.studio.spectrum.index))
                .is_some_and(|s| s.position != position)
            {
                super::action(state, super::Control::Spectrum(Control::Move(position)));
            }
        }
        _ => return false,
    }
    InvalidateRect(state.hwnd, null(), 0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_gradient_selection_tracks_stable_reordering_and_duplicates() {
        let stops = [10, 30, 30, 90].map(|position| GradientStop {
            position,
            color: None,
        });
        assert_eq!(moved_index(&stops, 0, 100), 3);
        assert_eq!(moved_index(&stops, 3, 0), 0);
        assert_eq!(moved_index(&stops, 0, 30), 0);
        assert_eq!(moved_index(&stops, 3, 30), 3);
        assert_eq!(moved_index(&stops, 1, 30), 1);
    }
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
