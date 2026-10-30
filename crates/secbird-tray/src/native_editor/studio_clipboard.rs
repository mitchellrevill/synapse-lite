//! Property actions share the draft's raw-preserving region/history path.
use super::*;
use secbird_core::studio::document::PropertyClipboard;

#[derive(Clone, Copy)]
pub enum Control {
    Copy,
    Paste,
    Reset,
    Remove,
    Dismiss,
}
#[derive(Default)]
pub(super) struct View {
    pub(super) clip: Option<PropertyClipboard>,
    pub menu: Option<(i32, i32)>,
    hover: Option<usize>,
}
fn eligibility(state: &State) -> [bool; 4] {
    let view = &state.studio;
    if !view.editable() || state.restricted_actions || state.read_only_smoke || view.output.busy {
        return [false; 4];
    }
    let (Some(draft), Some(id)) = (&view.draft, view.selected.as_deref()) else {
        return [false; 4];
    };
    let selection = view.canvas.selection();
    let one = !selection.is_empty() && selection.iter().all(|s| s.device == selection[0].device);
    let layer = draft
        .layers()
        .iter()
        .any(|row| row.id == id && row.kind == Kind::Layer);
    [
        one && draft.copy_selection_properties(id, &selection).is_ok(),
        !selection.is_empty()
            && view
                .properties
                .clip
                .as_ref()
                .is_some_and(|clip| draft.can_paste_properties(id, clip)),
        one && layer,
        one && layer,
    ]
}
pub(super) fn action(state: &mut State, control: Control) {
    state.studio.properties.menu = None;
    if matches!(control, Control::Dismiss) {
        return;
    }
    if !state.studio.editable()
        || state.restricted_actions
        || state.read_only_smoke
        || state.studio.output.busy
    {
        return;
    }
    let result = (|| -> Result<(), String> {
        let view = &mut state.studio;
        let id = view.selected.as_deref().ok_or("Select an effect layer")?;
        let selection = view.canvas.selection();
        let draft = view.draft.as_mut().ok_or("Open a composition first")?;
        match control {
            Control::Copy => {
                let clip = draft
                    .copy_selection_properties(id, &selection)
                    .map_err(|e| e.to_string())?;
                view.properties.clip = Some(clip);
                view.status = "Effect properties copied".into();
            }
            Control::Paste => {
                let clip = view
                    .properties
                    .clip
                    .as_ref()
                    .ok_or("Copy properties first")?;
                draft
                    .paste_selection_properties(id, &selection, clip)
                    .map_err(|e| e.to_string())?;
                view.status = "Properties pasted · save to keep changes".into();
            }
            Control::Reset => {
                draft
                    .reset_selection_properties(id, &selection)
                    .map_err(|e| e.to_string())?;
                view.status = "Effect defaults restored · save to keep changes".into();
            }
            Control::Remove => {
                draft
                    .remove_selection_effect(id, &selection)
                    .map_err(|e| e.to_string())?;
                view.canvas.clear_selection();
                view.status = "Effect removed from selected LEDs · save to keep changes".into();
            }
            Control::Dismiss => {}
        }
        view.failed = false;
        Ok(())
    })();
    if let Err(message) = result {
        error(state, message);
    }
    selection_changed(state);
}
const MENU_WIDTH: i32 = 256;
const MENU_HEIGHT: i32 = 183;
fn row_offset(index: usize) -> i32 {
    1 + index as i32 * 27 + if index >= 2 { 9 } else { 0 }
}
fn menu_position(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    (
        x.clamp(0, (width - MENU_WIDTH).max(0)),
        y.clamp(80, (height - MENU_HEIGHT).max(80)),
    )
}
pub(super) unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
    if !state.studio.open {
        return false;
    }
    let point = POINT {
        x: (lparam as u32 & 0xffff) as i16 as i32,
        y: ((lparam as u32 >> 16) & 0xffff) as i16 as i32,
    };
    let x = (point.x as f32 / state.scale) as i32;
    let y = (point.y as f32 / state.scale) as i32;
    let width = (state.width as f32 / state.scale) as i32;
    let height = (state.height as f32 / state.scale) as i32;
    if message == WM_RBUTTONUP && x > 250 && x < width - 250 && y >= 168 && y < height - 50 {
        if !state.studio.editable() || state.restricted_actions || state.read_only_smoke {
            return true;
        }
        cancel_gestures(state);
        state.studio.composition_menu = false;
        state.studio.canvas.context_target(
            source_layout::Point {
                x: f64::from(x),
                y: f64::from(y),
            },
            f64::from(width),
            f64::from(height),
            GetKeyState(17) < 0,
        );
        ReleaseCapture();
        state.studio.properties.menu = Some(menu_position(x, y, width, height));
        state.studio.properties.hover = None;
        state.studio.layer_menu = false;
        state.studio.dropdown = false;
        selection_changed(state);
        state.rebuild_fields();
        InvalidateRect(state.hwnd, null(), 0);
        return true;
    }
    if state.studio.properties.menu.is_none() {
        return false;
    }
    match message {
        WM_MOUSEMOVE => {
            let (left, top) = state.studio.properties.menu.unwrap();
            let hover = (0..4).find(|index| {
                x >= left
                    && x < left + MENU_WIDTH
                    && y >= top + row_offset(*index)
                    && y < top + row_offset(*index) + 27
            });
            if hover != state.studio.properties.hover {
                state.studio.properties.hover = hover;
                InvalidateRect(state.hwnd, null(), 0);
            }
            true
        }
        WM_LBUTTONDOWN => true,
        WM_LBUTTONUP => {
            let action = state
                .hits
                .iter()
                .rev()
                .find(|hit| PtInRect(&hit.rect, point) != 0)
                .and_then(|hit| match &hit.action {
                    Action::Studio(super::Control::Properties(control)) => Some(*control),
                    _ => None,
                })
                .unwrap_or(Control::Dismiss);
            state.action(Action::Studio(super::Control::Properties(action)));
            true
        }
        WM_CANCELMODE | WM_KILLFOCUS => {
            state.studio.properties.menu = None;
            InvalidateRect(state.hwnd, null(), 0);
            false
        }
        WM_MOUSEWHEEL => true,
        _ => false,
    }
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter) {
    let Some((x, y)) = state.studio.properties.menu else {
        return;
    };
    let enabled = eligibility(state);
    p.fill(x, y, MENU_WIDTH, MENU_HEIGHT, 0x5d5d5d);
    p.fill(x + 1, y + 1, MENU_WIDTH - 2, MENU_HEIGHT - 2, 0);
    p.hits.push(Hit {
        rect: p.rect(x, y, MENU_WIDTH, MENU_HEIGHT),
        action: Action::Studio(super::Control::Properties(Control::Dismiss)),
    });
    p.fill(x + 7, y + 59, MENU_WIDTH - 14, 1, 0x333333);
    for (index, (label, helper, control)) in [
        ("Copy", "Ctrl + C", Control::Copy),
        ("Paste", "Ctrl + V", Control::Paste),
        ("Reset Effect", "", Control::Reset),
        ("Remove Effect", "Del or Backspace", Control::Remove),
    ]
    .into_iter()
    .enumerate()
    {
        let row_y = y + row_offset(index);
        if enabled[index] && state.studio.properties.hover == Some(index) {
            p.fill(x + 1, row_y, MENU_WIDTH - 2, 27, 0x1a1a1a);
        }
        let color = if enabled[index] { TEXT } else { 0x454545 };
        p.text(
            label,
            x + 7,
            row_y,
            MENU_WIDTH - 14,
            27,
            p.fonts.normal,
            color,
            DT_LEFT,
        );
        p.text(
            helper,
            x + 7,
            row_y,
            MENU_WIDTH - 14,
            27,
            p.fonts.normal,
            color,
            DT_RIGHT,
        );
        if enabled[index] {
            p.hits.push(Hit {
                rect: p.rect(x + 1, row_y, MENU_WIDTH - 2, 27),
                action: Action::Studio(super::Control::Properties(control)),
            });
        }
    }
    // The property popup owns the canvas right-click. Retain the existing
    // Identify affordances here; neither receives a hit target until ownership
    // and exact lighting restoration are verified.
    p.fill(x + 7, y + 123, MENU_WIDTH - 14, 1, 0x333333);
    for (label, row_y) in [("Identify Device", y + 128), ("Identify LED", y + 155)] {
        p.text(label, x + 7, row_y, MENU_WIDTH - 14, 27, p.fonts.normal, 0x454545, DT_LEFT);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_menu_clamps_inside_client() {
        assert_eq!(menu_position(1000, 650, 1100, 700), (844, 517));
        assert_eq!(menu_position(400, 200, 1280, 720), (400, 200));
    }
}
