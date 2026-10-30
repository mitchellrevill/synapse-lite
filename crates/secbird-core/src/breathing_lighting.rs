//! Fixed/random and finite Studio Breathing5, distinct from Quick presets.
//! Recovered converter + native initializer/frame records are documented in
//! STUDIO-BREATHING.md. The caller supplies time; no input dispatch or device IO.
use crate::{
    lighting_pixels::{Palette, Pixel, Stop},
    studio::document::GradientStop,
    Error, Result, Rgb,
};

pub const DURATIONS: [u32; 3] = [28_000, 14_000, 6_000];
pub struct BreathingCycle {
    palette: Palette,
    next: usize,
    remaining: Option<u32>,
    random: Option<u32>,
    active: bool,
    triggered: bool,
    toggle: bool,
    last_pressed: bool,
    cycles: i64,
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
            let first = Pixel::from_studio_color(first.or(second).filter(|c| *c != 0x64000000))?;
            let second = Pixel::from_studio_color(second.filter(|c| *c != 0x64000000))?;
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
            remaining: None,
            random: None,
            active: true,
            triggered: false,
            toggle: false,
            last_pressed: false,
            cycles: -1,
        })
    }
    /// The native effect owns this stream separately from Fire's CRT stream.
    /// Supply Unix seconds for its recovered 1,800,000-second seed bucket.
    pub fn random(duration: Option<u32>, unix_seconds: u64) -> Result<Self> {
        let stops = random_stops(0xffffff);
        let mut cycle = Self::studio(None, None, Some(&stops), duration)?;
        cycle.random = Some(((unix_seconds / 1_800_000) * 1_800_000 | 31) as u32);
        Ok(cycle)
    }
    pub fn set_cycles(&mut self, cycles: i64) -> Result<()> {
        self.remaining = match cycles {
            -1 | 0 => None,
            1..=2147483647 => Some(cycles as u32),
            _ => return invalid("Breathing cycles must be -1, zero or a positive signed32 count"),
        };
        self.cycles = cycles;
        Ok(())
    }
    /// Synthetic/owned input only. Production preview deliberately does not
    /// connect this method to native input until its ownership gate is proven.
    pub fn wait_for_input(&mut self, toggle: bool) {
        self.triggered = true;
        self.toggle = toggle;
        self.active = false;
        self.last_pressed = false;
    }
    /// Mirrors native trigger's qualifying-event and press-state edge guards.
    /// The caller determines whether an event belongs to the selected keys.
    pub fn input_edge(&mut self, pressed: bool, qualifies: bool) {
        if !self.triggered || !qualifies || self.last_pressed == pressed {
            return;
        }
        self.last_pressed = pressed;
        if !pressed || (self.active && !self.toggle) {
            return;
        }
        self.remaining = if self.active {
            Some(0)
        } else if self.cycles == -1 {
            None
        } else {
            Some(self.cycles as u32)
        };
        self.active = true;
        self.next = 0;
    }
    pub fn next_pixel(&mut self) -> Pixel {
        if !self.active || self.remaining == Some(0) {
            self.active = false;
            return Pixel::EMPTY;
        }
        let pixel = self.palette.at(self.next);
        // Native frame emits the current pixel before rebuilding at index zero.
        if self.next == 0 {
            if let Some(state) = &mut self.random {
                let color = random_color(state);
                let stops: Vec<_> = random_stops(color)
                    .into_iter()
                    .map(|s| Stop {
                        percent: s.position,
                        pixel: Pixel::from_studio_color(s.color).unwrap(),
                    })
                    .collect();
                self.palette = Palette::new(&stops, self.palette.len()).unwrap();
            }
        }
        self.next = (self.next + 1) % self.palette.len();
        if self.next == 0 {
            if let Some(remaining) = &mut self.remaining {
                *remaining -= 1;
            }
        }
        pixel
    }
    pub fn frame_count(&self) -> usize {
        self.palette.len()
    }
    pub fn storage_bytes(&self) -> usize {
        self.palette.storage_bytes()
    }
}
fn random_stops(color: u32) -> [GradientStop; 5] {
    [(0, 0), (25, 0), (50, color), (75, color), (100, 0)].map(|(position, color)| GradientStop {
        position,
        color: Some(color),
    })
}
fn random_color(state: &mut u32) -> u32 {
    loop {
        let channels = std::array::from_fn::<_, 3, _>(|_| {
            *state = state.wrapping_mul(214013).wrapping_add(2531011);
            let value = ((*state >> 16) ^ *state) % 255;
            if value > 170 {
                255u8
            } else if value > 85 {
                128
            } else {
                0
            }
        });
        if channels[0] != channels[1] || channels[1] != channels[2] {
            // Native helper writes BGR bytes; Studio's descriptor uses RGB24.
            return u32::from(channels[2]) << 16
                | u32::from(channels[1]) << 8
                | u32::from(channels[0]);
        }
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
    fn explicit_picker_none_does_not_fall_back_to_the_second_color() {
        let mut none =
            BreathingCycle::studio(Some(0x64000000), Some(0xff0000), None, Some(6000)).unwrap();
        let mut missing = BreathingCycle::studio(None, Some(0xff0000), None, Some(6000)).unwrap();
        for _ in 0..40 {
            none.next_pixel();
            missing.next_pixel();
        }
        assert_eq!(none.next_pixel(), Pixel::EMPTY);
        assert_eq!(missing.next_pixel(), Pixel::opaque(Rgb(255, 0, 0)));
    }
    #[test]
    fn synthetic_input_obeys_qualification_edges_repeats_and_toggle() {
        let build = |cycles, toggle| {
            let mut cycle =
                BreathingCycle::studio(Some(0xff0000), Some(0x00ff00), None, Some(40)).unwrap();
            cycle.set_cycles(cycles).unwrap();
            cycle.wait_for_input(toggle);
            cycle
        };
        let mut cycle = build(1, false);
        assert_eq!(cycle.next_pixel(), Pixel::EMPTY);
        cycle.input_edge(true, false);
        assert_eq!(cycle.next_pixel(), Pixel::EMPTY);
        cycle.input_edge(true, true);
        assert_ne!(cycle.next_pixel(), Pixel::EMPTY);
        let next = cycle.next;
        cycle.input_edge(true, true);
        assert_eq!(cycle.next, next, "held press cannot retrigger");
        cycle.input_edge(false, true);
        cycle.input_edge(true, true);
        assert_eq!(cycle.next, next, "active onPress ignores another press");
        for _ in 1..cycle.frame_count() {
            cycle.next_pixel();
        }
        assert_eq!(cycle.next_pixel(), Pixel::EMPTY);
        cycle.input_edge(false, true);
        cycle.input_edge(true, true);
        assert_ne!(cycle.next_pixel(), Pixel::EMPTY);
        let mut toggle = build(-1, true);
        toggle.input_edge(true, true);
        assert_ne!(toggle.next_pixel(), Pixel::EMPTY);
        toggle.input_edge(false, true);
        assert_ne!(toggle.next_pixel(), Pixel::EMPTY);
        toggle.input_edge(true, true);
        assert_eq!(toggle.next_pixel(), Pixel::EMPTY);
        toggle.input_edge(false, true);
        toggle.input_edge(true, true);
        assert_ne!(toggle.next_pixel(), Pixel::EMPTY);
        // Native init normalizes zero to infinite, but trigger copies zero as-is.
        let mut zero = build(0, false);
        zero.input_edge(true, true);
        assert_eq!(zero.next_pixel(), Pixel::EMPTY);
    }
    #[test]
    fn random_stream_matches160_wide_arithmetic_reference_colors_and_states() {
        let reference: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/studio-breathing-random.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let seconds = case["unix_seconds"].as_u64().unwrap();
            let mut state = ((seconds / 1_800_000) * 1_800_000 | 31) as u32;
            let mut cycle = BreathingCycle::random(Some(6000), seconds).unwrap();
            for sample in case["samples"].as_array().unwrap() {
                let bgr = sample["bgr"].as_array().unwrap();
                let expected = (bgr[2].as_u64().unwrap() << 16
                    | bgr[1].as_u64().unwrap() << 8
                    | bgr[0].as_u64().unwrap()) as u32;
                assert_eq!(random_color(&mut state), expected);
                assert_eq!(u64::from(state), sample["state"].as_u64().unwrap());
                for frame in 0..cycle.frame_count() {
                    let pixel = cycle.next_pixel();
                    if frame == 0 {
                        assert_eq!(cycle.random, Some(state));
                        assert_eq!(pixel, Pixel::opaque(Rgb(0, 0, 0)));
                    }
                    if frame == 90 {
                        assert_eq!(pixel, Pixel::from_studio_color(Some(expected)).unwrap());
                    }
                }
            }
        }
    }
    #[test]
    fn finite_cycles_complete_at_wrap_and_zero_and_minus_one_are_continuous() {
        for random in [false, true] {
            let build = || {
                if random {
                    BreathingCycle::random(Some(40), 0).unwrap()
                } else {
                    BreathingCycle::studio(Some(1), Some(2), None, Some(40)).unwrap()
                }
            };
            for repeats in [1, 2, 3] {
                let mut cycle = build();
                cycle.set_cycles(repeats).unwrap();
                for _ in 0..repeats as usize * cycle.frame_count() {
                    assert_ne!(cycle.next_pixel(), Pixel::EMPTY);
                }
                let state = cycle.random;
                for _ in 0..20 {
                    assert_eq!(cycle.next_pixel(), Pixel::EMPTY);
                    assert_eq!(cycle.random, state);
                }
            }
            for count in [-1, 0] {
                let mut cycle = build();
                cycle.set_cycles(count).unwrap();
                for _ in 0..100 {
                    assert_ne!(cycle.next_pixel(), Pixel::EMPTY);
                }
            }
        }
    }
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
