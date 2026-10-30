//! Owns native bitmap pixels and their GDI+ view. The view borrows the DIB and
//! must be destroyed before its backing pixels. No image files remain open.
use super::{image_render, wide};
use std::{
    path::Path,
    ptr::null_mut,
};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    LoadImageW, IMAGE_BITMAP, LR_CREATEDIBSECTION, LR_LOADFROMFILE,
};

struct PixelLayout {
    width: u32,
    height: u32,
    length: usize,
}

impl PixelLayout {
    fn decode(header: &[u8; 16], file_length: u64) -> Option<Self> {
        if &header[..8] != b"SLBGRA1\0" {
            return None;
        }
        let width = u32::from_le_bytes(header[8..12].try_into().ok()?);
        let height = u32::from_le_bytes(header[12..16].try_into().ok()?);
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return None;
        }
        let length = width as usize * height as usize * 4;
        if file_length != (16 + length) as u64 {
            return None;
        }
        Some(Self {
            width,
            height,
            length,
        })
    }
}

pub(super) struct Bitmap {
    pub(super) handle: HBITMAP,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) alpha: bool,
    pub(super) smooth: *mut windows_sys::Win32::Graphics::GdiPlus::GpBitmap,
}

impl Bitmap {
    pub(super) unsafe fn load(path: &Path) -> Option<Self> {
        let alpha_path = path.with_extension("bgra");
        if alpha_path.exists() {
            return Self::load_alpha(&alpha_path);
        }
        let handle = LoadImageW(
            null_mut(),
            wide(&path.to_string_lossy()).as_ptr(),
            IMAGE_BITMAP,
            0,
            0,
            LR_LOADFROMFILE | LR_CREATEDIBSECTION,
        );
        if handle.is_null() {
            return None;
        }
        let mut info: BITMAP = std::mem::zeroed();
        if GetObjectW(
            handle,
            std::mem::size_of::<BITMAP>() as i32,
            (&mut info as *mut BITMAP).cast(),
        ) == 0
        {
            DeleteObject(handle);
            return None;
        }
        Some(Self {
            handle,
            width: info.bmWidth,
            height: info.bmHeight,
            alpha: false,
            smooth: null_mut(),
        })
    }

    // Provisioned files contain top-down, premultiplied BGRA pixels. Keeping
    // alpha separate from LoadImage avoids its legacy BMP alpha conversion.
    pub(super) unsafe fn load_alpha(path: &Path) -> Option<Self> {
        let file = std::fs::File::open(path).ok()?;
        let length = file.metadata().ok()?.len();
        Self::read_alpha(file, length)
    }

    pub(super) unsafe fn brand_logo() -> Option<Self> {
        let bytes = include_bytes!("../../../../assets/logo.bgra");
        Self::read_alpha(std::io::Cursor::new(bytes), bytes.len() as u64)
    }

    unsafe fn read_alpha(mut reader: impl std::io::Read, length: u64) -> Option<Self> {
        let mut header = [0u8; 16];
        reader.read_exact(&mut header).ok()?;
        let PixelLayout {
            width,
            height,
            length,
        } = PixelLayout::decode(&header, length)?;
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width as i32;
        info.bmiHeader.biHeight = -(height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = null_mut();
        let handle = CreateDIBSection(
            null_mut(),
            &info,
            DIB_RGB_COLORS,
            &mut pixels,
            null_mut(),
            0,
        );
        if handle.is_null() {
            return None;
        }
        let mut bitmap = Self {
            handle,
            width: width as i32,
            height: height as i32,
            alpha: true,
            smooth: null_mut(),
        };
        let buffer = std::slice::from_raw_parts_mut(pixels.cast::<u8>(), length);
        reader.read_exact(buffer).ok()?;
        // Refuse straight-alpha or malformed generated data rather than
        // producing bright fringes around semitransparent edges.
        if buffer
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|c| *c > p[3]))
        {
            return None;
        }
        bitmap.smooth = image_render::from_pixels(width as i32, height as i32, buffer.as_ptr());
        if bitmap.smooth.is_null() {
            return None;
        }
        Some(bitmap)
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            image_render::dispose(self.smooth);
            DeleteObject(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(width: u32, height: u32) -> [u8; 16] {
        let mut header = [0u8; 16];
        header[..8].copy_from_slice(b"SLBGRA1\0");
        header[8..12].copy_from_slice(&width.to_le_bytes());
        header[12..16].copy_from_slice(&height.to_le_bytes());
        header
    }

    #[test]
    fn pixel_layout_rejects_invalid_files_before_allocating_native_memory() {
        let layout = PixelLayout::decode(&header(96, 48), 16 + 96 * 48 * 4).unwrap();
        assert_eq!(
            (layout.width, layout.height, layout.length),
            (96, 48, 96 * 48 * 4)
        );
        assert!(PixelLayout::decode(&header(0, 48), 16).is_none());
        assert!(PixelLayout::decode(&header(u32::MAX, 1), u64::MAX).is_none());
        assert!(PixelLayout::decode(&header(4096, 4097), 16).is_none());
        assert!(PixelLayout::decode(&header(96, 48), 16 + 96 * 48 * 4 - 1).is_none());
        assert!(PixelLayout::decode(&header(96, 48), 16 + 96 * 48 * 4 + 1).is_none());
        let mut invalid = header(96, 48);
        invalid[0] = b'X';
        assert!(PixelLayout::decode(&invalid, 16 + 96 * 48 * 4).is_none());
    }
}
