//! Baselines measured from inert shipped DOM/CSS with recovered source fonts.
//! These align the text origin; GDI hinting/advances are not Chromium pixel parity.
#[derive(Clone, Copy, Debug)]
pub(super) enum Role {
    Header,
    Category,
    Heading,
    DefaultValue,
    Dropdown,
    Options,
    TurboLabel,
    Button,
    Layer,
    MouseLabel,
}
impl Role {
    pub fn baseline(self) -> f64 {
        match self {
            Self::Header | Self::Category => 24.0,
            Self::Heading => 15.0,
            Self::DefaultValue | Self::MouseLabel => 19.0,
            Self::Dropdown | Self::Button => 18.0,
            Self::Options | Self::Layer => 17.0,
            Self::TurboLabel => 13.0,
        }
    }
    pub fn pixel(self, top: f64, scale: f64) -> Option<i32> {
        let y = (top + self.baseline()) * scale;
        if !top.is_finite()
            || !scale.is_finite()
            || scale <= 0.0
            || !y.is_finite()
            || y < i32::MIN as f64
            || y > i32::MAX as f64
        {
            return None;
        }
        Some(y.round() as i32)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_normal_popup_and_125_percent_baselines_are_not_font_em_heights() {
        // Inert605 replay: popup product126 +host42; header native133, heading189.
        assert_eq!(Role::Header.pixel(133.0, 1.25), Some(196));
        assert_eq!(Role::Heading.pixel(189.0, 1.25), Some(255));
        assert_eq!(Role::Category.pixel(169.0, 1.25), Some(241));
        assert_eq!(Role::MouseLabel.pixel(148.0, 1.25), Some(209));
    }
    #[test]
    fn invalid_measurement_scale_cannot_produce_a_text_coordinate() {
        assert_eq!(Role::Heading.pixel(f64::NAN, 1.25), None);
        assert_eq!(Role::Heading.pixel(189.0, 0.0), None);
    }
}
