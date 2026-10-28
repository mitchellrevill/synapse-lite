//! Transient canvas selection over explicitly identified, locally recovered SVG paths.
use super::*;
use serde::Deserialize;
use source_layout::{Point, SourceLayout, Viewport};
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Document {
    devices: Vec<Box<serde_json::value::RawValue>>,
}
#[derive(Deserialize)]
struct Placement {
    id: String,
    #[serde(rename = "productId")]
    product: u16,
    #[serde(rename = "editionId")]
    edition: u16,
    layout: u16,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    orientation: u16,
    #[serde(rename = "ledRef")]
    leds: Vec<Led>,
}
#[derive(Deserialize)]
struct Led {
    row: u16,
    col: u16,
}
struct Device {
    placement: Placement,
    geometry: Arc<SourceLayout>,
    admitted: BTreeSet<String>,
}
#[derive(Default)]
pub(super) struct Canvas {
    devices: Vec<Device>,
    pub notes: Vec<String>,
    selected: BTreeSet<(String, String)>,
    zoom: f64,
    gesture: Option<Gesture>,
}
struct Gesture {
    start: Point,
    end: Point,
    before: BTreeSet<(String, String)>,
    additive: bool,
}

impl Device {
    fn dimensions(&self) -> (f64, f64) {
        let p = &self.placement;
        if p.orientation == 90 || p.orientation == 270 {
            (p.height, p.width)
        } else {
            (p.width, p.height)
        }
    }
    fn local(&self, point: Point) -> Point {
        let p = &self.placement;
        let x = point.x - p.x;
        let y = point.y - p.y;
        let (x, y) = match p.orientation {
            90 => (y, p.width - x),
            180 => (p.width - x, p.height - y),
            270 => (p.height - y, x),
            _ => (x, y),
        };
        let (w, h) = self.dimensions();
        let b = self.geometry.view_box;
        Point {
            x: b[0] + x * b[2] / w,
            y: b[1] + y * b[3] / h,
        }
    }
    fn world(&self, point: Point) -> Point {
        let p = &self.placement;
        let b = self.geometry.view_box;
        let (w, h) = self.dimensions();
        let x = (point.x - b[0]) * w / b[2];
        let y = (point.y - b[1]) * h / b[3];
        let (x, y) = match p.orientation {
            90 => (p.width - y, x),
            180 => (p.width - x, p.height - y),
            270 => (y, p.height - x),
            _ => (x, y),
        };
        Point {
            x: p.x + x,
            y: p.y + y,
        }
    }
    fn allowed(&self, id: &str) -> bool {
        self.admitted.contains(id)
    }
    fn viewport(&self) -> Viewport {
        let b = self.geometry.view_box;
        Viewport {
            left: b[0],
            top: b[1],
            width: b[2],
            height: b[3],
        }
    }
}

