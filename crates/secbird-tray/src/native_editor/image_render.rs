//! Smooth premultiplied-alpha image scaling using the Windows GDI+ renderer.
use super::*;
use windows_sys::Win32::Graphics::GdiPlus as gp;

pub(super) struct Runtime(usize);

impl Runtime {
    pub unsafe fn start() -> Result<Self, String> {
        let input = gp::GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: 0,
            SuppressExternalCodecs: 1,
        };
        let mut token = 0;
        let status = gp::GdiplusStartup(&mut token, &input, null_mut());
        if status != gp::Ok {
            return Err(format!("Could not start image renderer ({status})"));
        }
        Ok(Self(token))
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            gp::GdiplusShutdown(self.0);
        }
    }
}

// GDI+ PixelFormat32bppPARGB: 32 bits, alpha, premultiplied, GDI-compatible.
const PREMULTIPLIED_BGRA: i32 = 0x000e_200b;

/// Pixels belong to the caller's DIB and must outlive the GDI+ image.
pub(super) unsafe fn from_pixels(width: i32, height: i32, pixels: *const u8) -> *mut gp::GpBitmap {
    let mut bitmap = null_mut();
    if gp::GdipCreateBitmapFromScan0(
        width,
        height,
        width * 4,
        PREMULTIPLIED_BGRA,
        pixels,
        &mut bitmap,
    ) != gp::Ok
    {
        return null_mut();
    }
    bitmap
}

pub(super) unsafe fn dispose(bitmap: *mut gp::GpBitmap) {
    if !bitmap.is_null() {
        gp::GdipDisposeImage(bitmap.cast());
    }
}

pub(super) unsafe fn draw(dc: HDC, bitmap: *mut gp::GpBitmap, rect: RECT, width: i32, height: i32) {
    let mut graphics = null_mut();
    if gp::GdipCreateFromHDC(dc, &mut graphics) != gp::Ok {
        return;
    }
    // AlphaBlend forces COLORONCOLOR, even after SetStretchBltMode(HALFTONE).
    // GDI+ filters premultiplied colors and alpha together at the actual DPI.
    gp::GdipSetInterpolationMode(graphics, gp::InterpolationModeHighQualityBicubic);
    gp::GdipSetPixelOffsetMode(graphics, gp::PixelOffsetModeHighQuality);
    gp::GdipDrawImageRectRectI(
        graphics,
        bitmap.cast(),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        0,
        0,
        width,
        height,
        gp::UnitPixel,
        null(),
        0,
        null_mut(),
    );
    gp::GdipDeleteGraphics(graphics);
}

pub(super) unsafe fn draw_rotated(dc:HDC,bitmap:*mut gp::GpBitmap,rect:RECT,width:i32,height:i32,angle:f32){
    if bitmap.is_null(){return;}
    let mut graphics=null_mut();if gp::GdipCreateFromHDC(dc,&mut graphics)!=gp::Ok{return;}
    gp::GdipSetInterpolationMode(graphics,gp::InterpolationModeHighQualityBicubic);
    gp::GdipSetPixelOffsetMode(graphics,gp::PixelOffsetModeHighQuality);
    gp::GdipTranslateWorldTransform(graphics,(rect.left+rect.right) as f32/2.,(rect.top+rect.bottom) as f32/2.,gp::MatrixOrderAppend);
    gp::GdipRotateWorldTransform(graphics,angle,gp::MatrixOrderPrepend);
    let w=rect.right-rect.left;let h=rect.bottom-rect.top;
    gp::GdipDrawImageRectRectI(graphics,bitmap.cast(),-w/2,-h/2,w,h,0,0,width,height,gp::UnitPixel,null(),0,null_mut());
    gp::GdipDeleteGraphics(graphics);
}

