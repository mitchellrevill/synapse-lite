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
    #[serde(default, rename="editorPreset")]
    editor_preset: bool,
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
    pub fn can_preview(&self) -> bool {
        self.devices.len() == 1 && self.notes.is_empty()
            && self.devices[0].placement.product==605 && self.devices[0].placement.edition==0
            && self.devices[0].placement.orientation==0
            && !self.devices[0].placement.editor_preset
    }
    pub fn load(raw: &str, assets: &Path) -> Self {
        let mut canvas = Self::default();
        let result = (|| -> Result<(), String> {
            let document: Document = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            if document.devices.len() > 32 {
                return Err("Canvas supports at most 32 placements".into());
            }
            let mut ids = BTreeSet::new();
            let mut cache: HashMap<(u16, u16, u16), Arc<SourceLayout>> = HashMap::new();
            let mut source_bytes=0u64;
            let mut owned_bytes=0usize;
            for raw in document.devices {
                let item = (|| -> Result<Device, String> {
                    let p: Placement = serde_json::from_str(raw.get()).map_err(|_| {
                        "Placement needs explicit product, editionId, layout and geometry"
                            .to_string()
                    })?;
                    let leds = secbird_core::studio::inventory::led_inventory(raw.get())
                        .map_err(|error| error.to_string())?;
                    if p.id.is_empty() || p.id.len() > 512 || !ids.insert(p.id.clone()) {
                        return Err("Invalid or duplicate canvas device identity".into());
                    }
                    if ![0, 90, 180, 270].contains(&p.orientation)
                        || ![p.x, p.y, p.width, p.height].iter().all(|n| n.is_finite())
                        || p.width <= 0.0
                        || p.height <= 0.0
                        || p.width > 10000.0
                        || p.height > 10000.0
                        || p.x.abs() > 10000.0
                        || p.y.abs() > 10000.0
                    {
                        return Err("Placement is outside the supported canvas bounds".into());
                    }
                    let key = (p.product, p.edition, p.layout);
                    let geometry = if let Some(g) = cache.get(&key) {
                        g.clone()
                    } else {
                        let filename=format!("studio-{}-{}-{}.json",p.product,p.edition,p.layout);
                        let pack=assets.join("studio-presets").join(&filename);
                        let path=if pack.exists(){pack}else{assets.join(filename)};
                        let bytes=std::fs::metadata(&path).map_err(|e|e.to_string())?.len();
                        if source_bytes.saturating_add(bytes)>4*1024*1024 {return Err("Canvas source geometry exceeds4MiB".into());}
                        let g = Arc::new(SourceLayout::load(&path)?);
                        let owned=g.storage_bytes();
                        if owned_bytes.saturating_add(owned)>8*1024*1024 {return Err("Canvas geometry capacity exceeds8MiB".into());}
                        source_bytes+=bytes;owned_bytes+=owned;
                        if (g.product_id, g.edition_id, g.layout_id) != key {
                            return Err("Studio geometry identity mismatch".into());
                        }
                        cache.insert(key, g.clone());
                        g
                    };
                    let admitted = leds
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
        if studio_view::wheel_pointer(state,message,lparam) {return true;}
        if state.restricted_actions && !matches!(message, WM_CANCELMODE | WM_CAPTURECHANGED) {
            return false;
        }
        let point = Point {
            x: f64::from((lparam as u32 & 0xffff) as i16) / f64::from(state.scale),
            y: f64::from(((lparam as u32 >> 16) & 0xffff) as i16) / f64::from(state.scale),
        };
        let width = f64::from(state.width) / f64::from(state.scale);
        let height = f64::from(state.height) / f64::from(state.scale);
        if studio_view::wheel_picking(state) && matches!(message,WM_LBUTTONDOWN|WM_LBUTTONUP) && point.x>250.0 && point.x<width-250.0 && point.y>=168.0 && point.y<height-50.0 {
            if message==WM_LBUTTONUP {
            if let Some(world)=state.studio.canvas.screen_to_world(point,width,height) {
                if (0.0..=10000.0).contains(&world.x) && (0.0..=10000.0).contains(&world.y) {
                    studio_view::wheel_center(state,world.x.round() as u16,world.y.round() as u16);
                    studio_view::selection_changed(state);state.rebuild_fields();InvalidateRect(state.hwnd,null(),0);
                }
            }
            }
            return true;
        }
        if studio_view::tidal_center_active(state) && point.x>250.0 && point.x<width-250.0 && point.y>=168.0 && point.y<height-50.0 {
            if message==WM_LBUTTONUP {
                if let Some(world)=state.studio.canvas.screen_to_world(point,width,height) {
                    studio_view::place_tidal_center(state,world.x,world.y);
                    state.rebuild_fields(); InvalidateRect(state.hwnd,null(),0);
                }
                return true;
            }
            if message==WM_LBUTTONDOWN {return true;}
        }
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
        if message == WM_LBUTTONUP {
            studio_view::selection_changed(state);
            state.rebuild_fields();
        }
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
    pub fn clear_selection(&mut self) {
        self.gesture = None;
        self.selected.clear();
    }
    pub fn count(&self) -> usize {
        self.selected.len()
    }
    #[cfg(test)]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }
    #[cfg(test)]
    pub fn selectable_count(&self) -> usize {
        self.devices
            .iter()
            .map(|device| {
                device
                    .geometry
                    .keys
                    .iter()
                    .filter(|key| device.allowed(&key.input_id))
                    .count()
            })
            .sum()
    }
    pub fn selection(&self) -> Vec<secbird_core::studio::document::LedSelection> {
        self.selected
            .iter()
            .filter_map(|(device, key)| {
                let (row, col) = key.split_once(':')?;
                Some(secbird_core::studio::document::LedSelection {
                    device: device.clone(),
                    row: row.parse().ok()?,
                    col: col.parse().ok()?,
                })
            })
            .collect()
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
            let fill = studio_view::preview_color(state, &device.placement.id, &key.input_id)
                .unwrap_or(0x111111);
            let brush = CreateSolidBrush(rgb(fill));
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
    if let Some(g)=studio_view::wheel_geometry(state) {
        let x=(screen.x+(f64::from(g.x)-center.x)*zoom) as i32;
        let y=(screen.y+(f64::from(g.y)-center.y)*zoom) as i32;
        if x>260 && x<width-260 && y>178 && y<height-60 {
            p.fill(x-9,y,19,1,GREEN);p.fill(x,y-9,1,19,GREEN);
        }
    }
    if let Some((x,y))=studio_view::tidal_center(state) {
        let x=(screen.x+(f64::from(x)-center.x)*zoom).round() as i32;
        let y=(screen.y+(f64::from(y)-center.y)*zoom).round() as i32;
        if x>260 && x<width-260 && y>178 && y<height-60 {p.fill(x-8,y-1,17,3,GREEN);p.fill(x-1,y-8,3,17,GREEN);}
    }

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
                    editor_preset: false,
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
    #[test]
    #[ignore="requires an explicitly selected private source preset directory"]
    fn all_recovered_studio_presets_parse_and_keep_exact_identity() {
        let directory=PathBuf::from(std::env::var_os("SYNAPSE_LITE_STUDIO_PRESETS").expect("select preset directory"));
        let index:Value=serde_json::from_slice(&std::fs::read(directory.join("index.json")).unwrap()).unwrap();
        let mut largest=0;let mut layouts=0;let mut products=BTreeSet::new();let mut failures=vec![];
        for record in index["records"].as_array().unwrap() {
            let filename=record["file"].as_str().unwrap();
            match SourceLayout::load(&directory.join(filename)) {
                Ok(geometry)=>{
                    assert_eq!((geometry.product_id,geometry.edition_id,geometry.layout_id),(record["product_id"].as_u64().unwrap() as u16,record["edition_id"].as_u64().unwrap() as u16,record["layout_id"].as_u64().unwrap() as u16));
                    assert_eq!(geometry.keys.len(),record["leds"].as_u64().unwrap() as usize);
                    assert!(!geometry.keys.is_empty());largest=largest.max(geometry.storage_bytes());layouts+=1;products.insert(geometry.product_id);
                },
                Err(error)=>failures.push(json!({"file":filename,"error":error})),
            }
        }
        let report=json!({"layouts":layouts,"products":products.len(),"largest_owned_geometry_capacity":largest,"failures":failures,"hardware_verified":false});
        if let Some(path)=std::env::var_os("SYNAPSE_LITE_STUDIO_PRESETS_REPORT"){std::fs::write(path,serde_json::to_vec_pretty(&report).unwrap()).unwrap();}
        assert!(failures.is_empty(),"{}",report);assert!(layouts>0);assert!(largest<8*1024*1024);
    }

    fn generic_fixture()->(PathBuf,serde_json::Value) {
        let folder=std::env::temp_dir().join(format!("generic-studio-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(folder.join("studio-presets")).unwrap();
        let raw=json!({"product_id":123,"edition_id":4,"layout_id":2,"source_sha256":"0".repeat(64),"view_box":[0,0,100,50],"keys":[{"buttonKey":"light","inputID":"0:1","isEnabled":true,"d":"M5 5H20V20H5Z"}]});
        std::fs::write(folder.join("studio-presets/studio-123-4-2.json"),serde_json::to_vec(&raw).unwrap()).unwrap();
        let placement=json!({"id":"foreign","productId":123,"editionId":4,"layout":2,"x":0,"y":0,"width":100,"height":50,"leds":[{"row":0,"col":1}]});
        (folder,placement)
    }
    #[test]
    fn generic_presets_select_exact_leds_and_refuse_mismatched_identity_without_output_admission() {
        let (folder,placement)=generic_fixture();let raw=json!({"devices":[placement.clone()]}).to_string();let mut canvas=Canvas::load(&raw,&folder);
        assert_eq!(canvas.devices.len(),1);assert!(canvas.notes.is_empty());assert!(!canvas.can_preview());
        assert_eq!(canvas.hit(Point{x:10.,y:10.}),Some(("foreign".into(),"0:1".into())));
        canvas.selected.insert(("foreign".into(),"0:1".into()));assert_eq!(canvas.selection(),vec![secbird_core::studio::document::LedSelection{device:"foreign".into(),row:0,col:1}]);
        let path=folder.join("studio-presets/studio-123-4-2.json");let mut value:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();value["product_id"]=json!(124);std::fs::write(&path,serde_json::to_vec(&value).unwrap()).unwrap();
        // A present wrong pack file cannot fall back to a different projection.
        value["product_id"]=json!(123);std::fs::write(folder.join("studio-123-4-2.json"),serde_json::to_vec(&value).unwrap()).unwrap();let canvas=Canvas::load(&raw,&folder);assert!(canvas.devices.is_empty());assert_eq!(canvas.notes,["Studio geometry identity mismatch"]);
    }
    #[test]
    fn repeated_generic_placements_share_geometry_and_keep_placement_limit() {
        let (folder,placement)=generic_fixture();let placements:Vec<_>=(0..32).map(|i|{let mut p=placement.clone();p["id"]=json!(format!("device-{i}"));p}).collect();
        let canvas=Canvas::load(&json!({"devices":placements}).to_string(),&folder);assert_eq!(canvas.devices.len(),32);assert!(canvas.notes.is_empty());
        for device in &canvas.devices {assert!(Arc::ptr_eq(&canvas.devices[0].geometry,&device.geometry));}
        let oversized=vec![placement;33];let refused=Canvas::load(&json!({"devices":oversized}).to_string(),&folder);assert!(refused.devices.is_empty());assert_eq!(refused.notes.len(),1);
    }

}
