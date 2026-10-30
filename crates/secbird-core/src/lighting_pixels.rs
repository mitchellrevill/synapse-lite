//! Native lighting palette and layer composition math, without scheduling or I/O.
//!
//! The fourth source byte is transparency in percent, not ordinary RGBA alpha.
//! `docs/studio.md` records the instruction and codec evidence.
use crate::{Error, Result, Rgb};

/// A source pixel with ordinary RGB and bounded inverse coverage (0 is opaque).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pixel {
    pub color: Rgb,
    transparency: u8,
}
impl Pixel {
    pub const EMPTY: Self = Self {
        color: Rgb(0, 0, 0),
        transparency: 100,
    };
    pub const fn opaque(color: Rgb) -> Self {
        Self {
            color,
            transparency: 0,
        }
    }
    pub fn new(color: Rgb, transparency: u8) -> Result<Self> {
        if transparency > 100 {
            return invalid("pixel transparency must be 0..100 percent");
        }
        Ok(Self {
            color,
            transparency,
        })
    }
    pub fn from_studio_color(color: Option<u32>) -> Result<Self> {
        let Some(color) = color else {
            return Ok(Self::EMPTY);
        };
        if color > 0xffffff {
            return invalid("Studio colour must be RGB24 or None");
        }
        Ok(Self::opaque(Rgb(
            (color >> 16) as u8,
            (color >> 8) as u8,
            color as u8,
        )))
    }
    pub fn transparency(self) -> u8 {
        self.transparency
    }
    fn channels(self) -> [u8; 4] {
        [self.color.0, self.color.1, self.color.2, self.transparency]
    }
    fn from_channels(c: [u8; 4]) -> Self {
        Self {
            color: Rgb(c[0], c[1], c[2]),
            transparency: c[3],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stop {
    pub percent: u8,
    pub pixel: Pixel,
}

/// Native four-channel table interpolation, including repeated f32 rounding.
/// Callers supply already normalized/sorted stops including 0 and 100 anchors.
#[derive(Clone, Debug)]
pub struct Palette {
    pixels: Box<[Pixel]>,
}
impl Palette {
    pub fn new(stops: &[Stop], frames: usize) -> Result<Self> {
        if !(2..=10).contains(&stops.len()) || frames < stops.len() || frames > 16384 {
            return invalid("palette requires 2..10 stops and stop_count..16384 frames");
        }
        if stops[0].percent != 0
            || stops.last().unwrap().percent != 100
            || stops.iter().any(|s| s.percent > 100)
            || stops
                .windows(2)
                .any(|pair| pair[0].percent > pair[1].percent)
        {
            return invalid("palette stops must be sorted from 0 to 100; duplicates are allowed");
        }
        let mut percentages: Vec<_> = stops.iter().map(|stop| u32::from(stop.percent)).collect();
        for index in 0..stops.len() - 1 {
            if percentages[index] == 100 {
                break;
            }
            let start = percentages[index] * frames as u32 / 100;
            if percentages[index + 1] * frames as u32 / 100 <= start {
                percentages[index + 1] =
                    (((f64::from(start + 1) * 100.0) / frames as f64 + 1.0) as u32).min(100);
            }
        }
        let mut pixels = vec![Pixel::opaque(Rgb(0, 0, 0)); frames];
        for index in 0..stops.len() - 1 {
            if percentages[index] == 100 {
                break;
            }
            let start = percentages[index] as usize * frames / 100;
            let limit = percentages[index + 1] as usize * frames / 100;
            pixels[start] = stops[index].pixel;
            if start + 1 < limit {
                let end = limit - 1;
                pixels[end] = stops[index + 1].pixel;
                let a = stops[index].pixel.channels();
                let b = stops[index + 1].pixel.channels();
                let mut channels = a.map(f32::from);
                let delta = std::array::from_fn::<_, 4, _>(|c| {
                    (f32::from(b[c]) - channels[c]) / (end - start) as f32
                });
                for pixel in &mut pixels[start + 1..end] {
                    for c in 0..4 {
                        channels[c] += delta[c];
                    }
                    *pixel = Pixel::from_channels(channels.map(|c| c as u8));
                }
                // Preserve the native one-slot final-segment branch quirk.
                if percentages[index + 1] == 100 {
                    pixels[0] = stops[0].pixel;
                    pixels[frames - 1] = stops[index + 1].pixel;
                    break;
                }
            }
        }
        Ok(Self {
            pixels: pixels.into_boxed_slice(),
        })
    }
    pub fn len(&self) -> usize {
        self.pixels.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }
    pub fn at(&self, frame: usize) -> Pixel {
        self.pixels[frame % self.len()]
    }
    pub fn storage_bytes(&self) -> usize {
        self.len() * std::mem::size_of::<Pixel>()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BlendMode {
    /// Weight from inverse-coverage byte, then opacity and optional mask.
    Transparency,
    /// Native mode bit0x20: black contributes nothing, nonblack uses opacity.
    NonBlack,
}
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    pub pixels: &'a [Pixel],
    pub opacity: f32,
    pub mask: Option<&'a [u8]>,
    pub mode: BlendMode,
}
impl Layer<'_> {
    fn validate(&self, count: usize) -> Result<()> {
        if self.pixels.len() != count
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
            || self
                .mask
                .is_some_and(|mask| mask.len() != count || mask.iter().any(|v| *v > 100))
        {
            return invalid("invalid lighting layer length, opacity or percentage mask");
        }
        Ok(())
    }
    fn blend_validated(&self, output: &mut [Rgb]) {
        for (index, (dest, pixel)) in output.iter_mut().zip(self.pixels).enumerate() {
            let mut weight = match self.mode {
                BlendMode::Transparency => {
                    (f32::from(100 - pixel.transparency) * self.opacity) / 100.0
                }
                BlendMode::NonBlack => {
                    if pixel.color == Rgb(0, 0, 0) {
                        0.0
                    } else {
                        self.opacity
                    }
                }
            };
            if let Some(mask) = self.mask {
                weight = (weight * f32::from(mask[index])) / 100.0;
            }
            // CVTTSS2SI truncates the signed difference before adding the old
            // channel. Truncating a fully interpolated colour is not equivalent.
            let channel = |old: u8, new: u8| {
                (i32::from(old) + ((i32::from(new) - i32::from(old)) as f32 * weight) as i32) as u8
            };
            *dest = Rgb(
                channel(dest.0, pixel.color.0),
                channel(dest.1, pixel.color.1),
                channel(dest.2, pixel.color.2),
            );
        }
    }
    /// Blend into existing RGB, refusing invalid input before any output change.
    pub fn blend(&self, output: &mut [Rgb]) -> Result<()> {
        self.validate(output.len())?;
        self.blend_validated(output);
        Ok(())
    }
}
/// Compose caller-ordered layers from black. Validate every layer before clearing
/// output. Callers must flatten/reverse the source's visible layer order.
pub fn compose(output: &mut [Rgb], layers: &[Layer<'_>]) -> Result<()> {
    for layer in layers {
        layer.validate(output.len())?;
    }
    output.fill(Rgb(0, 0, 0));
    for layer in layers {
        layer.blend_validated(output);
    }
    Ok(())
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::Unsupported(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn palette_matches_independent_four_channel_native_reference() {
        let cases: Value =
            serde_json::from_str(include_str!("../tests/fixtures/native_pixel_palette.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let stops: Vec<_> = case["stops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(case["colors"].as_array().unwrap())
                .map(|(position, color)| Stop {
                    percent: position.as_u64().unwrap() as u8,
                    pixel: Pixel::new(
                        Rgb(
                            color[0].as_u64().unwrap() as u8,
                            color[1].as_u64().unwrap() as u8,
                            color[2].as_u64().unwrap() as u8,
                        ),
                        color[3].as_u64().unwrap() as u8,
                    )
                    .unwrap(),
                })
                .collect();
            let palette = Palette::new(&stops, case["frames"].as_u64().unwrap() as usize).unwrap();
            let actual: Vec<_> = (0..palette.len())
                .map(|i| palette.at(i).channels())
                .collect();
            assert_eq!(serde_json::to_value(actual).unwrap(), case["pixels"]);
            assert_eq!(palette.at(palette.len()), palette.at(0));
        }
    }
    #[test]
    fn rgb_projection_matches_all_existing_spectrum_frames() {
        let mut cycle = crate::uniform_lighting::UniformCycle::spectrum();
        let input = [
            (0, 0xff18ff),
            (18, 0xff1010),
            (28, 0xff8000),
            (35, 0xefff00),
            (42, 0xdfff00),
            (49, 0x00ff00),
            (52, 0x00ffff),
            (64, 0x1858ff),
            (77, 0x1858ff),
            (100, 0xff18ff),
        ];
        let stops: Vec<_> = input
            .into_iter()
            .map(|(percent, color)| Stop {
                percent,
                pixel: Pixel::from_studio_color(Some(color)).unwrap(),
            })
            .collect();
        let expected = include_bytes!("../tests/fixtures/native-spectrum-static-derived.rgb");
        let palette = Palette::new(&stops, 943).unwrap();
        for frame in 0..943 {
            assert_eq!(palette.at(frame).color, cycle.next_color());
            assert_eq!(
                palette.at(frame).color,
                Rgb(
                    expected[frame * 3],
                    expected[frame * 3 + 1],
                    expected[frame * 3 + 2]
                )
            );
            assert_eq!(palette.at(frame).transparency(), 0);
        }
    }
    #[test]
    fn blending_matches_589824_independent_signed_truncation_channel_pairs() {
        let cases: Value =
            serde_json::from_str(include_str!("../tests/fixtures/native_pixel_blend.json"))
                .unwrap();
        let source: Vec<_> = (0..=255)
            .flat_map(|_| 0..=255)
            .map(|new| Rgb(new, new, new))
            .collect();
        for case in cases.as_array().unwrap() {
            let pixels: Vec<_> = source
                .iter()
                .map(|&rgb| Pixel::new(rgb, case["transparency"].as_u64().unwrap() as u8).unwrap())
                .collect();
            let mut output: Vec<_> = (0..=255)
                .flat_map(|old| std::iter::repeat_n(Rgb(old, old, old), 256))
                .collect();
            let mask = case["mask"].as_u64().map(|v| vec![v as u8; output.len()]);
            Layer {
                pixels: &pixels,
                opacity: case["opacity"].as_f64().unwrap() as f32,
                mask: mask.as_deref(),
                mode: if case["nonblack"] == true {
                    BlendMode::NonBlack
                } else {
                    BlendMode::Transparency
                },
            }
            .blend(&mut output)
            .unwrap();
            assert!(output.iter().all(|p| p.0 == p.1 && p.1 == p.2));
            let bytes: Vec<_> = output.iter().map(|p| p.0).collect();
            assert_eq!(
                format!("{:x}", md5::compute(bytes)),
                case["md5"].as_str().unwrap()
            );
        }
    }
    #[test]
    fn compose_respects_caller_order_masks_transparency_and_preflights_before_clear() {
        let lower = [Pixel::opaque(Rgb(255, 128, 64)); 2];
        let upper = [Pixel::new(Rgb(0, 0, 0), 50).unwrap(), Pixel::EMPTY];
        let layers = [
            Layer {
                pixels: &lower,
                opacity: 1.0,
                mask: None,
                mode: BlendMode::Transparency,
            },
            Layer {
                pixels: &upper,
                opacity: 1.0,
                mask: None,
                mode: BlendMode::Transparency,
            },
        ];
        let mut output = [Rgb(7, 8, 9); 2];
        compose(&mut output, &layers).unwrap();
        assert_eq!(output, [Rgb(128, 64, 32), Rgb(255, 128, 64)]);
        let before = output;
        let invalid = Layer {
            pixels: &upper,
            opacity: f32::NAN,
            mask: None,
            mode: BlendMode::Transparency,
        };
        assert!(compose(&mut output, &[layers[0], invalid]).is_err());
        assert_eq!(output, before);
        let invalid = Layer {
            pixels: &upper,
            opacity: 1.0,
            mask: Some(&[100, 101]),
            mode: BlendMode::Transparency,
        };
        assert!(invalid.blend(&mut output).is_err());
        assert_eq!(output, before);
        compose(&mut output, &[]).unwrap();
        assert_eq!(output, [Rgb(0, 0, 0); 2]);
    }
    #[test]
    fn native_none_is_not_black_or_standard_255_alpha() {
        assert_eq!(Pixel::from_studio_color(None).unwrap(), Pixel::EMPTY);
        assert_ne!(Pixel::from_studio_color(Some(0)).unwrap(), Pixel::EMPTY);
        assert!(Pixel::new(Rgb(0, 0, 0), 255).is_err());
        assert!(Pixel::from_studio_color(Some(0x64000000)).is_err());
        assert!(Palette::new(
            &[
                Stop {
                    percent: 1,
                    pixel: Pixel::EMPTY
                },
                Stop {
                    percent: 100,
                    pixel: Pixel::EMPTY
                }
            ],
            5
        )
        .is_err());
    }
}
