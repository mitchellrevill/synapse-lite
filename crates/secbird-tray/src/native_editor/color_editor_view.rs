//! Source-specific color preview and draft handling. No OS eyedropper execution.
use super::{color_editor_layout as source, performance_layout::Rect, performance_view as draw, *};

const W: usize = 198;
const H: usize = 118;
const BW: usize = 130;
const BH: usize = 16;

pub(super) struct Editor {
    pub slot: usize,
    pub drafts: [String; 4],
    pub selected: [String; 3],
    pub cursor: source::Cursors,
    pub preview: source::Rgb,
    pub exact_assets: bool,
    plane: Vec<u8>,
    lut: Vec<u8>,
}

fn number_hex(value: f64) -> String {
    let mut result = format!("{:x}", value.trunc() as u32);
    let mut fraction = value.fract();
    if fraction != 0.0 {
        result.push('.');
        for _ in 0..32 {
            fraction *= 16.0;
            let digit = fraction.floor() as u8;
            result.push(char::from_digit(u32::from(digit), 16).unwrap_or('0'));
            fraction -= f64::from(digit);
            if fraction == 0.0 {
                break;
            }
        }
    }
    if result.len() < 2 {
        result.insert(0, '0')
    }
    result
}

impl Editor {
    pub fn new(slot: usize, color: Option<&str>, assets: &Path) -> Self {
        let text = color.unwrap_or("44d62c").trim_start_matches('#');
        let normalized = source::normalize_hex(text);
        let byte = |offset| u8::from_str_radix(&normalized[offset..offset + 2], 16).unwrap_or(0);
        let rgb = source::Rgb {
            r: byte(0),
            g: byte(2),
            b: byte(4),
        };
        let plane = std::fs::read(assets.join("lighting-hs-plane.rgb")).unwrap_or_default();
        let lut = std::fs::read(assets.join("lighting-value.lut")).unwrap_or_default();
        let exact_assets = plane.len() == W * H * 3 && lut.len() == 256 * BW * BH * 3;
        Self {
            slot,
            drafts: [
                normalized,
                rgb.r.to_string(),
                rgb.g.to_string(),
                rgb.b.to_string(),
            ],
            selected: [rgb.r.to_string(), rgb.g.to_string(), rgb.b.to_string()],
            cursor: source::cursors(rgb),
            preview: rgb,
            exact_assets,
            plane,
            lut,
        }
    }

    pub fn hex(&self) -> String {
        self.selected
            .iter()
            .map(|v| number_hex(source::numeric_rgb_text(v).unwrap_or(0.0)))
            .collect()
    }
    pub fn save_color(&self) -> Option<String> {
        (!self.selected.iter().any(String::is_empty)).then(|| format!("#{}", self.hex()))
    }

