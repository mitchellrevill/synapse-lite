//! Recovered Studio Fire generation. No vendor runtime, scheduler or device I/O.
use crate::lighting_pixels::{Palette, Pixel, Stop};
use crate::{Error, Result, Rgb};
use std::time::Duration;

#[path = "fire_tables.rs"]
mod tables;
/// Reuse the established bounded, half-open native grid-region representation.
pub use crate::wave_lighting::WaveRegion as FireRegion;

const ROWS: usize = 7;
const COLS: usize = 23;
const CELLS: usize = ROWS * COLS;
const KEYFRAMES: usize = 100;
const INTERVAL: usize = 5;
pub const STUDIO_FIRE_FRAMES: usize = KEYFRAMES * INTERVAL;
/// Local admission limit, not a limit claimed for the vendor engine.
pub const MAX_FIRE_FRAMES: usize = 4096;

/// Native engine parameters. Studio's codec always supplies `Default`.
/// Seed counts keyframes; randomness comes from the separate CRT stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireParameters {
    pub mode: u32,
    pub rate: u32,
    pub seed: u32,
    pub cycles: i32,
}

impl Default for FireParameters {
    fn default() -> Self {
        Self {
            mode: 0,
            rate: 160,
            seed: 100,
            cycles: -1,
        }
    }
}

impl FireParameters {
    fn normalized(self) -> Result<Self> {
        if self.mode != 0 || self.cycles < -1 {
            return Err(Error::InvalidArg(
                "Fire supports Mode0 and Cycles-1/0 or positive counts".into(),
            ));
        }
        let params = Self {
            rate: self.rate.max(40),
            seed: if self.seed == 0 { 50 } else { self.seed },
            cycles: if self.cycles == 0 { -1 } else { self.cycles },
            ..self
        };
        let interval = params.rate / 40 + 1;
        let frames = u64::from(interval) * u64::from(params.seed);
        // Native interpolation uses a 256-pixel scratch palette. Also cap the
        // complete retained cycle before converting lengths or allocating.
        if interval > 256 || frames > MAX_FIRE_FRAMES as u64 {
            return Err(Error::InvalidArg(
                "Fire exceeds the local 256-frame interpolation or 4096-frame cycle limit".into(),
            ));
        }
        Ok(params)
    }
}

pub struct FireCycle {
    region: FireRegion,
    intensities: Box<[u8]>,
    palette: Palette,
    next: usize,
    final_random_state: u32,
    parameters: FireParameters,
    remaining_cycles: i32,
}
impl FireCycle {
    /// Studio Mode0/Rate160/Seed100/Cycles-1 at25fps. Seed100 is the keyframe
    /// count, not an RNG seed. Supply an explicit incoming CRT stream state;
    /// a pristine native thread starts at1, a running stock thread may differ.
    pub fn studio(
        region: FireRegion,
        hot: Option<u32>,
        cold: Option<u32>,
        random_state: u32,
    ) -> Result<Self> {
        Self::new(region, hot, cold, random_state, FireParameters::default())
    }

