//! Source root/card viewport rules. Dashboard/unaudited views retain the legacy footer.
pub(super) fn covered(pid: &str, tab: &str) -> bool {
    matches!(
        (pid, tab),
        ("025D", "CUSTOMIZE" | "LIGHTING")
            | ("00C4" | "00C5", "CUSTOMIZE" | "PERFORMANCE" | "POWER")
    )
}
pub(super) fn bottom(client_height: f64, source: bool) -> f64 {
    client_height - if source { 0.0 } else { 35.0 }
}
/// Source widget margin-bottom10 + BodyWrapper padding-bottom20.
pub(super) fn card_scroll_max(last_card_bottom: f64, viewport_bottom: f64) -> f64 {
    (last_card_bottom + 30.0 - viewport_bottom).max(0.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audited_views_only() {
        for (pid, tab) in [
            ("025D", "CUSTOMIZE"),
            ("025D", "LIGHTING"),
            ("00C4", "PERFORMANCE"),
            ("00C5", "POWER"),
        ] {
            assert!(covered(pid, tab));
        }
        for (pid, tab) in [
            ("FFF3", "LIGHTING"),
            ("025D", "HELP"),
            ("00C4", "CALIBRATION"),
            ("00C4", "LIGHTING"),
        ] {
            assert!(!covered(pid, tab));
        }
    }
    #[test]
    fn source_replay_scroll_extents_include_card_margin_and_padding() {
        assert_eq!(bottom(706.0, true), 706.0);
        assert_eq!(bottom(706.0, false), 671.0);
        // Native coordinates include the separate42px host origin.
        for (last, expected) in [
            (775.0 + 42.0, 141.0),
            (923.0 + 42.0, 289.0),
            (839.0 + 42.0, 205.0),
            (885.0 + 42.0, 251.0),
        ] {
            assert_eq!(card_scroll_max(last, 706.0), expected);
        }
        assert_eq!(card_scroll_max(653.0 + 42.0, 720.0), 5.0);
    }
}
