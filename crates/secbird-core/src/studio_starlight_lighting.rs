//! Studio Starlight7 population, renewal and per-star palettes. No I/O or clock.
//! Native functions 603b0/60790/60900/60950/60de0 are documented in STUDIO-STARLIGHT.
use crate::{
    lighting_pixels::{Palette, Pixel, Stop},
    lighting_random,
    studio::document::GradientStop,
    Error, Result, Rgb,
};

struct Star {
    palette: Palette,
    at: usize,
    remaining: u8,
    active: bool,
}

pub struct StarlightCycle {
    slots: Vec<Option<Star>>,
    population: usize,
    maximum: usize,
    base_frames: usize,
    renewal_frames: usize,
    renewal_at: usize,
    stops: Vec<Stop>,
    random_color: bool,
    state: u32,
    budget: usize,
}

fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::Unsupported(message.into()))
}

impl StarlightCycle {
    pub fn new(
        rows: usize,
        cols: usize,
        duration: Option<u32>,
        density: Option<u8>,
        random_color: bool,
        stops: Option<&[GradientStop]>,
        unix_seconds: u64,
    ) -> Result<Self> {
        if rows == 0
            || cols == 0
            || rows > 64
            || cols > 64
            || density.is_some_and(|value| !(1..=10).contains(&value))
        {
            return invalid("Starlight requires a bounded grid and density1..10");
        }
        let mut normalized = Vec::new();
        if !random_color {
            if let Some(stops) = stops {
                if stops.len() > 10 {
                    return invalid("Starlight supports at most ten stops");
                }
                for stop in stops {
                    normalized.push(Stop {
                        percent: stop.position,
                        pixel: Pixel::from_studio_color(
                            stop.color.filter(|color| *color != 0x64000000),
                        )?,
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
            }
        }
        if normalized.len() > 10 {
            return invalid("Starlight endpoint anchors exceed ten native stops");
        }
        if let Some(first) = normalized.first_mut() {
            first.percent = 0;
        }
        if let Some(endpoint) = normalized.iter().position(|stop| stop.percent == 100) {
            normalized.truncate(endpoint + 1);
        }
        let fallback = normalized.len() < 2
            || normalized.first().is_some_and(|stop| stop.percent != 0)
            || normalized.last().is_some_and(|stop| stop.percent != 100)
            || normalized
                .windows(2)
                .any(|pair| pair[0].percent >= pair[1].percent)
            || normalized.iter().any(|stop| stop.percent > 100);
        if fallback {
            normalized = [(0, 0), (50, 0xff0000), (100, 0)]
                .map(|(percent, color)| Stop {
                    percent,
                    pixel: Pixel::opaque(Rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)),
                })
                .to_vec();
        }
        let random_color = random_color || fallback;
        let cells = rows * cols;
        let maximum = (cells * usize::from(density.unwrap_or(0)) * 10 / 100).max(1);
        let frames = (duration.filter(|value| *value != 0).unwrap_or(14_000) / 40) as usize;
        let base_frames = frames.max(normalized.len());
        if base_frames + 4 > 16384 {
            return invalid("Starlight duration exceeds its bounded palette");
        }
        let renewal_frames = base_frames >> 3;
        if renewal_frames == 0 {
            return invalid("Starlight duration is below the native renewal minimum");
        }
        // Charge the maximum future population, not just today's live stars.
        let budget = maximum * (base_frames + 4) * std::mem::size_of::<Pixel>()
            + cells * std::mem::size_of::<Option<Star>>()
            + normalized.len() * std::mem::size_of::<Stop>();
        if budget > 8 * 1024 * 1024 {
            return invalid("Starlight population exceeds8MiB");
        }
        let color_sum = normalized.iter().fold(0u32, |sum, stop| {
            let Rgb(r, g, b) = stop.pixel.color;
            sum.wrapping_add(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b))
        });
        let parameters = color_sum
            .wrapping_add(duration.unwrap_or(0))
            .wrapping_add(u32::from(density.unwrap_or(0)) * 10)
            .wrapping_add(10 + 1 + u32::from(random_color));
        let state = ((unix_seconds / 1_800_000) * 1_800_000)
            .wrapping_add(u64::from(parameters) * 31) as u32;
        Ok(Self {
            slots: (0..cells).map(|_| None).collect(),
            population: 0,
            maximum,
            base_frames,
            renewal_frames,
            renewal_at: 0,
            stops: normalized,
            random_color,
            state,
            budget,
        })
    }

    pub fn next_frame(&mut self, output: &mut [Pixel]) -> Result<()> {
        if output.len() != self.slots.len() {
            return invalid("Starlight output dimensions changed");
        }
        output.fill(Pixel::EMPTY);
        for (cell, slot) in self.slots.iter_mut().enumerate() {
            let Some(star) = slot else {
                continue;
            };
            if !star.active {
                continue;
            }
            if star.remaining == 0 {
                star.active = false;
                continue;
            }
            output[cell] = star.palette.at(star.at);
            star.at = (star.at + 1) % star.palette.len();
            if star.at == 0 {
                star.remaining -= 1;
            }
        }
        if self.renewal_at == 0 {
            self.renew()?;
            // The source renews before removing completed stars.
            for slot in &mut self.slots {
                if slot.as_ref().is_some_and(|star| !star.active) {
                    *slot = None;
                    self.population -= 1;
                }
            }
        }
        self.renewal_at = (self.renewal_at + 1) % self.renewal_frames;
        Ok(())
    }

    fn renew(&mut self) -> Result<()> {
        if self.population >= self.maximum {
            return Ok(());
        }
        let count =
            lighting_random::next(&mut self.state) as usize % (self.maximum - self.population + 1);
        for _ in 0..count {
            let mut cell = lighting_random::next(&mut self.state) as usize % self.slots.len();
            if self.slots[cell].is_some() {
                cell = lighting_random::next(&mut self.state) as usize % self.slots.len();
                if self.slots[cell].is_some() {
                    let third = lighting_random::next(&mut self.state) as usize % self.slots.len();
                    if self.slots[third].is_some() {
                        lighting_random::next(&mut self.state);
                    }
                    // Native renewal aborts after its third lookup, including a
                    // free third location. Preserve its random consumption.
                    return Ok(());
                }
            }
            let frames = (lighting_random::next(&mut self.state) as usize % self.base_frames + 5)
                .max(self.stops.len());
            let brightness = (lighting_random::next(&mut self.state) % 80 + 20) as f32 / 100.;
            let random = self
                .random_color
                .then(|| lighting_random::color(&mut self.state));
            let scaled = self
                .stops
                .iter()
                .map(|stop| {
                    let original = stop.pixel;
                    let color = if original.color != Rgb(0, 0, 0) {
                        random
                            .map(|color| Rgb((color >> 16) as u8, (color >> 8) as u8, color as u8))
                            .unwrap_or(original.color)
                    } else {
                        original.color
                    };
                    Stop {
                        percent: stop.percent,
                        pixel: Pixel::new(
                            Rgb(
                                (f32::from(color.0) * brightness) as u8,
                                (f32::from(color.1) * brightness) as u8,
                                (f32::from(color.2) * brightness) as u8,
                            ),
                            original.transparency(),
                        )
                        .expect("source transparency is bounded"),
                    }
                })
                .collect::<Vec<_>>();
            self.slots[cell] = Some(Star {
                palette: Palette::new(&scaled, frames)?,
                at: 0,
                remaining: 1,
                active: true,
            });
            self.population += 1;
        }
        Ok(())
    }

    pub fn storage_bytes(&self) -> usize {
        self.budget
    }
    pub fn final_random_state(&self) -> u32 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn frames_and_random_consumption_match_independent_native_instruction_reference() {
        let reference: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/native_studio_starlight_frames.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let rows = case["rows"].as_u64().unwrap() as usize;
            let cols = case["cols"].as_u64().unwrap() as usize;
            let stops = case["stops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|stop| GradientStop {
                    position: stop[0].as_u64().unwrap() as u8,
                    color: stop[1].as_u64().map(|color| color as u32),
                })
                .collect::<Vec<_>>();
            let mut cycle = StarlightCycle::new(
                rows,
                cols,
                case["duration"].as_u64().map(|value| value as u32),
                case["density"].as_u64().map(|value| value as u8),
                case["random"].as_bool().unwrap(),
                Some(&stops),
                case["seconds"].as_u64().unwrap(),
            )
            .unwrap();
            let mut frame = vec![Pixel::EMPTY; rows * cols];
            let mut bytes = Vec::new();
            for tick in 0..case["frames"].as_u64().unwrap() {
                cycle.next_frame(&mut frame).unwrap();
                for pixel in &frame {
                    bytes.extend([
                        pixel.color.0,
                        pixel.color.1,
                        pixel.color.2,
                        pixel.transparency(),
                    ]);
                }
                if let Some(snapshot) = case["snapshots"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|sample| sample["frame"] == tick)
                {
                    for (pixel, expected) in
                        frame.iter().zip(snapshot["pixels"].as_array().unwrap())
                    {
                        assert_eq!(
                            serde_json::json!([
                                pixel.color.0,
                                pixel.color.1,
                                pixel.color.2,
                                pixel.transparency()
                            ]),
                            *expected,
                            "frame {tick}"
                        );
                    }
                }
            }
            assert_eq!(
                format!("{:x}", md5::compute(&bytes)),
                case["md5"].as_str().unwrap()
            );
            assert_eq!(
                u64::from(cycle.final_random_state()),
                case["final_state"].as_u64().unwrap()
            );
            assert!(cycle.storage_bytes() <= 8 * 1024 * 1024);
        }
    }

    #[test]
    fn duration_density_dimensions_and_future_population_are_bounded_before_playback() {
        assert!(StarlightCycle::new(0, 23, Some(1700), Some(1), true, None, 0).is_err());
        assert!(StarlightCycle::new(7, 23, Some(1700), Some(11), true, None, 0).is_err());
        assert!(StarlightCycle::new(7, 23, Some(1), Some(1), true, None, 0).is_err());
        assert!(StarlightCycle::new(64, 64, Some(600000), Some(10), true, None, 0).is_err());
        let mut cycle = StarlightCycle::new(7, 23, Some(1700), Some(1), true, None, 0).unwrap();
        let before = cycle.final_random_state();
        assert!(cycle.next_frame(&mut [Pixel::EMPTY; 1]).is_err());
        assert_eq!(cycle.final_random_state(), before);
    }
}
