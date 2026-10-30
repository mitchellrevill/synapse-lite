//! Explicit colour entry for Studio stops; persisted values are always RGB24.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Hex,
    Rgb,
    Hsv,
}
impl ColorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "R,G,B",
            Self::Hsv => "H,S,V",
        }
    }
    pub fn format(self, rgb: u32) -> String {
        let [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255];
        match self {
            Self::Hex => format!("{rgb:06X}"),
            Self::Rgb => format!("{r},{g},{b}"),
            Self::Hsv => {
                let [r, g, b] = [r, g, b].map(|c| c as f64 / 255.0);
                let max = r.max(g).max(b);
                let delta = max - r.min(g).min(b);
                let h = if delta == 0.0 {
                    0.0
                } else if max == r {
                    ((g - b) / delta).rem_euclid(6.0) * 60.0
                } else if max == g {
                    ((b - r) / delta + 2.0) * 60.0
                } else {
                    ((r - g) / delta + 4.0) * 60.0
                };
                let s = if max == 0.0 { 0.0 } else { delta / max * 100.0 };
                [h, s, max * 100.0]
                    .map(|n| {
                        format!("{n:.4}")
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_owned()
                    })
                    .join(",")
            }
        }
    }
    pub fn parse(self, text: &str) -> Result<u32, String> {
        let text = text.trim();
        if self == Self::Hex {
            let text = text.strip_prefix('#').unwrap_or(text);
            if text.len() == 6 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
                return u32::from_str_radix(text, 16).map_err(|_| "Enter six hex digits".into());
            }
            return Err("Enter six hex digits".into());
        }
        let channels = text.split(',').map(str::trim).collect::<Vec<_>>();
        if channels.len() != 3 {
            return Err(format!(
                "Enter three comma-separated {} values",
                self.label()
            ));
        }
        if self == Self::Rgb {
            let values = channels
                .iter()
                .map(|v| v.parse::<u8>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "RGB channels must be integers from 0 to 255")?;
            return Ok((values[0] as u32) << 16 | (values[1] as u32) << 8 | values[2] as u32);
        }
        let values = channels
            .iter()
            .map(|v| v.parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "HSV requires hue 0..360 and saturation/value 0..100")?;
        let [h, s, v] = [values[0], values[1], values[2]];
        if !h.is_finite()
            || !s.is_finite()
            || !v.is_finite()
            || !(0.0..=360.0).contains(&h)
            || !(0.0..=100.0).contains(&s)
            || !(0.0..=100.0).contains(&v)
        {
            return Err("HSV requires hue 0..360 and saturation/value 0..100".into());
        }
        let c = v / 100.0 * s / 100.0;
        let sector = h.rem_euclid(360.0) / 60.0;
        let x = c * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
        let m = v / 100.0 - c;
        let rgb = match sector as u8 {
            0 => [c, x, 0.0],
            1 => [x, c, 0.0],
            2 => [0.0, c, x],
            3 => [0.0, x, c],
            4 => [x, 0.0, c],
            _ => [c, 0.0, x],
        };
        let [r, g, b] = rgb.map(|n| ((n + m) * 255.0).round() as u32);
        Ok(r << 16 | g << 8 | b)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn studio_gradient_color_vectors_and_representations() {
        for (hsv, rgb) in [
            ("0,100,100", 0xff0000),
            ("60,100,100", 0xffff00),
            ("120,100,100", 0x00ff00),
            ("180,100,100", 0x00ffff),
            ("240,100,100", 0x0000ff),
            ("300,100,100", 0xff00ff),
            ("360,100,100", 0xff0000),
            ("180,0,50", 0x808080),
            ("30,100,100", 0xff8000),
        ] {
            assert_eq!(ColorMode::Hsv.parse(hsv).unwrap(), rgb);
        }
        // Sample the RGB cube including dark/near-grey values: changing modes
        // must not modify the eventual RGB byte, even without an anchor cache.
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let rgb = r << 16 | g << 8 | b;
                    for mode in [ColorMode::Hex, ColorMode::Rgb, ColorMode::Hsv] {
                        assert_eq!(mode.parse(&mode.format(rgb)).unwrap(), rgb);
                    }
                }
            }
        }
    }
    #[test]
    fn studio_gradient_color_invalid_input_refuses() {
        for text in ["-1,0,0", "256,0,0", "1.5,0,0", "1,2", "1,2,3,4"] {
            assert!(ColorMode::Rgb.parse(text).is_err());
        }
        for text in ["NaN,0,0", "0,inf,0", "361,0,0", "0,-1,0", "0,0,101"] {
            assert!(ColorMode::Hsv.parse(text).is_err());
        }
        for text in ["FFF", "FFFFFFF", "GGGGGG", "##112233"] {
            assert!(ColorMode::Hex.parse(text).is_err());
        }
        assert_eq!(ColorMode::Hex.parse(" #112233 ").unwrap(), 0x112233);
    }
}
