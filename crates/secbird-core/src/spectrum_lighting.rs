//! Source Studio Spectrum parameter conversion and native initializer behaviour.
use crate::lighting_pixels::{Palette, Pixel, Stop};
use crate::studio::document::GradientStop;
use crate::{Error, Result, Rgb};
use std::time::Duration;

pub struct SpectrumCycle {
    palette: Palette,
    next: usize,
    native_fallback: bool,
}
impl SpectrumCycle {
    pub fn studio(stops: Option<&[GradientStop]>, duration_ms: Option<u32>) -> Result<Self> {
        let mut normalized = Vec::new();
        if let Some(stops) = stops.filter(|stops| !stops.is_empty()) {
            if stops.len() > 10 {
                return invalid("Spectrum native descriptor supports at most ten stops");
            }
            for stop in stops {
                if stop.position > 100 {
                    return invalid("Spectrum stop outside0..100 percent");
                }
                normalized.push(Stop {
                    percent: stop.position,
                    pixel: Pixel::from_studio_color(stop.color)?,
                });
            }
            // Match the actual converter: anchors are added using first/last
            // array colours. Its sorted temporary is discarded by case4.
            if !normalized.iter().any(|s| s.percent == 0) {
                normalized.insert(
                    0,
                    Stop {
                        percent: 0,
                        pixel: normalized[0].pixel,
                    },
                );
            }
            if !normalized.iter().any(|s| s.percent == 100) {
                normalized.push(Stop {
                    percent: 100,
                    pixel: normalized.last().unwrap().pixel,
                });
            }
            if normalized.len() > 10 {
                return invalid(
                    "Spectrum anchor expansion exceeds verified native descriptor capacity",
                );
            }
        }
        let native_fallback =
            normalized.len() < 2 || normalized.windows(2).any(|s| s[0].percent >= s[1].percent);
        if native_fallback {
            normalized = vec![
                Stop {
                    percent: 0,
                    pixel: Pixel::opaque(Rgb(255, 0, 0)),
                },
                Stop {
                    percent: 33,
                    pixel: Pixel::opaque(Rgb(0, 255, 0)),
                },
                Stop {
                    percent: 66,
                    pixel: Pixel::opaque(Rgb(0, 0, 255)),
                },
                Stop {
                    percent: 100,
                    pixel: Pixel::opaque(Rgb(255, 0, 0)),
                },
            ];
        }
        let duration = duration_ms.filter(|v| *v != 0).unwrap_or(37740);
        // Native fallback leaves its effective-stop-count field zero. Do not
        // reproduce an undersized allocation for malformed tiny durations.
        let frames = if native_fallback {
            (duration / 40) as usize
        } else {
            ((duration / 40) as usize).max(normalized.len())
        };
        if frames < normalized.len() {
            return invalid(
                "Spectrum fallback duration is too short for safe native reconstruction",
            );
        }
        let palette = Palette::new(&normalized, frames)?;
        Ok(Self {
            palette,
            next: 0,
            native_fallback,
        })
    }
    pub fn frame_count(&self) -> usize {
        self.palette.len()
    }
    pub fn frame_index(&self) -> usize {
        self.next
    }
    pub fn frame_interval(&self) -> Duration {
        Duration::from_millis(40)
    }
    pub fn used_native_fallback(&self) -> bool {
        self.native_fallback
    }
    pub fn storage_bytes(&self) -> usize {
        self.palette.storage_bytes()
    }
    pub fn pixel_at(&self, frame: usize) -> Pixel {
        self.palette.at(frame)
    }
    pub fn next_pixel(&mut self) -> Pixel {
        let value = self.palette.at(self.next);
        self.next = (self.next + 1) % self.palette.len();
        value
    }
    pub fn reset(&mut self) {
        self.next = 0;
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::InvalidArg(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_source_converter_replay_retains_unsorted_or_missing_stops_for_native_fallback() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/studio-native-codec-replay.json"
        ))
        .unwrap();
        for case in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["input"]["effect"] == 4)
        {
            let input = &case["input"]["params"];
            let stops: Option<Vec<GradientStop>> = input
                .get("colorStops")
                .map(|v| serde_json::from_value(v.clone()).unwrap());
            let cycle = SpectrumCycle::studio(
                stops.as_deref(),
                input
                    .get("duration")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            )
            .unwrap();
            assert!(cycle.used_native_fallback());
        }
        let passed = fixture["cases"][0]["output"]["params"]["ColorStops"]
            .as_array()
            .unwrap();
        let positions: Vec<_> = passed.iter().map(|s| s["Stop"].as_u64().unwrap()).collect();
        assert_eq!(positions, [0, 80, 20, 100]);
    }
    #[test]
    fn source_spectrum_matches_existing_full943_frame_reference() {
        let stops = [
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
        ]
        .map(|(position, color)| GradientStop {
            position,
            color: Some(color),
        });
        let mut cycle = SpectrumCycle::studio(Some(&stops), Some(37740)).unwrap();
        let expected = include_bytes!("../tests/fixtures/native-spectrum-static-derived.rgb");
        assert!(!cycle.used_native_fallback());
        assert_eq!(cycle.frame_count(), 943);
        for rgb in expected.chunks_exact(3) {
            assert_eq!(
                cycle.next_pixel(),
                Pixel::opaque(Rgb(rgb[0], rgb[1], rgb[2]))
            );
        }
        assert_eq!(cycle.frame_index(), 0);
    }
    #[test]
    fn duplicates_and_unsorted_imports_use_native_fallback_without_rewriting_input() {
        let repeated = [
            GradientStop {
                position: 0,
                color: Some(1),
            },
            GradientStop {
                position: 50,
                color: Some(2),
            },
            GradientStop {
                position: 50,
                color: None,
            },
            GradientStop {
                position: 100,
                color: Some(3),
            },
        ];
        let unsorted = [
            GradientStop {
                position: 80,
                color: Some(0xff0000),
            },
            GradientStop {
                position: 20,
                color: Some(0x00ff00),
            },
        ];
        let input = repeated.clone();
        let mut a = SpectrumCycle::studio(Some(&repeated), None).unwrap();
        let mut b = SpectrumCycle::studio(Some(&unsorted), None).unwrap();
        let mut default = SpectrumCycle::studio(None, Some(0)).unwrap();
        assert!(
            a.used_native_fallback() && b.used_native_fallback() && default.used_native_fallback()
        );
        for _ in 0..943 {
            assert_eq!(a.next_pixel(), b.next_pixel());
            assert_eq!(
                b.pixel_at((b.frame_index() + 942) % 943),
                default.next_pixel()
            );
        }
        assert_eq!(repeated, input);
        assert_eq!(a.pixel_at(0), Pixel::opaque(Rgb(255, 0, 0)));
        assert_eq!(a.pixel_at(943), a.pixel_at(0));
    }
    #[test]
    fn missing_anchors_none_and_raw_durations_follow_runtime_not_editor_projection() {
        let stops = [
            GradientStop {
                position: 20,
                color: None,
            },
            GradientStop {
                position: 80,
                color: Some(0x102030),
            },
        ];
        let cycle = SpectrumCycle::studio(Some(&stops), Some(1111)).unwrap();
        assert_eq!(cycle.frame_count(), 27);
        assert!(!cycle.used_native_fallback());
        assert_eq!(cycle.pixel_at(0), Pixel::EMPTY);
        assert_eq!(cycle.pixel_at(26), Pixel::opaque(Rgb(16, 32, 48)));
        assert!(cycle.storage_bytes() <= 108);
        let short = SpectrumCycle::studio(
            Some(&[
                GradientStop {
                    position: 0,
                    color: None,
                },
                GradientStop {
                    position: 100,
                    color: None,
                },
            ]),
            Some(1),
        )
        .unwrap();
        assert_eq!(short.frame_count(), 2);
        assert!(SpectrumCycle::studio(None, Some(1)).is_err());
    }
}
