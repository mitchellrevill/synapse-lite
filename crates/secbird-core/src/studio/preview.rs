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
    Static(Pixel),
    Spectrum(SpectrumCycle),
    Fire(FireCycle),
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
}

#[derive(Deserialize)]
struct Placement {
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
impl Generator {
    fn bytes(&self) -> usize {
        match self {
            Self::Static(_) => 0,
            Self::Spectrum(cycle) => cycle.storage_bytes(),
            Self::Fire(cycle) => cycle.storage_bytes(),
        }
    }
}
impl Preview {
    pub fn from_snapshot(snapshot: &ReadSnapshot, random_state: u32) -> Result<Self> {
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
        };
        // Match backend applyLayersEffect: reversed roots and reversed children;
        // each layer's own visibility controls generation.
        for raw in layers(&root)?.into_iter().rev() {
            let item = Object::parse(&raw)?;
            match item.text("type")?.as_str() {
                "layer" => view.compile_layer(&item, &admitted)?,
                "group" => {
                    for child in layers(&item)?.into_iter().rev() {
                        view.compile_layer(&Object::parse(&child)?, &admitted)?;
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
                6 => Generator::Static(Pixel::from_studio_color(color(&params, "color")?)?),
                4 => {
                    let stops = if params.get("colorStops").is_some() {
                        Some(
                            array_field(&params, "colorStops")?
                                .iter()
                                .map(|raw| {
                                    let stop = Object::parse(raw)?;
                                    let position = serde_json::from_str(
                                        stop.get("Stop")
                                            .ok_or_else(|| invalid("missing gradient Stop"))?
                                            .get(),
                                    )
                                    .map_err(invalid)?;
                                    Ok(super::GradientStop {
                                        position,
                                        color: color(&stop, "Color")?,
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?,
                        )
                    } else {
                        None
                    };
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
        self.output.fill(Rgb(0, 0, 0));
        for effect in &mut self.effects {
            match &mut effect.generator {
                Generator::Static(pixel) => self.scratch.fill(*pixel),
                Generator::Spectrum(cycle) => self.scratch.fill(cycle.next_pixel()),
                Generator::Fire(cycle) => cycle.next_frame(&mut self.scratch)?,
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
    fn unsupported_geometry_effects_and_foreign_memberships_refuse_instead_of_partial_preview() {
        let raw: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        let mut cases = vec![];
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
