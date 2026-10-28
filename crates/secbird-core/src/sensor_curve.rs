//! Source-backed Custom Dynamic Sensitivity interpolation and firmware samples.
//!
//! Product 196's UI uses a shape-preserving cubic Hermite spline, then chooses
//! the first of 1000 uniform samples whose rounded X matches each integer 1..250.
//! It does not evaluate the spline directly at those 250 integer coordinates.
//! X is input speed in count/ms; Y is the ratio of output to input.
//!
//! This module owns no profile JSON. Callers retain the original objects and
//! opaque fields; `Point` is a numeric projection, not an interchange format.
//! Invalid/unsupported point data is rejected, never sorted or repaired.
use crate::{Error, Result};

pub const SAMPLE_COUNT: usize = 250;
pub const ENCODED_BYTES: usize = SAMPLE_COUNT * 4;
pub const MIN_RATIO: f64 = 0.1;
pub const MAX_RATIO: f64 = 6.0;
pub const MAX_INPUT_SPEED: f64 = 280.0;
const REPLAY_SAMPLES: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug)]
pub struct SensorCurve {
    knots: Vec<f64>,
    coefficients: Vec<[f64; 4]>,
}

/// Accept the finite editable coordinate domain, without imposing the editor's
/// two-unit drag spacing on pre-existing valid curves with closer points.
pub fn validate_points(points: &[Point]) -> Result<()> {
    if points.len() < 3 {
        return Err(invalid(
            "Custom sensitivity needs at least three control points",
        ));
    }
    if points[0].x != 0.0 {
        return Err(invalid("Custom sensitivity must start at input speed zero"));
    }
    for (index, point) in points.iter().enumerate() {
        if !point.x.is_finite()
            || !point.y.is_finite()
            || !(0.0..=MAX_INPUT_SPEED).contains(&point.x)
            || !(MIN_RATIO..=MAX_RATIO).contains(&point.y)
        {
            return Err(invalid(
                "Custom sensitivity has unsupported control-point coordinates",
            ));
        }
        if index > 0 && point.x <= points[index - 1].x {
            return Err(invalid(
                "Custom sensitivity input speeds must increase strictly",
            ));
        }
    }
    Ok(())
}

impl SensorCurve {
    pub fn new(points: &[Point]) -> Result<Self> {
        validate_points(points)?;
        let intervals = points.len() - 1;
        let mut lengths = Vec::with_capacity(intervals);
        let mut slopes = Vec::with_capacity(intervals);
        for pair in points.windows(2) {
            let length = pair[1].x - pair[0].x;
            let slope = (pair[1].y - pair[0].y) / length;
            if !slope.is_finite() {
                return Err(invalid("Custom sensitivity control points are too close"));
            }
            lengths.push(length);
            slopes.push(slope);
        }

        let mut derivatives = vec![0.0; points.len()];
        for index in 1..intervals {
            let previous = slopes[index - 1];
            let next = slopes[index];
            if previous.abs() < f64::EPSILON
                || next.abs() < f64::EPSILON
                || js_sign(previous) != js_sign(next)
            {
                derivatives[index] = 0.0;
            } else {
                let previous_length = lengths[index - 1];
                let next_length = lengths[index];
                let first_weight = 2.0 * next_length + previous_length;
                let second_weight = next_length + 2.0 * previous_length;
                derivatives[index] = (first_weight + second_weight)
                    / (first_weight / previous + second_weight / next);
            }
        }
        derivatives[0] = endpoint_slope(lengths[0], lengths[1], slopes[0], slopes[1]);
        derivatives[intervals] = endpoint_slope(
            lengths[intervals - 1],
            lengths[intervals - 2],
            slopes[intervals - 1],
            slopes[intervals - 2],
        );

        let mut coefficients = Vec::with_capacity(intervals);
        for index in 0..intervals {
            let width = lengths[index];
            let delta = points[index + 1].y - points[index].y;
            let start = derivatives[index];
            let end = derivatives[index + 1];
            let row = [
                points[index].y,
                start,
                (3.0 * delta / width - 2.0 * start - end) / width,
                (2.0 * (points[index].y - points[index + 1].y) / width + start + end)
                    / (width * width),
            ];
            if row.iter().any(|coefficient| !coefficient.is_finite()) {
                return Err(invalid(
                    "Custom sensitivity control points produce a degenerate curve",
                ));
            }
            coefficients.push(row);
        }
        Ok(Self {
            knots: points.iter().map(|point| point.x).collect(),
            coefficients,
        })
    }

    /// The source also uses the end segments for finite outside-domain queries
    /// while drawing. Hardware sample generation below stays within the knots.
    pub fn interpolate(&self, x: f64) -> Result<f64> {
        if !x.is_finite() {
            return Err(invalid("Custom sensitivity query must be finite"));
        }
        let mut left = 0;
        let mut right = self.knots.len() - 1;
        while right - left > 1 {
            let middle = (left + right) / 2;
            if self.knots[middle] <= x {
                left = middle;
            } else {
                right = middle;
            }
        }
        let offset = x - self.knots[left];
        let [constant, linear, quadratic, cubic] = self.coefficients[left];
        let value = constant + offset * (linear + offset * (quadratic + offset * cubic));
        if !value.is_finite() {
            return Err(invalid(
                "Custom sensitivity query produced a nonfinite ratio",
            ));
        }
        Ok(value)
    }

