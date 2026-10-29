//! Runtime source geometry, without an approximate keyboard model.
//!
//! The local provisioner supplies literal paths/IDs recovered from the installed
//! Synapse product module. No vendor program or path table is embedded here.
//! Customize uses a different coordinate system from Chroma Studio: for the
//! recovered Ornata layout it is 730x387, not the Chroma SVG's 960x360.

use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Bounds {
    fn point(point: Point) -> Self {
        Self {
            left: point.x,
            top: point.y,
            right: point.x,
            bottom: point.y,
        }
    }

    fn include(&mut self, point: Point) {
        self.left = self.left.min(point.x);
        self.top = self.top.min(point.y);
        self.right = self.right.max(point.x);
        self.bottom = self.bottom.max(point.y);
    }

    fn contains(&self, point: Point) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    scale: f64,
    left: f64,
    top: f64,
}

impl Transform {
    /// SVG's default preserveAspectRatio="xMidYMid meet".
    pub fn meet(view_box: [f64; 4], viewport: Viewport) -> Option<Self> {
        let [x, y, width, height] = view_box;
        if ![
            x,
            y,
            width,
            height,
            viewport.left,
            viewport.top,
            viewport.width,
            viewport.height,
        ]
        .iter()
        .all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return None;
        }
        let scale = (viewport.width / width).min(viewport.height / height);
        Some(Self {
            scale,
            left: viewport.left + (viewport.width - width * scale) / 2.0 - x * scale,
            top: viewport.top + (viewport.height - height * scale) / 2.0 - y * scale,
        })
    }

    pub fn to_screen(&self, point: Point) -> Point {
        Point {
            x: self.left + point.x * self.scale,
            y: self.top + point.y * self.scale,
        }
    }

    pub fn to_source(&self, point: Point) -> Point {
        Point {
            x: (point.x - self.left) / self.scale,
            y: (point.y - self.top) / self.scale,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Segment {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    Close,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(tag="kind",rename_all="kebab-case",deny_unknown_fields)]
enum Primitive {
    Ellipse {cx:f64,cy:f64,rx:f64,ry:f64},
    RoundedRect {x:f64,y:f64,width:f64,height:f64,rx:f64,ry:f64},
}
impl Primitive {
    fn geometry(mut self)->Result<Geometry,String> {
        let bounds=match &mut self {
            Self::Ellipse{cx,cy,rx,ry}=>{
                if ![*cx,*cy,*rx,*ry].iter().all(|x|x.is_finite())||*rx<=0.||*ry<=0. {return Err("Invalid source ellipse".into());}
                Bounds{left:*cx-*rx,top:*cy-*ry,right:*cx+*rx,bottom:*cy+*ry}
            },
            Self::RoundedRect{x,y,width,height,rx,ry}=>{
                if ![*x,*y,*width,*height,*rx,*ry].iter().all(|x|x.is_finite())||*width<=0.||*height<=0.||*rx<0.||*ry<0. {return Err("Invalid source rounded rectangle".into());}
                *rx=rx.min(*width/2.);*ry=ry.min(*height/2.);
                Bounds{left:*x,top:*y,right:*x+*width,bottom:*y+*height}
            },
        };
        if ![bounds.left,bounds.top,bounds.right,bounds.bottom].iter().all(|v|v.is_finite()&&v.abs()<=1_000_000.) {return Err("Source shape exceeds coordinate bounds".into());}
        Ok(Geometry{segments:vec![],contours:vec![],bounds,primitive:Some(self)})
    }
    fn contains(self,p:Point)->bool {
        match self {
            Self::Ellipse{cx,cy,rx,ry}=>((p.x-cx)/rx).powi(2)+((p.y-cy)/ry).powi(2)<=1.,
            Self::RoundedRect{x,y,width,height,rx,ry}=>{
                if rx==0.||ry==0. {return true;}
                let cx=p.x.clamp(x+rx,x+width-rx);let cy=p.y.clamp(y+ry,y+height-ry);
                ((p.x-cx)/rx).powi(2)+((p.y-cy)/ry).powi(2)<=1.
            },
        }
    }
}
#[derive(Debug)]
struct Geometry {
    primitive:Option<Primitive>,
    segments: Vec<Segment>,
    bounds: Bounds,
    contours: Vec<Vec<Point>>,
}

impl Geometry {
    fn parse(source: &str) -> Result<Self, String> {
        if source.len() > 16_384 {
            return Err("A source key path exceeds the supported size".into());
        }
        let mut tokens = Tokens::new(source);
        let mut command = None;
        let mut current = Point { x: 0.0, y: 0.0 };
        let mut start = current;
        let mut segments = Vec::new();
        while !tokens.finished() {
            if let Some(letter) = tokens.command() {
                command = Some(letter);
            }
            let letter = command.ok_or("SVG path starts without a command")?;
            let relative = letter.is_ascii_lowercase();
            let upper = letter.to_ascii_uppercase();
            if upper == 'Z' {
                segments.push(Segment::Close);
                current = start;
                command = None;
                continue;
            }
            let coordinate = |tokens: &mut Tokens<'_>| -> Result<Point, String> {
                let x = tokens.number()?;
                let y = tokens.number()?;
                Ok(if relative {
                    Point {
                        x: current.x + x,
                        y: current.y + y,
                    }
                } else {
                    Point { x, y }
                })
            };
            match upper {
                'M' => {
                    current = coordinate(&mut tokens)?;
                    start = current;
                    segments.push(Segment::Move(current));
                    command = Some(if relative { 'l' } else { 'L' });
                }
                'L' => {
                    current = coordinate(&mut tokens)?;
                    segments.push(Segment::Line(current));
                }
                'H' => {
                    let x = tokens.number()?;
                    current.x = if relative { current.x + x } else { x };
                    segments.push(Segment::Line(current));
                }
                'V' => {
                    let y = tokens.number()?;
                    current.y = if relative { current.y + y } else { y };
                    segments.push(Segment::Line(current));
                }
                'C' => {
                    let first = coordinate(&mut tokens)?;
                    let second = coordinate(&mut tokens)?;
                    let end = coordinate(&mut tokens)?;
                    segments.push(Segment::Cubic(first, second, end));
                    current = end;
                }
                _ => {
                    return Err(format!(
                        "Unrecovered SVG command '{letter}'; refusing to substitute a shape"
                    ))
                }
            }
            if segments.len() > 4096 {
                return Err("Too many segments in one source key".into());
            }
        }
        if !matches!(segments.first(), Some(Segment::Move(_))) {
            return Err("Source geometry must start with a move".into());
        }
        let mut bounds = Bounds::point(Point { x: 0.0, y: 0.0 });
        let mut initialized = false;
        let mut contours = Vec::new();
        let mut contour = Vec::new();
        let mut previous = Point { x: 0.0, y: 0.0 };
        for &segment in &segments {
            match segment {
                Segment::Move(point) => {
                    if !contour.is_empty() {
                        contours.push(std::mem::take(&mut contour));
                    }
                    if !initialized {
                        bounds = Bounds::point(point);
                        initialized = true;
                    }
                    bounds.include(point);
                    contour.push(point);
                    previous = point;
                }
                Segment::Line(point) => {
                    bounds.include(point);
                    contour.push(point);
                    previous = point;
                }
                Segment::Cubic(first, second, end) => {
                    include_cubic_extrema(&mut bounds, previous, first, second, end);
                    flatten_cubic(&mut contour, previous, first, second, end, 0);
                    previous = end;
                }
                Segment::Close => {
                    if let Some(&first) = contour.first() {
                        contour.push(first);
                        previous = first;
                    }
                }
            }
        }
        if !contour.is_empty() {
            contours.push(contour);
        }
        Ok(Self {
            primitive:None,
            segments,
            bounds,
            contours,
        })
    }

    fn contains(&self, point: Point) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        if let Some(primitive)=self.primitive {return primitive.contains(point);}
        let mut winding = 0i32;
        for contour in &self.contours {
            if contour.len() < 3 {
                continue;
            }
            for index in 0..contour.len() {
                let a = contour[index];
                let b = contour[(index + 1) % contour.len()];
                let cross = (b.x - a.x) * (point.y - a.y) - (point.x - a.x) * (b.y - a.y);
                if cross.abs() < 0.0001
                    && point.x >= a.x.min(b.x)
                    && point.x <= a.x.max(b.x)
                    && point.y >= a.y.min(b.y)
                    && point.y <= a.y.max(b.y)
                {
                    return true;
                }
                if a.y <= point.y && b.y > point.y && cross > 0.0 {
                    winding += 1;
                }
                if a.y > point.y && b.y <= point.y && cross < 0.0 {
                    winding -= 1;
                }
            }
        }
        winding != 0
    }
}

fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
}

fn include_cubic_extrema(bounds: &mut Bounds, a: Point, b: Point, c: Point, d: Point) {
    bounds.include(d);
    for [p0, p1, p2, p3] in [[a.x, b.x, c.x, d.x], [a.y, b.y, c.y, d.y]] {
        let quadratic = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
        let linear = 2.0 * (p0 - 2.0 * p1 + p2);
        let constant = p1 - p0;
        let mut roots = Vec::new();
        if quadratic.abs() < 1e-12 {
            if linear.abs() > 1e-12 {
                roots.push(-constant / linear);
            }
        } else {
            let discriminant = linear * linear - 4.0 * quadratic * constant;
            if discriminant >= 0.0 {
                roots.push((-linear + discriminant.sqrt()) / (2.0 * quadratic));
                roots.push((-linear - discriminant.sqrt()) / (2.0 * quadratic));
            }
        }
        for t in roots.into_iter().filter(|t| *t > 0.0 && *t < 1.0) {
            bounds.include(Point {
                x: cubic(a.x, b.x, c.x, d.x, t),
                y: cubic(a.y, b.y, c.y, d.y, t),
            });
        }
    }
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
}

fn flatten_cubic(output: &mut Vec<Point>, a: Point, b: Point, c: Point, d: Point, depth: u8) {
    let chord_x = d.x - a.x;
    let chord_y = d.y - a.y;
    let length = (chord_x * chord_x + chord_y * chord_y).sqrt().max(1e-12);
    let distance = |p: Point| ((p.x - a.x) * chord_y - (p.y - a.y) * chord_x).abs() / length;
    // Hit testing alone is flattened, to 0.02 source pixels. GDI drawing below
    // preserves the original cubic control points without a polygon substitute.
    if depth >= 12 || distance(b).max(distance(c)) <= 0.02 {
        output.push(d);
        return;
    }
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let middle = midpoint(abc, bcd);
    flatten_cubic(output, a, ab, abc, middle, depth + 1);
    flatten_cubic(output, middle, bcd, cd, d, depth + 1);
}

