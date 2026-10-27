//! Bounded linear cardinal Wave math from the source Quick preset.
//!
//! Region bounds are explicit native half-open grid coordinates. This module
//! does not choose an installed device's region, map physical LEDs, schedule,
//! render rotational/split modes, call vendor code or send hardware frames.
//! See docs/WAVE-LIGHTING.md for scope and static evidence.

use crate::uniform_lighting::{ColorStop, UniformCycle, SOURCE_QUICK_FPS};
use crate::{Error, Result, Rgb};
use std::time::Duration;

pub const MAX_WAVE_CELLS: usize = 4096;
pub const MAX_WAVE_DIMENSION: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardinalDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Exact half-open region [left,right) x [top,bottom) inside a row-major canvas.
///
/// The caller supplies native logical coordinates. This is not a claim that an
/// installed keyboard's source region equals its advertised rows/columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveRegion {
    rows: u16,
    cols: u16,
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl WaveRegion {
    pub fn new(rows: u16, cols: u16, left: u16, top: u16, right: u16, bottom: u16) -> Result<Self> {
        if rows == 0
            || cols == 0
            || rows > MAX_WAVE_DIMENSION
            || cols > MAX_WAVE_DIMENSION
            || usize::from(rows) * usize::from(cols) > MAX_WAVE_CELLS
            || left >= right
            || top >= bottom
            || right > cols
            || bottom > rows
        {
            return Err(Error::InvalidArg(
                "Wave requires a nonempty half-open region inside a canvas of at most64x64 cells"
                    .into(),
            ));
        }
        Ok(Self {
            rows,
            cols,
            left,
            top,
            right,
            bottom,
        })
    }
    pub fn canvas_rows(self) -> u16 {
        self.rows
    }
    pub fn canvas_cols(self) -> u16 {
        self.cols
    }
    pub fn bounds(self) -> (u16, u16, u16, u16) {
        (self.left, self.top, self.right, self.bottom)
    }
    pub fn canvas_len(self) -> usize {
        usize::from(self.rows) * usize::from(self.cols)
    }
}

/// Source default speed25, width100, pause0, cycles-1 and25fps only.
///
/// Linear source mode requires counterclockwise absent (even false selects a
/// rotational source mode). Split, arbitrary angle/speed, finite/trigger modes
/// and outer timing policy are deliberately not accepted by this interface.
#[derive(Debug, Clone)]
pub struct WaveCycle {
    region: WaveRegion,
    palette: Box<[Rgb]>,
    phases: Box<[u8]>,
    next: usize,
}

impl WaveCycle {
    pub fn source_quick(region: WaveRegion, direction: CardinalDirection) -> Result<Self> {
        // Source angle+270 followed by native abs<0.0001 clamp reduces these
        // cardinal sinf/cosf pairs exactly; no general Rust trig is substituted.
        let (sin, cos) = match direction {
            CardinalDirection::Up => (-1.0_f32, 0.0_f32),
            CardinalDirection::Down => (1.0, 0.0),
            CardinalDirection::Left => (0.0, -1.0),
            CardinalDirection::Right => (0.0, 1.0),
        };
        let height = region.bottom - region.top;
        let width = region.right - region.left;
        let projected = match direction {
            CardinalDirection::Up | CardinalDirection::Down => height,
            _ => width,
        } as f32;
        // Native65DDE..65E3A: preserve each f32 operation, including division
        // then CVTTSS2SI. Some dimensions yield24 rather than algebraic25.
        let speed = (25_u32 * 4) as f32 / 100.0_f32;
        let step = (speed * projected) / f32::from(SOURCE_QUICK_FPS);
        let period = (projected / step) as u32;
        let count = period.max(4) as usize;
        let stops = [
            ColorStop::new(0.0, Rgb(255, 0, 0)),
            ColorStop::new(33.0, Rgb(0, 255, 0)),
            ColorStop::new(67.0, Rgb(0, 0, 255)),
            ColorStop::new(100.0, Rgb(255, 0, 0)),
        ];
        // Same692E0 palette builder as UniformCycle. This exact duration makes
        // its table count equal the separately recovered geometry count.
        let mut lut = UniformCycle::from_stops(&stops, count as u32 * 40, SOURCE_QUICK_FPS)?;
        let palette = (0..count)
            .map(|_| lut.next_color())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut phases = Vec::with_capacity(usize::from(height) * usize::from(width));
        for row in 0..height {
            for col in 0..width {
                // Native660BA..6612E uses separate f32 multiply/divide/add,
                // truncates toward zero, then performs unsigned positive mod.
                let row_projection = (f32::from(row) * sin) / f32::from(height);
                let col_projection = (f32::from(col) * cos) / f32::from(width);
                let phase = ((col_projection + row_projection) * period as f32) as i32;
                phases.push(phase.rem_euclid(count as i32) as u8);
            }
        }
        Ok(Self {
            region,
            palette,
            phases: phases.into_boxed_slice(),
            next: 0,
        })
    }

