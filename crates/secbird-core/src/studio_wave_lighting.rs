//! Continuous linear Studio Wave. Quick Wave has a different default palette.
//! See docs/studio.md for source evidence and explicitly deferred modes.
use crate::{
    lighting_pixels::{Palette, Pixel, Stop},
    studio::document::GradientStop,
    wave_lighting::{CardinalDirection, WaveRegion},
    Error, Result, Rgb,
};

pub struct StudioWaveCycle {
    region: WaveRegion,
    palette: Palette,
    phases: Box<[i32]>,
    length: usize,
    next: usize,
    moving: bool,
}

impl StudioWaveCycle {
    pub fn new(
        region: WaveRegion,
        direction: CardinalDirection,
        speed: u32,
        width: u32,
        pause_ms: u32,
        stops: Option<&[GradientStop]>,
    ) -> Result<Self> {
        // Widths below100 can exercise an unresolved unsigned-index branch in
        // negative directions. Refuse those until instruction evidence closes it.
        if speed > 50 || !(100..=400).contains(&width) || pause_ms > 60_000 {
            return invalid(
                "Studio Wave currently requires speed0..50, width100..400 and pause0..60000ms",
            );
        }
        let mut normalized = Vec::new();
        if let Some(stops) = stops {
            if stops.is_empty() || stops.len() > 10 {
                return invalid("Wave requires1..10 gradient stops");
            }
            for stop in stops {
                normalized.push(Stop {
                    percent: stop.position,
                    pixel: Pixel::from_studio_color(stop.color)?,
                });
            }
            // Source converter adds anchors, without sorting the original array.
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
        } else {
            // Native invalid/absent-stop fallback, NOT the Studio UI's seven-stop
            // rainbow preset and NOT Quick's blue stop67.
            for (percent, color) in [
                (0, Rgb(255, 0, 0)),
                (33, Rgb(0, 255, 0)),
                (66, Rgb(0, 0, 255)),
                (100, Rgb(255, 0, 0)),
            ] {
                normalized.push(Stop {
                    percent,
                    pixel: Pixel::opaque(color),
                });
            }
        }
        if normalized.len() > 10
            || normalized.iter().any(|s| s.percent > 100)
            || normalized.windows(2).any(|s| s[0].percent >= s[1].percent)
        {
            return invalid(
                "Wave invalid-gradient fallback is not implemented; stops must increase strictly",
            );
        }
        let (left, top, right, bottom) = region.bounds();
        let (height, columns) = (bottom - top, right - left);
        let (sin, cos, projected) = match direction {
            CardinalDirection::Up => (-1.0_f32, 0.0_f32, height),
            CardinalDirection::Down => (1.0, 0.0, height),
            CardinalDirection::Left => (0.0, -1.0, columns),
            CardinalDirection::Right => (0.0, 1.0, columns),
        };
        let projected = f32::from(projected);
        let step = if speed == 0 {
            1.0_f32 / 25.0
        } else {
            (((speed * 4) as f32 / 100.0) * projected) / 25.0
        };
        let period = (projected / step) as u32;
        let active =
            ((f64::from(period) * (f64::from(width) / 100.0)) as usize).max(normalized.len());
        let palette = Palette::new(&normalized, active)?;
        let length = active + (pause_ms / 40) as usize;
        let mut phases = Vec::with_capacity(usize::from(height) * usize::from(columns));
        for row in 0..height {
            for col in 0..columns {
                let phase = (((f32::from(col) * cos) / f32::from(columns)
                    + (f32::from(row) * sin) / f32::from(height))
                    * period as f32) as i32;
                phases.push(phase);
            }
        }
        Ok(Self {
            region,
            palette,
            phases: phases.into_boxed_slice(),
            length,
            next: 0,
            moving: speed != 0,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.palette.storage_bytes() + self.phases.len() * std::mem::size_of::<i32>()
    }
    pub fn frame_count(&self) -> usize {
        self.length
    }
    pub fn next_frame(&mut self, output: &mut [Pixel]) -> Result<()> {
        if output.len() != self.region.canvas_len() {
            return invalid("Studio Wave canvas size mismatch");
        }
        let (left, top, right, _) = self.region.bounds();
        let columns = usize::from(right - left);
        for (local, phase) in self.phases.iter().enumerate() {
            // Admitted widths keep this native numerator nonnegative, avoiding
            // assumptions about the decompiler's signed/unsigned underflow.
            let index =
                (*phase as i64 + 3 * self.length as i64 - self.next as i64) as usize % self.length;
            output[(usize::from(top) + local / columns) * usize::from(self.region.canvas_cols())
                + usize::from(left)
                + local % columns] = if index < self.palette.len() {
                self.palette.at(index)
            } else {
                Pixel::EMPTY
            };
        }
        if self.moving {
            self.next = (self.next + 1) % self.length;
        }
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::InvalidArg(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twenty_regional_cycles_match_independent_native_math_reference() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/studio-wave-reference.json"))
                .unwrap();
        let stops: Vec<_> = reference["stops"]
            .as_array()
            .unwrap()
            .iter()
            .zip(reference["colors"].as_array().unwrap())
            .map(|(p, c)| GradientStop {
                position: p.as_u64().unwrap() as u8,
                color: c.as_u64().map(|c| c as u32),
            })
            .collect();
        for case in reference["cases"].as_array().unwrap() {
            let direction = match case["angle"].as_u64().unwrap() {
                0 => CardinalDirection::Up,
                90 => CardinalDirection::Right,
                180 => CardinalDirection::Down,
                _ => CardinalDirection::Left,
            };
            let mut cycle = StudioWaveCycle::new(
                WaveRegion::new(7, 23, 2, 1, 21, 6).unwrap(),
                direction,
                case["speed"].as_u64().unwrap() as u32,
                case["width"].as_u64().unwrap() as u32,
                case["pause"].as_u64().unwrap() as u32,
                Some(&stops),
            )
            .unwrap();
            assert_eq!(
                cycle.frame_count(),
                case["length"].as_u64().unwrap() as usize
            );
            let mut bytes = vec![];
            for _ in 0..200 {
                let mut output = vec![Pixel::EMPTY; 161];
                cycle.next_frame(&mut output).unwrap();
                for pixel in output {
                    bytes.extend([
                        pixel.color.0,
                        pixel.color.1,
                        pixel.color.2,
                        pixel.transparency(),
                    ]);
                }
            }
            assert_eq!(
                format!("{:x}", md5::compute(bytes)),
                case["rgba_md5"].as_str().unwrap(),
                "{case}"
            );
            assert!(cycle.storage_bytes() < 30_000);
        }
    }
    #[test]
    fn wrong_size_does_not_advance_or_write_and_unverified_width_refuses() {
        let region = WaveRegion::new(2, 3, 0, 0, 3, 2).unwrap();
        assert!(StudioWaveCycle::new(region, CardinalDirection::Left, 15, 99, 0, None).is_err());
        let mut cycle =
            StudioWaveCycle::new(region, CardinalDirection::Right, 15, 100, 0, None).unwrap();
        let mut untouched = [Pixel::EMPTY; 5];
        assert!(cycle.next_frame(&mut untouched).is_err());
        assert_eq!(untouched, [Pixel::EMPTY; 5]);
        let mut expected =
            StudioWaveCycle::new(region, CardinalDirection::Right, 15, 100, 0, None).unwrap();
        let mut a = [Pixel::EMPTY; 6];
        let mut b = a;
        cycle.next_frame(&mut a).unwrap();
        expected.next_frame(&mut b).unwrap();
        assert_eq!(a, b);
    }
}
