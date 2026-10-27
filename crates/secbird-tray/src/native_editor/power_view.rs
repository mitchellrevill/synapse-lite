//! Source Power cards and commit-on-release interaction. See docs/POWER.md.
use super::*;
use performance_view::{faded, hit, rounded, text};
use power_layout::{PowerGeometry, Rect};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Sleep,
    Threshold,
}

pub(super) struct Gesture {
    device: String,
    kind: Kind,
    shape: Rect,
    value: u16,
}

fn r(r: Rect) -> performance_layout::Rect {
    performance_layout::Rect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
fn at(x: f64, y: f64, width: f64, height: f64) -> performance_layout::Rect {
    performance_layout::Rect {
        x,
        y,
        width,
        height,
    }
}

pub(super) fn layout(state: &State) -> Option<PowerGeometry> {
    let device = state.device()?;
    if state.tab != Tab::Power || !matches!(device["pid"].as_str(), Some("00C4" | "00C5")) {
        return None;
    }
    power_layout::geometry(
        f64::from(state.width) / f64::from(state.scale),
        128.0,
        0.0,
        device["critical_threshold"]["polling_hz"]
            .as_u64()
            .is_some_and(|hz| hz > 1000),
    )
}

pub(super) unsafe fn begin(state: &mut State, kind: Kind, shape: Rect) {
    let Some(device) = state.device() else { return };
    let value = match kind {
        Kind::Sleep => device["device_sleep"]["minutes"].as_u64(),
        Kind::Threshold => device["critical_threshold"]["percent"].as_u64(),
    };
    let Some(value) = value else { return };
    let available = match kind {
        Kind::Sleep => device["device_sleep"]["available"] == true,
        Kind::Threshold => device["critical_threshold"]["enabled"] == true,
    };
    if !available {
        return;
    }
    state.power_gesture = Some(Gesture {
        device: device["id"].as_str().unwrap_or_default().into(),
        kind,
        shape,
        value: value as u16,
    });
    SetCapture(state.hwnd);
}

pub(super) unsafe fn pointer(state: &mut State, point: POINT) {
    let selected = state.selected.as_deref();
    let Some(gesture) = state.power_gesture.as_mut() else {
        return;
    };
    if state.tab != Tab::Power || selected != Some(gesture.device.as_str()) {
        return;
    }
    let x = f64::from(point.x) / f64::from(state.scale);
    // Standard source range has a16px thumb. Native/browser transfer parity
    // remains a separate gate from recovered layout and commit semantics.
    let fraction = ((x - gesture.shape.x - 8.0) / (gesture.shape.width - 16.0)).clamp(0.0, 1.0);
    let (min, max, step) = match gesture.kind {
        Kind::Sleep => (1.0, 15.0, 1.0),
        Kind::Threshold => (5.0, 100.0, 5.0),
    };
    let value = (min + ((max - min) * fraction / step).round() * step) as u16;
    if gesture.value != value {
        gesture.value = value;
        InvalidateRect(state.hwnd, null(), 0);
    }
}

pub(super) unsafe fn finish(state: &mut State) {
    let Some(gesture) = state.power_gesture.take() else {
        return;
    };
    ReleaseCapture();
    if state.tab != Tab::Power || state.selected.as_deref() != Some(gesture.device.as_str()) {
        return;
    }
    let mut body = json!({"id":gesture.device,"expected_device":gesture.device});
    let path = match gesture.kind {
        Kind::Sleep => {
            body["minutes"] = json!(gesture.value);
            "/api/device-sleep"
        }
        Kind::Threshold => {
            body["critical_threshold_percent"] = json!(gesture.value);
            "/api/critical-threshold"
        }
    };
    state.queue(Job::Action(path, body));
    InvalidateRect(state.hwnd, null(), 0);
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, device: &Value) -> bool {
    let Some(g) = layout(state) else { return false };
    for (kind, card, title, description, min, max, low, high) in [
        (
            Kind::Sleep,
            g.sleep,
            "WIRELESS POWER SAVING",
            "Enter sleep mode if idle for (minutes)",
            1u16,
            15u16,
            "1",
            "15",
        ),
        (
            Kind::Threshold,
            g.threshold,
            "LOW POWER MODE",
            "When wireless, enter Low Power Mode if the battery level is below (%)",
            5,
            100,
            "5%",
            "100%",
        ),
    ] {
        rounded(p, r(card.bounds), PANEL, PANEL, 5.0);
        text(
            p,
            title,
            r(card.title),
            p.fonts.widget_heading,
            GREEN,
            DT_LEFT,
        );
        text(
            p,
            description,
            r(card.description),
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        let data = match kind {
            Kind::Sleep => &device["device_sleep"],
            Kind::Threshold => &device["critical_threshold"],
        };
        let enabled = data["available"] == true && (kind == Kind::Sleep || data["enabled"] == true);
        let value = state
            .power_gesture
            .as_ref()
            .filter(|gesture| {
                gesture.kind == kind && device["id"].as_str() == Some(gesture.device.as_str())
            })
            .map(|gesture| gesture.value)
            .or_else(|| {
                data[if kind == Kind::Sleep {
                    "minutes"
                } else {
                    "percent"
                }]
                .as_u64()
                .map(|v| v as u16)
            });
        let s = card.slider;
        let alpha = if enabled { 1.0 } else { 0.3 };
        let green = faded(GREEN, PANEL, alpha);
        let track = faded(faded(GREEN, PANEL, 77.0 / 255.0), PANEL, alpha);
        rounded(p, at(s.x, s.y + 33.0, s.width, 6.0), track, track, 3.0);
        if let Some(value) = value {
            let fraction = (f64::from(value.clamp(min, max) - min)) / f64::from(max - min);
            let center = 8.0 + fraction * (s.width - 16.0);
            rounded(p, at(s.x, s.y + 33.0, center, 6.0), green, green, 3.0);
            rounded(
                p,
                at(s.x + center - 8.0, s.y + 28.0, 16.0, 16.0),
                green,
                green,
                8.0,
            );
            // Generic source slider has no thumbTag, so its value bubble is
            // always visible. Position uses measured text width plus8px padding.
            let label = value.to_string();
            let encoded = wide(&label);
            let old_font = SelectObject(p.dc, p.fonts.small);
            let mut extent: SIZE = std::mem::zeroed();
            GetTextExtentPoint32W(p.dc, encoded.as_ptr(), label.len() as i32, &mut extent);
            SelectObject(p.dc, old_font);
            let bubble_width = f64::from(extent.cx) / f64::from(p.scale) + 16.0;
            let bubble = at(s.x + center - bubble_width / 2.0, s.y, bubble_width, 22.0);
            rounded(p, bubble, green, green, 3.0);
            text(
                p,
                &label,
                bubble,
                p.fonts.small,
                faded(0x212121, PANEL, alpha),
                DT_CENTER,
            );
        }
        text(
            p,
            low,
            at(s.x, s.y + 49.0, 60.0, 17.0),
            p.fonts.normal,
            faded(TEXT, PANEL, alpha),
            DT_LEFT,
        );
        text(
            p,
            high,
            at(s.x + s.width - 60.0, s.y + 49.0, 60.0, 17.0),
            p.fonts.normal,
            faded(TEXT, PANEL, alpha),
            DT_RIGHT,
        );
        if enabled {
            hit(
                p,
                at(s.x, s.y + 26.0, s.width, 20.0),
                Action::PowerSlider(kind, s),
            );
        }
        if let Some(warning) = card.warning {
            text(
                p,
                "Low Power Mode is not supported at polling rates of 2000 Hz and above.",
                r(warning),
                p.fonts.normal,
                TEXT,
                DT_LEFT,
            );
        } else if data["error"].as_str().is_some() || data["available"] != true {
            text(
                p,
                data["error"]
                    .as_str()
                    .unwrap_or("Power setting could not be read"),
                at(s.x, s.y, 520.0, 22.0),
                p.fonts.small,
                MUTED,
                DT_LEFT,
            );
        }
    }
    true
}