/// One batched antialiased curve and translucent feedback polygon, rather than
/// creating a separate GDI pen for every segment on every feedback frame.
pub(super) unsafe fn draw_plot(dc:HDC,line:&[POINT],fill:&[POINT],line_color:u32,fill_color:u32,stroke:f32){
    if line.len()<2{return;}
    let mut graphics=null_mut();if gp::GdipCreateFromHDC(dc,&mut graphics)!=gp::Ok{return;}
    gp::GdipSetSmoothingMode(graphics,gp::SmoothingModeAntiAlias);
    if fill.len()>=3 {
        let polygon:Vec<_>=fill.iter().map(|p|gp::Point{X:p.x,Y:p.y}).collect();let mut brush=null_mut();
        if gp::GdipCreateSolidFill(fill_color,&mut brush)==gp::Ok{gp::GdipFillPolygonI(graphics,brush.cast(),polygon.as_ptr(),polygon.len() as i32,gp::FillModeWinding);gp::GdipDeleteBrush(brush.cast());}
    }
    let points:Vec<_>=line.iter().map(|p|gp::Point{X:p.x,Y:p.y}).collect();let mut pen=null_mut();
    if gp::GdipCreatePen1(0xff000000|line_color,stroke,gp::UnitPixel,&mut pen)==gp::Ok{gp::GdipDrawLinesI(graphics,pen,points.as_ptr(),points.len() as i32);gp::GdipDeletePen(pen);}
    gp::GdipDeleteGraphics(graphics);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_plot_alpha_preserves_the_grid_background_and_respects_the_chart_clip(){unsafe{
        let _runtime=Runtime::start().unwrap();let mut info:BITMAPINFO=std::mem::zeroed();info.bmiHeader.biSize=std::mem::size_of::<BITMAPINFOHEADER>() as u32;info.bmiHeader.biWidth=40;info.bmiHeader.biHeight = -40;info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;let mut bits=null_mut();let surface=CreateDIBSection(null_mut(),&info,DIB_RGB_COLORS,&mut bits,null_mut(),0);assert!(!surface.is_null());let dc=CreateCompatibleDC(null_mut());let old=SelectObject(dc,surface);let brush=CreateSolidBrush(rgb(0x111111));let r=RECT{left:0,top:0,right:40,bottom:40};FillRect(dc,&r,brush);IntersectClipRect(dc,10,10,30,30);
        draw_plot(dc,&[POINT{x:0,y:11},POINT{x:39,y:11}],&[POINT{x:0,y:0},POINT{x:39,y:0},POINT{x:39,y:39},POINT{x:0,y:39}],GREEN,0x4d44d62c,1.);
        SelectClipRgn(dc,null_mut());let outside=GetPixel(dc,5,20);let inside=GetPixel(dc,20,20);SelectObject(dc,old);DeleteObject(brush);DeleteObject(surface);DeleteDC(dc);
        assert_eq!(outside,rgb(0x111111));let channels=[inside&255,(inside>>8)&255,(inside>>16)&255];for(actual,expected)in channels.into_iter().zip([32u32,76,25]){assert!(actual.abs_diff(expected)<=1,"Unexpected live fill alpha: {channels:?}");}
    }}

    #[test]
    fn fractional_scaling_filters_alpha_edges_instead_of_stepping() {
        unsafe {
            let _runtime = Runtime::start().unwrap();
            let source = [0, 0, 0, 0, 255, 255, 255, 255];
            let image = from_pixels(2, 1, source.as_ptr());
            assert!(!image.is_null());
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = 25;
            info.bmiHeader.biHeight = -9;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut bits = null_mut();
            let surface =
                CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            assert!(!surface.is_null());
            let dc = CreateCompatibleDC(null_mut());
            let old = SelectObject(dc, surface);
            let rect = RECT {
                left: 0,
                top: 0,
                right: 25,
                bottom: 9,
            };
            let background = CreateSolidBrush(rgb(0x111111));
            FillRect(dc, &rect, background);
            draw(dc, image, rect, 2, 1);
            let colors: Vec<u32> = (0..25).map(|x| GetPixel(dc, x, 4) & 255).collect();
            SelectObject(dc, old);
            DeleteObject(background);
            DeleteObject(surface);
            DeleteDC(dc);
            dispose(image);
            assert_eq!(
                colors[0], 17,
                "Transparent edge must preserve its background"
            );
            let intermediate = colors.iter().filter(|c| **c > 17 && **c < 250).count();
            assert!(
                intermediate > 5,
                "Expected a smooth alpha transition: {colors:?}"
            );
        }
    }
}
