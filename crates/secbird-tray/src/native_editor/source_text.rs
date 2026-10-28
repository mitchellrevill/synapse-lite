//! Natural DirectWrite for the two measured source text roles. See docs/TEXT-METRICS.md.
//! The C++ bridge is built against official Windows SDK COM declarations.
//! A caller must retain a creation/draw failure and expose its GDI fallback;
//! this module never silently chooses another font or text renderer.
use std::{
    ffi::c_void, fmt, marker::PhantomData, os::windows::ffi::OsStrExt, path::Path, ptr::NonNull,
    rc::Rc,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Metrics {
    pub advance_css: f32,
    pub layout_baseline_css: f32,
    pub source_baseline_physical: f32,
    pub cached_layouts: u32,
}
const _: [(); 16] = [(); std::mem::size_of::<Metrics>()];

#[derive(Clone, Copy, Debug)]
pub(super) struct Clip {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    pub status: u32,
    pub hresult: i32,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.status {
            1 => "invalid or out-of-bound heading request",
            2 => "DirectWrite Factory5 unavailable",
            3 => "exact private source fonts unavailable",
            4 => "natural text layout failed",
            5 => "bounded text bitmap failed",
            6 => "natural glyph rendering failed",
            7 => "text bitmap copy failed",
            8 => "text allocation failed",
            9 => "heading contains unsupported glyphs or controls",
            _ => "unknown text pilot failure",
        };
        write!(f, "{reason} (HRESULT 0x{:08X})", self.hresult as u32)
    }
}
impl std::error::Error for Failure {}

#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub(super) enum Role {
    AssignmentHeading = 0,
    ToolbarTitle = 1,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct SourceBox {
    pub left: f32,
    pub top: f32,
    pub width: f32,
}

extern "C" {
    fn sl_source_text_create(
        heading_file: *const u16,
        toolbar_file: *const u16,
        result: *mut *mut c_void,
        error: *mut i32,
    ) -> u32;
    fn sl_source_text_destroy(context: *mut c_void);
    fn sl_source_text_draw(
        context: *mut c_void,
        destination: *mut c_void,
        role: u32,
        text: *const u16,
        length: u32,
        clip_x: i32,
        clip_y: i32,
        width: u32,
        height: u32,
        box_left_css: f32,
        box_top_css: f32,
        box_width_css: f32,
        scale: f32,
        color_ref: u32,
        metrics: *mut Metrics,
        error: *mut i32,
    ) -> u32;
}

/// Own on the native window thread; dropping releases its isolated factory,
/// exact font collection, at most32 layouts and at most2048x256 bitmap pixels.
pub(super) struct TextPilot {
    context: NonNull<c_void>,
    _window_thread: PhantomData<Rc<()>>,
}
impl TextPilot {
    pub fn new(assets: &Path) -> Result<Self, Failure> {
        let path = |name: &str| -> Result<Vec<u16>, Failure> {
            let mut file: Vec<u16> = assets.join(name).as_os_str().encode_wide().collect();
            if file.is_empty() || file.contains(&0) || file.len() > 32767 {
                return Err(Failure {
                    status: 1,
                    hresult: 0x80070057u32 as i32,
                });
            }
            file.push(0);
            Ok(file)
        };
        let heading = path("RazerF5-Regular.ttf")?;
        let toolbar = path("Roboto-Regular.ttf")?;
        let mut context = std::ptr::null_mut();
        let mut hresult = 0;
        let status = unsafe {
            sl_source_text_create(
                heading.as_ptr(),
                toolbar.as_ptr(),
                &mut context,
                &mut hresult,
            )
        };
        if status != 0 {
            return Err(Failure { status, hresult });
        }
        let context = NonNull::new(context).ok_or(Failure {
            status: 3,
            hresult: 0x80004005u32 as i32,
        })?;
        Ok(Self {
            context,
            _window_thread: PhantomData,
        })
    }

