//! Deterministic uniform lighting derived from the shipped native engine.
//!
//! These are the source Quick presets: Spectrum UI 3 / native 4 and Breathing
//! UI 2 / native 5. Those IDs are not firmware effect IDs. The native engine
//! builds a color table, then advances one entry per render call. This module
//! reproduces that ordinary RGB math; it does not schedule, synchronize an
//! existing animation, communicate with a device, or prove vendor runtime parity.
//!
//! Evidence and rounding details: `docs/UNIFORM-LIGHTING.md`.

use crate::{Error, Result, Rgb};
use std::time::Duration;

pub const SOURCE_QUICK_FPS: u16 = 25;
pub const MAX_COLOR_STOPS: usize = 10;
/// At most 49,152 bytes of RGB table storage per cycle, with no frame history.
pub const MAX_FRAME_COUNT: usize = 16_384;

/// A whole percentage and an ordinary RGB color, with no alpha/sentinel channel.
///
/// `from_stops` rejects fractional, nonfinite and out-of-range percentages.
#[derive(Debug, Clone, Copy)]
pub struct ColorStop {
    pub percent: f64,
    pub color: Rgb,
}

impl ColorStop {
    pub const fn new(percent: f64, color: Rgb) -> Self {
        Self { percent, color }
    }
}

/// An infinite cycle of one RGB color per render call.
///
/// Keep one instance per animation. Use `frame_interval` in the caller's
/// scheduler and `next_color` exactly once for each intended frame. Choosing a
/// catch-up/drop policy or restoring another owner's phase is outside this API.
#[derive(Debug, Clone)]
pub struct UniformCycle {
    colors: Box<[Rgb]>,
    next: usize,
    interval: Duration,
}

impl UniformCycle {
    /// Build a bounded, normalized palette using the recovered native RGB math.
    ///
    /// Stops must increase strictly from 0 to 100, with 2..=10 whole percentages.
    /// Duration must be positive and fps must be 1..=1000. Quantization uses
    /// integer `1000 / fps`, then `duration_ms / interval_ms`. Very short cycles
    /// retain at least one table entry per stop. Invalid native fallback/sentinel
    /// behavior is deliberately outside this ordinary RGB interface.
    pub fn from_stops(stops: &[ColorStop], duration_ms: u32, fps: u16) -> Result<Self> {
        if !(2..=MAX_COLOR_STOPS).contains(&stops.len()) {
            return invalid("uniform lighting requires 2..=10 color stops");
        }
        if duration_ms == 0 || !(1..=1000).contains(&fps) {
            return invalid("uniform lighting requires positive duration and fps in 1..=1000");
        }

        let mut percentages = [0_u32; MAX_COLOR_STOPS];
        for (index, stop) in stops.iter().enumerate() {
            let percent = stop.percent;
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) || percent.fract() != 0.0 {
                return invalid("color stop percentages must be finite whole numbers in 0..=100");
            }
            percentages[index] = percent as u32;
            if index > 0 && percentages[index] <= percentages[index - 1] {
                return invalid("color stop percentages must increase strictly");
            }
        }
        if percentages[0] != 0 || percentages[stops.len() - 1] != 100 {
            return invalid("color stops must begin at 0 and end at 100");
        }

        let interval_ms = 1000 / u32::from(fps);
        let frame_count = (duration_ms / interval_ms).max(stops.len() as u32) as usize;
        if frame_count > MAX_FRAME_COUNT {
            return invalid("uniform lighting table exceeds the 16384 frame limit");
        }

        // Native RVA 692E0 moves colliding quantized stops forward before filling
        // the table. Keep this adjustment local; never mutate the caller's stops.
        for index in 0..stops.len() - 1 {
            if percentages[index] == 100 {
                break;
            }
            let start = percentages[index] * frame_count as u32 / 100;
            let next = percentages[index + 1] * frame_count as u32 / 100;
            if next <= start {
                let moved = ((f64::from(start + 1) * 100.0) / frame_count as f64 + 1.0) as u32;
                percentages[index + 1] = moved.min(100);
            }
        }

