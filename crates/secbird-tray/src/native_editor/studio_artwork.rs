//! Optional source-matched artwork, separate from editable LED hit regions.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

pub(super) struct Artwork {
    body: Bitmap,
    legends: Bitmap,
}

impl Artwork {
    pub fn load(assets: &Path, source: &source_layout::SourceLayout, budget: &mut u64) -> Option<Self> {
        let hash = &source.source_sha256;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) { return None; }
        let root = assets.join("studio-artwork");
        let body = root.join(format!("{hash}-body.bgra"));
        let legends = root.join(format!("{hash}-legends.bgra"));
        let bytes = std::fs::metadata(&body).ok()?.len().checked_add(std::fs::metadata(&legends).ok()?.len())?;
        if bytes > *budget { return None; }
        let artwork = unsafe { Self { body: Bitmap::load_alpha(&body)?, legends: Bitmap::load_alpha(&legends)? } };
        if (artwork.body.width, artwork.body.height) != (artwork.legends.width, artwork.legends.height) { return None; }
        *budget -= bytes;
        Some(artwork)
    }

    /// Use the same three-point source transform as the LED paths, including rotation.
    pub unsafe fn paint(&self, dc: HDC, transform: &XFORM, bounds: [f64; 4], foreground: bool) {
        let bitmap = if foreground { &self.legends } else { &self.body };
        let [x, y, width, height] = bounds;
        let points = [(x, y), (x + width, y), (x, y + height)].map(|(x, y)| gp::PointF {
            X: (f64::from(transform.eM11) * x + f64::from(transform.eM21) * y + f64::from(transform.eDx)) as f32,
            Y: (f64::from(transform.eM12) * x + f64::from(transform.eM22) * y + f64::from(transform.eDy)) as f32,
        });
        let saved = SaveDC(dc);
        ModifyWorldTransform(dc, null(), MWT_IDENTITY);
        let mut graphics = null_mut();
        if gp::GdipCreateFromHDC(dc, &mut graphics) == gp::Ok {
            gp::GdipSetInterpolationMode(graphics, gp::InterpolationModeHighQualityBicubic);
            gp::GdipSetPixelOffsetMode(graphics, gp::PixelOffsetModeHighQuality);
            gp::GdipDrawImagePointsRect(graphics, bitmap.smooth.cast(), points.as_ptr(), 3,
                0., 0., bitmap.width as f32, bitmap.height as f32, gp::UnitPixel, null(), 0, null_mut());
            gp::GdipDeleteGraphics(graphics);
        }
        RestoreDC(dc, saved);
    }
}
