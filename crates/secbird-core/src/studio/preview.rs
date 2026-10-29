//! Bounded local preview for a single source Ornata placement. Never device I/O.
use super::*;
use crate::fire_lighting::{FireCycle, FireRegion};
use crate::lighting_pixels::{BlendMode, Layer, Pixel};
use crate::spectrum_lighting::SpectrumCycle;
use crate::Rgb;

const ROWS: usize = 7;
const COLS: usize = 23;
const CELLS: usize = ROWS * COLS;
const MAX_STORAGE: usize = 32 * 1024 * 1024;
enum Generator {
    Ambient {rect:super::AmbientRect, blur:u8, bounds:[usize;4]},
    Static(Pixel),
    Spectrum(SpectrumCycle),
    Fire(FireCycle),
    Breathing(crate::breathing_lighting::BreathingCycle),
    Wave(crate::studio_wave_lighting::StudioWaveCycle),
}
struct Effect {
    generator: Generator,
    mask: Box<[u8]>,
}
pub struct Preview {
    device_id: String,
    effects: Vec<Effect>,
    scratch: Box<[Pixel]>,
    output: Box<[Rgb]>,
    listed: [bool; CELLS],
    random_state: u32,
    frames: u64,
    synthetic_ambient: bool,
}

#[derive(Deserialize)]
struct Placement {
    #[serde(default,rename="editorPreset")]
    editor_preset: bool,
    id: String,
    #[serde(rename = "productId")]
    product: u16,
    #[serde(rename = "editionId")]
    edition: u16,
    layout: u16,
    orientation: u16,
    x: f64,
    y: f64,
    config: Config,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Config {
    #[serde(rename = "VID")]
    vid: u16,
    #[serde(rename = "PID")]
    pid: u16,
    #[serde(rename = "EID")]
    eid: u16,
    layout: u16,
    matrix_max_row: u16,
    matrix_max_col: u16,
    led_width: u16,
    led_height: u16,
    led_width_offset: u16,
    led_height_offset: u16,
    led_matrix: Vec<Mapping>,
}
#[derive(Deserialize)]
struct Mapping {
    #[serde(rename = "MatrixPos")]
    logical: [u16; 2],
}

fn array_field(object: &Object, name: &str) -> Result<Vec<Box<RawValue>>> {
    array(
        object
            .get(name)
            .ok_or_else(|| invalid(format!("preview missing {name}")))?,
    )
}
fn color(params: &Object, key: &str) -> Result<Option<u32>> {
    params
        .get(key)
        .map(|raw| serde_json::from_str(raw.get()).map_err(invalid))
        .transpose()
}
fn gradient(params: &Object) -> Result<Option<Vec<super::GradientStop>>> {
    if params.get("colorStops").is_none() {return Ok(None);}
    array_field(params,"colorStops")?.iter().map(|raw|{
        let stop=Object::parse(raw)?;
        Ok(super::GradientStop {position:serde_json::from_str(stop.get("Stop").ok_or_else(||invalid("missing gradient Stop"))?.get()).map_err(invalid)?,color:color(&stop,"Color")?})
    }).collect::<Result<Vec<_>>>().map(Some)
}
impl Generator {
    fn sample_taps(&self,mask:&[u8])->usize {
        if let Self::Ambient{blur,..}=self {mask.iter().filter(|m|**m!=0).count()*(2*usize::from(*blur)-1).pow(2)} else {0}
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Ambient {..} | Self::Static(_) => 0,
            Self::Spectrum(cycle) => cycle.storage_bytes(),
            Self::Fire(cycle) => cycle.storage_bytes(),
            Self::Breathing(cycle) => cycle.storage_bytes(),
            Self::Wave(cycle) => cycle.storage_bytes(),
        }
    }
}
impl Preview {
    pub fn from_snapshot(snapshot: &ReadSnapshot, random_state: u32) -> Result<Self> {
        Self::build(snapshot, random_state, false)
    }
    /// Diagnostic only: deterministic colour bars, no desktop capture. This
    /// explicit constructor is intentionally never used by physical output.
    pub fn from_snapshot_synthetic_ambient(snapshot: &ReadSnapshot, random_state:u32) -> Result<Self> {
        Self::build(snapshot,random_state,true)
    }
    fn build(snapshot:&ReadSnapshot,random_state:u32,synthetic_ambient:bool) -> Result<Self> {
        let root = Object::parse(&snapshot.raw)?;
        let devices = array_field(&root, "devices")?;
        if devices.len() != 1 {
            return Err(invalid(
                "local preview currently requires one Ornata placement",
            ));
        }
        let placement: Placement = serde_json::from_str(devices[0].get()).map_err(invalid)?;
        let c = &placement.config;
        if placement.id.is_empty()
            || placement.editor_preset
            || placement.id.len() > 512
            || placement.product != 605
            || placement.edition != 0
            || placement.orientation != 0
            || c.vid != 5426
            || c.pid != 605
            || c.eid != 0
            || c.layout != placement.layout
            || c.matrix_max_row != 7
            || c.matrix_max_col != 23
            || [
                c.led_width,
                c.led_height,
                c.led_width_offset,
                c.led_height_offset,
            ] != [0; 4]
            || !placement.x.is_finite()
            || !placement.y.is_finite()
            || !(0.0..=10000.0).contains(&placement.x)
            || !(0.0..=10000.0).contains(&placement.y)
            || placement.x % 40.0 != 0.0
            || placement.y % 40.0 != 0.0
        {
            return Err(invalid(
                "local preview currently supports one unrotated Ornata V2 with its saved default layout",
            ));
        }
        if c.led_matrix.is_empty() || c.led_matrix.len() > 256 {
            return Err(invalid("invalid preview source LED matrix"));
        }
        let mut listed = [false; CELLS];
        for mapping in &c.led_matrix {
            let [row, col] = mapping.logical;
            if usize::from(row) >= ROWS || usize::from(col) >= COLS {
                return Err(invalid("source LED outside preview matrix"));
            }
            listed[usize::from(row) * COLS + usize::from(col)] = true;
        }
        let inventory = crate::studio::inventory::led_inventory(devices[0].get())?;
        let admitted: BTreeSet<_> = inventory.iter().map(|led| (led.row, led.col)).collect();
        for &(row, col) in &admitted {
            if usize::from(row) >= ROWS
                || usize::from(col) >= COLS
                || !listed[usize::from(row) * COLS + usize::from(col)]
            {
                return Err(invalid(
                    "composition LED is not in the source logical matrix",
                ));
            }
        }
        let mut view = Self {
            device_id: placement.id,
            effects: vec![],
            scratch: vec![Pixel::EMPTY; CELLS].into_boxed_slice(),
            output: vec![Rgb(0, 0, 0); CELLS].into_boxed_slice(),
            listed,
            random_state,
            frames: 0,
            synthetic_ambient,
        };
        // Match backend applyLayersEffect: reversed roots and reversed children;
        // each layer's own visibility controls generation.
        for raw in layers(&root)?.into_iter().rev() {
            let item = Object::parse(&raw)?;
            match item.text("type")?.as_str() {
                "layer" => view.compile_layer(&item, &admitted)?,
                "group" => {
                    if item.visible()? == Some(true) {
                        for child in layers(&item)?.into_iter().rev() {
                            view.compile_layer(&Object::parse(&child)?, &admitted)?;
                        }
                    }
                }
                _ => return Err(invalid("unsupported preview item type")),
            }
        }
        view.advance()?;
        Ok(view)
    }
    fn compile_layer(&mut self, item: &Object, admitted: &BTreeSet<(u16, u16)>) -> Result<()> {
        if item.visible()? != Some(true) {
            return Ok(());
        }
        let name = item.text("name")?;
        let expected = match name.as_str() {
            "static" => 6,
            "spectrum" => 4,
            "fire" => 8,
            "breathing" => 5,
            "wave" => 2,
            "ambient" if self.synthetic_ambient => 11,
            "ambient" => return Err(invalid("Ambient native screen sampling and projection are unverified; only the explicit synthetic diagnostic is available")),
            _ => return Err(invalid(format!("preview does not yet implement {name}"))),
        };
        if let Some(value) = item.get("value") {
            if serde_json::from_str::<u16>(value.get()).map_err(invalid)? != expected {
                return Err(invalid("conflicting preview effect ID"));
            }
        }
        for raw in array_field(item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut mask = vec![0u8; CELLS];
            for raw in array_field(&region, "devices")? {
                let device = Object::parse(&raw)?;
                if device.text("id")? != self.device_id {
                    return Err(invalid("preview region refers to a different placement"));
                }
                for raw in array_field(&device, "leds")? {
                    #[derive(Deserialize)]
                    struct Coordinate {
                        row: u16,
                        col: u16,
                    }
                    let led: Coordinate = serde_json::from_str(raw.get()).map_err(invalid)?;
                    if !admitted.contains(&(led.row, led.col)) {
                        return Err(invalid(
                            "preview region LED is not in explicit composition inventory",
                        ));
                    }
                    mask[usize::from(led.row) * COLS + usize::from(led.col)] = 100;
                }
            }
            if !mask.iter().any(|m| *m != 0) {
                continue;
            }
            let mut params = match item.get("params") {
                Some(raw) => Object::parse(raw)?,
                None => Object(vec![]),
            };
            for (name, value) in Object::parse(
                region
                    .get("params")
                    .ok_or_else(|| invalid("preview region has no params"))?,
            )?
            .0
            {
                params.set(&name, Some(&value));
            }
            let generator = match expected {
                11 => {
                    let screen=params.text("screen")?;
                    let rect=if screen=="custom" {
                        serde_json::from_str::<super::AmbientRect>(params.get("screenRect").ok_or_else(||invalid("Ambient custom region requires screenRect"))?.get()).map_err(invalid)?
                    } else {
                        super::AmbientRect::preset(super::AMBIENT_PRESETS.iter().position(|name|*name==screen).ok_or_else(||invalid("unknown Ambient screen region"))?).unwrap()
                    };
                    rect.validate()?;
                    let blur:u8=serde_json::from_str(params.get("blur").ok_or_else(||invalid("Ambient requires explicit blur"))?.get()).map_err(invalid)?;
                    if !(1..=9).contains(&blur) {return Err(invalid("Ambient blur outside1..9"));}
                    let points:Vec<_>=mask.iter().enumerate().filter(|(_,m)|**m!=0).map(|(i,_)|(i%COLS,i/COLS)).collect();
                    let left=points.iter().map(|p|p.0).min().unwrap();
                    let top=points.iter().map(|p|p.1).min().unwrap();
                    let right=points.iter().map(|p|p.0).max().unwrap()+1;
                    let bottom=points.iter().map(|p|p.1).max().unwrap()+1;
                    Generator::Ambient{rect,blur,bounds:[left,top,right,bottom]}
                }
                2 => {
                    let value = |key: &str| -> Result<u32> {
                        serde_json::from_str(params.get(key).ok_or_else(||invalid(format!("Wave needs explicit {key}")))?.get()).map_err(invalid)
                    };
                    let start=params.get("playbackStart").map(|raw|serde_json::from_str::<String>(raw.get()).map_err(invalid)).transpose()?;
                    let end=params.get("playbackEnd").map(|raw|serde_json::from_str::<String>(raw.get()).map_err(invalid)).transpose()?;
                    let split=params.get("split").map(|raw|serde_json::from_str::<bool>(raw.get()).map_err(invalid)).transpose()?.unwrap_or(false);
                    let cycles=params.get("cycles").map(|raw|serde_json::from_str::<i64>(raw.get()).map_err(invalid)).transpose()?;
                    if split || params.get("counterclockwise").is_some() || cycles!=Some(-1)
                        || start.as_deref().is_some_and(|s|s!="random") || end.as_deref().is_some_and(|s|s!="never") {
                        return Err(invalid("Wave split, rotational, finite and input-triggered modes are not implemented; continuous cycles must be explicit"));
                    }
                    use crate::wave_lighting::{CardinalDirection as Direction,WaveRegion};
                    let direction=match value("angle")? {0=>Direction::Up,90=>Direction::Right,180=>Direction::Down,270=>Direction::Left,_=>return Err(invalid("Wave arbitrary angles are not implemented"))};
                    let points:Vec<_>=mask.iter().enumerate().filter(|(_,m)|**m!=0).map(|(i,_)|(i/COLS,i%COLS)).collect();
                    let top=points.iter().map(|p|p.0).min().unwrap() as u16;
                    let bottom=points.iter().map(|p|p.0).max().unwrap() as u16+1;
                    let left=points.iter().map(|p|p.1).min().unwrap() as u16;
                    let right=points.iter().map(|p|p.1).max().unwrap() as u16+1;
                    let stops=gradient(&params)?;
                    Generator::Wave(crate::studio_wave_lighting::StudioWaveCycle::new(WaveRegion::new(7,23,left,top,right,bottom)?,direction,value("speed")?,value("width")?,value("pause")?,stops.as_deref())?)
                }
                5 => {
                    let random=params.get("randomColor").map(|raw|serde_json::from_str::<bool>(raw.get()).map_err(invalid)).transpose()?.unwrap_or(false);
                    let start=params.get("playbackStart").map(|raw|serde_json::from_str::<String>(raw.get()).map_err(invalid)).transpose()?;
                    let end=params.get("playbackEnd").map(|raw|serde_json::from_str::<String>(raw.get()).map_err(invalid)).transpose()?;
                    let cycles=params.get("cycles").map(|raw|serde_json::from_str::<i64>(raw.get()).map_err(invalid)).transpose()?.unwrap_or(0);
                    if random || start.as_deref().is_some_and(|mode|mode!="random") || end.as_deref().is_some_and(|mode|mode!="never") || ![0,-1].contains(&cycles) {
                        return Err(invalid("Breathing random, finite-cycle and input-triggered modes are not implemented"));
                    }
                    let stops=gradient(&params)?;
                    let duration=params.get("duration").map(|raw|serde_json::from_str(raw.get()).map_err(invalid)).transpose()?;
                    Generator::Breathing(crate::breathing_lighting::BreathingCycle::studio(color(&params,"color")?,color(&params,"color2")?,stops.as_deref(),duration)?)
                }
                6 => Generator::Static(Pixel::from_studio_color(color(&params, "color")?)?),
                4 => {
                    let stops=gradient(&params)?;
                    let duration = params
                        .get("duration")
                        .map(|raw| serde_json::from_str(raw.get()).map_err(invalid))
                        .transpose()?;
                    Generator::Spectrum(SpectrumCycle::studio(stops.as_deref(), duration)?)
                }
                8 => {
                    let hot = color(&params, "color")?;
                    let cold = color(&params, "color2")?;
                    // AddEffect initializes a whole-canvas build; SetMask then
                    // rebuilds from the nonzero bounding box, consuming another
                    // native CRT sequence even when the bounds are unchanged.
                    let initial = FireCycle::studio(
                        FireRegion::new(7, 23, 0, 0, 23, 7)?,
                        hot,
                        cold,
                        self.random_state,
                    )?;
                    self.random_state = initial.final_random_state();
                    drop(initial);
                    let points: Vec<_> = mask
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| **m != 0)
                        .map(|(i, _)| (i / COLS, i % COLS))
                        .collect();
                    let top = points.iter().map(|p| p.0).min().unwrap() as u16;
                    let bottom = points.iter().map(|p| p.0).max().unwrap() as u16 + 1;
                    let left = points.iter().map(|p| p.1).min().unwrap() as u16;
                    let right = points.iter().map(|p| p.1).max().unwrap() as u16 + 1;
                    let cycle = FireCycle::studio(
                        FireRegion::new(7, 23, left, top, right, bottom)?,
                        hot,
                        cold,
                        self.random_state,
                    )?;
                    self.random_state = cycle.final_random_state();
                    Generator::Fire(cycle)
                }
                _ => unreachable!(),
            };
            if self.effects.iter().map(|e|e.generator.sample_taps(&e.mask)).sum::<usize>() + generator.sample_taps(&mask) > 262144 {
                return Err(invalid("Ambient diagnostic exceeds262144 sampling taps per frame"));
            }
            if self.effects.len() >= MAX_LAYERS
                || self.storage_bytes()
                    + generator.bytes()
                    + mask.len()
                    + std::mem::size_of::<Effect>()
                    > MAX_STORAGE
            {
                return Err(invalid(
                    "too many animated layers to preview within the memory limit",
                ));
            }
            self.effects.push(Effect {
                generator,
                mask: mask.into_boxed_slice(),
            });
        }
        Ok(())
    }
    pub fn advance(&mut self) -> Result<()> {
        if !self.synthetic_ambient {return self.render_frame(None);}
        let mut synthetic=[0u8;crate::ambient_sampling::TEST_WIDTH*crate::ambient_sampling::TEST_HEIGHT*4];
        crate::ambient_sampling::synthetic_frame(self.frames,&mut synthetic);
        let frame=crate::ambient_sampling::DesktopFrame::new(crate::ambient_sampling::TEST_WIDTH,crate::ambient_sampling::TEST_HEIGHT,&synthetic)?;
        self.render_frame(Some(&frame))
    }
    /// Inject a caller-owned synthetic frame. Ordinary runtime plans cannot
    /// acquire an Ambient generator, even if this method is called.
    pub fn advance_with_synthetic_frame(&mut self,frame:&crate::ambient_sampling::DesktopFrame<'_>) -> Result<()> {
        if !self.synthetic_ambient {return Err(invalid("explicit synthetic Ambient plan required"));}
        self.render_frame(Some(frame))
    }
    pub fn is_synthetic_ambient(&self) -> bool {self.synthetic_ambient}
    fn render_frame(&mut self,frame:Option<&crate::ambient_sampling::DesktopFrame<'_>>) -> Result<()> {
        self.output.fill(Rgb(0, 0, 0));
        for effect in &mut self.effects {
            match &mut effect.generator {
                Generator::Ambient{rect,blur,bounds} => {
                    self.scratch.fill(Pixel::EMPTY);
                    let [left,top,right,bottom]=*bounds;
                    for y in top..bottom {for x in left..right {
                        if effect.mask[y*COLS+x]!=0 {
                            self.scratch[y*COLS+x]=crate::ambient_sampling::project(frame.ok_or_else(||invalid("synthetic frame required"))?,*rect,*blur,[right-left,bottom-top],[x-left,y-top])?;
                        }
                    }}
                }
                Generator::Static(pixel) => self.scratch.fill(*pixel),
                Generator::Spectrum(cycle) => self.scratch.fill(cycle.next_pixel()),
                Generator::Fire(cycle) => cycle.next_frame(&mut self.scratch)?,
                Generator::Breathing(cycle) => self.scratch.fill(cycle.next_pixel()),
                Generator::Wave(cycle) => {self.scratch.fill(Pixel::EMPTY);cycle.next_frame(&mut self.scratch)?;},
            }
            Layer {
                pixels: &self.scratch,
                opacity: 1.0,
                mask: Some(&effect.mask),
                mode: BlendMode::Transparency,
            }
            .blend(&mut self.output)?;
        }
        for (pixel, listed) in self.output.iter_mut().zip(self.listed) {
            if !listed {
                *pixel = Rgb(0, 0, 0);
            }
        }
        self.frames = self.frames.saturating_add(1);
        Ok(())
    }
    pub fn device_id(&self) -> &str {
        &self.device_id
    }
    pub fn frame(&self) -> &[Rgb] {
        &self.output
    }
    pub fn rendered_frames(&self) -> u64 {
        self.frames
    }
    pub fn final_random_state(&self) -> u32 {
        self.random_state
    }
    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }
    pub fn storage_bytes(&self) -> usize {
        self.device_id.capacity()
            + self.scratch.len() * std::mem::size_of::<Pixel>()
            + self.output.len() * std::mem::size_of::<Rgb>()
            + self.effects.capacity() * std::mem::size_of::<Effect>()
            + self
                .effects
                .iter()
                .map(|e| e.mask.len() + e.generator.bytes())
                .sum::<usize>()
    }
}

