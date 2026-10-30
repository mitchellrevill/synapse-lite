//! Source Customize assignment-popup shell; no renderer, mapping state or I/O.
//! Authority:605 chunk1270.c45bee4e and196 chunk257.8f5c638b,
//! main CSS plus KeyMappingComponent.b396b242; inert source-font replay.
//! Product viewport excludes the separate42px AppEngine host. Popup positioning
//! arguments are CSS pixels from the actual key/body DOM equivalents, not outer
//! window bounds. Effect/category-specific form heights are intentionally absent.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

pub(super) const POPUP_WIDTH: f64 = 292.0;
pub(super) const WRAPPER_TOP: f64 = 90.0;
pub(super) const HEADER_HEIGHT: f64 = 36.0;
pub(super) const DRAWER_WIDTH: f64 = 230.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shell {
    pub popup: Rect,
    pub header: Rect,
    pub close: Rect,
    pub rail: Rect,
    pub body: Rect,
    pub heading: Rect,
}

pub(super) fn shell(
    product_viewport_height: f64,
    host_origin: f64,
    popup_left: f64,
    inline_top: f64,
    rail_expanded: bool,
) -> Option<Shell> {
    if ![product_viewport_height, host_origin, popup_left, inline_top]
        .into_iter()
        .all(f64::is_finite)
        || product_viewport_height <= 0.0
    {
        return None;
    }
    let y = host_origin + WRAPPER_TOP + inline_top;
    let height = (product_viewport_height - 140.0).clamp(310.0, 570.0);
    let popup = rect(popup_left, y, POPUP_WIDTH, height);
    // Absolute children are relative to the1px border's padding edge. Header
    // top:-36 therefore places its outer edge35px above the popup outer edge.
    let header = rect(
        popup_left,
        y + 1.0 - HEADER_HEIGHT,
        POPUP_WIDTH,
        HEADER_HEIGHT,
    );
    Some(Shell {
        popup,
        header,
        close: rect(header.x + 255.0, header.y + 1.0, 36.0, 36.0),
        rail: rect(
            popup_left + 1.0,
            y + 1.0,
            if rail_expanded { 250.0 } else { 40.0 },
            height - 2.0,
        ),
        body: rect(popup_left + 41.0, y + 1.0, 250.0, height - 2.0),
        heading: rect(popup_left + 61.0, y + 21.0, 210.0, 19.0),
    })
}

/// Literal calculateKeyConfigPopupLeft behavior, including the source's strict
///20px clearance and230px panel clamp. Body scroll affects side selection only;
/// the final x-coordinate does not add that scroll offset.
pub(super) fn popup_left(
    key: Rect,
    body_client_width: Option<f64>,
    body_scroll_left: f64,
    popup_width: f64,
    panel_open: bool,
    from_panel: bool,
    drawer_client_width: f64,
) -> f64 {
    if panel_open && from_panel {
        return drawer_client_width;
    }
    let fits_right = body_client_width
        .is_none_or(|width| key.x + key.width + body_scroll_left + popup_width < width - 20.0);
    let left = if fits_right {
        key.x + key.width
    } else {
        key.x - popup_width
    };
    if panel_open && left < drawer_client_width {
        DRAWER_WIDTH
    } else {
        left.max(0.0)
    }
}

/// Normal non-two-tap callback positioning. The parent must supply the actual
/// key anchor. The source config-wrapper includes mouse label rows as well as
/// keyboard SVG keys; command-dial and panel transitions have additional source states.
#[cfg(test)]
pub(super) fn popup_inline_top(
    from_panel: bool,
    within_config_wrapper: bool,
    body_scroll_top: f64,
    key_viewport_top: f64,
    command_dial_viewport_top: Option<f64>,
) -> f64 {
    if from_panel {
        return 36.0;
    }
    if within_config_wrapper {
        return 36.0 - body_scroll_top;
    }
    match command_dial_viewport_top {
        Some(top) if top < 450.0 => 36.0,
        Some(_) => key_viewport_top - 560.0,
        None => key_viewport_top - 218.0,
    }
}