impl Canvas {
    pub fn load(raw: &str, assets: &Path) -> Self {
        let mut canvas = Self::default();
        let result = (|| -> Result<(), String> {
            let document: Document = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            if document.devices.len() > 32 {
                return Err("Canvas supports at most 32 placements".into());
            }
            let mut ids = BTreeSet::new();
            let mut cache: HashMap<(u16, u16, u16), Arc<SourceLayout>> = HashMap::new();
            for raw in document.devices {
                let item = (|| -> Result<Device, String> {
                    let p:Placement=serde_json::from_str(raw.get()).map_err(|_|"Placement needs explicit product, editionId, layout, geometry and ledRef".to_string())?;
                    if p.id.is_empty() || p.id.len() > 512 || !ids.insert(p.id.clone()) {
                        return Err("Invalid or duplicate canvas device identity".into());
                    }
                    if p.product != 605 || p.edition != 0 {
                        return Err("No researched Studio geometry for this product/edition".into());
                    }
                    if ![0, 90, 180, 270].contains(&p.orientation)
                        || ![p.x, p.y, p.width, p.height].iter().all(|n| n.is_finite())
                        || p.width <= 0.0
                        || p.height <= 0.0
                        || p.width > 10000.0
                        || p.height > 10000.0
                        || p.x.abs() > 10000.0
                        || p.y.abs() > 10000.0
                        || p.leds.len() > 512
                    {
                        return Err("Placement is outside the supported canvas bounds".into());
                    }
                    let key = (p.product, p.edition, p.layout);
                    let geometry = if let Some(g) = cache.get(&key) {
                        g.clone()
                    } else {
                        let g = Arc::new(SourceLayout::load(&assets.join(format!(
                            "studio-{}-{}-{}.json",
                            p.product, p.edition, p.layout
                        )))?);
                        if (g.product_id, g.edition_id, g.layout_id) != key {
                            return Err("Studio geometry identity mismatch".into());
                        }
                        cache.insert(key, g.clone());
                        g
                    };
                    let admitted = p
                        .leds
                        .iter()
                        .map(|led| format!("{}:{}", led.row, led.col))
                        .collect();
                    Ok(Device {
                        placement: p,
                        geometry,
                        admitted,
                    })
                })();
                match item {
                    Ok(device) => canvas.devices.push(device),
                    Err(error) => canvas.notes.push(error),
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            canvas.notes.push(error);
        }
        canvas
    }
    fn frame(&self, width: f64, height: f64) -> Option<(Point, f64, Point)> {
        if self.devices.is_empty() || width <= 520.0 || height <= 210.0 {
            return None;
        }
        let left = self
            .devices
            .iter()
            .map(|d| d.placement.x)
            .fold(f64::INFINITY, f64::min);
        let top = self
            .devices
            .iter()
            .map(|d| d.placement.y)
            .fold(f64::INFINITY, f64::min);
        let right = self
            .devices
            .iter()
            .map(|d| d.placement.x + d.placement.width)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = self
            .devices
            .iter()
            .map(|d| d.placement.y + d.placement.height)
            .fold(f64::NEG_INFINITY, f64::max);
        let fit = ((width - 540.0) / (right - left))
            .min((height - 250.0) / (bottom - top))
            .clamp(0.05, 4.0);
        Some((
            Point {
                x: (left + right) / 2.0,
                y: (top + bottom) / 2.0,
            },
            if self.zoom == 0.0 { fit } else { self.zoom },
            Point {
                x: width / 2.0,
                y: (168.0 + height) / 2.0,
            },
        ))
    }
    fn screen_to_world(&self, point: Point, width: f64, height: f64) -> Option<Point> {
        let (center, zoom, screen) = self.frame(width, height)?;
        Some(Point {
            x: center.x + (point.x - screen.x) / zoom,
            y: center.y + (point.y - screen.y) / zoom,
        })
    }
    fn hit(&self, point: Point) -> Option<(String, String)> {
        for device in self.devices.iter().rev() {
            let local = device.local(point);
            if let Some(key) = device.geometry.hit_test(local, device.viewport()) {
                if device.allowed(&key.input_id) {
                    return Some((device.placement.id.clone(), key.input_id.clone()));
                }
            }
        }
        None
    }
    pub unsafe fn pointer(state: &mut State, message: u32, lparam: isize) -> bool {
        if !state.studio.open {
            return false;
        }
        if state.restricted_actions && !matches!(message, WM_CANCELMODE | WM_CAPTURECHANGED) {
            return false;
        }
        let point = Point {
            x: f64::from((lparam as u32 & 0xffff) as i16) / f64::from(state.scale),
            y: f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale),
        };
        let width = f64::from(state.width) / f64::from(state.scale);
        let height = f64::from(state.height) / f64::from(state.scale);
        let canvas = &mut state.studio.canvas;
        if message == WM_CANCELMODE || message == WM_CAPTURECHANGED {
            if let Some(gesture) = canvas.gesture.take() {
                canvas.selected = gesture.before;
                if message == WM_CANCELMODE {
                    ReleaseCapture();
                }
                InvalidateRect(state.hwnd, null(), 0);
                return true;
            }
            return false;
        }
        let inside = point.x > 250.0
            && point.x < width - 250.0
            && point.y >= 168.0
            && point.y < height - 50.0;
        match message {
            WM_LBUTTONDOWN if inside => {
                let Some(world) = canvas.screen_to_world(point, width, height) else {
                    return false;
                };
                canvas.gesture = Some(Gesture {
                    start: world,
                    end: world,
                    before: canvas.selected.clone(),
                    additive: GetKeyState(17) < 0,
                });
                SetCapture(state.hwnd);
            }
            WM_MOUSEMOVE | WM_LBUTTONUP if canvas.gesture.is_some() => {
                let world = canvas.screen_to_world(point, width, height).unwrap();
                canvas.gesture.as_mut().unwrap().end = world;
                if message == WM_LBUTTONUP {
                    let gesture = canvas.gesture.take().unwrap();
                    canvas.selected = if gesture.additive {
                        gesture.before
                    } else {
                        BTreeSet::new()
                    };
                    let distance = (gesture.start.x - world.x).hypot(gesture.start.y - world.y);
                    let zoom = canvas.frame(width, height).unwrap().1;
                    if distance * zoom < 4.0 {
                        if let Some(key) = canvas.hit(world) {
                            if !canvas.selected.remove(&key) {
                                canvas.selected.insert(key);
                            }
                        }
                    } else {
                        for device in &canvas.devices {
                            for key in &device.geometry.keys {
                                if !device.allowed(&key.input_id) {
                                    continue;
                                }
                                let Some(bounds) =
                                    device.geometry.bounds(&key.input_id, device.viewport())
                                else {
                                    continue;
                                };
                                let corners = [
                                    Point {
                                        x: bounds.left,
                                        y: bounds.top,
                                    },
                                    Point {
                                        x: bounds.right,
                                        y: bounds.top,
                                    },
                                    Point {
                                        x: bounds.right,
                                        y: bounds.bottom,
                                    },
                                    Point {
                                        x: bounds.left,
                                        y: bounds.bottom,
                                    },
                                ]
                                .map(|p| device.world(p));
                                let minx =
                                    corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
                                let maxx = corners
                                    .iter()
                                    .map(|p| p.x)
                                    .fold(f64::NEG_INFINITY, f64::max);
                                let miny =
                                    corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
                                let maxy = corners
                                    .iter()
                                    .map(|p| p.y)
                                    .fold(f64::NEG_INFINITY, f64::max);
                                if minx <= world.x.max(gesture.start.x)
                                    && maxx >= world.x.min(gesture.start.x)
                                    && miny <= world.y.max(gesture.start.y)
                                    && maxy >= world.y.min(gesture.start.y)
                                {
                                    canvas.selected.insert((
                                        device.placement.id.clone(),
                                        key.input_id.clone(),
                                    ));
                                }
                            }
                        }
                    }
                    ReleaseCapture();
                }
            }
            _ => return false,
        }
        InvalidateRect(state.hwnd, null(), 0);
        true
    }
    pub fn zoom(&mut self, steps: i32, width: f64, height: f64) {
        if steps == 0 {
            self.zoom = 0.0;
        } else if let Some((_, zoom, _)) = self.frame(width, height) {
            self.zoom = (zoom * 1.25f64.powi(steps.signum())).clamp(0.05, 4.0);
        }
        self.gesture = None;
    }
    pub fn cancel(&mut self) {
        if let Some(g) = self.gesture.take() {
            self.selected = g.before;
        }
    }
    pub fn count(&self) -> usize {
        self.selected.len()
    }
}

pub(super) unsafe fn paint(state: &State, p: &mut Painter, width: i32, height: i32) {
    let canvas = &state.studio.canvas;
    p.fill(251, 168, (width - 501).max(1), height - 168, 0x383838);
    let Some((center, zoom, screen)) = canvas.frame(f64::from(width), f64::from(height)) else {
        p.text(
            "No device geometry available for this composition",
            270,
            290,
            (width - 540).max(1),
            40,
            p.fonts.normal,
            MUTED,
            DT_LEFT,
        );
        return;
    };
    let saved = SaveDC(p.dc);
    IntersectClipRect(
        p.dc,
        p.px(251),
        p.px(168),
        p.px(width - 250),
        p.px(height - 50),
    );
    SetGraphicsMode(p.dc, GM_ADVANCED);
    for device in &canvas.devices {
        // Derive the affine transform from three exact source-space points.
        let b = device.geometry.view_box;
        let a = device.world(Point { x: 0.0, y: 0.0 });
        let x = device.world(Point { x: 1.0, y: 0.0 });
        let y = device.world(Point { x: 0.0, y: 1.0 });
        let scale = zoom * f64::from(state.scale);
        let matrix = XFORM {
            eM11: ((x.x - a.x) * scale) as f32,
            eM12: ((x.y - a.y) * scale) as f32,
            eM21: ((y.x - a.x) * scale) as f32,
            eM22: ((y.y - a.y) * scale) as f32,
            eDx: ((screen.x + (a.x - center.x) * zoom) * f64::from(state.scale)) as f32,
            eDy: ((screen.y + (a.y - center.y) * zoom) * f64::from(state.scale)) as f32,
        };
        SetWorldTransform(p.dc, &matrix);
        for key in &device.geometry.keys {
            let selected = canvas
                .selected
                .iter()
                .any(|(id, led)| id == &device.placement.id && led == &key.input_id);
            let color = if selected {
                GREEN
            } else if device.allowed(&key.input_id) {
                BORDER
            } else {
                0x444444
            };
            let pen = CreatePen(PS_SOLID, 2, rgb(color));
            let brush = CreateSolidBrush(rgb(0x111111));
            let old_pen = SelectObject(p.dc, pen);
            let old_brush = SelectObject(p.dc, brush);
            if device.geometry.trace_key_path(
                p.dc,
                &key.input_id,
                Viewport {
                    left: b[0],
                    top: b[1],
                    width: b[2],
                    height: b[3],
                },
            ) {
                StrokeAndFillPath(p.dc);
            }
            SelectObject(p.dc, old_pen);
            SelectObject(p.dc, old_brush);
            DeleteObject(pen);
            DeleteObject(brush);
        }
    }
    RestoreDC(p.dc, saved);
    if let Some(gesture) = &canvas.gesture {
        let a = Point {
            x: screen.x + (gesture.start.x - center.x) * zoom,
            y: screen.y + (gesture.start.y - center.y) * zoom,
        };
        let b = Point {
            x: screen.x + (gesture.end.x - center.x) * zoom,
            y: screen.y + (gesture.end.y - center.y) * zoom,
        };
        let clip = SaveDC(p.dc);
        IntersectClipRect(
            p.dc,
            p.px(251),
            p.px(168),
            p.px(width - 250),
            p.px(height - 50),
        );
        let pen = CreatePen(PS_DASH, 1, rgb(GREEN));
        let old = SelectObject(p.dc, pen);
        let brush = SelectObject(p.dc, GetStockObject(NULL_BRUSH));
        Rectangle(
            p.dc,
            (a.x.min(b.x) * f64::from(state.scale)) as i32,
            (a.y.min(b.y) * f64::from(state.scale)) as i32,
            (a.x.max(b.x) * f64::from(state.scale)) as i32,
            (a.y.max(b.y) * f64::from(state.scale)) as i32,
        );
        SelectObject(p.dc, brush);
        SelectObject(p.dc, old);
        DeleteObject(pen);
        RestoreDC(p.dc, clip);
    }
    p.text(
        &format!("{} LEDs selected · {:.0}%", canvas.count(), zoom * 100.0),
        270,
        height - 42,
        (width - 540).max(1),
        28,
        p.fonts.normal,
        TEXT,
        DT_LEFT,
    );
    if !canvas.notes.is_empty() {
        p.text(
            &format!("{} placement(s) unavailable", canvas.notes.len()),
            270,
            170,
            (width - 540).max(1),
            28,
            p.fonts.small,
            0xfd4949,
            DT_LEFT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_identity_never_selects_a_default_layout() {
        let canvas = Canvas::load(
            r#"{"devices":[{"id":"x","productId":605,"x":0,"y":0,"width":960,"height":360,"ledRef":[]}]}"#,
            Path::new("missing"),
        );
        assert!(canvas.devices.is_empty());
        assert_eq!(canvas.notes.len(), 1);
    }

    #[test]
    fn quarter_turns_round_trip_positions_and_hit_the_same_source_path() {
        let geometry=Arc::new(SourceLayout::from_slice(&serde_json::to_vec(&json!({
            "product_id":605,"edition_id":0,"layout_id":6,"source_sha256":"0".repeat(64),
            "view_box":[0,0,960,360],"keys":[{"buttonKey":"","inputID":"0:1","isEnabled":true,"d":"M50 60L84 60L84 94L50 94Z"}]
        })).unwrap()).unwrap());
        for orientation in [0, 90, 180, 270] {
            let (width, height) = if orientation == 90 || orientation == 270 {
                (180.0, 480.0)
            } else {
                (480.0, 180.0)
            };
            let device = Device {
                placement: Placement {
                    id: "keyboard".into(),
                    product: 605,
                    edition: 0,
                    layout: 6,
                    x: 4000.0,
                    y: 4500.0,
                    width,
                    height,
                    orientation,
                    leds: vec![Led { row: 0, col: 1 }],
                },
                geometry: geometry.clone(),
                admitted: BTreeSet::from(["0:1".into()]),
            };
            let source = Point { x: 67.0, y: 76.0 };
            let world = device.world(source);
            let local = device.local(world);
            assert!((source.x - local.x).abs() < 1e-8 && (source.y - local.y).abs() < 1e-8);
            let canvas = Canvas {
                devices: vec![device],
                ..Canvas::default()
            };
            assert_eq!(canvas.hit(world), Some(("keyboard".into(), "0:1".into())));
        }
    }

    #[test]
    #[ignore = "requires explicitly provisioned private Studio SVG projections"]
    fn all_exact_packaged_studio_layouts_parse_without_substitute_paths() {
        for layout in [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 15, 16, 17, 18, 20] {
            let path = assets_dir().join(format!("studio-605-0-{layout}.json"));
            let geometry = SourceLayout::load(&path).unwrap();
            assert_eq!(
                (geometry.product_id, geometry.edition_id, geometry.layout_id),
                (605, 0, layout)
            );
            assert!((108..=113).contains(&geometry.keys.len()));
            assert!(geometry.keys.iter().all(|key| geometry
                .bounds(
                    &key.input_id,
                    Viewport {
                        left: 0.0,
                        top: 0.0,
                        width: 960.0,
                        height: 360.0
                    }
                )
                .is_some()));
        }
    }
}