struct Tokens<'a> {
    source: &'a [u8],
    index: usize,
}

impl<'a> Tokens<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            index: 0,
        }
    }
    fn skip(&mut self) {
        while self
            .source
            .get(self.index)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b',')
        {
            self.index += 1;
        }
    }
    fn finished(&mut self) -> bool {
        self.skip();
        self.index == self.source.len()
    }
    fn command(&mut self) -> Option<char> {
        self.skip();
        let value = *self.source.get(self.index)?;
        if value.is_ascii_alphabetic() {
            self.index += 1;
            Some(value as char)
        } else {
            None
        }
    }
    fn number(&mut self) -> Result<f64, String> {
        self.skip();
        let start = self.index;
        if self
            .source
            .get(self.index)
            .is_some_and(|b| *b == b'-' || *b == b'+')
        {
            self.index += 1;
        }
        let mut digits = 0;
        while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
            self.index += 1;
            digits += 1;
        }
        if self.source.get(self.index) == Some(&b'.') {
            self.index += 1;
            while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
                self.index += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(format!("Expected a source path number at byte {start}"));
        }
        if self
            .source
            .get(self.index)
            .is_some_and(|b| *b == b'e' || *b == b'E')
        {
            self.index += 1;
            if self
                .source
                .get(self.index)
                .is_some_and(|b| *b == b'-' || *b == b'+')
            {
                self.index += 1;
            }
            let exponent = self.index;
            while self.source.get(self.index).is_some_and(u8::is_ascii_digit) {
                self.index += 1;
            }
            if self.index == exponent {
                return Err("Invalid source path exponent".into());
            }
        }
        let value = std::str::from_utf8(&self.source[start..self.index])
            .map_err(|e| e.to_string())?
            .parse::<f64>()
            .map_err(|e| e.to_string())?;
        if !value.is_finite() || value.abs() > 1_000_000.0 {
            return Err("Source path coordinate is outside supported bounds".into());
        }
        Ok(value)
    }
}

