//! Recovered Customize CSS: centered 22px tiles, 20px masks, radial corner fade.
//! The raster is cached once at the current physical size/phase, never per frame.
use super::*;

#[derive(Clone, Copy, PartialEq)]
struct Key {
    width: i32,
    height: i32,
    css_width: f64,
    css_height: f64,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
}

#[derive(Default)]
pub(super) struct Cache {
    key: Option<Key>,
    surface: Option<Surface>,
}

impl Cache {
    pub fn ready(&self) -> bool {
        self.surface.is_some()
    }

    pub unsafe fn draw(
        &mut self,
        dc: HDC,
        rect: RECT,
        css: assignment_layout::Rect,
        scale: f64,
    ) -> bool {
        let key = Key {
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
            css_width: css.width,
            css_height: css.height,
            scale,
            offset_x: rect.left as f64 / scale - css.x,
            offset_y: rect.top as f64 / scale - css.y,
        };
        if self.key != Some(key) {
            // Release the old allocation before making the next size.
            self.surface = None;
            self.key = Some(key);
            self.surface = Surface::new(key);
        }
        let Some(surface) = &self.surface else {
            return false;
        };
        BitBlt(
            dc, rect.left, rect.top, key.width, key.height, surface.dc, 0, 0, SRCCOPY,
        ) != 0
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}

impl Surface {
    unsafe fn new(key: Key) -> Option<Self> {
        // Current source wrapper: at most 1220x340 CSS pixels, scale 0.5..4.
        if !(0.5..=4.0).contains(&key.scale)
            || key.width <= 0
            || key.height <= 0
            || key.width > 4881
            || key.height > 1361
        {
            return None;
        }
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = key.width;
        info.bmiHeader.biHeight = -key.height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut pixels = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, bitmap);
        if previous.is_null() {
            DeleteObject(bitmap);
            DeleteDC(dc);
            return None;
        }
        let surface = Self {
            dc,
            bitmap,
            previous,
        };
        let pixels = std::slice::from_raw_parts_mut(
            pixels.cast::<u8>(),
            key.width as usize * key.height as usize * 4,
        );
        rasterize(key, pixels);
        Some(surface)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

fn coverage(index: i32, extent: f64, scale: f64, offset: f64) -> f64 {
    let start = (index as f64 / scale + offset).max(0.0);
    let end = ((index + 1) as f64 / scale + offset).min(extent);
    let origin = (extent - 22.0) / 2.0;
    // Chromium first rasterizes the fractional device-pixel tile to ceil(size),
    // then repeats it at the original CSS size. This matters at 125%/175%.
    let dot_start = 20.0 * scale * 22.0 / (22.0 * scale).ceil();
    let tile = ((start - origin - dot_start) / 22.0).floor();
    let mut amount = 0.0;
    for next in [tile, tile + 1.0] {
        let dot = origin + next * 22.0 + dot_start;
        amount += (end.min(dot + 22.0 - dot_start) - start.max(dot)).max(0.0) * scale;
    }
    amount.clamp(0.0, 1.0)
}

fn rasterize(key: Key, pixels: &mut [u8]) {
    let root_two = std::f64::consts::SQRT_2;
    let horizontal: Vec<_> = (0..key.width)
        .map(|x| {
            let position = (x as f64 + 0.5) / key.scale + key.offset_x;
            let normalized = (position - key.css_width / 2.0) / (key.css_width / 2.0 * root_two);
            (
                coverage(x, key.css_width, key.scale, key.offset_x),
                normalized * normalized,
            )
        })
        .collect();
    for y in 0..key.height {
        let vertical = coverage(y, key.css_height, key.scale, key.offset_y);
        let position = (y as f64 + 0.5) / key.scale + key.offset_y;
        let normalized = (position - key.css_height / 2.0) / (key.css_height / 2.0 * root_two);
        for (x, &(area, distance_x)) in horizontal.iter().enumerate() {
            let fade = 1.0 - (distance_x + normalized * normalized).sqrt().min(1.0);
            let color = (34.0 + 59.0 * area * vertical * fade).round() as u8;
            let offset = (y as usize * key.width as usize + x) * 4;
            pixels[offset..offset + 4].copy_from_slice(&[color, color, color, 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resized_cache_draws_real_pixels_and_releases_gdi_objects() {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetGuiResources, GR_GDIOBJECTS};
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 1600;
            info.bmiHeader.biHeight = -500;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut pixels = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(!bitmap.is_null());
            let previous = SelectObject(dc, bitmap);
            let before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            let mut colors = Vec::new();
            {
                let mut cache = Cache::default();
                for width in [1000.0, 1220.0, 1000.0, 1220.0, 1000.0] {
                    let css = assignment_layout::Rect {
                        x: 0.0,
                        y: 0.0,
                        width,
                        height: 340.0,
                    };
                    let rect = RECT {
                        left: 0,
                        top: 0,
                        right: (width * 1.25) as i32,
                        bottom: 425,
                    };
                    assert!(cache.draw(dc, rect, css, 1.25));
                    assert!(cache.ready());
                    if width == 1000.0 {
                        colors.push(GetPixel(dc, 636, 224) & 255);
                        assert_eq!(GetPixel(dc, 0, 0) & 255, 34);
                    }
                }
            }
            let after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            SelectObject(dc, previous);
            DeleteObject(bitmap);
            DeleteDC(dc);
            assert_eq!(before, after, "Resizing must not retain old DCs/bitmaps");
            assert!(colors.iter().all(|color| *color >= 89 && *color <= 92));
            assert!(colors.windows(2).all(|pair| pair[0] == pair[1]));
        }
    }

    #[test]
    #[ignore = "exports fixtures for independent recovered-CSS browser comparison"]
    fn export_native_grid_fixtures() {
        let directory =
            std::env::var_os("SYNAPSE_LITE_GRID_PROBE_DIR").expect("fixture output directory");
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        for scale in [1.0_f64, 1.25, 1.5, 2.0] {
            for width in [770.0_f64, 1000.0, 1220.0] {
                let key = Key {
                    width: (width * scale).round() as i32,
                    height: (340.0 * scale).round() as i32,
                    css_width: width,
                    css_height: 340.0,
                    scale,
                    offset_x: 0.0,
                    offset_y: 0.0,
                };
                let mut pixels = vec![0; key.width as usize * key.height as usize * 4];
                rasterize(key, &mut pixels);
                std::fs::write(
                    directory.join(format!("native-{}-{scale}.bgra", width as i32)),
                    pixels,
                )
                .unwrap();
            }
        }
    }
}
