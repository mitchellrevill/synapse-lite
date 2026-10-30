//! Native Tidal authoring. No native renderer or input stream is admitted.
use super::*;
use secbird_core::studio::document::{
    SelectedTidalValue as Selected, TidalLayout, TidalProperty as Property, TIDAL_NONE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
const FIELDS: [u16; 4] = [2140, 2141, 2142, 2143];
#[derive(Clone)]
pub enum Control {
    Color(usize, bool),
    Random,
    Speed,
    Direction,
    Center,
    Place(u16, u16),
    Start(u32),
    End(u32),
    Cycles,
}
#[derive(Default)]
pub(super) struct Properties {
    values: [String; 4],
    selected: [Option<Selected>; 7],
    layout: Option<TidalLayout>,
    picking: bool,
    cached_cycles: Option<u32>,
}
pub(super) fn refresh(view: &mut View) {
    if view.effect() != Some("tidal") {
        view.tidal = Properties::default();
        return;
    }
    let Some(draft) = &view.draft else { return };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        for (i, property) in [
            Property::Color,
            Property::Color2,
            Property::Speed,
            Property::Cycles,
            Property::Random,
            Property::Start,
            Property::End,
        ]
        .into_iter()
        .enumerate()
        {
            let value = draft.tidal_selection_value(id, &selection, property)?;
            view.tidal.selected[i] = Some(value);
            if i < 4 {
                view.tidal.values[i] = match value {
                    Selected::Uniform(Some(v)) if v != TIDAL_NONE && !(i == 3 && v == 0) => {
                        if i < 2 {
                            format!("{v:06X}")
                        } else {
                            v.to_string()
                        }
                    }
                    _ => String::new(),
                };
            }
        }
        view.tidal.layout = draft.tidal_layout(id)?;
        Ok(())
    })();
    if let Err(e) = result {
        view.failed = true;
        view.status = e.to_string();
    }
}
fn colors_enabled(view: &View) -> bool {
    matches!(
        view.tidal.selected[4],
        Some(Selected::Uniform(None | Some(0)))
    )
}
pub(super) fn action(state: &mut State, control: Control) {
    let view = &mut state.studio;
    if !view.editable() || view.effect() != Some("tidal") {
        return;
    }
    if matches!(control, Control::Center) {
        view.tidal.picking = !view.tidal.picking;
        return;
    }
    let result = (|| -> Result<bool, String> {
        let id = view.selected.as_deref().ok_or("Select Tidal first")?;
        let selection = view.canvas.selection();
        let property_value = match control {
            Control::Color(i, clear) => {
                if !colors_enabled(view) {
                    return Err("Turn off Random Colour first".into());
                }
                let value = if clear {
                    None
                } else {
                    let text = view.tidal.values[i].trim().trim_start_matches('#');
                    if text.len() != 6 {
                        return Err("Enter six hexadecimal colour digits".into());
                    }
                    Some(
                        u32::from_str_radix(text, 16)
                            .map_err(|_| "Enter six hexadecimal colour digits")?,
                    )
                };
                Some((
                    if i == 0 {
                        Property::Color
                    } else {
                        Property::Color2
                    },
                    value,
                ))
            }
            Control::Random => Some((
                Property::Random,
                Some(u32::from(
                    view.tidal.selected[4] != Some(Selected::Uniform(Some(1))),
                )),
            )),
            Control::Speed => Some((
                Property::Speed,
                Some(
                    view.tidal.values[2]
                        .trim()
                        .parse()
                        .map_err(|_| "Enter speed 0-50")?,
                ),
            )),
            Control::Cycles => Some((
                Property::Cycles,
                Some(
                    view.tidal.values[3]
                        .trim()
                        .parse()
                        .map_err(|_| "Enter cycles 1-100")?,
                ),
            )),
            Control::Start(v) => Some((Property::Start, Some(v))),
            Control::End(v) => Some((Property::End, Some(v))),
            _ => None,
        };
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        if let Some((property, value)) = property_value {
            if matches!(property, Property::Start | Property::End) {
                return draft
                    .set_tidal_playback_mode(
                        id,
                        &selection,
                        property,
                        value.unwrap(),
                        view.tidal.cached_cycles,
                    )
                    .map_err(|e| e.to_string());
            }
            let result = draft
                .set_tidal_property(id, &selection, property, value)
                .map_err(|e| e.to_string());
            if result.is_ok() && property == Property::Cycles {
                view.tidal.cached_cycles = value;
            }
            return result;
        }
        let mut layout = view
            .tidal
            .layout
            .ok_or("Imported center settings are incomplete; retained without guessing")?;
        match control {
            Control::Direction => {
                return draft
                    .set_tidal_direction(id, !layout.inward)
                    .map_err(|e| e.to_string())
            }
            Control::Place(x, y) => {
                layout.x = x;
                layout.y = y;
                view.tidal.picking = false;
            }
            _ => unreachable!(),
        }
        draft
            .set_tidal_layout(id, layout)
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            view.failed = false;
            if changed {
                view.status = "Tidal updated; save to keep changes.".into();
            }
        }
        Err(e) => {
            view.failed = true;
            view.status = e;
        }
    }
    refresh(view);
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.studio.effect() != Some("tidal") {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    for (i, id) in FIELDS.into_iter().enumerate() {
        let (x, y, w) = match i {
            0 => (right + 10, 274, 104),
            1 => (right + 126, 274, 104),
            2 => (right + 10, 400, 70),
            _ => (right + 10, 614, 70),
        };
        let text = state.studio.tidal.values[i].clone();
        state.retained_fields.remove(&id);
        state.field(id, &text, x, y, w);
        if let Some(&field) = state.fields.get(&id) {
            SendMessageW(field, EM_SETLIMITTEXT, if i < 2 { 7 } else { 3 }, 0);
            SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
            let enabled = state.studio.editable()
                && state.studio.canvas.count() > 0
                && (i >= 2 || colors_enabled(&state.studio))
                && (i != 3 || state.studio.tidal.selected[6] == Some(Selected::Uniform(Some(1))));
            EnableWindow(field, i32::from(enabled));
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !state.studio.open || state.studio.effect()!=Some("tidal") {return false;}
    let Some(i) = FIELDS.iter().position(|&f| f == id) else {
        return false;
    };
    if notification != EN_CHANGE as u16 {
        return false;
    }
    state.studio.tidal.values[i] = state.raw_text(id);
    true
}
pub(super) fn enter(state: &State) -> Option<Control> {
    let focus = unsafe { GetFocus() };
    let i = FIELDS
        .iter()
        .position(|id| state.fields.get(id) == Some(&focus))?;
    Some(match i {
        0 | 1 => Control::Color(i, false),
        2 => Control::Speed,
        _ => Control::Cycles,
    })
}
pub(super) fn center_active(view: &View) -> bool {
    view.effect() == Some("tidal") && view.tidal.picking && view.editable()
}
pub(super) fn center(view: &View) -> Option<(u16, u16)> {
    if view.effect() != Some("tidal") {
        return None;
    }
    view.tidal.layout.map(|v| (v.x, v.y))
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    let enabled = view.editable() && view.canvas.count() > 0;
    let props = &view.tidal;
    let text = |p: &mut Painter, label: &str, y| {
        p.text(label, right + 10, y, 225, 24, p.fonts.small, TEXT, DT_LEFT);
    };
    text(p, "COLOUR", 245);
    for i in 0..2 {
        let x = right + 10 + i as i32 * 116;
        if let Some(Selected::Uniform(Some(v))) = props.selected[i] {
            if v <= 0xffffff {
                p.fill(x, 299, 104, 3, v);
            }
        }
        button(
            p,
            "SET",
            x,
            307,
            52,
            super::Control::Tidal(Control::Color(i, false)),
            enabled && colors_enabled(view),
        );
        button(
            p,
            "NONE",
            x + 56,
            307,
            52,
            super::Control::Tidal(Control::Color(i, true)),
            enabled && colors_enabled(view),
        );
    }
    let random = match props.selected[4] {
        Some(Selected::Mixed) => "RANDOM COLOUR - MIXED",
        Some(Selected::Uniform(Some(1))) => "[x] RANDOM COLOUR",
        _ => "[ ] RANDOM COLOUR",
    };
    button(
        p,
        random,
        right + 10,
        341,
        220,
        super::Control::Tidal(Control::Random),
        enabled,
    );
    text(p, "PROPERTIES - SPEED 0-50", 373);
    button(
        p,
        "APPLY SPEED",
        right + 90,
        399,
        140,
        super::Control::Tidal(Control::Speed),
        enabled,
    );
    button(
        p,
        if props.picking {
            "CLICK CANVAS TO SET CENTER"
        } else {
            "SELECT CENTER POINT"
        },
        right + 10,
        437,
        220,
        super::Control::Tidal(Control::Center),
        view.editable() && props.layout.is_some(),
    );
    button(
        p,
        "OUTWARD",
        right + 10,
        474,
        104,
        super::Control::Tidal(Control::Direction),
        view.editable() && props.layout.is_some(),
    );
    button(
        p,
        "INWARD",
        right + 126,
        474,
        104,
        super::Control::Tidal(Control::Direction),
        view.editable() && props.layout.is_some(),
    );
    if let Some(layout) = props.layout {
        p.fill(
            right + 10 + if layout.inward { 116 } else { 0 },
            501,
            104,
            2,
            GREEN,
        );
    }
    text(p, "PLAYBACK - START", 508);
    for (i, label) in ["RANDOM", "ANY KEY", "ON KEYS"].into_iter().enumerate() {
        let x = right + 10 + i as i32 * 76;
        button(
            p,
            label,
            x,
            534,
            70,
            super::Control::Tidal(Control::Start(i as u32)),
            enabled,
        );
        if props.selected[5] == Some(Selected::Uniform(Some(i as u32))) {
            p.fill(x, 561, 70, 2, GREEN);
        }
    }
    text(p, "END", 567);
    for (i, label) in ["NEVER", "AFTER", "TOGGLE"].into_iter().enumerate() {
        let x = right + 10 + i as i32 * 76;
        button(
            p,
            label,
            x,
            586,
            70,
            super::Control::Tidal(Control::End(i as u32)),
            enabled
                && (i != 2 || matches!(props.selected[5], Some(Selected::Uniform(Some(1 | 2))))),
        );
        if props.selected[6] == Some(Selected::Uniform(Some(i as u32))) {
            p.fill(x, 613, 70, 2, GREEN);
        }
    }
    button(
        p,
        "APPLY CYCLES",
        right + 90,
        620,
        140,
        super::Control::Tidal(Control::Cycles),
        enabled && props.selected[6] == Some(Selected::Uniform(Some(1))),
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        659,
        220,
        super::Control::Erase,
        enabled,
    );
}
