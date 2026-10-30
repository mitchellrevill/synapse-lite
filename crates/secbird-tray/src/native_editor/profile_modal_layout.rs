//! Source ImportExportModal geometry; product-local top104 plus separate host42.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
#[derive(Clone, Debug)]
pub(super) struct Geometry {
    pub modal: Rect,
    pub header: Rect,
    pub close: Rect,
    pub local: Rect,
    pub cloud: Rect,
    pub browse: Rect,
    pub all: Rect,
    pub content: Rect,
    pub footer: Rect,
    pub cancel: Rect,
    pub commit: Rect,
    pub warning: Rect,
}
pub(super) fn geometry(width: f64, import: bool, native: bool, warning: bool) -> Geometry {
    let x = width / 2.0 - 300.0;
    let y = 146.0;
    let footer_y = y + if import { 420.0 } else { 394.0 };
    let cancel_w = 95.28125;
    let commit_w = if native {
        180.0
    } else if import {
        93.640625
    } else {
        94.25
    };
    let group = cancel_w + 10.0 + commit_w + if warning { 10.0 } else { 0.0 };
    let button_x = x + 1.0 + (600.0 - group) / 2.0;
    Geometry {
        modal: rect(x, y, 602.0, 481.0),
        header: rect(x + 1.0, y + 1.0, 600.0, 36.0),
        close: rect(x + 576.0, y + 8.5, 20.0, 20.0),
        local: rect(x + 243.515625, y + 53.0, 53.859375, 24.0),
        cloud: rect(x + 302.375, y + 53.0, 56.109375, 24.0),
        browse: rect(x + 261.984375, y + 96.0, 250.0, 27.0),
        all: rect(x + 1.0, y + if import { 133.0 } else { 37.0 }, 598.0, 40.0),
        content: rect(
            x + 1.0,
            y + if import { 173.0 } else { 77.0 },
            598.0,
            if import { 247.0 } else { 317.0 },
        ),
        footer: rect(x + 1.0, footer_y, 600.0, if import { 60.0 } else { 86.0 }),
        cancel: rect(button_x, y + 437.0, cancel_w, 27.0),
        commit: rect(button_x + cancel_w + 10.0, y + 437.0, commit_w, 27.0),
        warning: rect(
            button_x + cancel_w + 10.0 + commit_w + 10.0,
            y + 437.0,
            20.0,
            27.0,
        ),
    }
}
#[derive(Clone, Debug)]
pub(super) struct Row {
    pub index: usize,
    pub bounds: Rect,
    pub check: Rect,
    pub macros: Vec<Rect>,
}
pub(super) fn rows(g: &Geometry, expanded: &[(bool, usize)], scroll: f64) -> (Vec<Row>, f64) {
    let mut y = g.content.y + 23.0 - scroll;
    let mut out = Vec::new();
    for (index, (selected, count)) in expanded.iter().enumerate() {
        let n = if *selected { *count } else { 0 };
        let height = if n > 0 { 25.0 + 31.0 * n as f64 } else { 20.0 };
        out.push(Row {
            index,
            bounds: rect(g.content.x + 20.0, y, 551.0, height),
            check: rect(g.content.x + 20.0, y, 20.0, 20.0),
            macros: (0..n)
                .map(|i| rect(g.content.x + 95.0, y + 30.5 + i as f64 * 31.0, 20.0, 20.0))
                .collect(),
        });
        y += height + 10.0;
    }
    let extent = (y + scroll - g.content.y).max(g.content.height);
    (out, (extent - g.content.height).max(0.0))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_fixed_modal_and_row_geometry() {
        let g = geometry(1280.0, false, false, false);
        assert_eq!(g.modal, rect(340.0, 146.0, 602.0, 481.0));
        assert_eq!(g.content, rect(341.0, 223.0, 598.0, 317.0));
        let (r, _) = rows(&g, &[(true, 2), (false, 1)], 0.0);
        assert_eq!(r[0].check, rect(361.0, 246.0, 20.0, 20.0));
        assert_eq!(r[0].macros[0], rect(436.0, 276.5, 20.0, 20.0));
        assert_eq!(r[1].check.y, 343.0);
        let i = geometry(1000.0, true, false, false);
        assert_eq!(i.modal.x, 200.0);
        assert_eq!(i.content.y, 319.0);
        assert_eq!(i.footer.y, 566.0);
    }
}

/// Intrinsic English source Roboto16 layout measured in inert CSS replay. The
/// border and 20px padding are included; source icon is40px plus10px margin.
pub(super) fn cloud_message(g: &Geometry) -> Rect {
    rect(g.modal.x + 124.515625, g.modal.y + 236.5, 352.953125, 80.0)
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Scrollbar {
    pub track: Rect,
    pub thumb: Rect,
    pub maximum: f64,
}
pub(super) fn scrollbar(
    g: &Geometry,
    expanded: &[(bool, usize)],
    scroll: f64,
) -> Option<Scrollbar> {
    let (_, maximum) = rows(g, expanded, 0.0);
    if maximum <= 0.0 {
        return None;
    }
    let track = rect(
        g.content.x + g.content.width - 8.0,
        g.content.y,
        8.0,
        g.content.height,
    );
    // Source custom thumb height6px supplies its minimum. CSS border1px is
    // transparent and is painted separately inside this border-box geometry.
    let height = (track.height * track.height / (track.height + maximum))
        .round()
        .max(6.0)
        .min(track.height);
    let position = scroll.clamp(0.0, maximum) / maximum * (track.height - height);
    let position = if position > 0.0 && position < 1.0 {
        1.0
    } else {
        position.floor()
    };
    let thumb = rect(track.x, track.y + position, 8.0, height);
    Some(Scrollbar {
        track,
        thumb,
        maximum,
    })
}
impl Scrollbar {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.track.x
            && x < self.track.x + self.track.width
            && y >= self.track.y
            && y < self.track.y + self.track.height
    }
    pub fn thumb_contains(&self, y: f64) -> bool {
        y >= self.thumb.y && y < self.thumb.y + self.thumb.height
    }
    pub fn page(&self, scroll: f64, y: f64) -> f64 {
        let step = (self.track.height * 0.875).floor().max(1.0);
        (scroll + if y < self.thumb.y { -step } else { step }).clamp(0.0, self.maximum)
    }
    pub fn drag(&self, y: f64, grab: f64) -> f64 {
        ((y - self.track.y - grab) / (self.track.height - self.thumb.height) * self.maximum)
            .clamp(0.0, self.maximum)
    }
}
#[cfg(test)]
mod detail_tests {
    use super::*;
    #[test]
    fn long_source_scroll_extent_and_drag_endpoints() {
        let g = geometry(1280.0, true, false, false);
        let expanded = vec![(true, 2); 20];
        let bar = scrollbar(&g, &expanded, 0.0).unwrap();
        assert_eq!(bar.maximum, 1963.0 - 247.0);
        assert_eq!(bar.track.width, 8.0);
        assert_eq!(bar.drag(bar.track.y + 4.0, 4.0), 0.0);
        assert_eq!(
            bar.drag(bar.track.y + bar.track.height - bar.thumb.height + 4.0, 4.0),
            bar.maximum
        );
        assert!(bar.thumb_contains(bar.track.y + 4.0));
    }
    #[test]
    fn measured_intrinsic_cloud_center() {
        let g = geometry(1280.0, true, false, false);
        let r = cloud_message(&g);
        assert_eq!(r.x, 464.515625);
        assert_eq!(r.y, 382.5);
        assert!((r.x + r.width / 2.0 - (g.modal.x + 301.0)).abs() <= 1.0 / 64.0);
    }
}
