//! Ambient Awareness authoring. Region editing never captures the desktop.
use super::*;
use secbird_core::studio::document::{
    AmbientEdit, AmbientProperty, AmbientRect, AmbientValue, SelectedAmbientValue, AMBIENT_PRESETS,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;

const FIELDS: [u16; 5] = [2110, 2111, 2112, 2113, 2114];
#[derive(Clone)]
pub enum Control {
    Preset(usize),
    Rect,
    Blur,
}
#[derive(Default)]
pub(super) struct Properties {
    values: [String; 5],
    screen: Option<SelectedAmbientValue>,
    rect: Option<AmbientRect>,
    mixed_blur: bool,
    mixed_rect: bool,
}
pub(super) fn refresh(view: &mut View) {
    view.ambient = Properties::default();
    if view.effect() != Some("ambient") {
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let selection = view.canvas.selection();
    let result = (|| -> secbird_core::Result<()> {
        view.ambient.screen =
            Some(draft.ambient_selection_value(id, &selection, AmbientProperty::Screen)?);
        let rect = draft.ambient_selection_value(id, &selection, AmbientProperty::Rect)?;
        view.ambient.mixed_rect = rect == SelectedAmbientValue::Mixed;
        if let SelectedAmbientValue::Uniform(Some(AmbientValue::Rect(rect))) = rect {
            for (index, value) in [rect.left, rect.top, rect.right, rect.bottom]
                .into_iter()
                .enumerate()
            {
                view.ambient.values[index] = value.to_string();
            }
            view.ambient.rect = Some(rect);
        }
        let blur = draft.ambient_selection_value(id, &selection, AmbientProperty::Blur)?;
        view.ambient.mixed_blur = blur == SelectedAmbientValue::Mixed;
        if let SelectedAmbientValue::Uniform(Some(AmbientValue::Blur(value))) = blur {
            view.ambient.values[4] = value.to_string();
        }
        Ok(())
    })();
    if let Err(error) = result {
        view.status = error.to_string();
        view.failed = true;
    }
}
pub(super) fn action(state: &mut State, control: Control) {
    if !state.studio.editable()
        || state.read_only_smoke
        || state.restricted_actions
        || state.studio.effect() != Some("ambient")
    {
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        let edit = match control {
            Control::Preset(index) => AmbientEdit::Preset(index),
            Control::Blur => AmbientEdit::Blur(
                view.ambient.values[4]
                    .trim()
                    .parse::<u8>()
                    .map_err(|_| "Enter blur 1–9")?,
            ),
            Control::Rect => {
                let mut values = [0.0; 4];
                for (index, text) in view.ambient.values[..4].iter().enumerate() {
                    values[index] = text
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "Enter region edges in 0–65535")?;
                }
                AmbientEdit::Rect(AmbientRect {
                    left: values[0],
                    top: values[1],
                    right: values[2],
                    bottom: values[3],
                })
            }
        };
        let id = view
            .selected
            .as_deref()
            .ok_or("Select Ambient Awareness first")?;
        view.draft
            .as_mut()
            .ok_or("Open a composition first")?
            .edit_ambient(id, &view.canvas.selection(), edit)
            .map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            if changed {
                view.status = "Ambient settings updated; save to keep them. Live screen playback unavailable.".into();
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
    if state.studio.effect() != Some("ambient") {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    let enabled = state.studio.editable()
        && state.studio.canvas.count() > 0
        && !state.read_only_smoke
        && !state.restricted_actions;
    for (index, id) in FIELDS.into_iter().enumerate() {
        let text = state.studio.ambient.values[index].clone();
        let (x, y, w) = if index == 4 {
            (right + 10, 534, 64)
        } else {
            (
                right + 10 + (index as i32 % 2) * 115,
                371 + (index as i32 / 2) * 55,
                105,
            )
        };
        state.retained_fields.remove(&id);
        state.field(id, &text, x, y, w);
        if let Some(&field) = state.fields.get(&id) {
            SendMessageW(field, EM_SETLIMITTEXT, 24, 0);
            SendMessageW(field, WM_SETFONT, state.fonts.normal as usize, 1);
            EnableWindow(field, i32::from(enabled));
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    let Some(index) = FIELDS.iter().position(|&field| field == id) else {
        return false;
    };
    if notification != EN_CHANGE as u16 {
        return false;
    }
    state.studio.ambient.values[index] = state.raw_text(id);
    true
}
pub(super) fn focused_control(state: &State) -> Option<Control> {
    let focus = unsafe { GetFocus() };
    FIELDS
        .iter()
        .position(|id| state.fields.get(id) == Some(&focus))
        .map(|index| {
            if index == 4 {
                Control::Blur
            } else {
                Control::Rect
            }
        })
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    let enabled = view.editable() && view.canvas.count() > 0;
    p.text(
        if view.ambient.screen == Some(SelectedAmbientValue::Mixed) || view.ambient.mixed_rect {
            "REGION · MIXED"
        } else {
            "REGION"
        },
        right + 10,
        245,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    // Source region indicator colours; compact layout leaves room for native fields.
    // This depicts normalized region coordinates and never contains desktop pixels.
    p.fill(right + 164, 313, 66, 28, 0x707070);
    p.fill(right + 165, 314, 64, 26, 0x111111);
    let region = match &view.ambient.screen {
        Some(SelectedAmbientValue::Uniform(Some(AmbientValue::Screen(value)))) => {
            match value.as_str() {
                "full" => Some([0., 0., 1., 1.]),
                "left" => Some([0., 0., 0.1, 1.]),
                "top" => Some([0., 0., 1., 0.1]),
                "right" => Some([0.9, 0., 1., 1.]),
                "bottom" => Some([0., 0.9, 1., 1.]),
                "custom" => view.ambient.rect.as_ref().map(|r| {
                    [
                        r.left / 65535.,
                        r.top / 65535.,
                        r.right / 65535.,
                        r.bottom / 65535.,
                    ]
                }),
                _ => None,
            }
        }
        _ => None,
    };
    if let Some([left, top, right_edge, bottom]) = region {
        if [left, top, right_edge, bottom]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && left < right_edge
            && top < bottom
        {
            p.fill(
                right + 165 + (left * 64.) as i32,
                314 + (top * 26.) as i32,
                ((right_edge - left) * 64.).round().max(1.) as i32,
                ((bottom - top) * 26.).round().max(1.) as i32,
                0xcccccc,
            );
        }
    }
    for (index, label) in ["FULL", "LEFT", "TOP", "RIGHT", "BOTTOM"]
        .into_iter()
        .enumerate()
    {
        let x = right + 10 + (index as i32 % 3) * 77;
        let y = 277 + (index as i32 / 3) * 36;
        button(
            p,
            label,
            x,
            y,
            72,
            super::Control::Ambient(Control::Preset(index)),
            enabled,
        );
        if matches!(&view.ambient.screen, Some(SelectedAmbientValue::Uniform(Some(AmbientValue::Screen(value))))
            if value == AMBIENT_PRESETS[index])
        {
            p.fill(x, y + 27, 72, 2, GREEN);
        }
    }
    for (index, label) in ["LEFT", "TOP", "RIGHT", "BOTTOM"].into_iter().enumerate() {
        p.text(
            label,
            right + 10 + (index as i32 % 2) * 115,
            348 + (index as i32 / 2) * 55,
            105,
            22,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
    }
    button(
        p,
        "APPLY REGION",
        right + 10,
        464,
        220,
        super::Control::Ambient(Control::Rect),
        enabled,
    );
    p.text(
        if view.ambient.mixed_blur {
            "SHARP–BLUR · MIXED"
        } else {
            "SHARP–BLUR · 1–9"
        },
        right + 10,
        509,
        230,
        22,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY BLUR",
        right + 84,
        534,
        146,
        super::Control::Ambient(Control::Blur),
        enabled,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        574,
        220,
        super::Control::Erase,
        enabled,
    );
}