    /// Construct the recovered native Mode0 generator at 25fps. Validation and
    /// allocation limits apply before any random draws or frame allocation.
    pub fn new(
        region: FireRegion,
        hot: Option<u32>,
        cold: Option<u32>,
        random_state: u32,
        parameters: FireParameters,
    ) -> Result<Self> {
        let parameters = parameters.normalized()?;
        let keyframe_count = parameters.seed as usize;
        let interval = (parameters.rate / 40 + 1) as usize;
        let frame_count = keyframe_count * interval;
        let palette = Palette::new(
            &[
                // Source converter falls back to Cold when Hot is absent.
                Stop {
                    percent: 0,
                    pixel: Pixel::from_studio_color(hot.or(cold))?,
                },
                Stop {
                    percent: 50,
                    pixel: Pixel::from_studio_color(cold)?,
                },
                Stop {
                    percent: 100,
                    pixel: Pixel::opaque(Rgb(0, 0, 0)),
                },
            ],
            256,
        )?;
        let mut state = random_state;
        let mut keyframes = vec![0u8; keyframe_count * CELLS];
        for frame in 0..keyframe_count {
            for col in 0..COLS {
                state = state.wrapping_mul(214013).wrapping_add(2531011);
                let random = (state >> 16) & 32767;
                let value = random % 192 + 32;
                let cooled = value - value * u32::from(tables::COOLING[ROWS - 1][col]) / 255;
                keyframes[frame * CELLS + (ROWS - 1) * COLS + col] = cooled as u8;
            }
        }
        // Native propagation is row-major in *time*: finish one spatial row
        // across all keyframes before reading it for the next row above.
        for row in (1..ROWS).rev() {
            for frame in 0..keyframe_count {
                for col in 0..COLS {
                    let value = u32::from(keyframes[frame * CELLS + row * COLS + col]);
                    let cooling = value * u32::from(tables::COOLING[row - 1][col]) / 255;
                    let cooled = if cooling < value { value - cooling } else { 1 };
                    let target = (frame + 1) % keyframe_count;
                    keyframes[target * CELLS + (row - 1) * COLS + col] = cooled as u8;
                }
            }
        }
        let mut intensities = vec![0; frame_count * CELLS];
        for frame in 0..keyframe_count {
            for cell in 0..CELLS {
                let first = keyframes[frame * CELLS + cell];
                let last = keyframes[((frame + 1) % keyframe_count) * CELLS + cell];
                let delta = (f32::from(last) - f32::from(first)) / (interval - 1) as f32;
                let mut value = f32::from(first);
                intensities[frame * interval * CELLS + cell] = first;
                for between in 1..interval - 1 {
                    value += delta;
                    intensities[(frame * interval + between) * CELLS + cell] = value as u8;
                }
                intensities[(frame * interval + interval - 1) * CELLS + cell] = last;
            }
        }
        Ok(Self {
            region,
            intensities: intensities.into_boxed_slice(),
            palette,
            next: 0,
            final_random_state: state,
            parameters,
            remaining_cycles: parameters.cycles,
        })
    }
    pub fn frame_count(&self) -> usize {
        self.intensities.len() / CELLS
    }
    pub fn frame_index(&self) -> usize {
        self.next
    }
    pub fn frame_interval(&self) -> Duration {
        Duration::from_millis(40)
    }
    pub fn final_random_state(&self) -> u32 {
        self.final_random_state
    }
    pub fn region(&self) -> FireRegion {
        self.region
    }
    pub fn storage_bytes(&self) -> usize {
        self.intensities.len() + self.palette.storage_bytes()
    }
    pub fn reset(&mut self) {
        self.next = 0;
        self.remaining_cycles = self.parameters.cycles;
    }
    pub fn parameters(&self) -> FireParameters {
        self.parameters
    }
    pub fn remaining_cycles(&self) -> i32 {
        self.remaining_cycles
    }
    pub fn is_finished(&self) -> bool {
        self.remaining_cycles == 0
    }
    /// Native SetMask/colour rebuild: consume the following CRT sequence and
    /// retain replay index and remaining counter. Failed validation is atomic.
    /// Parameters remain fixed; changing them requires a new initializer.
    pub fn rebuild(
        &mut self,
        region: FireRegion,
        hot: Option<u32>,
        cold: Option<u32>,
    ) -> Result<()> {
        let mut replacement =
            Self::new(region, hot, cold, self.final_random_state, self.parameters)?;
        replacement.next = self.next;
        replacement.remaining_cycles = self.remaining_cycles;
        *self = replacement;
        Ok(())
    }
    /// Copy a complete native canvas frame. Outside-region pixels are opaque
    /// black, as in the zeroed native frame volume; layer masks belong to the
    /// compositor. This differs from Wave's caller-owned outside-region API.
    pub fn render_frame(&self, frame: usize, output: &mut [Pixel]) -> Result<()> {
        if output.len() != self.region.canvas_len() {
            return Err(Error::InvalidArg(
                "Fire output must match the complete native canvas".into(),
            ));
        }
        let (left, top, right, bottom) = self.region.bounds();
        let height = usize::from(bottom - top);
        let width = usize::from(right - left);
        let small = height < ROWS && width < COLS;
        let ratio = ROWS as f32 / height as f32;
        let frame = frame % self.frame_count();
        output.fill(Pixel::opaque(Rgb(0, 0, 0)));
        for from_bottom in 0..height {
            let row = if small {
                ROWS - 1 - from_bottom
            } else {
                ROWS - 1 - (from_bottom as f32 * ratio) as usize
            };
            let target_row = usize::from(bottom) - 1 - from_bottom;
            for col in 0..width {
                let source_col = if small { col } else { col % COLS };
                let value = u32::from(self.intensities[frame * CELLS + row * COLS + source_col]);
                let color = self.palette.at(usize::from(tables::MASK[row][source_col]));
                let scale = |channel: u8| (u32::from(channel) * value / 255) as u8;
                let mut pixel = color;
                pixel.color = Rgb(
                    scale(color.color.0),
                    scale(color.color.1),
                    scale(color.color.2),
                );
                output[target_row * usize::from(self.region.canvas_cols())
                    + usize::from(left)
                    + col] = pixel;
            }
        }
        Ok(())
    }
    pub fn next_frame(&mut self, output: &mut [Pixel]) -> Result<()> {
        if output.len() != self.region.canvas_len() {
            return Err(Error::InvalidArg(
                "Fire output must match the complete native canvas".into(),
            ));
        }
        // RVA633b0 leaves the caller's buffer untouched after completion. The
        // counter decrements on frame zero, including the very first frame.
        if self.is_finished() {
            return Ok(());
        }
        self.render_frame(self.next, output)?;
        if self.next == 0 && self.remaining_cycles != -1 {
            self.remaining_cycles -= 1;
        }
        self.next = (self.next + 1) % self.frame_count();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn variable_parameters_match_independent_complete_frame_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/native_fire_parameters.json"
        ))
        .unwrap();
        let region = FireRegion::new(7, 23, 0, 0, 23, 7).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let parameters = FireParameters {
                rate: case["rate"].as_u64().unwrap() as u32,
                seed: case["seed"].as_u64().unwrap() as u32,
                ..FireParameters::default()
            };
            let cycle = FireCycle::new(
                region,
                Some(0xff0000),
                None,
                case["initial_random_state"].as_u64().unwrap() as u32,
                parameters,
            )
            .unwrap();
            assert_eq!(
                cycle.frame_count(),
                case["frames"].as_u64().unwrap() as usize
            );
            assert_eq!(
                cycle.final_random_state(),
                case["final_random_state"].as_u64().unwrap() as u32
            );
            assert_eq!(
                format!("{:x}", md5::compute(&cycle.intensities)),
                case["grayscale_md5"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(cycle.frame_interval(), Duration::from_millis(40));
            assert_eq!(cycle.storage_bytes(), cycle.frame_count() * CELLS + 1024);
        }
    }

    #[test]
    fn native_defaults_and_admission_are_bounded_before_allocation() {
        let region = FireRegion::new(7, 23, 0, 0, 23, 7).unwrap();
        let make = |parameters| FireCycle::new(region, None, None, 1, parameters);
        let cycle = make(FireParameters {
            rate: 0,
            seed: 0,
            cycles: 0,
            mode: 0,
        })
        .unwrap();
        assert_eq!(
            cycle.parameters(),
            FireParameters {
                rate: 40,
                seed: 50,
                cycles: -1,
                mode: 0
            }
        );
        assert_eq!(cycle.frame_count(), 100);
        for parameters in [
            FireParameters {
                mode: 1,
                ..FireParameters::default()
            },
            FireParameters {
                cycles: -2,
                ..FireParameters::default()
            },
            FireParameters {
                rate: u32::MAX,
                seed: u32::MAX,
                ..FireParameters::default()
            },
            FireParameters {
                rate: 40,
                seed: 2049,
                ..FireParameters::default()
            },
            FireParameters {
                rate: 10240,
                seed: 1,
                ..FireParameters::default()
            },
        ] {
            assert!(make(parameters).is_err(), "{parameters:?}");
        }
        let maximum = make(FireParameters {
            rate: 40,
            seed: 2048,
            ..FireParameters::default()
        })
        .unwrap();
        assert_eq!(maximum.frame_count(), MAX_FIRE_FRAMES);
        assert_eq!(maximum.storage_bytes(), 660480);
    }

    #[test]
    fn finite_counter_decrements_at_frame_zero_then_leaves_output_untouched() {
        let region = FireRegion::new(7, 23, 0, 0, 23, 7).unwrap();
        for cycles in [1, 2, 3] {
            let parameters = FireParameters {
                rate: 40,
                seed: 2,
                cycles,
                mode: 0,
            };
            let mut cycle =
                FireCycle::new(region, Some(0xff0000), Some(0xff8000), 1, parameters).unwrap();
            let initial_state = cycle.final_random_state();
            let mut actual = vec![Pixel::EMPTY; region.canvas_len()];
            let mut expected = actual.clone();
            let emitted = (cycles as usize - 1) * cycle.frame_count() + 1;
            for frame in 0..emitted {
                assert!(!cycle.is_finished());
                cycle.render_frame(frame, &mut expected).unwrap();
                cycle.next_frame(&mut actual).unwrap();
                assert_eq!(actual, expected);
            }
            assert!(cycle.is_finished());
            assert_eq!(cycle.remaining_cycles(), 0);
            assert_eq!(cycle.frame_index(), 1);
            actual.fill(Pixel::EMPTY);
            cycle.next_frame(&mut actual).unwrap();
            assert!(actual.iter().all(|pixel| *pixel == Pixel::EMPTY));
            assert_eq!(cycle.frame_index(), 1);
            assert_eq!(cycle.final_random_state(), initial_state);
            assert!(cycle.next_frame(&mut []).is_err());
            cycle.reset();
            assert_eq!(cycle.remaining_cycles(), cycles);
            assert_eq!(cycle.frame_index(), 0);
            assert_eq!(cycle.final_random_state(), initial_state);
        }
    }

    #[test]
    fn rebuild_consumes_following_stream_preserves_phase_and_is_atomic_on_failure() {
        let region = FireRegion::new(7, 23, 0, 0, 23, 7).unwrap();
        let parameters = FireParameters {
            rate: 80,
            seed: 7,
            cycles: 3,
            mode: 0,
        };
        let mut cycle = FireCycle::new(region, Some(0xff0000), None, 123, parameters).unwrap();
        let mut output = vec![Pixel::EMPTY; region.canvas_len()];
        for _ in 0..9 {
            cycle.next_frame(&mut output).unwrap();
        }
        let state = cycle.final_random_state();
        let before = cycle.intensities.clone();
        assert!(cycle.rebuild(region, Some(0x1000000), None).is_err());
        assert_eq!(cycle.intensities, before);
        assert_eq!(cycle.final_random_state(), state);
        assert_eq!(cycle.frame_index(), 9);
        assert_eq!(cycle.remaining_cycles(), 2);
        let bounds = FireRegion::new(7, 23, 1, 1, 20, 6).unwrap();
        let expected = FireCycle::new(bounds, None, Some(0x123456), state, parameters).unwrap();
        cycle.rebuild(bounds, None, Some(0x123456)).unwrap();
        assert_eq!(cycle.region(), bounds);
        assert_eq!(cycle.frame_index(), 9);
        assert_eq!(cycle.remaining_cycles(), 2);
        assert_eq!(cycle.final_random_state(), expected.final_random_state());
        let mut reference = output.clone();
        expected.render_frame(9, &mut reference).unwrap();
        cycle.next_frame(&mut output).unwrap();
        assert_eq!(output, reference);
        assert_eq!(cycle.frame_index(), 10);
        while !cycle.is_finished() {
            cycle.next_frame(&mut output).unwrap();
        }
        cycle.rebuild(region, Some(1), Some(2)).unwrap();
        assert!(
            cycle.is_finished(),
            "rebuild must not resurrect finite playback"
        );
    }
    #[test]
    fn complete_cycles_match_independent_native_instruction_reference() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/native_fire_vectors.json"))
                .unwrap();
        assert_eq!(
            fixture["native_sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa"
        );
        for case in fixture["cases"].as_array().unwrap() {
            let rows = case["canvas"][0].as_u64().unwrap() as u16;
            let cols = case["canvas"][1].as_u64().unwrap() as u16;
            let b: Vec<_> = case["bounds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect();
            let region = FireRegion::new(rows, cols, b[0], b[1], b[2], b[3]).unwrap();
            let color = |field: &str| case[field].as_u64().map(|v| v as u32);
            let mut cycle = FireCycle::studio(
                region,
                color("hot"),
                color("cold"),
                case["initial_random_state"].as_u64().unwrap() as u32,
            )
            .unwrap();
            assert_eq!(
                cycle.final_random_state(),
                case["final_random_state"].as_u64().unwrap() as u32
            );
            assert_eq!(cycle.frame_interval(), Duration::from_millis(40));
            assert_eq!(cycle.frame_count(), 500);
            assert!(cycle.storage_bytes() < 82000);
            let mut output = vec![Pixel::EMPTY; region.canvas_len()];
            let mut hash = md5::Context::new();
            for frame in 0..500 {
                cycle.next_frame(&mut output).unwrap();
                for p in &output {
                    hash.consume([p.color.0, p.color.1, p.color.2, p.transparency()]);
                }
                assert_eq!(cycle.frame_index(), (frame + 1) % 500);
            }
            assert_eq!(
                format!("{:x}", hash.compute()),
                case["rgba_md5"].as_str().unwrap(),
                "case {case}"
            );
            assert_eq!(cycle.frame_index(), 0);
            let state = cycle.final_random_state();
            let mut zero = output.clone();
            cycle.render_frame(0, &mut zero).unwrap();
            cycle.next_frame(&mut output).unwrap();
            assert_eq!(output, zero);
            assert_eq!(cycle.final_random_state(), state);
            cycle.reset();
            assert_eq!(cycle.frame_index(), 0);
            cycle.render_frame(usize::MAX, &mut output).unwrap();
            assert_eq!(cycle.frame_index(), 0);
        }
    }
    #[test]
    fn invalid_output_is_atomic_and_does_not_advance_or_regenerate() {
        let region = FireRegion::new(6, 22, 1, 1, 20, 6).unwrap();
        let mut cycle = FireCycle::studio(region, Some(0xff0000), None, 1).unwrap();
        let mut output = [Pixel::EMPTY; 1];
        assert!(cycle.next_frame(&mut output).is_err());
        assert_eq!(output, [Pixel::EMPTY]);
        assert_eq!(cycle.frame_index(), 0);
        assert!(FireCycle::studio(region, Some(0x1000000), None, 1).is_err());
    }
    #[test]
    fn scaled_mask_transparency_does_not_change_with_intensity_and_outside_is_zero() {
        let region = FireRegion::new(10, 30, 3, 2, 26, 9).unwrap();
        let cycle = FireCycle::studio(region, Some(0xff0000), None, 1).unwrap();
        let mut first = vec![Pixel::EMPTY; region.canvas_len()];
        let mut second = first.clone();
        cycle.render_frame(0, &mut first).unwrap();
        cycle.render_frame(137, &mut second).unwrap();
        assert!(first
            .iter()
            .zip(&second)
            .all(|(a, b)| a.transparency() == b.transparency()));
        assert_ne!(first, second);
        assert_eq!(first[0], Pixel::opaque(Rgb(0, 0, 0)));
    }
}
