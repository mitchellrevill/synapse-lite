//! Capability-specific wheel controls. Pointer actions enqueue one worker job.
use super::*;

pub(super) unsafe fn paint(state: &State, p: &mut Painter<'_>, device: &Value) -> bool {
    let Some(controls) = device["wheel_controls"]
        .as_array()
        .filter(|v| !v.is_empty())
    else {
        return false;
    };
    let center = (state.width as f32 / state.scale) as i32 / 2;
    let left = center - 310;
    p.text(
        "SCROLL WHEEL",
        left,
        164,
        620,
        32,
        p.fonts.heading,
        GREEN,
        DT_LEFT,
    );
    p.text(
        "Onboard slot 1 · Source-supported; physical validation pending",
        left,
        201,
        620,
        28,
        p.fonts.small,
        MUTED,
        DT_LEFT,
    );
    for (i, item) in controls.iter().enumerate() {
        let y = 250 + i as i32 * 115;
        p.text(
            item["title"].as_str().unwrap_or("WHEEL CONTROL"),
            left,
            y,
            450,
            28,
            p.fonts.normal,
            TEXT,
            DT_LEFT,
        );
        let available = item["writable"] == true;
        let enabled = item["enabled"] == true;
        p.text(
            if enabled {
                "ON"
            } else if item["available"] == true {
                "OFF"
            } else {
                "UNAVAILABLE"
            },
            left,
            y + 31,
            130,
            24,
            p.fonts.small,
            MUTED,
            DT_LEFT,
        );
        if let Ok(control) = serde_json::from_value(item["control"].clone()) {
            p.button(
                if enabled { "TURN OFF" } else { "TURN ON" },
                left + 450,
                y,
                150,
                Action::WheelControl(
                    device["id"].as_str().unwrap_or_default().into(),
                    control,
                    !enabled,
                ),
                available,
            );
        }
        if let Some(error) = item["error"].as_str() {
            p.text(error, left, y + 58, 620, 44, p.fonts.small, MUTED, DT_LEFT);
        }
    }
    true
}