        let mut colors = vec![Rgb(0, 0, 0); frame_count];
        for index in 0..stops.len() - 1 {
            if percentages[index] == 100 {
                break;
            }
            let start = percentages[index] as usize * frame_count / 100;
            let limit = percentages[index + 1] as usize * frame_count / 100;
            let first = stops[index].color;
            let last = stops[index + 1].color;
            colors[start] = first;

            // The native endpoint is limit-1, followed by another exact copy at
            // the next segment's start. A one-slot segment stores only `first`.
            if start + 1 < limit {
                let end = limit - 1;
                colors[end] = last;
                let distance = (end - start) as f32;
                let mut channels = [f32::from(first.0), f32::from(first.1), f32::from(first.2)];
                let delta = [
                    (f32::from(last.0) - channels[0]) / distance,
                    (f32::from(last.1) - channels[1]) / distance,
                    (f32::from(last.2) - channels[2]) / distance,
                ];
                for color in &mut colors[start + 1..end] {
                    // Native ADDPS rounds after every addition. Computing t or
                    // multiplying the delta by the index changes some RGB bytes.
                    for channel in 0..3 {
                        channels[channel] += delta[channel];
                    }
                    *color = Rgb(channels[0] as u8, channels[1] as u8, channels[2] as u8);
                }
                if percentages[index + 1] == 100 {
                    // Native 69419 bypasses this restoration for a one-slot
                    // final segment; that segment retains its starting color.
                    colors[0] = stops[0].color;
                    colors[frame_count - 1] = last;
                    break;
                }
            }
        }

        Ok(Self {
            colors: colors.into_boxed_slice(),
            next: 0,
            interval: Duration::from_millis(u64::from(interval_ms)),
        })
    }

    /// The shipped ten-stop Spectrum preset at 25 fps, with 943 table entries.
    pub fn spectrum() -> Self {
        let stops = [
            ColorStop::new(0.0, Rgb(255, 24, 255)),
            ColorStop::new(18.0, Rgb(255, 16, 16)),
            ColorStop::new(28.0, Rgb(255, 128, 0)),
            ColorStop::new(35.0, Rgb(239, 255, 0)),
            ColorStop::new(42.0, Rgb(223, 255, 0)),
            ColorStop::new(49.0, Rgb(0, 255, 0)),
            ColorStop::new(52.0, Rgb(0, 255, 255)),
            ColorStop::new(64.0, Rgb(24, 88, 255)),
            ColorStop::new(77.0, Rgb(24, 88, 255)),
            ColorStop::new(100.0, Rgb(255, 24, 255)),
        ];
        Self::from_stops(&stops, 37_740, SOURCE_QUICK_FPS)
            .expect("fixed source Spectrum parameters are valid")
    }

    /// The source one-color Breathing preset: 7000 ms, 175 entries at 25 fps.
    pub fn breathing(color: Rgb) -> Self {
        let black = Rgb(0, 0, 0);
        let stops = [
            ColorStop::new(0.0, black),
            ColorStop::new(25.0, black),
            ColorStop::new(50.0, color),
            ColorStop::new(75.0, color),
            ColorStop::new(100.0, black),
        ];
        Self::from_stops(&stops, 7_000, SOURCE_QUICK_FPS)
            .expect("fixed source Breathing parameters are valid")
    }

    /// The source two-color Breathing preset: 14000 ms, 350 entries at 25 fps.
    pub fn breathing_two_colors(first: Rgb, second: Rgb) -> Self {
        let black = Rgb(0, 0, 0);
        let stops = [
            ColorStop::new(0.0, black),
            ColorStop::new(12.0, black),
            ColorStop::new(25.0, first),
            ColorStop::new(38.0, first),
            ColorStop::new(50.0, black),
            ColorStop::new(62.0, black),
            ColorStop::new(75.0, second),
            ColorStop::new(88.0, second),
            ColorStop::new(100.0, black),
        ];
        Self::from_stops(&stops, 14_000, SOURCE_QUICK_FPS)
            .expect("fixed source two-color Breathing parameters are valid")
    }

    pub fn next_color(&mut self) -> Rgb {
        let color = self.colors[self.next];
        self.next += 1;
        if self.next == self.colors.len() {
            self.next = 0;
        }
        color
    }

    pub fn reset(&mut self) {
        self.next = 0;
    }

    pub fn frame_count(&self) -> usize {
        self.colors.len()
    }

    pub fn frame_interval(&self) -> Duration {
        self.interval
    }
}

fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::InvalidArg(message.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPECTRUM_BYTES: &[u8] =
        include_bytes!("../tests/fixtures/native-spectrum-static-derived.rgb");
    const VECTORS: &str = include_str!("../tests/fixtures/native-spectrum-static-derived.json");

    fn cycle_bytes(cycle: &mut UniformCycle) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(cycle.frame_count() * 3);
        for _ in 0..cycle.frame_count() {
            let Rgb(red, green, blue) = cycle.next_color();
            bytes.extend_from_slice(&[red, green, blue]);
        }
        bytes
    }

    #[test]
    fn every_spectrum_byte_matches_static_derived_fixture() {
        let mut cycle = UniformCycle::spectrum();
        assert_eq!(cycle.frame_count(), 943);
        assert_eq!(cycle.frame_interval(), Duration::from_millis(40));
        assert_eq!(SPECTRUM_BYTES.len(), 943 * 3);
        assert_eq!(cycle_bytes(&mut cycle), SPECTRUM_BYTES);
    }

    #[test]
    fn spectrum_boundary_samples_match_recorded_vectors() {
        let vectors: serde_json::Value = serde_json::from_str(VECTORS).unwrap();
        assert!(vectors["provenance"]
            .as_str()
            .unwrap()
            .contains("not vendor runtime"));
        assert_eq!(vectors["nominal_cycle_ms"], 37_720);
        let mut cycle = UniformCycle::spectrum();
        let bytes = cycle_bytes(&mut cycle);
        for sample in vectors["samples"].as_array().unwrap() {
            let offset = sample["frame"].as_u64().unwrap() as usize * 3;
            let expected: Vec<u8> = sample["rgb"]
                .as_array()
                .unwrap()
                .iter()
                .map(|channel| channel.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(&bytes[offset..offset + 3], expected, "frame {}", offset / 3);
        }
    }

    // Uses only the existing Windows crypto dependency, in tests. Runtime palette
    // generation has no OS calls or crypto dependency. Full byte equality above
    // also verifies the sequence on non-Windows builds.
    #[cfg(windows)]
    #[test]
    fn all_spectrum_rgb_bytes_have_the_recorded_sha256() {
        use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
        let mut cycle = UniformCycle::spectrum();
        let bytes = cycle_bytes(&mut cycle);
        let mut digest = [0_u8; 32];
        let status = unsafe {
            BCryptHash(
                BCRYPT_SHA256_ALG_HANDLE,
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as u32,
                digest.as_mut_ptr(),
                digest.len() as u32,
            )
        };
        assert_eq!(status, 0, "Windows SHA256 failed");
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            hex,
            "6258f96d86f84a7e73b6dcee488e3d844d36523e2728b5edca1d1393cb162017"
        );
    }

    #[test]
    fn ramp_truncates_and_preserves_rgb_channel_order() {
        let stops = [
            ColorStop::new(0.0, Rgb(0, 0, 0)),
            ColorStop::new(100.0, Rgb(255, 128, 64)),
        ];
        let mut cycle = UniformCycle::from_stops(&stops, 200, 25).unwrap();
        let colors: Vec<_> = (0..5).map(|_| cycle.next_color()).collect();
        assert_eq!(
            colors,
            [
                Rgb(0, 0, 0),
                Rgb(63, 32, 16),
                Rgb(127, 64, 32),
                Rgb(191, 96, 48),
                Rgb(255, 128, 64)
            ]
        );
    }

    #[test]
    fn neighboring_segments_duplicate_their_boundary_color() {
        let stops = [
            ColorStop::new(0.0, Rgb(0, 0, 0)),
            ColorStop::new(50.0, Rgb(255, 0, 0)),
            ColorStop::new(100.0, Rgb(0, 0, 0)),
        ];
        let mut cycle = UniformCycle::from_stops(&stops, 400, 25).unwrap();
        let colors: Vec<_> = (0..10).map(|_| cycle.next_color()).collect();
        assert_eq!(
            colors,
            [
                Rgb(0, 0, 0),
                Rgb(63, 0, 0),
                Rgb(127, 0, 0),
                Rgb(191, 0, 0),
                Rgb(255, 0, 0),
                Rgb(255, 0, 0),
                Rgb(191, 0, 0),
                Rgb(127, 0, 0),
                Rgb(63, 0, 0),
                Rgb(0, 0, 0)
            ]
        );
    }

    #[test]
    fn cycle_wrap_and_reset_return_to_the_first_color() {
        let mut cycle = UniformCycle::spectrum();
        let first = cycle.next_color();
        for _ in 1..cycle.frame_count() {
            cycle.next_color();
        }
        assert_eq!(cycle.next_color(), first);
        cycle.next_color();
        cycle.next_color();
        cycle.reset();
        assert_eq!(cycle.next_color(), first);
    }

    #[test]
    fn fixed_breathing_presets_have_source_periods_and_holds() {
        let red = Rgb(255, 0, 0);
        let blue = Rgb(0, 0, 255);
        let mut one = UniformCycle::breathing(red);
        assert_eq!(one.frame_count(), 175);
        let frames: Vec<_> = (0..175).map(|_| one.next_color()).collect();
        assert!(frames[..43].iter().all(|color| *color == Rgb(0, 0, 0)));
        assert!(frames[86..131].iter().all(|color| *color == red));
        assert_eq!(frames[174], Rgb(0, 0, 0));
        let mut two = UniformCycle::breathing_two_colors(red, blue);
        assert_eq!(two.frame_count(), 350);
        assert_eq!(two.frame_interval(), Duration::from_millis(40));
        let frames: Vec<_> = (0..350).map(|_| two.next_color()).collect();
        assert_eq!(frames[87], red);
        assert_eq!(frames[262], blue);
        assert_eq!(frames[349], Rgb(0, 0, 0));
    }

    #[test]
    fn short_quantized_stops_are_moved_without_mutating_the_input() {
        let stops = [
            ColorStop::new(0.0, Rgb(1, 0, 0)),
            ColorStop::new(1.0, Rgb(2, 0, 0)),
            ColorStop::new(2.0, Rgb(3, 0, 0)),
            ColorStop::new(100.0, Rgb(4, 0, 0)),
        ];
        let mut cycle = UniformCycle::from_stops(&stops, 1, 25).unwrap();
        assert_eq!(cycle.frame_count(), 4);
        let colors: Vec<_> = (0..4).map(|_| cycle.next_color()).collect();
        assert_eq!(
            colors,
            [Rgb(1, 0, 0), Rgb(2, 0, 0), Rgb(3, 0, 0), Rgb(4, 0, 0)]
        );
        assert_eq!(stops[1].percent, 1.0);
        assert_eq!(stops[2].percent, 2.0);
    }

    #[test]
    fn one_slot_final_segment_retains_its_starting_color() {
        let first = Rgb(17, 23, 31);
        let middle = Rgb(73, 89, 101);
        let last = Rgb(151, 173, 199);
        let stops = [
            ColorStop::new(0.0, first),
            ColorStop::new(99.0, middle),
            ColorStop::new(100.0, last),
        ];
        let mut cycle = UniformCycle::from_stops(&stops, 1, 25).unwrap();
        assert_eq!(cycle.frame_count(), 3);
        // Static branch oracle: native RVA 69419 -> 693CB -> 693DB -> 6957B
        // skips the 6956C endpoint rewrite for the final [2,3) segment.
        assert_eq!(cycle.next_color(), first);
        assert_eq!(cycle.next_color(), middle);
        assert_eq!(cycle.next_color(), middle);
        assert_eq!(cycle.next_color(), first);
    }

    #[test]
    fn malformed_or_nonfinite_palettes_are_rejected() {
        let black = Rgb(0, 0, 0);
        for percent in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            101.0,
            12.5,
        ] {
            let stops = [
                ColorStop::new(0.0, black),
                ColorStop::new(percent, black),
                ColorStop::new(100.0, black),
            ];
            assert!(UniformCycle::from_stops(&stops, 1000, 25).is_err());
        }
        for percentages in [
            &[0.0][..],
            &[1.0, 100.0],
            &[0.0, 99.0],
            &[0.0, 50.0, 50.0, 100.0],
            &[0.0, 60.0, 50.0, 100.0],
        ] {
            let stops: Vec<_> = percentages
                .iter()
                .map(|percent| ColorStop::new(*percent, black))
                .collect();
            assert!(UniformCycle::from_stops(&stops, 1000, 25).is_err());
        }
        let too_many: Vec<_> = (0..=10)
            .map(|i| ColorStop::new(f64::from(i * 10), black))
            .collect();
        assert!(UniformCycle::from_stops(&too_many, 1000, 25).is_err());
    }

    #[test]
    fn timing_and_allocation_limits_are_enforced_before_allocating() {
        let stops = [
            ColorStop::new(0.0, Rgb(0, 0, 0)),
            ColorStop::new(100.0, Rgb(255, 255, 255)),
        ];
        for (duration, fps) in [(0, 25), (1000, 0), (1000, 1001), (u32::MAX, 1000)] {
            assert!(UniformCycle::from_stops(&stops, duration, fps).is_err());
        }
        let cycle = UniformCycle::from_stops(&stops, MAX_FRAME_COUNT as u32, 1000).unwrap();
        assert_eq!(cycle.frame_count(), MAX_FRAME_COUNT);
        assert_eq!(std::mem::size_of::<Rgb>(), 3);
        assert!(UniformCycle::from_stops(&stops, MAX_FRAME_COUNT as u32 + 1, 1000).is_err());
        let cycle = UniformCycle::from_stops(&stops, 1000, 60).unwrap();
        assert_eq!(cycle.frame_interval(), Duration::from_millis(16));
        assert_eq!(cycle.frame_count(), 62);
    }
}