    pub fn firmware_samples(&self) -> Result<[f64; SAMPLE_COUNT]> {
        let mut replay = [(0.0, 0.0); REPLAY_SAMPLES];
        let final_x = *self.knots.last().unwrap();
        for (index, sample) in replay.iter_mut().enumerate() {
            let x = index as f64 * final_x / 999.0;
            *sample = (x, self.interpolate(x)?);
        }
        let mut values = [0.0; SAMPLE_COUNT];
        let mut last_value = 0.0;
        for (index, value) in values.iter_mut().enumerate() {
            let target = (index + 1) as f64;
            for &(x, y) in &replay {
                // All supported X values are nonnegative, so Rust round has
                // the same tie behavior as the source Math.round here.
                if x.round() == target {
                    last_value = y;
                    *value = y.max(MIN_RATIO);
                    break;
                }
                if x > target {
                    break;
                }
            }
            if *value == 0.0 {
                *value = last_value.max(MIN_RATIO);
            }
        }
        Ok(values)
    }
}

/// Match the source Float32Array byte representation on its Windows x64 host.
/// Reject nonfinite/out-of-range encoded ratios rather than normalizing them.
pub fn encode_samples(values: &[f64; SAMPLE_COUNT]) -> Result<[u8; ENCODED_BYTES]> {
    let mut bytes = [0; ENCODED_BYTES];
    for (value, destination) in values.iter().zip(bytes.chunks_exact_mut(4)) {
        let narrowed = *value as f32;
        if !value.is_finite()
            || !narrowed.is_finite()
            || !(MIN_RATIO as f32..=MAX_RATIO as f32).contains(&narrowed)
        {
            return Err(invalid(
                "Custom sensitivity has an unsupported encoded ratio",
            ));
        }
        destination.copy_from_slice(&narrowed.to_le_bytes());
    }
    Ok(bytes)
}

fn endpoint_slope(width: f64, adjacent_width: f64, slope: f64, adjacent_slope: f64) -> f64 {
    let derivative = ((2.0 * width + adjacent_width) * slope - width * adjacent_slope)
        / (width + adjacent_width);
    if js_sign(derivative) != js_sign(slope) {
        0.0
    } else if js_sign(slope) != js_sign(adjacent_slope) && derivative.abs() > 3.0 * slope.abs() {
        3.0 * slope
    } else {
        derivative
    }
}

fn js_sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        value
    }
}

fn invalid(message: &str) -> Error {
    Error::InvalidArg(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_and_float32_bytes_match_extracted_source_goldens() {
        let fixtures: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/validation/sensor-curve-fixtures.json"
        ))
        .unwrap();
        for fixture in fixtures["curves"].as_array().unwrap() {
            let name = fixture["name"].as_str().unwrap();
            let points: Vec<_> = fixture["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| Point {
                    x: point["x"].as_f64().unwrap(),
                    y: point["y"].as_f64().unwrap(),
                })
                .collect();
            let curve = SensorCurve::new(&points).unwrap();
            let values = curve.firmware_samples().unwrap();
            for checkpoint in fixture["checkpoints"].as_array().unwrap() {
                let index = checkpoint["index"].as_u64().unwrap() as usize;
                let expected = checkpoint["value"].as_f64().unwrap();
                assert!(
                    (values[index] - expected).abs() < 1e-11,
                    "{name} sample {index}: {} != {expected}",
                    values[index]
                );
            }
            let hex = fixture["float32_le_hex"].as_str().unwrap();
            assert_eq!(hex.len(), ENCODED_BYTES * 2);
            let expected: Vec<_> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(
                encode_samples(&values).unwrap().as_slice(),
                expected,
                "{name} float32 bytes"
            );
        }
    }

    #[test]
    fn invalid_points_and_encoding_are_rejected_without_repair() {
        let valid = [
            Point { x: 0.0, y: 1.0 },
            Point { x: 20.0, y: 1.1 },
            Point { x: 100.0, y: 1.5 },
        ];
        assert!(SensorCurve::new(&valid[..2]).is_err());
        for bad in [
            Point { x: 0.0, y: 1.1 },
            Point { x: -1.0, y: 1.1 },
            Point { x: 281.0, y: 1.1 },
            Point {
                x: f64::NAN,
                y: 1.1,
            },
            Point {
                x: 20.0,
                y: f64::INFINITY,
            },
            Point { x: 20.0, y: 0.09 },
            Point { x: 20.0, y: 6.01 },
        ] {
            let mut points = valid;
            points[1] = bad;
            assert!(SensorCurve::new(&points).is_err(), "accepted {bad:?}");
        }
        let mut shifted = valid;
        shifted[0].x = 1.0;
        assert!(SensorCurve::new(&shifted).is_err());
        let mut unsorted = valid;
        unsorted.swap(1, 2);
        assert!(SensorCurve::new(&unsorted).is_err());
        let mut table = [1.0; SAMPLE_COUNT];
        table[3] = f64::NAN;
        assert!(encode_samples(&table).is_err());
        table[3] = 6.1;
        assert!(encode_samples(&table).is_err());
    }

    #[test]
    fn firmware_sampling_retains_the_source_first_match_and_tail() {
        let curve = SensorCurve::new(&[
            Point { x: 0.0, y: 1.0 },
            Point { x: 20.0, y: 1.1 },
            Point { x: 40.0, y: 1.2 },
            Point { x: 70.0, y: 1.35 },
            Point { x: 100.0, y: 1.5 },
        ])
        .unwrap();
        assert!((curve.interpolate(100.0).unwrap() - 1.5).abs() < 1e-12);
        let values = curve.firmware_samples().unwrap();
        assert!((values[0] - 1.0025025025025025).abs() < 1e-12);
        assert!((values[249] - 1.497997997997998).abs() < 1e-12);
        assert_eq!(values[99], values[249]);
        assert!(curve.interpolate(f64::NAN).is_err());
    }
}
