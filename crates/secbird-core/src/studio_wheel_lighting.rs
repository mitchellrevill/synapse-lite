//! Continuous Wheel13 at the source25fps. No input hooks or hardware I/O.
use crate::{
    lighting_pixels::{Palette, Pixel, Stop},
    studio::document::GradientStop,
    Error, Result,
};
pub struct StudioWheelCycle {
    palette: Palette,
    phases: Box<[u16]>,
    period: u32,
    next: u32,
    counterclockwise: bool,
}
impl StudioWheelCycle {
    /// Origin and center are explicit native grid coordinates, not editor pixels.
    pub fn new(
        rows: u16,
        cols: u16,
        origin: [i32; 2],
        center: [i32; 2],
        speed: u32,
        counterclockwise: bool,
        stops: Option<&[GradientStop]>,
    ) -> Result<Self> {
        if rows == 0
            || cols == 0
            || rows > 64
            || cols > 64
            || speed > 360
            || origin
                .iter()
                .chain(center.iter())
                .any(|n| !(-65535..=65535).contains(n))
        {
            return Err(Error::InvalidArg(
                "Wheel needs a bounded grid, explicit center and speed0..360".into(),
            ));
        }
        let fallback = [
            GradientStop {
                position: 0,
                color: Some(0xff0000),
            },
            GradientStop {
                position: 33,
                color: Some(0x00ff00),
            },
            GradientStop {
                position: 66,
                color: Some(0x0000ff),
            },
            GradientStop {
                position: 100,
                color: Some(0xff0000),
            },
        ];
        let stops = stops.unwrap_or(&fallback);
        if stops.is_empty()
            || stops.len() > 10
            || stops.iter().any(|s| s.position > 100)
            || stops.windows(2).any(|s| s[0].position >= s[1].position)
        {
            return Err(Error::InvalidArg("Wheel requires increasing gradient stops; native malformed-gradient fallback is unverified".into()));
        }
        let mut normalized = stops
            .iter()
            .map(|s| {
                Ok(Stop {
                    percent: s.position,
                    pixel: Pixel::from_studio_color(s.color)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if normalized[0].percent != 0 {
            normalized.insert(
                0,
                Stop {
                    percent: 0,
                    pixel: normalized[0].pixel,
                },
            );
        }
        if normalized.last().unwrap().percent != 100 {
            normalized.push(Stop {
                percent: 100,
                pixel: normalized.last().unwrap().pixel,
            });
        }
        let palette = Palette::new(&normalized, 1024)?;
        // Native initializes uint period from f32 division, then integer frame time.
        let period = if speed == 0 {
            0
        } else {
            (360000.0_f32 / speed as f32) as u32 / 40
        };
        let mut phases = Vec::with_capacity(usize::from(rows) * usize::from(cols));
        for row in 0..rows {
            for col in 0..cols {
                let y = f64::from((i32::from(row) + origin[0] - center[0]) as f32);
                let x = f64::from(i32::from(col) + origin[1] - center[1]);
                let angle = y.atan2(x);
                phases.push(
                    ((1024.0 * (angle + std::f64::consts::PI) / std::f64::consts::TAU) as u32
                        % 1024) as u16,
                );
            }
        }
        Ok(Self {
            palette,
            phases: phases.into_boxed_slice(),
            period,
            next: 0,
            counterclockwise,
        })
    }
    pub fn storage_bytes(&self) -> usize {
        self.palette.storage_bytes() + self.phases.len() * 2
    }
    pub fn frame_count(&self) -> u32 {
        self.period
    }
    pub fn next_frame(&mut self, output: &mut [Pixel]) -> Result<()> {
        if output.len() != self.phases.len() {
            return Err(Error::InvalidArg("Wheel canvas size mismatch".into()));
        }
        let phase = if self.period == 0 {
            0
        } else {
            let offset = (self.next as f32 * 1024.0) / self.period as f32;
            if self.counterclockwise {
                offset as u32
            } else {
                (1024.0 - offset) as u32
            }
        };
        for (pixel, index) in output.iter_mut().zip(self.phases.iter()) {
            *pixel = self.palette.at((u32::from(*index) + phase) as usize % 1024);
        }
        if self.period != 0 {
            self.next = (self.next + 1) % self.period;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_cardinal_geometry_rotation_period_freeze_and_atomic_size() {
        let mut cw = StudioWheelCycle::new(3, 3, [0, 0], [1, 1], 60, false, None).unwrap();
        assert_eq!(cw.frame_count(), 150);
        assert_eq!(&*cw.phases, &[128, 256, 384, 0, 512, 512, 896, 768, 640]);
        let mut ccw = StudioWheelCycle::new(3, 3, [0, 0], [1, 1], 60, true, None).unwrap();
        let mut a = [Pixel::EMPTY; 9];
        let mut b = a;
        assert!(cw.next_frame(&mut a[..8]).is_err());
        cw.next_frame(&mut a).unwrap();
        ccw.next_frame(&mut b).unwrap();
        assert_eq!(a, b);
        cw.next_frame(&mut a).unwrap();
        ccw.next_frame(&mut b).unwrap();
        assert_ne!(a, b);
        let mut frozen = StudioWheelCycle::new(3, 3, [0, 0], [1, 1], 0, true, None).unwrap();
        frozen.next_frame(&mut a).unwrap();
        for _ in 0..200 {
            frozen.next_frame(&mut b).unwrap();
            assert_eq!(a, b);
        }
        assert!(frozen.storage_bytes() < 5000);
    }
    #[test]
    fn wheel_independent_scalar_reference_frames() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/studio-wheel-reference.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let origin = [
                case["origin"][0].as_i64().unwrap() as i32,
                case["origin"][1].as_i64().unwrap() as i32,
            ];
            let center = [
                case["center"][0].as_i64().unwrap() as i32,
                case["center"][1].as_i64().unwrap() as i32,
            ];
            let stops = [
                GradientStop {
                    position: 0,
                    color: Some(0),
                },
                GradientStop {
                    position: 100,
                    color: Some(0xffffff),
                },
            ];
            let mut cycle = StudioWheelCycle::new(
                3,
                3,
                origin,
                center,
                case["speed"].as_u64().unwrap() as u32,
                case["ccw"].as_bool().unwrap(),
                Some(&stops),
            )
            .unwrap();
            let mut output = [Pixel::EMPTY; 9];
            for expected in case["frames"].as_array().unwrap() {
                cycle.next_frame(&mut output).unwrap();
                assert_eq!(
                    output.iter().map(|p| p.color.0).collect::<Vec<_>>(),
                    expected
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| p.as_u64().unwrap() as u8)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