#[derive(Deserialize)]
struct RawLayout {
    product_id: u16,
    edition_id: u16,
    layout_id: u16,
    view_box: [f64; 4],
    source_sha256: String,
    keys: Vec<RawKey>,
}

#[derive(Deserialize)]
struct RawKey {
    #[serde(rename = "buttonKey")]
    button_key: String,
    #[serde(default, rename = "inputType")]
    input_type: String,
    #[serde(default, rename = "isSidePanelList")]
    side_panel: Option<bool>,
    #[serde(default, rename = "isKeyToggle")]
    key_toggle: Option<bool>,
    #[serde(rename = "inputID")]
    input_id: String,
    #[serde(default)]
    counter: String,
    #[serde(default)]
    firmware_id: Option<u16>,
    #[serde(default, rename = "functionList")]
    function_list: Vec<String>,
    #[serde(default)]
    d: Option<String>,
    #[serde(default)]
    shape:Option<Primitive>,
    #[serde(default, rename = "isEnabled")]
    enabled: bool,
}

#[derive(Debug)]
pub struct KeyRegion {
    pub button_key: String,
    pub input_type: String,
    pub side_panel: Option<bool>,
    pub key_toggle: Option<bool>,
    pub input_id: String,
    pub label: String,
    pub firmware_id: Option<u16>,
    pub function_list: Vec<String>,
    pub enabled: bool,
    geometry: Option<Geometry>,
}

#[derive(Debug)]
pub struct SourceLayout {
    pub product_id: u16,
    pub edition_id: u16,
    pub layout_id: u16,
    pub view_box: [f64; 4],
    pub source_sha256: String,
    pub keys: Vec<KeyRegion>,
}

