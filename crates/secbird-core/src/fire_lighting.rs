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

pub struct FireCycle {
    region: FireRegion,
    intensities: Box<[u8]>,
    palette: Palette,
    next: usize,
    final_random_state: u32,
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
        let mut keyframes = vec![0u8; KEYFRAMES * CELLS];
        for frame in 0..KEYFRAMES {
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
            for frame in 0..KEYFRAMES {
                for col in 0..COLS {
                    let value = u32::from(keyframes[frame * CELLS + row * COLS + col]);
                    let cooling = value * u32::from(tables::COOLING[row - 1][col]) / 255;
                    let cooled = if cooling < value { value - cooling } else { 1 };
                    let target = (frame + 1) % KEYFRAMES;
                    keyframes[target * CELLS + (row - 1) * COLS + col] = cooled as u8;
                }
            }
        }
        let mut intensities = vec![0; STUDIO_FIRE_FRAMES * CELLS];
        for frame in 0..KEYFRAMES {
            for cell in 0..CELLS {
                let first = keyframes[frame * CELLS + cell];
                let last = keyframes[((frame + 1) % KEYFRAMES) * CELLS + cell];
                let delta = (f32::from(last) - f32::from(first)) / (INTERVAL - 1) as f32;
                let mut value = f32::from(first);
                intensities[frame * INTERVAL * CELLS + cell] = first;
                for between in 1..INTERVAL - 1 {
                    value += delta;
                    intensities[(frame * INTERVAL + between) * CELLS + cell] = value as u8;
                }
                intensities[(frame * INTERVAL + INTERVAL - 1) * CELLS + cell] = last;
            }
        }
        Ok(Self {
            region,
            intensities: intensities.into_boxed_slice(),
            palette,
            next: 0,
            final_random_state: state,
        })
    }
    pub fn frame_count(&self) -> usize {
        STUDIO_FIRE_FRAMES
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
        let frame = frame % STUDIO_FIRE_FRAMES;
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
        self.render_frame(self.next, output)?;
        self.next = (self.next + 1) % STUDIO_FIRE_FRAMES;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
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