pub(super) fn category_rows(shell: Shell, count: usize, scroll: f64) -> (Vec<Rect>, f64) {
    let maximum = (count as f64 * 40.0 - shell.rail.height).max(0.0);
    let scroll = scroll.clamp(0.0, maximum);
    (
        (0..count)
            .map(|index| {
                rect(
                    shell.rail.x,
                    shell.rail.y + index as f64 * 40.0 - scroll,
                    shell.rail.width,
                    40.0,
                )
            })
            .collect(),
        maximum,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_matches_wide_normal_and_narrow_inert_source_replays() {
        for (viewport, expected_height) in [(678.0, 538.0), (664.0, 524.0), (594.0, 454.0)] {
            for expanded in [false, true] {
                let g = shell(viewport, 0.0, 100.0, 36.0, expanded).unwrap();
                assert_eq!(g.popup, rect(100.0, 126.0, 292.0, expected_height));
                assert_eq!(g.header, rect(100.0, 91.0, 292.0, 36.0));
                assert_eq!(g.close, rect(355.0, 92.0, 36.0, 36.0));
                assert_eq!(g.body, rect(141.0, 127.0, 250.0, expected_height - 2.0));
                assert_eq!(g.heading, rect(161.0, 147.0, 210.0, 19.0));
                assert_eq!(g.rail.width, if expanded { 250.0 } else { 40.0 });
            }
        }
    }

    #[test]
    fn host_origin_is_separate_and_height_limits_are_source_limits() {
        let native = shell(678.0, 42.0, 100.0, 36.0, false).unwrap();
        assert_eq!(native.popup.y, 168.0);
        assert_eq!(native.header.y, 133.0);
        assert_eq!(
            shell(1000.0, 0.0, 0.0, 36.0, false).unwrap().popup.height,
            570.0
        );
        assert_eq!(
            shell(400.0, 0.0, 0.0, 36.0, false).unwrap().popup.height,
            310.0
        );
        assert!(shell(f64::NAN, 0.0, 0.0, 36.0, false).is_none());
    }

    #[test]
    fn anchor_side_selection_uses_strict_clearance_and_body_scroll() {
        let key = rect(200.0, 300.0, 30.0, 20.0);
        assert_eq!(
            popup_left(key, Some(1000.0), 0.0, 292.0, false, false, 0.0),
            230.0
        );
        // Equality with the available right edge chooses the left side.
        assert_eq!(
            popup_left(key, Some(542.0), 0.0, 292.0, false, false, 0.0),
            0.0
        );
        assert_eq!(
            popup_left(key, Some(1000.0), 500.0, 292.0, false, false, 0.0),
            0.0
        );
        assert_eq!(
            popup_left(key, None, 500.0, 292.0, false, false, 0.0),
            230.0
        );
        assert_eq!(
            popup_left(key, Some(1000.0), 0.0, 292.0, true, true, 230.0),
            230.0
        );
        assert_eq!(
            popup_left(key, Some(542.0), 0.0, 292.0, true, false, 230.0),
            230.0
        );
    }

    #[test]
    fn callback_top_branches_retain_the_source_thresholds() {
        assert_eq!(popup_inline_top(true, true, 50.0, 300.0, None), 36.0);
        assert_eq!(popup_inline_top(false, true, 50.0, 300.0, None), -14.0);
        assert_eq!(popup_inline_top(false, false, 0.0, 300.0, None), 82.0);
        assert_eq!(
            popup_inline_top(false, false, 0.0, 600.0, Some(449.0)),
            36.0
        );
        assert_eq!(
            popup_inline_top(false, false, 0.0, 600.0, Some(450.0)),
            40.0
        );
    }

    #[test]
    fn source_category_order_is_supplied_not_invented_or_sorted() {
        let g = shell(678.0, 0.0, 100.0, 36.0, true).unwrap();
        let (rows, maximum) = category_rows(g, 15, 0.0);
        assert_eq!(rows[0], rect(101.0, 127.0, 250.0, 40.0));
        assert_eq!(maximum, 64.0);
        let (scrolled, _) = category_rows(g, 15, 64.0);
        assert_eq!(scrolled[14].y + 40.0, g.rail.y + g.rail.height);
    }
}
