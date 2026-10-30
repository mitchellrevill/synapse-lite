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
const SPECTRUM_CONTENT_BOTTOM: i32 = 760;
#[derive(Clone, Copy)]
enum StopTrack {
    Shared,
    Spectrum,
}
#[derive(Clone, Copy)]
struct StopDrag {
    original: u8,
    position: u8,
    track: StopTrack,
}
#[derive(Default)]
pub(super) struct Properties {
    duration: Option<SelectedDuration>,
    scroll: i32,
    drag: Option<StopDrag>,
    duration_drag: Option<usize>,
    gradient: Option<SelectedGradient>,
    index: usize,
    position: String,
    color: String,
    mode: ColorMode,
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
    if view.is_breathing() && (!view.breathing_gradient || view.breathing_playback) {
        view.spectrum = Properties::default();
        return;
    }
    if !matches!(
        view.effect(),
        Some("spectrum" | "wheel" | "wave" | "starlight" | "breathing" | "audio")
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
    let wheel = view.effect() == Some("wheel");
    let star = view.effect() == Some("starlight");
    let breath = view.effect() == Some("breathing");
    let audio = view.effect() == Some("audio");
    if !wheel && !wave && !star && !breath && !audio {
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
    match if wheel {
        draft.wheel_selection_gradient(id, &selection)
    } else if wave {
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
        unsafe {
            cancel_drag(state);
        }
        return;
    }
    if !visible(&state.studio) {
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
            Some("spectrum" | "wheel" | "wave" | "starlight" | "breathing" | "audio")
        ) {
            return Err("Select a gradient effect first".into());
        }
        let wave = view.effect() == Some("wave");
        let wheel = view.effect() == Some("wheel");
        let star = view.effect() == Some("starlight");
        let breath = view.effect() == Some("breathing");
        let audio = view.effect() == Some("audio");
        let id = view.selected.as_deref().ok_or("Select a layer")?;
        let selection = view.canvas.selection();
        let properties = &view.spectrum;
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let mut edit_stop = |edit| {
            if wheel {
                draft.edit_wheel_stop(id, &selection, edit)
            } else if wave {
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
            Control::Duration(_) if wheel || wave || star || breath || audio => {
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
            Control::Preset(index) if wheel => draft.set_wheel_preset(id, &selection, index),
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
            if changed {
                view.status = "Gradient updated · save to keep changes".into();
            }
            if let Some(index) = next_index {
                view.spectrum.index = index;
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
    let max_scroll = maximum_scroll(state.content_bottom_css() as i32 - 40);
    state.studio.spectrum.scroll = state.studio.spectrum.scroll.clamp(0, max_scroll);
    if !visible(&state.studio) || state.studio.spectrum.stops().is_none_or(|s| s.is_empty()) {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    let scroll = if state.studio.effect() == Some("spectrum") {
        state.studio.spectrum.scroll
    } else {
        0
    };
    for (id, text, x, y, width, limit) in [
        (
            POSITION,
            state.studio.spectrum.position.clone(),
            right + 10,
            440 - scroll,
            65,
            3,
        ),
        (
            STOP_COLOR,
            state.studio.spectrum.color.clone(),
            right + 90,
            440 - scroll,
            130,
            32,
        ),
    ] {
        if y < 246 || y + 28 > state.content_bottom_css() as i32 - 40 {
            continue;
        }
        state.retained_fields.remove(&id);
        state.field(id, &text, x, y, width);
        if let Some(&edit) = state.fields.get(&id) {
            SendMessageW(edit, EM_SETLIMITTEXT, limit, 0);
            SendMessageW(edit, WM_SETFONT, state.fonts.normal as usize, 1);
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !visible(&state.studio)
        || notification != EN_CHANGE as u16
        || !matches!(id, POSITION | STOP_COLOR)
    {
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
    if !visible(&state.studio) {
        return false;
    }
    let focus = unsafe { GetFocus() };
    !focus.is_null()
        && [POSITION, STOP_COLOR]
            .iter()
            .any(|id| state.fields.get(id).copied() == Some(focus))
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    paint_scrolled(p, view, right, 10000);
}
pub(super) unsafe fn paint_scrolled(p: &mut Painter, view: &View, right: i32, bottom: i32) {
    if !visible(view) {
        return;
    }
    let scroll = if view.effect() == Some("spectrum") {
        view.spectrum.scroll
    } else {
        0
    };
    let saved = SaveDC(p.dc);
    IntersectClipRect(
        p.dc,
        p.px(right),
        p.px(246),
        p.px(right + 250),
        p.px(bottom),
    );
    let first_hit = p.hits.len();
    let enabled = view.editable()
        && view.canvas.count() > 0
        && (view.effect() != Some("starlight") || starlight::gradient_enabled(view));
    let properties = &view.spectrum;
    if view.effect() == Some("breathing") {
        button(
            p,
            "BACK TO COLOURS",
            right + 10,
            278 - scroll,
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
            278 - scroll,
            220,
            super::Control::Starlight(starlight::Control::Gradient),
            true,
        );
    } else if view.effect() == Some("wheel") {
        button(
            p,
            "BACK TO PROPERTIES",
            right + 10,
            278,
            220,
            super::Control::Wheel(wheel::Control::Gradient),
            true,
        );
    } else if view.effect() == Some("wave") {
        button(
            p,
            "BACK TO MOTION",
            right + 10,
            278 - scroll,
            220,
            super::Control::Wave(wave::Control::Gradient),
            true,
        );
    } else {
        let text = match properties.duration {
            Some(SelectedDuration::Mixed) => "DURATION · MIXED",
            _ => "DURATION",
        };
        p.text(
            text,
            right + 10,
            246 - scroll,
            230,
            28,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        p.fill(right + 20, 290 - scroll, 200, 3, TEXT);
        for (index, label) in ["SLOW", "MEDIUM", "FAST"].iter().enumerate() {
            let selected = properties.duration_drag == Some(index)
                || (properties.duration_drag.is_none()
                    && properties.duration
                        == Some(SelectedDuration::Uniform(SPECTRUM_DURATIONS[index])));
            p.fill(
                right + 16 + index as i32 * 100,
                284 - scroll,
                9,
                16,
                if selected { GREEN } else { TEXT },
            );
            p.text(
                label,
                right + index as i32 * 76,
                302 - scroll,
                76,
                16,
                p.fonts.small,
                TEXT,
                DT_CENTER,
            );
        }
    }
    p.text(
        if view.is_breathing() {
            "CUSTOM GRADIENT"
        } else {
            "GRADIENT PRESETS"
        },
        right + 10,
        316 - scroll,
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
            347 - scroll,
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
        _ if count > 0 => format!(
            "STOP {} OF {} · % / {}",
            properties.index + 1,
            count,
            properties.mode.label()
        ),
        _ => "CHOOSE A GRADIENT".into(),
    };
    p.text(
        &text,
        right + 10,
        462 - scroll,
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
        409 - scroll,
        105,
        super::Control::Spectrum(Control::Stop(-1)),
        count > 0 && properties.index > 0,
    );
    button(
        p,
        "NEXT",
        right + 125,
        409 - scroll,
        105,
        super::Control::Spectrum(Control::Stop(1)),
        properties.index + 1 < count,
    );
    button(
        p,
        "APPLY STOP",
        right + 10,
        480 - scroll,
        130,
        super::Control::Spectrum(Control::ApplyStop(false)),
        enabled && count > 0,
    );
    button(
        p,
        "NONE",
        right + 150,
        480 - scroll,
        80,
        super::Control::Spectrum(Control::ApplyStop(true)),
        enabled && count > 0,
    );
    button(
        p,
        "ADD STOP",
        right + 10,
        516 - scroll,
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
        516 - scroll,
        105,
        super::Control::Spectrum(Control::Delete),
        enabled && count > 2,
    );
    let tile_y = if view.effect() == Some("spectrum") {
        582 - scroll
    } else {
        588 - scroll
    };
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
            tile_y,
            24,
            super::Control::Spectrum(Control::Color(color)),
            enabled && count > 0,
        );
        if let Some(color) = color {
            p.fill(x + 3, tile_y + 4, 18, 20, color);
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
            552 - scroll,
            70,
            super::Control::Spectrum(Control::ColorMode(mode)),
            count > 0,
        );
        if properties.mode == mode {
            p.fill(right + 10 + i as i32 * 76, 579 - scroll, 70, 2, GREEN);
        }
    }
    if let Some(stops) = properties.stops() {
        paint_strip(p, properties, stops, right + 15, 378 - scroll);
    }
    if view.effect() == Some("spectrum") {
        button(
            p,
            "ERASE SELECTION",
            right + 10,
            646 - scroll,
            220,
            super::Control::Erase,
            enabled,
        );
    }
    if view.effect() == Some("spectrum") {
        p.fill(right + 10, 618 - scroll, 220, 4, TEXT);
        if let Some(stops) = properties.stops() {
            for (index, stop) in stops.iter().enumerate() {
                let position = if index == properties.index && properties.drag.is_some() {
                    properties.drag.map_or(stop.position, |drag| drag.position)
                } else {
                    stop.position
                };
                let x = right + 10 + i32::from(position) * 220 / 100;
                p.fill(
                    x - 4,
                    610 - scroll,
                    9,
                    20,
                    if index == properties.index {
                        GREEN
                    } else {
                        stop.color.unwrap_or(0x777777)
                    },
                );
            }
        }
    }
    if view.effect() == Some("spectrum") {
        paragraph(
            p,
            &view.status,
            right + 10,
            684 - scroll,
            230,
            60,
            if view.failed { 0xfd4949 } else { MUTED },
        );
    }
    if view.effect() == Some("spectrum") && bottom < SPECTRUM_CONTENT_BOTTOM {
        let visible = (bottom - 246).max(1);
        let thumb = (visible * visible / (SPECTRUM_CONTENT_BOTTOM - 246)).clamp(16, visible);
        p.fill(right + 244, 246, 4, visible, 0x303030);
        p.fill(
            right + 244,
            246 + (visible - thumb) * scroll / (SPECTRUM_CONTENT_BOTTOM - bottom).max(1),
            4,
            thumb,
            0x777777,
        );
    }
    RestoreDC(p.dc, saved);
    // Offscreen controls must never receive clicks above the property body.
    let top = p.px(246);
    let bottom = p.px(bottom);
    for hit in &mut p.hits[first_hit..] {
        hit.rect.top = hit.rect.top.max(top);
        hit.rect.bottom = hit.rect.bottom.min(bottom);
    }
}

// Stable selection under crossing and equal-position stops.
fn moved_index(stops: &[GradientStop], index: usize, position: u8) -> usize {
    stops
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            *i != index && (s.position < position || (s.position == position && *i < index))
        })
        .count()
}
unsafe fn paint_strip(
    p: &mut Painter,
    properties: &Properties,
    stops: &[GradientStop],
    left: i32,
    top: i32,
) {
    // Checkerboard denotes None; swatches are authoring feedback, not runtime math.
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
                let a = ((a_color >> shift) & 255) as f64;
                let b = ((b_color >> shift) & 255) as f64;
                (a + (b - a) * fraction).round() as u32
            };
            Some(channel(16) << 16 | channel(8) << 8 | channel(0))
        });
        for y in 0..2 {
            p.fill(
                left + x,
                top + y * 8,
                1,
                8,
                color.unwrap_or(if (x / 6 + y) % 2 == 0 {
                    0x666666
                } else {
                    0x333333
                }),
            );
        }
    }
    for (index, stop) in stops.iter().enumerate() {
        let position = if index == properties.index {
            properties.drag.map_or(stop.position, |drag| drag.position)
        } else {
            stop.position
        };
        let x = left + (f64::from(position) * 209.0 / 100.0).round() as i32;
        p.fill(
            x - 4,
            top + 16,
            9,
            10,
            if index == properties.index {
                GREEN
            } else {
                TEXT
            },
        );
        p.fill(x - 2, top + 18, 5, 6, 0);
    }
}

fn inspector_scroll(view: &View) -> i32 {
    if view.effect() == Some("spectrum") {
        view.spectrum.scroll
    } else {
        0
    }
}
fn maximum_scroll(bottom: i32) -> i32 {
    (SPECTRUM_CONTENT_BOTTOM - bottom).max(0)
}

pub(super) fn visible(view: &View) -> bool {
    view.open
        && (view.effect() == Some("spectrum")
            || (view.effect() == Some("wheel") && view.wheel.gradient_open)
            || (view.effect() == Some("wave") && view.wave.gradient_open)
            || (view.effect() == Some("audio") && view.audio.gradient_open)
            || (view.effect() == Some("starlight") && view.starlight.gradient_open)
            || (view.effect() == Some("breathing")
                && view.breathing_gradient
                && !view.breathing_playback))
}
pub(super) fn dragging(view: &View) -> bool {
    view.spectrum.drag.is_some() || view.spectrum.duration_drag.is_some()
}
pub(super) unsafe fn cancel_drag(state: &mut State) {
    let stop = state.studio.spectrum.drag.take().is_some();
    let duration = state.studio.spectrum.duration_drag.take().is_some();
    if stop {
        state.studio.spectrum.field_values();
    }
    if stop || duration {
        ReleaseCapture();
        InvalidateRect(state.hwnd, null(), 0);
    }
}
pub(super) fn wheel(state: &mut State, delta: i32, x: i32) -> bool {
    if !visible(&state.studio)
        || state.studio.effect() != Some("spectrum")
        || x < (state.width as f32 / state.scale) as i32 - 250
    {
        return false;
    }
    let max = maximum_scroll(state.content_bottom_css() as i32 - 40);
    state.studio.spectrum.scroll =
        (state.studio.spectrum.scroll + if delta < 0 { 48 } else { -48 }).clamp(0, max);
    true
}
fn stop_position(track: StopTrack, x: f64, right: i32) -> u8 {
    match track {
        StopTrack::Shared => ((x - f64::from(right + 15)) / 209.0 * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8,
        // Keep Spectrum's established integer track mapping, including 90 at x+210.
        StopTrack::Spectrum => (((x as i32 - (right + 10)) * 100 / 220).clamp(0, 100)) as u8,
    }
}
/// Captured gestures update view state only. Release makes one stable Move or
/// duration edit; Escape/capture loss leaves the document and history unchanged.
pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if matches!(message, WM_CANCELMODE | WM_CAPTURECHANGED) {
        let stop = state.studio.spectrum.drag.take().is_some();
        let duration = state.studio.spectrum.duration_drag.take().is_some();
        if !(stop || duration) {
            return false;
        }
        if stop {
            state.studio.spectrum.field_values();
        }
        if message == WM_CANCELMODE {
            ReleaseCapture();
        }
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
        return true;
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
    let x = f64::from((lparam as u32 & 0xffff) as i16) / f64::from(state.scale);
    let y = f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale);
    let right = (state.width as f32 / state.scale) as i32 - 250;
    let scroll = inspector_scroll(&state.studio);
    let bottom = state.content_bottom_css() as i32 - 40;
    let spectrum = state.studio.effect() == Some("spectrum");
    let duration_index = (((x as i32 - (right + 10) + 40) / 100).clamp(0, 2)) as usize;
    if state.studio.spectrum.duration_drag.is_some() {
        match message {
            WM_MOUSEMOVE => state.studio.spectrum.duration_drag = Some(duration_index),
            WM_LBUTTONUP => {
                state.studio.spectrum.duration_drag = None;
                ReleaseCapture();
                state.action(Action::Studio(super::Control::Spectrum(Control::Duration(
                    duration_index,
                ))));
                redraw(state);
                return true;
            }
            _ => return false,
        }
        InvalidateRect(state.hwnd, null(), 0);
        return true;
    }
    if let Some(drag) = state.studio.spectrum.drag {
        let position = stop_position(drag.track, x, right);
        match message {
            WM_MOUSEMOVE => {
                state.studio.spectrum.drag = Some(StopDrag { position, ..drag });
                state.studio.spectrum.position = position.to_string();
            }
            WM_LBUTTONUP => {
                state.studio.spectrum.drag = None;
                ReleaseCapture();
                if position != drag.original {
                    super::action(state, super::Control::Spectrum(Control::Move(position)));
                } else {
                    state.studio.spectrum.field_values();
                    state.rebuild_fields();
                }
            }
            _ => return false,
        }
        InvalidateRect(state.hwnd, null(), 0);
        return true;
    }
    if message != WM_LBUTTONDOWN || y < 246.0 || y >= f64::from(bottom) {
        return false;
    }
    if spectrum
        && (f64::from(278 - scroll)..f64::from(302 - scroll)).contains(&y)
        && (f64::from(right + 10)..=f64::from(right + 230)).contains(&x)
    {
        state.studio.spectrum.duration_drag = Some(duration_index);
        SetCapture(state.hwnd);
        InvalidateRect(state.hwnd, null(), 0);
        return true;
    }
    let track = if (f64::from(374 - scroll)..=f64::from(406 - scroll)).contains(&y)
        && (f64::from(right + 10)..=f64::from(right + 229)).contains(&x)
    {
        StopTrack::Shared
    } else if spectrum
        && (f64::from(610 - scroll)..=f64::from(634 - scroll)).contains(&y)
        && (f64::from(right + 5)..=f64::from(right + 235)).contains(&x)
    {
        StopTrack::Spectrum
    } else {
        return false;
    };
    let position = stop_position(track, x, right);
    let nearest = state.studio.spectrum.stops().and_then(|stops| {
        stops
            .iter()
            .enumerate()
            .filter(|(_, s)| match track {
                StopTrack::Shared => true,
                StopTrack::Spectrum => {
                    (f64::from(right + 10 + i32::from(s.position) * 220 / 100) - x).abs() <= 6.0
                }
            })
            .min_by_key(|(_, s)| s.position.abs_diff(position))
            .map(|(i, s)| (i, s.position))
    });
    if let Some((index, original)) = nearest {
        state.studio.spectrum.index = index;
        state.studio.spectrum.field_values();
        state.studio.spectrum.drag = Some(StopDrag {
            original,
            position: original,
            track,
        });
        SetCapture(state.hwnd);
        state.rebuild_fields();
    } else if matches!(track, StopTrack::Spectrum) {
        state.studio.spectrum.position = position.to_string();
        state.action(Action::Studio(super::Control::Spectrum(Control::Add)));
    } else {
        return false;
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
    fn crossed_stop_retains_identity_in_stable_order() {
        let stops = [10, 40, 40, 90].map(|position| GradientStop {
            position,
            color: None,
        });
        assert_eq!(moved_index(&stops, 0, 80), 2);
        assert_eq!(moved_index(&stops, 3, 40), 3);
        assert_eq!(moved_index(&stops, 0, 40), 0);
        assert_eq!(moved_index(&stops, 2, 40), 2);
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
