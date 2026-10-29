//! Bounded synthetic-frame projection diagnostic, NOT the vendor Ambient kernel.
//!
//! The retained native sampler is unresolved. This isolated reconstruction lets
//! authors inspect region geometry without capturing a desktop or admitting output.
use crate::{lighting_pixels::Pixel, studio::document::AmbientRect, Error, Result, Rgb};

pub struct DesktopFrame<'a> {
    width: usize,
    height: usize,
    rgba: &'a [u8],
}
impl<'a> DesktopFrame<'a> {
    /// Caller-owned tightly packed RGBA8. No capture, storage or filesystem API.
    pub fn new(width: usize, height: usize, rgba: &'a [u8]) -> Result<Self> {
        let bytes = width.checked_mul(height).and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || bytes.is_none_or(|n| n > 8 * 1024 * 1024 || n != rgba.len())
        {
            return Err(Error::InvalidArg(
                "Ambient test frame exceeds 8 MiB or has invalid dimensions".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
}

/// Explicit diagnostic geometry: selected LED bounding grid to normalized screen
/// rectangle, nearest center sample with a square box filter of radius blur-1.
/// These choices are intentionally separate from native runtime admission.
pub fn project(
    frame: &DesktopFrame<'_>,
    rect: AmbientRect,
    blur: u8,
    grid: [usize; 2],
    cell: [usize; 2],
) -> Result<Pixel> {
    rect.validate()?;
    let [cols, rows] = grid;
    let [col, row] = cell;
    if !(1..=9).contains(&blur) || cols == 0 || rows == 0 || col >= cols || row >= rows {
        return Err(Error::InvalidArg(
            "Invalid Ambient diagnostic grid or blur".into(),
        ));
    }
    let x = ((rect.left + (rect.right - rect.left) * (col as f64 + 0.5) / cols as f64) / 65535.
        * frame.width as f64)
        .floor() as usize;
    let y = ((rect.top + (rect.bottom - rect.top) * (row as f64 + 0.5) / rows as f64) / 65535.
        * frame.height as f64)
        .floor() as usize;
    let radius = usize::from(blur - 1);
    // Clamp taps to the requested screen region, never sample unrelated pixels.
    let left = (rect.left / 65535. * frame.width as f64).floor() as usize;
    let top = (rect.top / 65535. * frame.height as f64).floor() as usize;
    let right = ((rect.right / 65535. * frame.width as f64).ceil() as usize)
        .min(frame.width)
        .max(left + 1);
    let bottom = ((rect.bottom / 65535. * frame.height as f64).ceil() as usize)
        .min(frame.height)
        .max(top + 1);
    let (x, y) = (x.min(right - 1), y.min(bottom - 1));
    let mut sum = [0u64; 4];
    let mut count = 0u64;
    for sy in y.saturating_sub(radius).max(top)..=(y + radius).min(bottom - 1) {
        for sx in x.saturating_sub(radius).max(left)..=(x + radius).min(right - 1) {
            let rgba = &frame.rgba[(sy * frame.width + sx) * 4..][..4];
            let alpha = u64::from(rgba[3]);
            for c in 0..3 {
                sum[c] += u64::from(rgba[c]) * alpha;
            }
            sum[3] += alpha;
            count += 1;
        }
    }
    if sum[3] == 0 {
        return Ok(Pixel::EMPTY);
    }
    let channel = |c: usize| (sum[c] / sum[3]) as u8;
    let transparency = 100 - ((sum[3] * 100) / (count * 255)) as u8;
    Pixel::new(Rgb(channel(0), channel(1), channel(2)), transparency)
}

pub const TEST_WIDTH: usize = 64;
pub const TEST_HEIGHT: usize = 36;
/// Deterministic moving colour bars at the existing 40ms Studio clock.
pub fn synthetic_frame(index: u64, rgba: &mut [u8; TEST_WIDTH * TEST_HEIGHT * 4]) {
    for (i, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (i % TEST_WIDTH, i / TEST_WIDTH);
        pixel.copy_from_slice(&[
            ((x * 4 + index as usize % 256) % 256) as u8,
            (y * 7) as u8,
            if x < TEST_WIDTH / 2 { 32 } else { 224 },
            255,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spatial_centers_crops_blur_and_alpha_are_deterministic() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let frame = DesktopFrame::new(4, 1, &pixels).unwrap();
        let full = AmbientRect::preset(0).unwrap();
        for (col, color) in [
            Rgb(255, 0, 0),
            Rgb(0, 255, 0),
            Rgb(0, 0, 255),
            Rgb(255, 255, 255),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                project(&frame, full, 1, [4, 1], [col, 0]).unwrap(),
                Pixel::opaque(color)
            );
        }
        assert_eq!(
            project(&frame, full, 2, [4, 1], [1, 0]).unwrap().color,
            Rgb(85, 85, 85)
        );
        let left = AmbientRect::preset(1).unwrap();
        assert_eq!(
            project(&frame, left, 9, [1, 1], [0, 0]).unwrap().color,
            Rgb(255, 0, 0)
        );
        let rgba = [255, 0, 0, 0, 0, 0, 255, 128];
        let frame = DesktopFrame::new(2, 1, &rgba).unwrap();
        let pixel = project(&frame, full, 2, [1, 1], [0, 0]).unwrap();
        assert_eq!(pixel.color, Rgb(0, 0, 255));
        assert_eq!(pixel.transparency(), 75);
        let mut output = [Rgb(200, 100, 0)];
        crate::lighting_pixels::Layer {
            pixels: &[pixel],
            opacity: 1.,
            mask: None,
            mode: crate::lighting_pixels::BlendMode::Transparency,
        }
        .blend(&mut output)
        .unwrap();
        assert_eq!(output, [Rgb(150, 75, 63)]);
    }
    #[test]
    fn bad_geometry_buffers_and_grid_refuse_without_capture() {
        for (w, h) in [(0, 1), (usize::MAX, 2), (2048, 2048), (2, 2)] {
            assert!(DesktopFrame::new(w, h, &[0; 4]).is_err());
        }
        let frame = DesktopFrame::new(1, 1, &[0; 4]).unwrap();
        assert!(project(&frame, AmbientRect::preset(0).unwrap(), 0, [1, 1], [0, 0]).is_err());
        assert!(project(&frame, AmbientRect::preset(0).unwrap(), 1, [1, 1], [1, 0]).is_err());
    }
}
