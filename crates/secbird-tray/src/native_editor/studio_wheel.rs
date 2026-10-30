//! Wheel's regional speed/gradient and layer-wide center/rotation controls.
use super::*;
use secbird_core::studio::document::{SelectedWheelValue, WheelGeometry, WheelProperty};
const SPEED: u16 = 2130;
const X: u16 = 2131;
const Y: u16 = 2132;
#[derive(Clone)]
pub enum Control {
    Speed,
    Geometry,
    Direction(bool),
    Gradient,
    Center,
}
#[derive(Default)]
pub(super) struct Properties {
    pub(super) gradient_open: bool,
    pub(super) picking: bool,
    dragging: bool,
    speed: String,
    x: String,
    y: String,
    geometry: Option<WheelGeometry>,
    mixed: bool,
}
pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if state.studio.effect() != Some("wheel") || state.studio.wheel.gradient_open {
        return false;
    }
    if matches!(message, WM_CANCELMODE | WM_CAPTURECHANGED) && state.studio.wheel.dragging {
        state.studio.wheel.dragging = false;
        if message == WM_CANCELMODE {
            ReleaseCapture();
        }
        refresh(&mut state.studio);
        state.rebuild_fields();
        return true;
    }
    if state.restricted_actions || !state.studio.editable() || state.studio.canvas.count() == 0 {
        return false;
    }
    let x = f64::from((lparam as u32 & 0xffff) as i16) / f64::from(state.scale);
    let y = f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale);
    let left = f64::from(state.width) / f64::from(state.scale) - 240.;
    if message == WM_LBUTTONDOWN
        && (left - 5.0..=left + 225.0).contains(&x)
        && (310.0..=323.0).contains(&y)
    {
        state.studio.wheel.dragging = true;
        SetCapture(state.hwnd);
    } else if !state.studio.wheel.dragging || !matches!(message, WM_MOUSEMOVE | WM_LBUTTONUP) {
        return false;
    }
    let speed = (((x - left) / 220.).clamp(0., 1.) * 360.).round() as u32;
    state.studio.wheel.speed = speed.to_string();
    state.studio.wheel.mixed = false;
    if let Some(&field) = state.fields.get(&SPEED) {
        SendMessageW(
            field,
            WM_SETTEXT,
            0,
            wide(&speed.to_string()).as_ptr() as isize,
        );
    }
    if message == WM_LBUTTONUP {
        state.studio.wheel.dragging = false;
        ReleaseCapture();
        action(state, Control::Speed);
        state.rebuild_fields();
    }
    InvalidateRect(state.hwnd, null(), 0);
    true
}
pub(super) fn refresh(view: &mut View) {
    let open = view.wheel.gradient_open && view.effect() == Some("wheel");
    let picking = view.wheel.picking && view.effect() == Some("wheel");
    view.wheel = Properties {
        gradient_open: open,
        picking,
        ..Properties::default()
    };
    if view.effect() != Some("wheel") {
        return;
    }
    let Some(draft) = &view.draft else {
        return;
    };
    let id = view.selected.as_deref().unwrap();
    let result = (|| -> secbird_core::Result<()> {
        let geometry = draft.wheel_geometry(id)?;
        view.wheel.geometry = Some(geometry);
        view.wheel.x = geometry.x.to_string();
        view.wheel.y = geometry.y.to_string();
        let speed =
            draft.wheel_selection_value(id, &view.canvas.selection(), WheelProperty::Speed)?;
        view.wheel.mixed = speed == SelectedWheelValue::Mixed;
        view.wheel.speed = match speed {
            SelectedWheelValue::Uniform(Some(n)) => n.to_string(),
            _ => String::new(),
        };
        Ok(())
    })();
    if let Err(e) = result {
        view.status = e.to_string();
        view.failed = true;
    }
}
pub(super) fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Gradient) {
        state.studio.wheel.gradient_open = !state.studio.wheel.gradient_open;
        spectrum::refresh(&mut state.studio);
        return;
    }
    if matches!(control, Control::Center) {
        state.studio.wheel.picking = !state.studio.wheel.picking;
        return;
    }
    let view = &mut state.studio;
    let result = (|| -> Result<bool, String> {
        if view.effect() != Some("wheel") {
            return Err("Select Wheel first".into());
        }
        let id = view.selected.as_deref().ok_or("Select a Wheel layer")?;
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        let result = match control {
            Control::Speed => draft.set_wheel_property(
                id,
                &view.canvas.selection(),
                WheelProperty::Speed,
                view.wheel
                    .speed
                    .trim()
                    .parse()
                    .map_err(|_| "Enter speed0..360")?,
            ),
            Control::Geometry => {
                let mut g = draft.wheel_geometry(id).map_err(|e| e.to_string())?;
                g.x = view
                    .wheel
                    .x
                    .trim()
                    .parse()
                    .map_err(|_| "Enter center X0..10000")?;
                g.y = view
                    .wheel
                    .y
                    .trim()
                    .parse()
                    .map_err(|_| "Enter center Y0..10000")?;
                draft.set_wheel_geometry(id, g)
            }
            Control::Direction(counterclockwise) => {
                let mut g = draft.wheel_geometry(id).map_err(|e| e.to_string())?;
                g.counterclockwise = counterclockwise;
                draft.set_wheel_geometry(id, g)
            }
            _ => unreachable!(),
        };
        result.map_err(|e| e.to_string())
    })();
    match result {
        Ok(changed) => {
            view.failed = false;
            if changed {
                view.status = "Wheel updated; save to keep changes".into();
            }
            refresh(view);
        }
        Err(e) => {
            view.failed = true;
            view.status = e;
        }
    }
}
pub(super) fn center(state: &mut State, x: u16, y: u16) {
    state.studio.wheel.x = x.to_string();
    state.studio.wheel.y = y.to_string();
    action(state, Control::Geometry);
    state.studio.wheel.picking = false;
}
pub(super) unsafe fn fields(state: &mut State) {
    if state.studio.effect() != Some("wheel") || state.studio.wheel.gradient_open {
        return;
    }
    let right = (state.width as f32 / state.scale) as i32 - 250;
    for (id, text, x, y, w) in [
        (SPEED, state.studio.wheel.speed.clone(), right + 10, 285, 90),
        (X, state.studio.wheel.x.clone(), right + 10, 397, 100),
        (Y, state.studio.wheel.y.clone(), right + 130, 397, 100),
    ] {
        state.retained_fields.remove(&id);
        state.field(id, &text, x, y, w);
        if let Some(&field) = state.fields.get(&id) {
            SendMessageW(field, EM_SETLIMITTEXT, 5, 0);
        }
    }
}
pub(super) fn command(state: &mut State, id: u16, notification: u16) -> bool {
    if !state.studio.open || state.studio.effect()!=Some("wheel") {return false;}
    if notification != EN_CHANGE as u16 {
        return false;
    }
    let text = state.raw_text(id);
    match id {
        SPEED => state.studio.wheel.speed = text,
        X => state.studio.wheel.x = text,
        Y => state.studio.wheel.y = text,
        _ => return false,
    }
    true
}
pub(super) fn enter(state: &State) -> Option<Control> {
    let focus = unsafe { GetFocus() };
    if state.fields.get(&SPEED) == Some(&focus) {
        Some(Control::Speed)
    } else if [X, Y].iter().any(|id| state.fields.get(id) == Some(&focus)) {
        Some(Control::Geometry)
    } else {
        None
    }
}
pub(super) unsafe fn paint(p: &mut Painter, view: &View, right: i32) {
    if view.wheel.gradient_open {
        spectrum::paint(p, view, right);
        return;
    }
    let enabled = view.editable();
    p.text(
        "WHEEL PROPERTIES",
        right + 10,
        245,
        230,
        25,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    p.text(
        if view.wheel.mixed {
            "SPEED · MIXED"
        } else {
            "SPEED (0–360)"
        },
        right + 10,
        265,
        230,
        20,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY",
        right + 120,
        285,
        110,
        super::Control::Wheel(Control::Speed),
        enabled && view.canvas.count() > 0,
    );
    p.fill(right + 10, 316, 220, 3, 0x285b1e);
    if !view.wheel.mixed {
        if let Ok(speed) = view.wheel.speed.parse::<u32>() {
            let width = (speed.min(360) * 220 / 360) as i32;
            p.fill(right + 10, 316, width, 3, GREEN);
            p.fill(right + 6 + width, 312, 8, 11, GREEN);
        }
    }
    p.text(
        "ROTATION",
        right + 10,
        324,
        230,
        22,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    for (x, label, ccw) in [
        (right + 10, "CLOCKWISE", false),
        (right + 125, "COUNTER CW", true),
    ] {
        button(
            p,
            label,
            x,
            347,
            105,
            super::Control::Wheel(Control::Direction(ccw)),
            enabled,
        );
        if view
            .wheel
            .geometry
            .is_some_and(|g| g.counterclockwise == ccw)
        {
            p.fill(x, 374, 105, 2, GREEN);
        }
    }
    p.text(
        "CENTER X / Y",
        right + 10,
        377,
        230,
        20,
        p.fonts.small,
        TEXT,
        DT_LEFT,
    );
    button(
        p,
        "APPLY CENTER",
        right + 10,
        433,
        220,
        super::Control::Wheel(Control::Geometry),
        enabled,
    );
    button(
        p,
        if view.wheel.picking {
            "CLICK CANVAS…"
        } else {
            "SELECT CENTER POINT"
        },
        right + 10,
        473,
        220,
        super::Control::Wheel(Control::Center),
        enabled,
    );
    button(
        p,
        "WHEEL COLOR",
        right + 10,
        513,
        220,
        super::Control::Wheel(Control::Gradient),
        enabled && view.canvas.count() > 0,
    );
    button(
        p,
        "ERASE SELECTION",
        right + 10,
        553,
        220,
        super::Control::Erase,
        enabled && view.canvas.count() > 0,
    );
}
