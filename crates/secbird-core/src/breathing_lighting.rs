//! Deterministic continuous Studio Breathing5, distinct from Quick presets.
//! Recovered converter + native initializer/frame records are documented in
//! STUDIO-BREATHING.md. No random clock, input dispatch, scheduling or device IO.
use crate::{
    lighting_pixels::{Palette, Pixel, Stop},
    studio::document::GradientStop,
    Error, Result, Rgb,
};

pub const DURATIONS: [u32; 3] = [28_000, 14_000, 6_000];
pub struct BreathingCycle {
    palette: Palette,
    next: usize,
}
impl BreathingCycle {
    pub fn studio(
        first: Option<u32>,
        second: Option<u32>,
        stops: Option<&[GradientStop]>,
        duration: Option<u32>,
    ) -> Result<Self> {
        let mut normalized = Vec::new();
        if let Some(stops) = stops {
            if stops.len() > 10 {
                return invalid("Breathing descriptor supports at most ten stops");
            }
            for stop in stops {
                if stop.position > 100 {
                    return invalid("Breathing stop is outside0..100");
                }
                normalized.push(Stop {
                    percent: stop.position,
                    pixel: Pixel::from_studio_color(stop.color)?,
                });
            }
            if !normalized.is_empty() {
                if !normalized.iter().any(|stop| stop.percent == 0) {
                    normalized.insert(
                        0,
                        Stop {
                            percent: 0,
                            pixel: normalized[0].pixel,
                        },
                    );
                }
                if !normalized.iter().any(|stop| stop.percent == 100) {
                    normalized.push(Stop {
                        percent: 100,
                        pixel: normalized.last().unwrap().pixel,
                    });
                }
            }
        } else {
            // The source converter falls back from missing color1 to color2.
            let first = Pixel::from_studio_color(first.or(second))?;
            let second = Pixel::from_studio_color(second)?;
            let black = Pixel::opaque(Rgb(0, 0, 0));
            normalized = vec![
                Stop {
                    percent: 0,
                    pixel: black,
                },
                Stop {
                    percent: 12,
                    pixel: black,
                },
                Stop {
                    percent: 25,
                    pixel: first,
                },
                Stop {
                    percent: 38,
                    pixel: first,
                },
                Stop {
                    percent: 50,
                    pixel: black,
                },
                Stop {
                    percent: 62,
                    pixel: black,
                },
                Stop {
                    percent: 75,
                    pixel: second,
                },
                Stop {
                    percent: 88,
                    pixel: second,
                },
                Stop {
                    percent: 100,
                    pixel: black,
                },
            ];
        }
        if normalized.len() < 2
            || normalized.len() > 10
            || normalized
                .windows(2)
                .any(|pair| pair[0].percent >= pair[1].percent)
        {
            // Native invalid-stop fallback sets random mode and a time-derived
            // seed. Do not replace it with an invented deterministic white loop.
            return invalid("Breathing invalid/duplicate/unsorted stops require unimplemented native random fallback");
        }
        let duration = duration
            .filter(|duration| *duration != 0)
            .unwrap_or(DURATIONS[1]);
        let frames = ((duration / 40) as usize).max(normalized.len());
        Ok(Self {
            palette: Palette::new(&normalized, frames)?,
            next: 0,
        })
    }
    pub fn next_pixel(&mut self) -> Pixel {
        let pixel = self.palette.at(self.next);
        self.next = (self.next + 1) % self.palette.len();
        pixel
    }
    pub fn frame_count(&self) -> usize {
        self.palette.len()
    }
    pub fn storage_bytes(&self) -> usize {
        self.palette.storage_bytes()
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::Unsupported(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn all1952_palette_pixels_match_independent_shipped_converter_native_reference() {
        let reference: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/studio-breathing-reference.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let p = &case["params"];
            let stops: Option<Vec<GradientStop>> = p
                .get("colorStops")
                .map(|raw| serde_json::from_value(raw.clone()).unwrap());
            let mut cycle = BreathingCycle::studio(
                p["color"].as_u64().map(|v| v as u32),
                p["color2"].as_u64().map(|v| v as u32),
                stops.as_deref(),
                p["duration"].as_u64().map(|v| v as u32),
            )
            .unwrap();
            assert_eq!(cycle.frame_count() as u64, case["frames"].as_u64().unwrap());
            let mut bytes = vec![];
            for index in 0..cycle.frame_count() {
                let pixel = cycle.next_pixel();
                bytes.extend([
                    pixel.color.0,
                    pixel.color.1,
                    pixel.color.2,
                    pixel.transparency(),
                ]);
                for sample in case["samples"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|sample| sample["frame"] == index)
                {
                    assert_eq!(
                        serde_json::json!(&bytes[bytes.len() - 4..]),
                        sample["pixel"]
                    );
                }
            }
            assert_eq!(
                format!("{:x}", md5::compute(&bytes)),
                case["pixel_md5"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            let wrap = cycle.next_pixel();
            assert_eq!(
                [
                    wrap.color.0,
                    wrap.color.1,
                    wrap.color.2,
                    wrap.transparency()
                ],
                bytes[..4]
            );
            assert!(cycle.storage_bytes() <= 65536);
        }
    }
    #[test]
    fn random_fallback_and_unbounded_tables_are_explicitly_refused() {
        for stops in [
            vec![],
            vec![
                GradientStop {
                    position: 0,
                    color: Some(1),
                },
                GradientStop {
                    position: 0,
                    color: Some(2),
                },
                GradientStop {
                    position: 100,
                    color: None,
                },
            ],
            vec![
                GradientStop {
                    position: 80,
                    color: Some(1),
                },
                GradientStop {
                    position: 20,
                    color: Some(2),
                },
            ],
        ] {
            assert!(BreathingCycle::studio(None, None, Some(&stops), None).is_err());
        }
        assert!(BreathingCycle::studio(Some(0x1000000), None, None, None).is_err());
        assert!(BreathingCycle::studio(None, None, None, Some(u32::MAX)).is_err());
        assert_eq!(DURATIONS, [28000, 14000, 6000]);
    }
}