    pub fn change(&mut self, index: usize, text: &str) -> bool {
        if index > 3 {
            return false;
        }
        if index == 0 {
            if !source::accepts_hex_edit(text) {
                return false;
            }
            self.drafts[0] = text.to_string();
            let h = source::normalize_hex(text);
            for i in 0..3 {
                let v = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0);
                self.selected[i] = v.to_string();
                self.drafts[i + 1] = v.to_string();
            }
        } else {
            if !source::accepts_rgb_edit(text) {
                return false;
            }
            self.drafts[index] = text.to_string();
            for i in 0..3 {
                self.selected[i] = source::filter_rgb_text(&self.drafts[i + 1]);
            }
            self.drafts[0] = self.hex();
        }
        self.refresh_preview_and_cursor();
        true
    }

    pub fn blur(&mut self, index: usize) {
        if index == 0 {
            self.drafts[0] = self.hex()
        } else {
            for i in 0..3 {
                self.drafts[i + 1] = self.selected[i].clone()
            }
        }
    }

    fn refresh_preview_and_cursor(&mut self) {
        let values = self
            .selected
            .each_ref()
            .map(|s| source::numeric_rgb_text(s).unwrap_or(0.0));
        if !self.selected.iter().any(String::is_empty) {
            self.preview = source::Rgb {
                r: values[0].round() as u8,
                g: values[1].round() as u8,
                b: values[2].round() as u8,
            };
        }
        let [r, g, b] = values.map(|v| v / 255.0);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let (mut h, s) = (0.0, if delta == 0.0 { 0.0 } else { delta / max });
        if delta != 0.0 {
            let c = |v: f64| (max - v) / 6.0 / delta + 0.5;
            h = if r == max {
                c(b) - c(g)
            } else if g == max {
                1.0 / 3.0 + c(r) - c(b)
            } else {
                2.0 / 3.0 + c(g) - c(r)
            };
            if h < 0.0 {
                h += 1.0
            } else if h > 1.0 {
                h -= 1.0
            }
        }
        self.cursor = source::Cursors {
            plane_left: ((h * 360.0).round() / 360.0 * 198.0).floor() - 7.0,
            plane_top: 118.0 - ((s * 100.0).round() / 100.0 * 118.0).floor() - 7.0,
            value_left: ((max * 100.0).round() / 100.0 * 122.0).floor() + 34.0,
        };
    }

    fn plane_pixel(&self, x: i32, y: i32) -> source::Rgb {
        if !(0..W as i32).contains(&x) || !(0..H as i32).contains(&y) {
            return source::Rgb {
                r: 255,
                g: 255,
                b: 255,
            };
        }
        if self.plane.len() == W * H * 3 {
            let i = (y as usize * W + x as usize) * 3;
            let rgb = source::Rgb {
                r: self.plane[i],
                g: self.plane[i + 1],
                b: self.plane[i + 2],
            };
            if rgb == (source::Rgb { r: 0, g: 0, b: 0 }) {
                source::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }
            } else {
                rgb
            }
        } else {
            let hue = x as f64 / W as f64 * 6.0;
            let sector = hue.floor() as usize;
            let f = hue.fract();
            let base = match sector {
                0 => [1.0, f, 0.0],
                1 => [1.0 - f, 1.0, 0.0],
                2 => [0.0, 1.0, f],
                3 => [0.0, 1.0 - f, 1.0],
                4 => [f, 0.0, 1.0],
                _ => [1.0, 0.0, 1.0 - f],
            };
            let white = y as f64 / H as f64;
            let c = base.map(|v| ((v * (1.0 - white) + white) * 255.0).round() as u8);
            source::Rgb {
                r: c[0],
                g: c[1],
                b: c[2],
            }
        }
    }
    fn base(&self) -> source::Rgb {
        let x = self.cursor.plane_left.trunc() as i32;
        let x = if x + 7 >= 198 { x + 5 } else { x + 7 };
        self.plane_pixel(x, self.cursor.plane_top.trunc() as i32 + 7)
    }
    fn value_pixel(&self, base: source::Rgb, x: usize, y: usize) -> source::Rgb {
        let c = [base.r, base.g, base.b];
        let c = std::array::from_fn::<_, 3, _>(|channel| {
            if self.lut.len() == 256 * BW * BH * 3 {
                self.lut[((c[channel] as usize * BH + y) * BW + x) * 3 + channel]
            } else {
                (f64::from(c[channel]) * (x as f64 + 0.5) / BW as f64).round() as u8
            }
        });
        source::Rgb {
            r: c[0],
            g: c[1],
            b: c[2],
        }
    }
    fn picked(&mut self, rgb: source::Rgb) {
        self.preview = rgb;
        self.selected = [rgb.r.to_string(), rgb.g.to_string(), rgb.b.to_string()];
        self.drafts = [
            self.hex(),
            self.selected[0].clone(),
            self.selected[1].clone(),
            self.selected[2].clone(),
        ];
    }
    pub fn plane_pointer(&mut self, x: f64, y: f64) {
        if let Some((x, y)) = source::plane_event_point(x, y) {
            self.cursor.plane_left = x - 7.0;
            self.cursor.plane_top = y - 7.0;
            let sample = (self.cursor.value_left.trunc() + 4.0 - 34.0).clamp(0.0, 129.0) as usize;
            self.picked(self.value_pixel(self.base(), sample, 1));
        }
    }
    pub fn value_pointer(&mut self, x: f64) {
        if let Some((sample, left)) = source::value_event_sample(x, 130.0) {
            self.cursor.value_left = left;
            self.picked(self.value_pixel(self.base(), sample as usize, 1));
        }
    }
    pub fn eyedropper(&mut self, color: u32) {
        self.picked(source::Rgb {
            r: (color >> 16) as u8,
            g: (color >> 8) as u8,
            b: color as u8,
        });
        self.cursor = source::cursors(self.preview);
    }
}

