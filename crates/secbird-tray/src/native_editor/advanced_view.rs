//! Source196 Advanced cards; worker owns IO, pointer drafts never write per frame.
use super::*;
use secbird_core::dynamic_sensitivity::{DynamicSensitivity, Rotation};
use secbird_core::sensor_curve::{Point, SensorCurve};

#[derive(Clone, Copy)]
pub(super) enum Control {
    Enable,
    Mode(u8),
    Template(u8),
    Zoom(bool, i8),
    Point(usize),
    Retry,
    RotationEnable,
    RotationSlider,
}
pub(super) struct View {
    owner: Option<(String, String)>,
    intent: DynamicSensitivity,
    available: bool,
    rotation: Rotation,
    rotation_available: bool,
    rotation_error: String,
    rotation_drag: Option<Rotation>,
    pub(super) drag: Option<(usize, DynamicSensitivity)>,
    pub(super) scroll: f64,
    gap_x: f64,
    gap_y: f64,
}
impl Default for View {
    fn default() -> Self {
        Self {
            owner: None,
            intent: Default::default(),
            available: false,
            rotation: Default::default(),
            rotation_available: false,
            rotation_error: String::new(),
            rotation_drag: None,
            drag: None,
            scroll: 0.,
            gap_x: 15.,
            gap_y: 0.25,
        }
    }
}
fn owner(state: &State) -> Option<(String, String)> {
    let d = state.device()?;
    if state.tab != Tab::Advanced || !matches!(d["pid"].as_str(), Some("00C4" | "00C5")) {
        return None;
    }
    Some((d["id"].as_str()?.into(), d["active"].as_str()?.into()))
}
pub(super) fn sync(state: &mut State) {
    let Some(current) = owner(state) else { return };
    if state.calibration.owner.as_ref() != Some(&current) {
        return;
    }
    let Some(snapshot) = state.calibration.snapshot.as_ref() else {
        return;
    };
    let data = &snapshot["dynamic"];
    let mut intent =
        serde_json::from_value::<DynamicSensitivity>(data["intent"].clone()).unwrap_or_default();
    // Hardware owns the active switch/mode; absent profile intent is never proof.
    if let Some(raw) = data["raw"].as_object() {
        if let Some(n) = raw.get("state").and_then(Value::as_u64) {
            intent.state = n as u8;
        }
        if let Some(n) = raw.get("mode").and_then(Value::as_u64) {
            intent.mode = n as u8;
        }
    }
    if state.advanced.owner.as_ref() != Some(&current) {
        state.advanced.scroll = 0.;
        state.advanced.gap_x = 15.;
        state.advanced.gap_y = 0.25;
    }
    state.advanced.owner = Some(current);
    state.advanced.intent = intent;
    state.advanced.available = data["available"] == true;
    state.advanced.drag = None;
    let rotation = &snapshot["rotation"];
    let mut intent =
        serde_json::from_value::<Rotation>(rotation["intent"].clone()).unwrap_or_default();
    if rotation["intent"].is_null() {
        if let Some(value) = rotation["observed"]["value"].as_i64() {
            intent.value = value as i32;
        }
        if let Some(on) = rotation["observed"]["enabled"].as_bool() {
            intent.is_enabled = on;
        }
    }
    state.advanced.rotation = intent;
    state.advanced.rotation_available = rotation["available"] == true;
    state.advanced.rotation_error = rotation["error"].as_str().unwrap_or("").into();
    state.advanced.rotation_drag = None;
}
fn allowed(state: &State) -> bool {
    owner(state).as_ref() == state.advanced.owner.as_ref()
        && !state.calibration.loading
        && state.pending == 0
        && !state.read_only_smoke
        && !state.restricted_actions
}
fn enabled(state: &State) -> bool {
    allowed(state) && state.advanced.available
}
pub(super) fn capturing(state: &State) -> bool {
    state.advanced.drag.is_some() || state.advanced.rotation_drag.is_some()
}
unsafe fn commit_rotation(state: &mut State) {
    let Some((id, profile)) = owner(state) else {
        return;
    };
    if !allowed(state) || !state.advanced.rotation_available {
        return;
    }
    state.queue(Job::Action("/api/sensor-rotation",json!({"id":id,"expected_device":id,"expected_profile":profile,"rotation":state.advanced.rotation})));
    calibration_view::request(state, true);
}
unsafe fn commit(state: &mut State) {
    let Some((id, profile)) = owner(state) else {
        return;
    };
    if !enabled(state) {
        return;
    }
    if let Err(error) = state.advanced.intent.validate() {
        state.error(&error.to_string());
        return;
    }
    state.queue(Job::Action("/api/dynamic-sensitivity",json!({"id":id,"expected_device":id,"expected_profile":profile,"dynamic_sensitivity":state.advanced.intent})));
    calibration_view::request(state, true);
}
pub(super) unsafe fn action(state: &mut State, control: Control) {
    if matches!(control, Control::Retry) {
        calibration_view::request(state, true);
        return;
    }
    if matches!(control, Control::RotationEnable | Control::RotationSlider) {
        if !allowed(state) || !state.advanced.rotation_available {
            return;
        }
        match control {
            Control::RotationEnable => {
                state.advanced.rotation.is_enabled = !state.advanced.rotation.is_enabled;
                commit_rotation(state);
            }
            Control::RotationSlider if state.advanced.rotation.is_enabled => {
                state.advanced.rotation_drag = Some(state.advanced.rotation.clone());
                SetCapture(state.hwnd);
            }
            _ => {}
        }
        InvalidateRect(state.hwnd, null(), 0);
        return;
    }
    if !enabled(state) {
        return;
    }
    let on = state.advanced.intent.state == 1;
    match control {
        Control::Enable => {
            state.advanced.intent.state = u8::from(!on);
            commit(state);
        }
        Control::Mode(mode) if on => {
            state.advanced.intent.mode = mode;
            if mode == 3 && state.advanced.intent.points.is_empty() {
                let seed = state.advanced.intent.template_id.min(2);
                if let Err(error) = state.advanced.intent.seed(seed) {
                    state.error(&error.to_string());
                    return;
                }
            }
            commit(state);
        }
        Control::Template(template) if on && state.advanced.intent.mode == 3 => {
            if let Err(error) = state.advanced.intent.seed(template) {
                state.error(&error.to_string());
                return;
            }
            commit(state);
        }
        Control::Zoom(horizontal, delta) if on && state.advanced.intent.mode == 3 => {
            if horizontal {
                state.advanced.gap_x =
                    (state.advanced.gap_x + f64::from(delta) * 5.).clamp(5., 40.);
            } else {
                state.advanced.gap_y =
                    (state.advanced.gap_y + f64::from(delta) * 0.25).clamp(0.25, 1.);
            }
        }
        Control::Point(index) if on && state.advanced.intent.mode == 3 => {
            if state
                .advanced
                .intent
                .control_points()
                .and_then(|p| SensorCurve::new(&p))
                .is_err()
            {
                state.error("This imported curve cannot be edited safely; select a template first");
                return;
            }
            state.advanced.drag = Some((index, state.advanced.intent.clone()));
            SetCapture(state.hwnd);
        }
        _ => {}
    }
    InvalidateRect(state.hwnd, null(), 0);
}
fn geometry(state: &State) -> (f64, f64) {
    let width = f64::from(state.width) / f64::from(state.scale);
    (
        ((width - 1240.) / 2.).max(0.) + 10.,
        343. - state.advanced.scroll,
    )
}
pub(super) unsafe fn pointer(state: &mut State, point: POINT) {
    if state.advanced.rotation_drag.is_some() {
        if !allowed(state) {
            cancel(state);
            return;
        }
        let width = f64::from(state.width) / f64::from(state.scale);
        let left = (width - 600.) / 2. + 40.;
        let x = f64::from(point.x) / f64::from(state.scale);
        state.advanced.rotation.value =
            (((x - left - 8.) / 504.).clamp(0., 1.) * 88. - 44.).round() as i32;
        InvalidateRect(state.hwnd, null(), 0);
        return;
    }
    let Some((index, _)) = &state.advanced.drag else {
        return;
    };
    let index = *index;
    if !enabled(state) {
        cancel(state);
        return;
    }
    let (card_x, chart_y) = geometry(state);
    let chart_x = card_x + 40.;
    let x = f64::from(point.x) / f64::from(state.scale);
    let y = f64::from(point.y) / f64::from(state.scale);
    let n = state.advanced.intent.points.len();
    if index >= n {
        return;
    }
    let max_x = 7. * state.advanced.gap_x;
    if max_x < 2. * (n - 1) as f64 {
        cancel(state);
        state.error("Zoom out before editing this curve");
        return;
    }
    let target_x = if index == 0 {
        0.
    } else {
        ((x - chart_x - 70.) / 1010. * max_x)
            .clamp(2. * index as f64, max_x - 2. * (n - 1 - index) as f64)
    };
    let target_y = ((chart_y + 280. - y) / 260. * 6. * state.advanced.gap_y)
        .clamp(0.1, 6. * state.advanced.gap_y);
    let rows = &mut state.advanced.intent.points;
    rows[index]["x"] = json!(target_x);
    rows[index]["y"] = json!(target_y);
    for i in (1..index).rev() {
        let limit = rows[i + 1]["x"].as_f64().unwrap() - 2.;
        if rows[i]["x"].as_f64().unwrap() > limit {
            rows[i]["x"] = json!(limit);
        }
    }
    for i in index + 1..n {
        let limit = rows[i - 1]["x"].as_f64().unwrap() + 2.;
        if rows[i]["x"].as_f64().unwrap() < limit {
            rows[i]["x"] = json!(limit);
        }
    }
    InvalidateRect(state.hwnd, null(), 0);
}
pub(super) unsafe fn finish(state: &mut State) {
    if let Some(before) = state.advanced.rotation_drag.take() {
        ReleaseCapture();
        if state.advanced.rotation != before {
            commit_rotation(state);
        }
        return;
    }
    if let Some((_, before)) = state.advanced.drag.take() {
        ReleaseCapture();
        if state.advanced.intent != before {
            match state
                .advanced
                .intent
                .control_points()
                .and_then(|p| SensorCurve::new(&p))
                .and_then(|c| c.firmware_samples())
            {
                Ok(samples) => {
                    state.advanced.intent.custom_sensor_accelerations = samples.to_vec();
                    commit(state);
                }
                Err(error) => {
                    state.advanced.intent = before;
                    state.error(&error.to_string());
                }
            }
        }
    }
}
pub(super) fn cancel(state: &mut State) {
    if let Some((_, before)) = state.advanced.drag.take() {
        state.advanced.intent = before;
    }
    if let Some(before) = state.advanced.rotation_drag.take() {
        state.advanced.rotation = before;
    }
}
pub(super) unsafe fn wheel(state: &mut State, delta: i16) {
    if capturing(state) {
        return;
    }
    state.advanced.scroll = (state.advanced.scroll - f64::from(delta) / 120. * 60.)
        .clamp(0., (1123. - state.content_bottom_css()).max(0.));
    InvalidateRect(state.hwnd, null(), 0);
}
unsafe fn segment(p: &Painter<'_>, a: (f64, f64), b: (f64, f64), color: u32, width: i32) {
    let pen = CreatePen(PS_SOLID, p.px(width).max(1), rgb(color));
    let old = SelectObject(p.dc, pen);
    MoveToEx(
        p.dc,
        p.px(a.0.round() as i32),
        p.px(a.1.round() as i32),
        null_mut(),
    );
    LineTo(p.dc, p.px(b.0.round() as i32), p.px(b.1.round() as i32));
    SelectObject(p.dc, old);
    DeleteObject(pen);
}
pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>) {
    if owner(state).is_none() {
        return;
    }
    let saved = SaveDC(p.dc);
    let bottom = state.content_bottom_css();
    let clip = p.rect(
        0,
        128,
        (f64::from(state.width) / f64::from(state.scale)).round() as i32,
        (bottom - 128.).max(0.) as i32,
    );
    IntersectClipRect(p.dc, clip.left, clip.top, clip.right, clip.bottom);
    let (card_x, chart_y) = geometry(state);
    let x = card_x.round() as i32;
    let y = (148. - state.advanced.scroll).round() as i32;
    performance_view::rounded(
        p,
        performance_layout::Rect {
            x: card_x,
            y: y as f64,
            width: 1182.,
            height: 555.,
        },
        PANEL,
        PANEL,
        5.,
    );
    let available = state.advanced.available;
    let enabled = enabled(state);
    let on = state.advanced.intent.state == 1;
    let custom = state.advanced.intent.mode == 3;
    let color = if available { GREEN } else { MUTED };
    p.text(
        "DYNAMIC SENSITIVITY",
        x + 40,
        y + 40,
        190,
        22,
        p.fonts.widget_heading,
        color,
        DT_LEFT,
    );
    p.button(
        if on { "ON" } else { "OFF" },
        x + 240,
        y + 40,
        50,
        Action::Advanced(Control::Enable),
        on,
    );
    p.text(
        "Select a dynamic sensitivity preset or customize your own sensitivity curve.",
        x + 40,
        y + 82,
        1102,
        17,
        p.fonts.normal,
        if on { TEXT } else { MUTED },
        DT_LEFT,
    );
    for (mode, label) in [(0, "CLASSIC"), (1, "NATURAL"), (2, "JUMP"), (3, "CUSTOM")] {
        let bx = x + 40 + i32::from(mode) * 278;
        performance_view::rounded(
            p,
            performance_layout::Rect {
                x: bx as f64,
                y: (y + 119) as f64,
                width: 268.,
                height: 27.,
            },
            if state.advanced.intent.mode == mode {
                0x292929
            } else {
                PANEL
            },
            if on && state.advanced.intent.mode == mode {
                GREEN
            } else {
                0x5d5d5d
            },
            3.,
        );
        p.text(
            label,
            bx,
            y + 119,
            268,
            27,
            p.fonts.small,
            if on { TEXT } else { MUTED },
            DT_CENTER,
        );
        if enabled && on {
            p.hits.push(Hit {
                rect: p.rect(bx, y + 119, 268, 27),
                action: Action::Advanced(Control::Mode(mode)),
            });
        }
    }
    let cy = chart_y.round() as i32;
    let cx = x + 40;
    if custom {
        p.text(
            "SELECT A TEMPLATE",
            cx + 580,
            cy - 37,
            180,
            27,
            p.fonts.small,
            TEXT,
            DT_LEFT,
        );
        for (i, label) in ["Classic", "Natural", "Jump", "None"].iter().enumerate() {
            p.button(
                label,
                cx + 760 + i as i32 * 85,
                cy - 37,
                80,
                Action::Advanced(Control::Template(i as u8)),
                state.advanced.intent.template_id == i as u8,
            );
        }
        for (horizontal, zx, zy) in [(false, cx, cy - 37), (true, cx + 1042, cy + 320)] {
            p.button(
                "−",
                zx,
                zy,
                30,
                Action::Advanced(Control::Zoom(horizontal, 1)),
                false,
            );
            p.button(
                "+",
                zx + 30,
                zy,
                30,
                Action::Advanced(Control::Zoom(horizontal, -1)),
                false,
            );
        }
    }
    let (max_x, start_y, gap_y) = if custom {
        (7. * state.advanced.gap_x, 0., state.advanced.gap_y)
    } else {
        (70., 1., 0.1)
    };
    for tick in 0..=7 {
        let gx = f64::from(cx) + 70. + 1010. * f64::from(tick) / 7.;
        segment(p, (gx, chart_y + 13.), (gx, chart_y + 287.), 0x333333, 1);
        p.text(
            &format!("{:.0}", max_x * f64::from(tick) / 7.),
            gx as i32 - 20,
            cy + 281,
            40,
            16,
            p.fonts.small,
            MUTED,
            DT_CENTER,
        );
    }
    for tick in 0..=6 {
        let gy = chart_y + 280. - 260. * f64::from(tick) / 6.;
        segment(
            p,
            (f64::from(cx) + 58., gy),
            (f64::from(cx) + 1092., gy),
            0x333333,
            1,
        );
        if tick % 2 == 0 {
            p.text(
                &format!("{:.2}", start_y + gap_y * f64::from(tick)),
                cx + 10,
                gy as i32 - 8,
                42,
                16,
                p.fonts.small,
                MUTED,
                DT_RIGHT,
            );
        }
    }
    p.text(
        "Ratio of Output to Input",
        cx + 5,
        cy + 2,
        180,
        17,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    p.text(
        "Input Speed (count/ms)",
        cx,
        cy + 320,
        1102,
        17,
        p.fonts.normal,
        TEXT,
        DT_CENTER,
    );
    let mut preset = DynamicSensitivity::default();
    let _ = preset.seed(state.advanced.intent.mode.min(2));
    let points = if custom {
        state.advanced.intent.control_points().ok()
    } else {
        preset.control_points().ok()
    };
    if available {
        if let Some(points) = points {
            if let Ok(curve) = SensorCurve::new(&points) {
                let clip_save = SaveDC(p.dc);
                let r = p.rect(cx + 66, cy + 16, 1018, 264);
                IntersectClipRect(p.dc, r.left, r.top, r.right, r.bottom);
                let map = |v: Point| {
                    (
                        f64::from(cx) + 70. + v.x / max_x * 1010.,
                        chart_y + 280. - (v.y - start_y) / (6. * gap_y) * 260.,
                    )
                };
                let line_color = if !on {
                    0x36532f
                } else if custom {
                    0xe4790f
                } else {
                    GREEN
                };
                let mut previous = None;
                for n in 0..=1010 {
                    let speed = f64::from(n) / 1010. * max_x;
                    if let Ok(ratio) = curve.interpolate(speed) {
                        let current = map(Point { x: speed, y: ratio });
                        if let Some(last) = previous {
                            segment(p, last, current, line_color, 1);
                        }
                        previous = Some(current);
                    }
                }
                if custom {
                    for (index, point) in points.iter().enumerate() {
                        let (px, py) = map(*point);
                        let brush = CreateSolidBrush(rgb(line_color));
                        let old = SelectObject(p.dc, brush);
                        let pen = SelectObject(p.dc, GetStockObject(NULL_PEN));
                        let r = p.rect(px as i32 - 4, py as i32 - 4, 8, 8);
                        Ellipse(p.dc, r.left, r.top, r.right, r.bottom);
                        SelectObject(p.dc, old);
                        SelectObject(p.dc, pen);
                        DeleteObject(brush);
                        if enabled && on && point.x <= max_x {
                            p.hits.push(Hit {
                                rect: p.rect(px as i32 - 9, py as i32 - 9, 18, 18),
                                action: Action::Advanced(Control::Point(index)),
                            });
                        }
                    }
                }
                RestoreDC(p.dc, clip_save);
            } else {
                p.text(
                    "Curve data cannot be edited safely. Select a template to begin.",
                    cx + 100,
                    cy + 120,
                    850,
                    25,
                    p.fonts.normal,
                    MUTED,
                    DT_CENTER,
                );
            }
        }
    }
    if !available {
        p.text(
            if state.calibration.loading {
                "Reading sensor settings…"
            } else {
                "Unable to read Advanced settings from the mouse."
            },
            cx + 100,
            cy + 120,
            850,
            25,
            p.fonts.normal,
            MUTED,
            DT_CENTER,
        );
        if !state.calibration.loading {
            p.button(
                "RETRY",
                cx + 500,
                cy + 160,
                100,
                Action::Advanced(Control::Retry),
                false,
            );
        }
    }
    // Rotation uses its own source-backed controller; never simulate it with cursor hooks.
    let rx = ((f64::from(state.width) / f64::from(state.scale) - 600.) / 2.).round() as i32;
    let ry = (723. - state.advanced.scroll).round() as i32;
    performance_view::rounded(
        p,
        performance_layout::Rect {
            x: rx as f64,
            y: ry as f64,
            width: 600.,
            height: 380.,
        },
        PANEL,
        PANEL,
        5.,
    );
    p.text(
        "ROTATION",
        rx + 40,
        ry + 30,
        200,
        22,
        p.fonts.widget_heading,
        GREEN,
        DT_LEFT,
    );
    p.text(
        "Rotate the mouse's angle to get straight horizontal movement with any kind of grip style.",
        rx + 40,
        ry + 72,
        520,
        40,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    let rotation = &state.advanced.rotation;
    let rotation_enabled = allowed(state) && state.advanced.rotation_available;
    p.button(
        if rotation.is_enabled { "ON" } else { "OFF" },
        rx + 155,
        ry + 30,
        50,
        Action::Advanced(Control::RotationEnable),
        rotation.is_enabled,
    );
    if state.advanced.rotation_available {
        for n in 0..=32 {
            p.fill(rx + 218 + n * 5, ry + 196, 2, 2, 0x707070);
            p.fill(rx + 299, ry + 116 + n * 5, 2, 2, 0x707070);
        }
        if let Some(image) = state.icons.get("rotation-mouse") {
            image_render::draw_rotated(
                p.dc,
                image.smooth,
                p.rect(rx + 265, ry + 148, 70, 108),
                image.width,
                image.height,
                rotation.value as f32,
            );
        }
        let slider_y = ry + 329;
        p.fill(rx + 48, slider_y, 504, 6, 0x5d5d5d);
        let thumb = rx + 48 + ((rotation.value + 44) as f64 / 88. * 504.).round() as i32;
        p.fill(
            thumb - 8,
            slider_y - 5,
            16,
            16,
            if rotation.is_enabled { GREEN } else { MUTED },
        );
        p.text(
            "-44°",
            rx + 40,
            ry + 296,
            80,
            18,
            p.fonts.small,
            MUTED,
            DT_LEFT,
        );
        p.text(
            "44°",
            rx + 480,
            ry + 296,
            80,
            18,
            p.fonts.small,
            MUTED,
            DT_RIGHT,
        );
        p.text(
            &format!("{}°", rotation.value),
            rx + 240,
            ry + 343,
            120,
            18,
            p.fonts.small,
            TEXT,
            DT_CENTER,
        );
        if rotation_enabled && rotation.is_enabled {
            p.hits.push(Hit {
                rect: p.rect(rx + 40, slider_y - 12, 520, 30),
                action: Action::Advanced(Control::RotationSlider),
            });
        }
    } else {
        p.text(
            if state.calibration.loading {
                "Reading Rotation settings…"
            } else {
                "Rotation is unavailable for this connection."
            },
            rx + 40,
            ry + 170,
            520,
            25,
            p.fonts.normal,
            MUTED,
            DT_CENTER,
        );
        p.text(
            &state.advanced.rotation_error,
            rx + 40,
            ry + 210,
            520,
            60,
            p.fonts.small,
            MUTED,
            DT_LEFT,
        );
        if !state.calibration.loading {
            p.button(
                "RETRY",
                rx + 250,
                ry + 285,
                100,
                Action::Advanced(Control::Retry),
                false,
            );
        }
    }
    RestoreDC(p.dc, saved);
}
#[cfg(test)]
pub(super) fn diagnostics(state: &State) -> Value {
    json!({"available":state.advanced.available,"intent":state.advanced.intent,"drag":capturing(state),"scroll":state.advanced.scroll,"rotation":state.advanced.rotation,"rotation_available":state.advanced.rotation_available})
}