#[cfg(test)]
#[path="breathing_tests.rs"]
mod breathing_tests;
#[cfg(test)]
#[path="wave_tests.rs"]
mod wave_tests;
#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = include_str!("../../tests/fixtures/studio-preview-profile.json");
    #[test]
    fn complete_fire_spectrum_composition_matches_independent500_frame_reference() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-preview-reference.json"
        ))
        .unwrap();
        let draft = Draft::open(PROFILE).unwrap();
        let before = draft.json().to_owned();
        let snapshot = draft.read_snapshot();
        let mut preview = Preview::from_snapshot(&snapshot, 1).unwrap();
        assert_eq!(preview.effect_count(), 2);
        assert_eq!(preview.frame().len(), 161);
        assert_eq!(
            preview.final_random_state(),
            reference["final_crt_state"].as_u64().unwrap() as u32
        );
        assert!(preview.storage_bytes() < 90000);
        let mut hash = md5::Context::new();
        for _ in 0..500 {
            for c in preview.frame() {
                hash.consume([c.0, c.1, c.2]);
            }
            assert!(preview.frame()[6 * 23..].iter().all(|c| *c == Rgb(0, 0, 0)));
            preview.advance().unwrap();
        }
        assert_eq!(
            format!("{:x}", hash.compute()),
            reference["rgb_md5"].as_str().unwrap()
        );
        assert_eq!(preview.rendered_frames(), 501);
        assert!(!draft.is_dirty());
        assert!(draft.json() == before, "preview changed the raw draft");
        assert!(draft.matches_snapshot(&snapshot));
    }
    #[test]
    fn placement_origin_cancels_and_hidden_layers_do_not_generate() {
        let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        raw["devices"][0]["x"] = 0.into();
        raw["devices"][0]["y"] = 0.into();
        let alternate = Draft::open(&raw.to_string()).unwrap();
        let original = Draft::open(PROFILE).unwrap();
        let a = Preview::from_snapshot(&original.read_snapshot(), 1).unwrap();
        let b = Preview::from_snapshot(&alternate.read_snapshot(), 1).unwrap();
        assert_eq!(a.frame(), b.frame());
        raw["layers"][0]["visible"] = false.into();
        let alternate = Draft::open(&raw.to_string()).unwrap();
        let b = Preview::from_snapshot(&alternate.read_snapshot(), 1).unwrap();
        assert_eq!(b.effect_count(), 1);
        assert_eq!(b.final_random_state(), 1);
        assert_ne!(a.frame(), b.frame());
    }
    #[test]
    fn hidden_groups_do_not_render_children_even_when_their_own_visibility_is_true() {
        let mut raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        let mut children = raw["layers"].take();
        for child in children.as_array_mut().unwrap() { child["groupId"] = "g".into(); }
        raw["layers"] = serde_json::json!([{"id":"g","type":"group","title":"Group",
            "opened":true,"visible":false,"layers":children}]);
        let draft = Draft::open(&raw.to_string()).unwrap();
        let hidden = Preview::from_snapshot(&draft.read_snapshot(), 1).unwrap();
        assert_eq!(hidden.effect_count(), 0);
        assert!(hidden.frame().iter().all(|pixel|*pixel == Rgb(0,0,0)));
        raw["layers"][0]["visible"] = true.into();
        let shown = Draft::open(&raw.to_string()).unwrap();
        let shown = Preview::from_snapshot(&shown.read_snapshot(), 1).unwrap();
        let original = Draft::open(PROFILE).unwrap();
        let original = Preview::from_snapshot(&original.read_snapshot(), 1).unwrap();
        assert_eq!(shown.frame(), original.frame());
    }
    #[test]
    fn unsupported_geometry_effects_and_foreign_memberships_refuse_instead_of_partial_preview() {
        let raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        let mut cases = vec![];
        let mut x = raw.clone();
        x["devices"][0]["editorPreset"] = true.into();
        cases.push(x);
        let mut x = raw.clone();
        x["devices"][0]["orientation"] = 90.into();
        cases.push(x);
        let mut x = raw.clone();
        x["devices"][0]["x"] = 4521.into();
        cases.push(x);
        let mut x = raw.clone();
        x["devices"][0]["config"]["LedWidth"] = 40.into();
        cases.push(x);
        let mut x = raw.clone();
        x["layers"][0]["name"] = "audio".into();
        cases.push(x);
        let mut x = raw.clone();
        x["layers"][0]["regions"][0]["devices"][0]["id"] = "other".into();
        cases.push(x);
        let mut x = raw.clone();
        x["layers"][0]["regions"][0]["devices"][0]["leds"][0]["row"] = 6.into();
        cases.push(x);
        let mut x = raw.clone();
        x["layers"][1]["regions"][0]["params"]["colorStops"][0]["Color"] = serde_json::Value::Null;
        cases.push(x);
        for raw in cases {
            let draft = Draft::open(&raw.to_string()).unwrap();
            assert!(Preview::from_snapshot(&draft.read_snapshot(), 1).is_err());
            assert!(!draft.is_dirty());
        }
    }
    #[test]
    fn background_snapshots_are_session_scoped_and_changes_invalidate_without_row_clones() {
        let mut draft = Draft::open(PROFILE).unwrap();
        let initial = draft.read_snapshot();
        let other = Draft::open(PROFILE).unwrap();
        assert!(!other.matches_snapshot(&initial));
        draft.rename_layer("fire", "Renamed").unwrap();
        assert!(!draft.matches_snapshot(&initial));
        let checkpoint = draft.read_snapshot();
        let saved = draft.checkpoint();
        draft.accept_saved(saved).unwrap();
        assert!(draft.matches_snapshot(&checkpoint));
        draft.set_visible("fire", false).unwrap();
        assert!(!draft.matches_snapshot(&checkpoint));
        assert!(initial.json().contains("\"title\": \"Fire\""));
        draft.discard();
        assert!(draft.matches_snapshot(&checkpoint));
    }
}