    pub fn region(&self) -> WaveRegion {
        self.region
    }
    pub fn frame_count(&self) -> usize {
        self.palette.len()
    }
    pub fn frame_interval(&self) -> Duration {
        Duration::from_millis(40)
    }
    pub fn next_frame_index(&self) -> usize {
        self.next
    }
    pub fn reset(&mut self) {
        self.next = 0;
    }

    /// Write only the explicit region; leave the rest of the canvas unchanged.
    /// Wrong output size fails before any pixel or cycle state changes.
    pub fn next_frame(&mut self, output: &mut [Rgb]) -> Result<()> {
        self.render_frame(self.next, output)?;
        self.next = (self.next + 1) % self.frame_count();
        Ok(())
    }

    /// Deterministic frame-index query, independent of wall time and state.
    pub fn render_frame(&self, index: usize, output: &mut [Rgb]) -> Result<()> {
        if output.len() != self.region.canvas_len() {
            return Err(Error::InvalidArg(format!(
                "Wave canvas has{} cells, expected{}",
                output.len(),
                self.region.canvas_len()
            )));
        }
        let count = self.frame_count();
        let frame = index % count;
        let width = usize::from(self.region.right - self.region.left);
        for (local, phase) in self.phases.iter().enumerate() {
            let row = usize::from(self.region.top) + local / width;
            let col = usize::from(self.region.left) + local % width;
            output[row * usize::from(self.region.cols) + col] =
                self.palette[(usize::from(*phase) + count - frame) % count];
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn full(rows: u16, cols: u16) -> WaveRegion {
        WaveRegion::new(rows, cols, 0, 0, cols, rows).unwrap()
    }
    fn dir(name: &str) -> CardinalDirection {
        match name {
            "up" => CardinalDirection::Up,
            "down" => CardinalDirection::Down,
            "left" => CardinalDirection::Left,
            "right" => CardinalDirection::Right,
            _ => panic!("fixture direction"),
        }
    }

    #[test]
    fn every_derived_cycle_byte_matches_independent_scalar_fixture() {
        let meta: Value =
            serde_json::from_str(include_str!("../tests/fixtures/wave_cardinal.json")).unwrap();
        let raw = include_bytes!("../tests/fixtures/wave_cardinal.rgb");
        for case in meta["cases"].as_array().unwrap() {
            let n = |k: &str| case[k].as_u64().unwrap() as u16;
            let region = WaveRegion::new(
                n("rows"),
                n("cols"),
                n("left"),
                n("top"),
                n("right"),
                n("bottom"),
            )
            .unwrap();
            let mut wave =
                WaveCycle::source_quick(region, dir(case["direction"].as_str().unwrap())).unwrap();
            assert_eq!(
                wave.frame_count(),
                case["frames"].as_u64().unwrap() as usize
            );
            let mut out = vec![Rgb(17, 18, 19); region.canvas_len()];
            let mut bytes = Vec::new();
            for _ in 0..wave.frame_count() {
                wave.next_frame(&mut out).unwrap();
                bytes.extend(out.iter().flat_map(|p| [p.0, p.1, p.2]));
            }
            let offset = case["offset"].as_u64().unwrap() as usize;
            let length = case["bytes"].as_u64().unwrap() as usize;
            assert_eq!(
                bytes.as_slice(),
                &raw[offset..offset + length],
                "{}",
                case["name"]
            );
            assert_eq!(
                format!("{:x}", md5::compute(&bytes)),
                case["md5"].as_str().unwrap()
            );
            assert_eq!(wave.next_frame_index(), 0);
        }
    }

    #[test]
    fn geometry_preserves_f32_quantization_in_both_axes() {
        assert_eq!(
            WaveCycle::source_quick(full(6, 22), CardinalDirection::Right)
                .unwrap()
                .frame_count(),
            25
        );
        assert_eq!(
            WaveCycle::source_quick(full(6, 22), CardinalDirection::Up)
                .unwrap()
                .frame_count(),
            25
        );
        assert_eq!(
            WaveCycle::source_quick(full(3, 9), CardinalDirection::Right)
                .unwrap()
                .frame_count(),
            24
        );
        assert_eq!(
            WaveCycle::source_quick(full(9, 3), CardinalDirection::Up)
                .unwrap()
                .frame_count(),
            24
        );
    }

    #[test]
    fn reset_wrap_and_index_query_have_no_time_dependency() {
        let mut wave = WaveCycle::source_quick(full(6, 22), CardinalDirection::Left).unwrap();
        let mut first = vec![Rgb(0, 0, 0); 132];
        wave.next_frame(&mut first).unwrap();
        let mut queried = first.clone();
        wave.render_frame(usize::MAX, &mut queried).unwrap();
        assert_eq!(wave.next_frame_index(), 1);
        wave.reset();
        let mut reset = queried;
        wave.next_frame(&mut reset).unwrap();
        assert_eq!(reset, first);
        wave.render_frame(wave.frame_count(), &mut reset).unwrap();
        assert_eq!(reset, first);
        assert_eq!(wave.frame_interval(), Duration::from_millis(40));
    }

    #[test]
    fn bounds_and_output_size_fail_before_mutation_or_advance() {
        for args in [
            (0, 22, 0, 0, 22, 1),
            (6, 0, 0, 0, 1, 6),
            (65, 1, 0, 0, 1, 1),
            (6, 22, 0, 0, 23, 6),
            (6, 22, 1, 0, 1, 6),
            (6, 22, 0, 6, 22, 6),
        ] {
            assert!(WaveRegion::new(args.0, args.1, args.2, args.3, args.4, args.5).is_err());
        }
        let mut wave = WaveCycle::source_quick(full(6, 22), CardinalDirection::Right).unwrap();
        let mut short = vec![Rgb(1, 2, 3); 131];
        let before = short.clone();
        assert!(wave.next_frame(&mut short).is_err());
        assert_eq!(short, before);
        assert_eq!(wave.next_frame_index(), 0);
    }

    #[test]
    fn explicit_subregion_preserves_outside_pixels_and_maximum_is_bounded() {
        let region = WaveRegion::new(6, 22, 2, 1, 20, 5).unwrap();
        let mut wave = WaveCycle::source_quick(region, CardinalDirection::Down).unwrap();
        assert_eq!(wave.region().bounds(), (2, 1, 20, 5));
        let mut out = vec![Rgb(17, 18, 19); 132];
        wave.next_frame(&mut out).unwrap();
        assert_eq!(out[0], Rgb(17, 18, 19));
        assert_eq!(out[22 + 2], Rgb(255, 0, 0));
        let max = WaveCycle::source_quick(full(64, 64), CardinalDirection::Up).unwrap();
        assert_eq!(max.phases.len(), 4096);
        assert!(max.palette.len() <= 25);
    }
}