impl SourceLayout {
    /// Owned vector/string capacity estimate, excluding allocator/GDI overhead.
    pub fn storage_bytes(&self)->usize {
        std::mem::size_of::<Self>()+self.source_sha256.capacity()+self.keys.capacity()*std::mem::size_of::<KeyRegion>()+
        self.keys.iter().map(|key|key.button_key.capacity()+key.input_type.capacity()+key.input_id.capacity()+key.label.capacity()+
            key.function_list.capacity()*std::mem::size_of::<String>()+key.function_list.iter().map(String::capacity).sum::<usize>()+
            key.geometry.as_ref().map_or(0,|g|g.segments.capacity()*std::mem::size_of::<Segment>()+g.contours.capacity()*std::mem::size_of::<Vec<Point>>()+g.contours.iter().map(|c|c.capacity()*std::mem::size_of::<Point>()).sum::<usize>())).sum::<usize>()
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 2 * 1024 * 1024 {
            return Err("Source geometry file exceeds 2 MB".into());
        }
        Self::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        let raw: RawLayout = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if raw.keys.len() > 512 || raw.source_sha256.len() != 64 {
            return Err("Invalid source layout provenance or key count".into());
        }
        Transform::meet(
            raw.view_box,
            Viewport {
                left: 0.0,
                top: 0.0,
                width: 730.0,
                height: 387.0,
            },
        )
        .ok_or("Invalid source viewBox")?;
        let keys = raw
            .keys
            .into_iter()
            .map(|key| {
                let geometry = match (key.d.as_deref(),key.shape) {
                    (Some(_),Some(_))=>return Err("Source key has conflicting path and primitive geometry".into()),
                    (Some(path),None)=>Some(Geometry::parse(path)?),
                    (None,Some(shape))=>Some(shape.geometry()?),
                    (None,None)=>None,
                };
                Ok(KeyRegion {
                    button_key: key.button_key,
                    input_type: key.input_type,
                    side_panel: key.side_panel,
                    key_toggle: key.key_toggle,
                    input_id: key.input_id,
                    label: key.counter,
                    firmware_id: key.firmware_id,
                    function_list: key.function_list,
                    enabled: key.enabled,
                    geometry,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            product_id: raw.product_id,
            edition_id: raw.edition_id,
            layout_id: raw.layout_id,
            view_box: raw.view_box,
            source_sha256: raw.source_sha256,
            keys,
        })
    }

    pub fn hit_test(&self, screen: Point, viewport: Viewport) -> Option<&KeyRegion> {
        let source = Transform::meet(self.view_box, viewport)?.to_source(screen);
        self.keys.iter().rev().find(|key| {
            key.enabled
                && key
                    .geometry
                    .as_ref()
                    .is_some_and(|shape| shape.contains(source))
        })
    }

    pub fn bounds(&self, input_id: &str, viewport: Viewport) -> Option<Bounds> {
        let transform = Transform::meet(self.view_box, viewport)?;
        let bounds = self
            .keys
            .iter()
            .find(|key| key.input_id == input_id)?
            .geometry
            .as_ref()?
            .bounds;
        let top_left = transform.to_screen(Point {
            x: bounds.left,
            y: bounds.top,
        });
        let bottom_right = transform.to_screen(Point {
            x: bounds.right,
            y: bounds.bottom,
        });
        Some(Bounds {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x,
            bottom: bottom_right.y,
        })
    }

    /// Leaves the exact transformed source path in the supplied GDI context.
    /// The caller chooses fill/stroke/alpha composition for the active UI state.
    #[cfg(windows)]
    pub unsafe fn trace_key_path(
        &self,
        dc: windows_sys::Win32::Graphics::Gdi::HDC,
        input_id: &str,
        viewport: Viewport,
    ) -> bool {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::*;
        let Some(transform) = Transform::meet(self.view_box, viewport) else {
            return false;
        };
        let Some(shape) = self
            .keys
            .iter()
            .find(|key| key.input_id == input_id)
            .and_then(|key| key.geometry.as_ref())
        else {
            return false;
        };
        let convert = |point| {
            let point = transform.to_screen(point);
            POINT {
                x: point.x.round() as i32,
                y: point.y.round() as i32,
            }
        };
        if BeginPath(dc) == 0 {
            return false;
        }
        if let Some(primitive)=shape.primitive {
            let a=convert(Point{x:shape.bounds.left,y:shape.bounds.top});let b=convert(Point{x:shape.bounds.right,y:shape.bounds.bottom});
            let result=match primitive {
                Primitive::Ellipse{..}=>Ellipse(dc,a.x,a.y,b.x,b.y),
                Primitive::RoundedRect{rx,ry,..}=>{
                    if rx==0.||ry==0. {Rectangle(dc,a.x,a.y,b.x,b.y)}else{
                        RoundRect(dc,a.x,a.y,b.x,b.y,(2.*rx*transform.scale).round() as i32,(2.*ry*transform.scale).round() as i32)
                    }
                },
            };
            if result==0 {AbortPath(dc);return false;}
            return EndPath(dc)!=0;
        }
        for &segment in &shape.segments {
            let result = match segment {
                Segment::Move(point) => {
                    let p = convert(point);
                    MoveToEx(dc, p.x, p.y, std::ptr::null_mut())
                }
                Segment::Line(point) => {
                    let p = convert(point);
                    LineTo(dc, p.x, p.y)
                }
                Segment::Cubic(first, second, end) => {
                    let points = [convert(first), convert(second), convert(end)];
                    PolyBezierTo(dc, points.as_ptr(), 3)
                }
                Segment::Close => CloseFigure(dc),
            };
            if result == 0 {
                AbortPath(dc);
                return false;
            }
        }
        EndPath(dc) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_shaped_enter_does_not_hit_its_missing_corner() {
        let path = Geometry::parse("M0,0H20V10H10V30H0z").unwrap();
        assert!(path.contains(Point { x: 5.0, y: 20.0 }));
        assert!(path.contains(Point { x: 15.0, y: 5.0 }));
        assert!(!path.contains(Point { x: 15.0, y: 20.0 }));
    }

    #[test]
    fn cubic_bounds_use_extrema_not_control_point_rectangle() {
        let path = Geometry::parse("M0 0C100 100 100 -100 200 0").unwrap();
        assert!((path.bounds.top + 28.867513459).abs() < 1e-6);
        assert!((path.bounds.bottom - 28.867513459).abs() < 1e-6);
    }

    #[test]
    fn svg_meet_letterboxes_and_roundtrips() {
        let transform = Transform::meet(
            [0.0, 0.0, 730.0, 387.0],
            Viewport {
                left: 10.0,
                top: 20.0,
                width: 1460.0,
                height: 1000.0,
            },
        )
        .unwrap();
        let source = Point { x: 137.0, y: 207.0 };
        let roundtrip = transform.to_source(transform.to_screen(source));
        assert!((roundtrip.x - source.x).abs() < 1e-10);
        assert!((roundtrip.y - source.y).abs() < 1e-10);
        assert_eq!(
            transform.to_screen(Point { x: 0.0, y: 0.0 }),
            Point { x: 10.0, y: 133.0 }
        );
    }

    #[test]
    fn unsupported_paths_are_not_replaced_by_guessed_boxes() {
        assert!(Geometry::parse("M0,0A20,20,0,0,0,30,40").is_err());
        assert!(Geometry::parse("M0,0C1,2,3,4").is_err());
    }
    #[test]
    fn curved_primitives_have_analytic_hit_boundaries_and_svg_radius_clamping() {
        let circle=Primitive::Ellipse{cx:20.,cy:10.,rx:5.,ry:5.}.geometry().unwrap();
        assert!(circle.contains(Point{x:20.,y:10.}));assert!(circle.contains(Point{x:25.,y:10.}));assert!(!circle.contains(Point{x:15.,y:5.}));
        let rounded=Primitive::RoundedRect{x:0.,y:0.,width:20.,height:10.,rx:50.,ry:50.}.geometry().unwrap();
        assert!(matches!(rounded.primitive,Some(Primitive::RoundedRect{rx:10.,ry:5.,..})));assert!(!rounded.contains(Point{x:0.,y:0.}));assert!(rounded.contains(Point{x:0.,y:5.}));
        for shape in [Primitive::Ellipse{cx:0.,cy:0.,rx:0.,ry:1.},Primitive::Ellipse{cx:f64::NAN,cy:0.,rx:1.,ry:1.},Primitive::RoundedRect{x:0.,y:0.,width:1.,height:1.,rx:-1.,ry:0.}] {assert!(shape.geometry().is_err());}
    }
    #[test]
    fn primitive_layouts_refuse_ambiguous_path_data_and_keep_exact_transformed_bounds() {
        let mut raw=serde_json::json!({"product_id":100,"edition_id":0,"layout_id":0,"source_sha256":"0".repeat(64),"view_box":[0,0,100,100],"keys":[{"inputID":"0:1","buttonKey":"light","isEnabled":true,"shape":{"kind":"ellipse","cx":25,"cy":25,"rx":10,"ry":5}}]});
        let layout=SourceLayout::from_slice(&serde_json::to_vec(&raw).unwrap()).unwrap();let viewport=Viewport{left:20.,top:20.,width:200.,height:200.};
        let bounds=layout.bounds("0:1",viewport).unwrap();assert_eq!((bounds.left,bounds.top,bounds.right,bounds.bottom),(50.,60.,90.,80.));
        assert_eq!(layout.hit_test(Point{x:70.,y:70.},viewport).unwrap().input_id,"0:1");assert!(layout.hit_test(Point{x:50.,y:60.},viewport).is_none());
        raw["keys"][0]["d"]=serde_json::json!("M0 0H10V10Z");assert!(SourceLayout::from_slice(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
    #[test]
    fn native_gdi_primitive_paths_keep_rounded_corners_outside_the_fill_region() {
        use windows_sys::Win32::Graphics::Gdi::*;
        unsafe {
            let dc=CreateCompatibleDC(std::ptr::null_mut());assert!(!dc.is_null());
            for shape in [serde_json::json!({"kind":"ellipse","cx":50,"cy":50,"rx":40,"ry":30}),serde_json::json!({"kind":"rounded-rect","x":10,"y":20,"width":80,"height":60,"rx":20,"ry":20})] {
                let raw=serde_json::json!({"product_id":100,"edition_id":0,"layout_id":0,"source_sha256":"0".repeat(64),"view_box":[0,0,100,100],"keys":[{"inputID":"0:1","buttonKey":"light","isEnabled":true,"shape":shape}]});
                let layout=SourceLayout::from_slice(&serde_json::to_vec(&raw).unwrap()).unwrap();assert!(layout.trace_key_path(dc,"0:1",Viewport{left:0.,top:0.,width:100.,height:100.}));let region=PathToRegion(dc);assert!(!region.is_null());assert_ne!(PtInRegion(region,50,50),0);assert_eq!(PtInRegion(region,10,20),0);DeleteObject(region);
            }
            DeleteDC(dc);
        }
    }

}