fn r(value: source::Rect) -> Rect {
    Rect {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}
fn at(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn packed(c: source::Rgb) -> u32 {
    u32::from(c.r) << 16 | u32::from(c.g) << 8 | u32::from(c.b)
}

unsafe fn pixels(p: &Painter<'_>, target: Rect, width: i32, height: i32, values: &[u32]) {
    let mut info: BITMAPINFO = std::mem::zeroed();
    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let px = |v: f64| (v * f64::from(p.scale)).round() as i32;
    StretchDIBits(
        p.dc,
        px(target.x),
        px(target.y),
        px(target.width),
        px(target.height),
        0,
        0,
        width,
        height,
        values.as_ptr().cast(),
        &info,
        DIB_RGB_COLORS,
        SRCCOPY,
    );
}

unsafe fn outline(p: &Painter<'_>, target: Rect, color: u32, width: i32, ellipse: bool) {
    let pen = CreatePen(
        PS_SOLID,
        ((width as f32 * p.scale).round() as i32).max(1),
        rgb(color),
    );
    let oldp = SelectObject(p.dc, pen);
    let oldb = SelectObject(p.dc, GetStockObject(NULL_BRUSH));
    let px = |v: f64| (v * f64::from(p.scale)).round() as i32;
    if ellipse {
        Ellipse(
            p.dc,
            px(target.x),
            px(target.y),
            px(target.x + target.width),
            px(target.y + target.height),
        );
    } else {
        Rectangle(
            p.dc,
            px(target.x),
            px(target.y),
            px(target.x + target.width),
            px(target.y + target.height),
        );
    }
    SelectObject(p.dc, oldp);
    SelectObject(p.dc, oldb);
    DeleteObject(pen);
}

pub(super) unsafe fn paint(
    state: &State,
    p: &mut Painter<'_>,
    editor: &Editor,
    g: &source::Geometry,
) {
    draw::rounded(p, r(g.editor), PANEL, BORDER, 0.0);
    draw::rounded(p, r(g.plane_wrapper), PANEL, BORDER, 0.0);
    let plane: Vec<u32> = (0..H)
        .flat_map(|y| (0..W).map(move |x| packed(editor.plane_pixel(x as i32, y as i32))))
        .collect();
    pixels(p, r(g.plane_canvas), W as i32, H as i32, &plane);
    let saved = SaveDC(p.dc);
    let px = |v: f64| (v * f64::from(p.scale)).round() as i32;
    IntersectClipRect(
        p.dc,
        px(g.plane_canvas.x),
        px(g.plane_canvas.y),
        px(g.plane_canvas.x + 198.0),
        px(g.plane_canvas.y + 118.0),
    );
    outline(
        p,
        at(
            g.plane_canvas.x + editor.cursor.plane_left,
            g.plane_canvas.y + editor.cursor.plane_top,
            14.0,
            14.0,
        ),
        0xffffff,
        2,
        true,
    );
    RestoreDC(p.dc, saved);
    draw::rounded(p, r(g.preview), packed(editor.preview), 0xffffff, 0.0);
    let base = editor.base();
    let bar: Vec<u32> = (0..BH)
        .flat_map(|y| (0..BW).map(move |x| packed(editor.value_pixel(base, x, y))))
        .collect();
    pixels(p, r(g.value_bar), BW as i32, BH as i32, &bar);
    outline(p, r(g.value_bar), PANEL, 1, false);
    let marker_x = g.editor.x + 10.0 + editor.cursor.value_left;
    let marker_y = g.editor.y + 140.0;
    outline(
        p,
        at(marker_x, marker_y + 4.0, 8.0, 16.0),
        0xffffff,
        1,
        false,
    );
    let points = [
        POINT {
            x: px(marker_x),
            y: px(marker_y),
        },
        POINT {
            x: px(marker_x + 8.0),
            y: px(marker_y),
        },
        POINT {
            x: px(marker_x + 4.0),
            y: px(marker_y + 4.0),
        },
    ];
    let brush = CreateSolidBrush(rgb(0xffffff));
    let old = SelectObject(p.dc, brush);
    let pen = CreatePen(PS_SOLID, 1, rgb(0xffffff));
    let oldp = SelectObject(p.dc, pen);
    Polygon(p.dc, points.as_ptr(), 3);
    SelectObject(p.dc, old);
    SelectObject(p.dc, oldp);
    DeleteObject(pen);
    DeleteObject(brush);
    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    ScreenToClient(state.hwnd, &mut cursor);
    let hover = cursor.x >= px(g.eyedropper.x)
        && cursor.x < px(g.eyedropper.x + 24.0)
        && cursor.y >= px(g.eyedropper.y)
        && cursor.y < px(g.eyedropper.y + 24.0);
    p.image(
        state.icons.get(if hover {
            "lighting-eyedropper-hover"
        } else {
            "lighting-eyedropper"
        }),
        g.eyedropper.x.round() as i32 + 4,
        g.eyedropper.y.round() as i32 + 4,
        16,
        16,
    );
    for (input, label) in
        std::iter::once((g.hex_input, "HEX")).chain(g.rgb_inputs.into_iter().zip(["R", "G", "B"]))
    {
        draw::rounded(p, r(input), PANEL, BORDER, 0.0);
        let i = match label {
            "HEX" => 0,
            "R" => 1,
            "G" => 2,
            _ => 3,
        };
        draw::text(
            p,
            label,
            r(g.input_labels[i]),
            p.fonts.normal,
            TEXT,
            DT_CENTER,
        );
    }
    draw::rounded(p, r(g.cancel), 0x707070, 0x707070, 3.0);
    draw::text(
        p,
        "CANCEL",
        r(g.cancel),
        p.fonts.normal,
        0xffffff,
        DT_CENTER,
    );
    draw::rounded(p, r(g.save), GREEN, GREEN, 3.0);
    draw::text(p, "SAVE", r(g.save), p.fonts.normal, 0, DT_CENTER);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_short_hex_and_empty_save_guard() {
        let mut e = Editor::new(0, None, Path::new("missing"));
        assert!(e.change(0, "abc"));
        assert_eq!(e.save_color().as_deref(), Some("#abcabc"));
        assert!(e.change(1, ""));
        assert!(e.save_color().is_none());
        e.blur(1);
        assert_eq!(e.drafts[1], "");
    }
    #[test]
    fn preview_updates_without_slot_commit() {
        let mut e = Editor::new(4, None, Path::new("missing"));
        assert!(e.change(1, "999"));
        assert_eq!(e.preview.r, 0);
        assert_eq!(e.slot, 4);
        e.eyedropper(0x123456);
        assert_eq!(e.save_color().as_deref(), Some("#123456"));
    }
}
