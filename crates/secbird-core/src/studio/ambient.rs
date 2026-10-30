//! Source-shaped Ambient Awareness regional authoring. Capture is never implicit.
use super::*;

pub const AMBIENT_PRESETS: [&str; 5] = ["full", "left", "top", "right", "bottom"];
pub const AMBIENT_SCALE: f64 = 65535.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AmbientRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
impl AmbientRect {
    pub fn validate(self) -> Result<()> {
        if [self.left, self.top, self.right, self.bottom]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=AMBIENT_SCALE).contains(v))
            || self.left >= self.right
            || self.top >= self.bottom
        {
            return Err(invalid(
                "Ambient rectangle requires ordered coordinates in 0..65535",
            ));
        }
        Ok(())
    }
    pub fn preset(index: usize) -> Option<Self> {
        let mut rect = Self {
            left: 0.,
            top: 0.,
            right: AMBIENT_SCALE,
            bottom: AMBIENT_SCALE,
        };
        match index {
            0 => {}
            1 => rect.right *= 0.1,
            2 => rect.bottom *= 0.1,
            3 => rect.left = AMBIENT_SCALE * 0.9,
            4 => rect.top = AMBIENT_SCALE * 0.9,
            _ => return None,
        }
        Some(rect)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientProperty {
    Screen,
    Rect,
    Blur,
}
impl AmbientProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Rect => "screenRect",
            Self::Blur => "blur",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum AmbientValue {
    Screen(String),
    Rect(AmbientRect),
    Blur(u8),
}
#[derive(Clone, Debug, PartialEq)]
pub enum SelectedAmbientValue {
    Empty,
    Uniform(Option<AmbientValue>),
    Mixed,
}
#[derive(Clone, Copy, Debug)]
pub enum AmbientEdit {
    Preset(usize),
    Rect(AmbientRect),
    Blur(u8),
}

fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "ambient" || item.get("value").is_some_and(|v| v.get() != "11") {
        return Err(invalid("selected layer is not Studio Ambient11"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(&serde_json::json!({"screen":"full","blur":5}))?)?
    } else {
        Object(vec![])
    };
    for raw in [item.get("params"), region.and_then(|r| r.get("params"))]
        .into_iter()
        .flatten()
    {
        for (key, value) in Object::parse(raw)?.0 {
            result.set(&key, Some(&value));
        }
    }
    Ok(result)
}
impl AmbientEdit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (key, value) in &projection.0 {
                params.set(key, Some(value));
            }
        }
        match self {
            Self::Preset(index) => params.set("screen", Some(&encode(&AMBIENT_PRESETS[index])?)),
            Self::Blur(value) => params.set("blur", Some(&encode(&value)?)),
            Self::Rect(value) => {
                // Preserve future rectangle fields while editing its known coordinates.
                let mut rect = params
                    .get("screenRect")
                    .or_else(|| projection.get("screenRect"))
                    .map(Object::parse)
                    .transpose()?
                    .unwrap_or(Object(vec![]));
                for (name, coordinate) in [
                    ("Left", value.left),
                    ("Top", value.top),
                    ("Right", value.right),
                    ("Bottom", value.bottom),
                ] {
                    rect.set(name, Some(&encode(&coordinate)?));
                }
                params.set("screenRect", Some(&encode(&rect)?));
                params.set("screen", Some(&encode(&"custom")?));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ambient_tests.rs"]
mod tests;
impl Draft {
    fn ambient_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
        let selection = selected(selection)?;
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if selection.is_empty() {
            return Ok(vec![]);
        }
        inventory(self, &selection)?;
        let mut seen = BTreeSet::new();
        let mut result = vec![];
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut matched = false;
            for raw in entries(&region, "devices")? {
                let device = Object::parse(&raw)?;
                let id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected Ambient memberships"));
                        }
                        matched = true;
                    }
                }
            }
            if matched {
                result.push(merged(&item, Some(&region), false)?);
            }
        }
        if seen.len() < selection.len() {
            result.push(merged(&item, None, true)?);
        }
        Ok(result)
    }
    pub fn ambient_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: AmbientProperty,
    ) -> Result<SelectedAmbientValue> {
        let values = self.ambient_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<AmbientValue>> {
            params
                .get(property.field())
                .map(|raw| match property {
                    AmbientProperty::Screen => serde_json::from_str(raw.get())
                        .map(AmbientValue::Screen)
                        .map_err(invalid),
                    AmbientProperty::Rect => serde_json::from_str(raw.get())
                        .map(AmbientValue::Rect)
                        .map_err(invalid),
                    AmbientProperty::Blur => serde_json::from_str(raw.get())
                        .map(AmbientValue::Blur)
                        .map_err(invalid),
                })
                .transpose()
        };
        let Some(first) = values.first() else {
            return Ok(SelectedAmbientValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedAmbientValue::Mixed);
            }
        }
        Ok(SelectedAmbientValue::Uniform(first))
    }
    pub fn edit_ambient(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        edit: AmbientEdit,
    ) -> Result<bool> {
        match edit {
            AmbientEdit::Preset(index) if index >= AMBIENT_PRESETS.len() => {
                return Err(invalid("unknown Ambient region"))
            }
            AmbientEdit::Blur(value) if !(1..=9).contains(&value) => {
                return Err(invalid("Ambient blur requires 1..9"))
            }
            AmbientEdit::Rect(rect) => rect.validate()?,
            _ => {}
        }
        self.ambient_params(id, selection)?; // Validate overlaps before mutation.
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Ambient(edit, &projection))
    }
}
