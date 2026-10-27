//! Full-client custom frame. The installed frameless AppEngine gives its product
//! view the complete window width and height minus the42px host on Windows11.
//! These functions run before any editor State borrow, including creation and
//! synchronous resize callbacks. They perform no device or profile operations.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl From<RECT> for Bounds {
    fn from(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}
impl Bounds {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// Half-open resize zones in physical screen coordinates, independent of the
/// editor's cached client size. Maximized windows retain header dragging only.
pub(super) fn resize_zone(
    bounds: Bounds,
    x: i32,
    y: i32,
    edge_x: i32,
    edge_y: i32,
    maximized: bool,
) -> Option<i32> {
    if x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom {
        return Some(HTNOWHERE as i32);
    }
    if maximized {
        return None;
    }
    let left = x - bounds.left < edge_x;
    let right = bounds.right - x <= edge_x;
    let top = y - bounds.top < edge_y;
    let bottom = bounds.bottom - y <= edge_y;
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => HTTOPLEFT,
        (_, true, true, _) => HTTOPRIGHT,
        (true, _, _, true) => HTBOTTOMLEFT,
        (_, true, _, true) => HTBOTTOMRIGHT,
        (true, _, _, _) => HTLEFT,
        (_, true, _, _) => HTRIGHT,
        (_, _, true, _) => HTTOP,
        (_, _, _, true) => HTBOTTOM,
        _ => return None,
    } as i32)
}

pub(super) fn maximum(monitor: Bounds, work: Bounds) -> Option<(POINT, POINT)> {
    if work.width() <= 0 || work.height() <= 0 {
        return None;
    }
    Some((
        POINT {
            x: work.left - monitor.left,
            y: work.top - monitor.top,
        },
        POINT {
            x: work.width(),
            y: work.height(),
        },
    ))
}

unsafe fn monitor_info(monitor: HMONITOR) -> Option<MONITORINFO> {
    if monitor.is_null() {
        return None;
    }
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info)
}
pub(super) unsafe fn monitor(hwnd: HWND) -> Option<MONITORINFO> {
    monitor_info(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST))
}

/// A restore/DPI notification may arrive before the maximized style clears.
/// Do not clip a smaller normal proposal merely because that style is still set.
pub(super) fn maximized_client(proposed: Bounds, work: Bounds) -> Bounds {
    if proposed.width() < work.width() || proposed.height() < work.height() {
        return proposed;
    }
    let clipped = Bounds {
        left: proposed.left.max(work.left),
        top: proposed.top.max(work.top),
        right: proposed.right.min(work.right),
        bottom: proposed.bottom.min(work.bottom),
    };
    if clipped.width() > 0 && clipped.height() > 0 {
        clipped
    } else {
        proposed
    }
}

pub(super) unsafe fn limits(hwnd: HWND, info: &mut MINMAXINFO) {
    let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
    info.ptMinTrackSize = POINT {
        x: (1000.0 * scale).round() as i32,
        y: (650.0 * scale).round() as i32,
    };
    if let Some(monitor) = monitor(hwnd) {
        if let Some((position, size)) = maximum(monitor.rcMonitor.into(), monitor.rcWork.into()) {
            info.ptMaxPosition = position;
            info.ptMaxSize = size;
        }
    }
}

pub(super) unsafe fn calculate(hwnd: HWND, wparam: usize, lparam: isize) {
    if lparam == 0 || IsZoomed(hwnd) == 0 {
        return;
    }
    let rect = if wparam != 0 {
        &mut (*(lparam as *mut NCCALCSIZE_PARAMS)).rgrc[0]
    } else {
        &mut *(lparam as *mut RECT)
    };
    // Use the proposed rectangle's monitor while Windows moves a maximized
    // window between displays; the old HWND bounds may still name its origin.
    if let Some(monitor) = monitor_info(MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST)) {
        let clipped = maximized_client((*rect).into(), monitor.rcWork.into());
        *rect = RECT {
            left: clipped.left,
            top: clipped.top,
            right: clipped.right,
            bottom: clipped.bottom,
        };
    }
}

pub(super) unsafe fn border_hit(hwnd: HWND, lparam: isize) -> Option<LRESULT> {
    let x = (lparam as u32 & 0xffff) as i16 as i32;
    let y = ((lparam as u32 >> 16) & 0xffff) as i16 as i32;
    let mut bounds: RECT = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut bounds) == 0 {
        return None;
    }
    let dpi = GetDpiForWindow(hwnd).max(96);
    let padding = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    let edge_x = (GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + padding).max(1);
    let edge_y = (GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + padding).max(1);
    resize_zone(bounds.into(), x, y, edge_x, edge_y, IsZoomed(hwnd) != 0).map(|hit| hit as LRESULT)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximized_border_is_clamped_but_smaller_restore_proposals_keep_full_client() {
        let work = Bounds {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1380,
        };
        let frame = Bounds {
            left: -8,
            top: -8,
            right: 2568,
            bottom: 1388,
        };
        assert_eq!(maximized_client(frame, work), work);
        let restore = Bounds {
            left: -20,
            top: 80,
            right: 1580,
            bottom: 980,
        };
        assert_eq!(maximized_client(restore, work), restore);
        let disjoint = Bounds {
            left: 3000,
            top: 0,
            right: 5560,
            bottom: 1380,
        };
        assert_eq!(maximized_client(disjoint, work), disjoint);
    }
    #[test]
    fn all_eight_edges_use_physical_bounds_with_negative_monitor_origin() {
        let bounds = Bounds {
            left: -1600,
            top: 30,
            right: 0,
            bottom: 930,
        };
        for (x, y, expected) in [
            (-1599, 31, HTTOPLEFT),
            (-1, 31, HTTOPRIGHT),
            (-1599, 929, HTBOTTOMLEFT),
            (-1, 929, HTBOTTOMRIGHT),
            (-1599, 400, HTLEFT),
            (-1, 400, HTRIGHT),
            (-800, 31, HTTOP),
            (-800, 929, HTBOTTOM),
        ] {
            assert_eq!(
                resize_zone(bounds, x, y, 10, 10, false),
                Some(expected as i32)
            );
        }
        assert_eq!(resize_zone(bounds, -800, 400, 10, 10, false), None);
        assert_eq!(resize_zone(bounds, -1599, 31, 10, 10, true), None);
        assert_eq!(
            resize_zone(bounds, 0, 400, 10, 10, false),
            Some(HTNOWHERE as i32)
        );
    }
    #[test]
    fn maximize_position_is_relative_to_monitor_not_primary_screen() {
        let monitor = Bounds {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        let work = Bounds {
            left: -1880,
            top: 30,
            right: 0,
            bottom: 1040,
        };
        let (position, size) = maximum(monitor, work).unwrap();
        assert_eq!((position.x, position.y), (40, 30));
        assert_eq!((size.x, size.y), (1880, 1010));
    }
}