    /// Render one fixed source role with natural advances: RazerF5-16/400
    /// left/baseline15 or Roboto14/400 centered/baseline23. SourceBox uses
    /// absolute client CSS pixels, Clip uses physical pixels. Clip rounding is
    /// compensated before drawing; source baselines remain fractional.
    /// Pass the source uppercase label (this does not perform localization).
    /// COLORREF is the Windows BGR integer representation, not CSS RGB.
    ///
    /// # Safety
    /// `destination` must be a live window-thread HDC with an MM_TEXT physical
    /// pixel mapping, identity world transform, and the caller's desired clip.
    pub unsafe fn draw(
        &mut self,
        destination: *mut c_void,
        text: &str,
        role: Role,
        clip: Clip,
        source: SourceBox,
        scale: f32,
        color_ref: u32,
    ) -> Result<Metrics, Failure> {
        // Bound before allocating UTF-16; longer input takes a visible fallback.
        if text.len() > 1024 {
            return Err(Failure {
                status: 1,
                hresult: 0x80070057u32 as i32,
            });
        }
        let text: Vec<u16> = text.encode_utf16().collect();
        let mut metrics = Metrics::default();
        let mut hresult = 0;
        let status = sl_source_text_draw(
            self.context.as_ptr(),
            destination,
            role as u32,
            text.as_ptr(),
            text.len() as u32,
            clip.x,
            clip.y,
            clip.width,
            clip.height,
            source.left,
            source.top,
            source.width,
            scale,
            color_ref,
            &mut metrics,
            &mut hresult,
        );
        if status == 0 {
            Ok(metrics)
        } else {
            Err(Failure { status, hresult })
        }
    }
}
impl Drop for TextPilot {
    fn drop(&mut self) {
        unsafe { sl_source_text_destroy(self.context.as_ptr()) }
    }
}

/// One initialization per font set, with a retained failure and GDI fallback.
/// A failed draw disables this renderer until the font set is recreated.
pub(super) struct Renderer {
    state: std::cell::RefCell<Result<TextPilot, Failure>>,
    last_metrics: std::cell::Cell<Metrics>,
}

impl Renderer {
    pub fn new(assets: &Path) -> Self {
        Self {
            state: std::cell::RefCell::new(TextPilot::new(assets)),
            last_metrics: std::cell::Cell::new(Metrics::default()),
        }
    }

    pub fn diagnostic(&self) -> Option<String> {
        self.state.borrow().as_ref().err().map(ToString::to_string)
    }

    pub fn metrics(&self) -> Metrics {
        self.last_metrics.get()
    }

    pub unsafe fn draw(
        &self,
        destination: *mut c_void,
        text: &str,
        role: Role,
        clip: Clip,
        source: SourceBox,
        scale: f32,
        color_ref: u32,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let Ok(pilot) = &mut *state else { return false };
        match pilot.draw(destination, text, role, clip, source, scale, color_ref) {
            Ok(metrics) => {
                self.last_metrics.set(metrics);
                true
            }
            Err(failure) => {
                *state = Err(failure);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::*;

    #[test]
    fn missing_fonts_leave_a_retained_fallback_reason() {
        let renderer = Renderer::new(Path::new("missing-source-font-fixture"));
        assert!(renderer.diagnostic().is_some());
        assert_eq!(renderer.metrics().cached_layouts, 0);
    }

    #[test]
    #[ignore = "Requires SYNAPSE_LITE_TEST_FONT_DIR pointing to recovered local fonts"]
    fn linked_bridge_preserves_natural_advances_and_fractional_source_baselines() {
        let assets = std::env::var_os("SYNAPSE_LITE_TEST_FONT_DIR").expect("local font directory");
        let mut pilot = TextPilot::new(Path::new(&assets)).unwrap();
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 512;
            info.bmiHeader.biHeight = -512;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut pixels = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                dc,
                &info,
                DIB_RGB_COLORS,
                &mut pixels,
                std::ptr::null_mut(),
                0,
            );
            assert!(!bitmap.is_null());
            let old = SelectObject(dc, bitmap);
            let mut results = Vec::new();
            for scale in [1.0_f32, 1.25] {
                for (role, label, advance, baseline) in [
                    (Role::AssignmentHeading, "MOUSE FUNCTION", 127.3120, 204.0),
                    (Role::ToolbarTitle, "RAZER ORNATA V2", 119.9570, 212.0),
                ] {
                    let clip = Clip {
                        x: 0,
                        y: (189.0 * scale).round() as i32,
                        width: 512,
                        height: 64,
                    };
                    let result = pilot
                        .draw(
                            dc,
                            label,
                            role,
                            clip,
                            SourceBox {
                                left: 0.0,
                                top: 189.0,
                                width: 400.0,
                            },
                            scale,
                            0x00cccccc,
                        )
                        .unwrap();
                    results.push((result, advance, baseline * scale));
                }
            }
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            for (result, advance, baseline) in results {
                assert!((result.advance_css - advance).abs() < 0.001);
                assert!((result.source_baseline_physical - baseline).abs() < 0.001);
            }
        }
    }
}
