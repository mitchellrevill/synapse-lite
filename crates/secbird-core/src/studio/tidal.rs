//! Studio Tidal region authoring. Radial playback is a separate, gated contract.
use super::*;

pub const TIDAL_NONE: u32 = 1677721600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TidalProperty {
    Color,
    Color2,
    Speed,
    Start,
    End,
    Cycles,
    Random,
}

impl TidalProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Color2 => "color2",
            Self::Speed => "speed",
            Self::Start => "playbackStart",
            Self::End => "playbackEnd",
            Self::Cycles => "cycles",
            Self::Random => "randomColor",
        }
    }
    fn validate(self, value: Option<u32>) -> Result<()> {
        if match (self, value) {
            (Self::Color | Self::Color2, None) => true,
            (Self::Color | Self::Color2, Some(v)) => v <= 0xffffff || v == TIDAL_NONE,
            (Self::Speed, Some(v)) => v <= 50,
            (Self::Start | Self::End, Some(v)) => v <= 2,
            (Self::Cycles, Some(v)) => (1..=100).contains(&v),
            (Self::Random, Some(v)) => v <= 1,
            _ => false,
        } {
            Ok(())
        } else {
            Err(invalid(
                "Tidal requires RGB24/None, speed 0-50, a source playback mode, cycles 1-100 or boolean Random",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedTidalValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "tidal"
        || item
            .get("value")
            .is_some_and(|v| serde_json::from_str::<u16>(v.get()).ok() != Some(19))
    {
        return Err(invalid("selected layer is not Studio Tidal19"));
    }
    Ok(())
}

fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(&serde_json::json!({
            "color":65280,"color2":255,"randomColor":false,"speed":10,"angle":160,"split":true,
            "playbackStart":"random","playbackEnd":"never","cycles":-1
        }))?)?
    } else {
        Object(vec![])
    };
    for raw in [item.get("params"), region.and_then(|r| r.get("params"))]
        .into_iter()
        .flatten()
    {
        for (key, value) in Object::parse(raw)?.0 {
            if ![
                "centerPointX",
                "centerPointY",
                "centerPointX2",
                "centerPointY2",
                "counterclockwise",
            ]
            .contains(&key.as_str())
            {
                result.set(&key, Some(&value));
            }
        }
    }
    Ok(result)
}

#[derive(Clone, Copy)]
pub(super) struct Edit {
    pub property: TidalProperty,
    pub value: Option<u32>,
    pub cached_cycles: Option<u32>,
}

impl Edit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (key, value) in &projection.0 {
                params.set(key, Some(value));
            }
        }
        let raw = match self.property {
            TidalProperty::Random => encode(&(self.value == Some(1)))?,
            TidalProperty::Start => {
                let mode = self.value.unwrap();
                params.set(
                    "playbackEnd",
                    Some(&encode(&if mode == 0 { "never" } else { "after" })?),
                );
                params.set(
                    "cycles",
                    Some(&encode(&if mode == 0 {
                        -1
                    } else {
                        self.cached_cycles.unwrap_or(1) as i32
                    })?),
                );
                encode(&["random", "onPress", "onSelectedKeys"][mode as usize])?
            }
            TidalProperty::End => {
                let mode = self.value.unwrap();
                params.set(
                    "cycles",
                    Some(&encode(&if mode == 1 {
                        self.cached_cycles.unwrap_or(1) as i32
                    } else {
                        -1
                    })?),
                );
                encode(&["never", "after", "toggle"][mode as usize])?
            }
            TidalProperty::Color | TidalProperty::Color2 => {
                encode(&self.value.unwrap_or(TIDAL_NONE))?
            }
            _ => encode(&self.value.unwrap())?,
        };
        params.set(self.property.field(), Some(&raw));
        Ok(())
    }
}

impl Draft {
    fn tidal_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                            return Err(invalid("overlapping selected Tidal memberships"));
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
    pub fn tidal_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: TidalProperty,
    ) -> Result<SelectedTidalValue> {
        let values = self.tidal_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<u32>> {
            params
                .get(property.field())
                .map(|raw| {
                    if property == TidalProperty::Random {
                        serde_json::from_str::<bool>(raw.get())
                            .map(u32::from)
                            .map_err(invalid)
                    } else if matches!(property, TidalProperty::Start | TidalProperty::End) {
                        let text: String = serde_json::from_str(raw.get()).map_err(invalid)?;
                        let modes = if property == TidalProperty::Start {
                            ["random", "onPress", "onSelectedKeys"]
                        } else {
                            ["never", "after", "toggle"]
                        };
                        modes
                            .iter()
                            .position(|&m| m == text)
                            .map(|v| v as u32)
                            .ok_or_else(|| invalid("unknown Tidal playback mode"))
                    } else {
                        let value = serde_json::from_str::<i64>(raw.get()).map_err(invalid)?;
                        if property == TidalProperty::Cycles && value == -1 {
                            Ok(0)
                        } else {
                            u32::try_from(value).map_err(invalid)
                        }
                    }
                })
                .transpose()
        };
        let Some(first) = values.first() else {
            return Ok(SelectedTidalValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedTidalValue::Mixed);
            }
        }
        Ok(SelectedTidalValue::Uniform(first))
    }
    pub fn set_tidal_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: TidalProperty,
        value: Option<u32>,
    ) -> Result<bool> {
        self.set_tidal_property_with_cycles(id, selection, property, value, None)
    }
    /// The shared source Playback widget remembers the last cycle field edit
    /// across Start/End changes. The hint is transient UI state, never a profile field.
    pub fn set_tidal_playback_mode(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: TidalProperty,
        mode: u32,
        cached_cycles: Option<u32>,
    ) -> Result<bool> {
        if !matches!(property, TidalProperty::Start | TidalProperty::End)
            || cached_cycles.is_some_and(|v| !(1..=100).contains(&v))
        {
            return Err(invalid(
                "Tidal playback requires a source mode and cycles 1-100",
            ));
        }
        self.set_tidal_property_with_cycles(id, selection, property, Some(mode), cached_cycles)
    }
    fn set_tidal_property_with_cycles(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: TidalProperty,
        value: Option<u32>,
        cached_cycles: Option<u32>,
    ) -> Result<bool> {
        property.validate(value)?;
        if matches!(property, TidalProperty::Start | TidalProperty::End)
            && self.tidal_selection_value(id, selection, property)?
                == SelectedTidalValue::Uniform(value)
        {
            return Ok(false);
        }
        let params = self.tidal_params(id, selection)?;
        if matches!(property, TidalProperty::Color | TidalProperty::Color2) {
            for params in params {
                if params
                    .get("randomColor")
                    .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Err(invalid(
                        "turn off Random Colour before editing the Tidal colour",
                    ));
                }
            }
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if matches!(
            property,
            TidalProperty::Color | TidalProperty::Color2 | TidalProperty::Random
        ) {
            for params in self.tidal_params(id, selection)? {
                if params.get("colorStops").is_some() {
                    return Err(invalid("Imported Tidal gradient overrides colour pickers; retained without replacement"));
                }
            }
        }
        if matches!(
            property,
            TidalProperty::Start | TidalProperty::End | TidalProperty::Cycles
        ) {
            for params in self.tidal_params(id, selection)? {
                let start = if property == TidalProperty::Start {
                    value.unwrap()
                } else {
                    match params
                        .get("playbackStart")
                        .map(|r| serde_json::from_str::<String>(r.get()))
                        .transpose()
                        .map_err(invalid)?
                        .as_deref()
                    {
                        None | Some("random") => 0,
                        Some("onPress") => 1,
                        Some("onSelectedKeys") => 2,
                        _ => return Err(invalid("unknown Tidal playback start")),
                    }
                };
                let end = if property == TidalProperty::End {
                    value.unwrap()
                } else {
                    match params
                        .get("playbackEnd")
                        .map(|r| serde_json::from_str::<String>(r.get()))
                        .transpose()
                        .map_err(invalid)?
                        .as_deref()
                    {
                        None | Some("never") => 0,
                        Some("after") => 1,
                        Some("toggle") => 2,
                        _ => return Err(invalid("unknown Tidal playback end")),
                    }
                };
                if property != TidalProperty::Start && start == 0 && end == 2 {
                    return Err(invalid("Tidal Toggle requires a key-triggered start"));
                }
                if property == TidalProperty::Cycles && end != 1 {
                    return Err(invalid("Tidal cycles require After playback"));
                }
            }
        }
        let projection = merged(&item, None, true)?;
        self.edit_regions(
            id,
            selection,
            RegionEdit::Tidal(
                Edit {
                    property,
                    value,
                    cached_cycles,
                },
                &projection,
            ),
        )
    }
}
/// Layer-wide source canvas settings. Canvas points use a 40-pixel grid; Y is offset by two rows.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TidalLayout {
    pub x: u16,
    pub y: u16,
    pub inward: bool,
}

impl Draft {
    pub fn tidal_layout(&self, id: &str) -> Result<Option<TidalLayout>> {
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let Some(raw) = item.get("params") else {
            return Ok(None);
        };
        let params = Object::parse(raw)?;
        let (Some(x), Some(y), Some(inward)) = (
            params.get("centerPointX2"),
            params.get("centerPointY2"),
            params.get("counterclockwise"),
        ) else {
            return Ok(None);
        };
        Ok(Some(TidalLayout {
            x: serde_json::from_str(x.get()).map_err(invalid)?,
            y: serde_json::from_str(y.get()).map_err(invalid)?,
            inward: serde_json::from_str(inward.get()).map_err(invalid)?,
        }))
    }
    /// Direction is a layer-wide leaf; toggling must not rewrite imported center coordinates.
    pub fn set_tidal_direction(&mut self, id: &str, inward: bool) -> Result<bool> {
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let before = item.get("params");
        let mut params = before
            .map(Object::parse)
            .transpose()?
            .unwrap_or(Object(vec![]));
        params.set("counterclockwise", Some(&encode(&inward)?));
        let after = encode(&params)?;
        if before.is_some_and(|b| b.get() == after.get()) {
            return Ok(false);
        }
        self.commit(vec![Change {
            id: id.into(),
            field: "params",
            before: before.map(ToOwned::to_owned),
            after,
        }])
    }
    pub fn set_tidal_layout(&mut self, id: &str, layout: TidalLayout) -> Result<bool> {
        if layout.x > 10000 || layout.y > 10000 {
            return Err(invalid(
                "Tidal center exceeds the source 10000-pixel canvas",
            ));
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let before = item.get("params");
        let mut params = before
            .map(Object::parse)
            .transpose()?
            .unwrap_or(Object(vec![]));
        for (field, raw) in [
            ("centerPointX2", encode(&layout.x)?),
            ("centerPointY2", encode(&layout.y)?),
            (
                "centerPointX",
                encode(&((f64::from(layout.x) / 40.0 + 0.5).floor() as i32))?,
            ),
            (
                "centerPointY",
                encode(&((f64::from(layout.y) / 40.0 + 0.5).floor() as i32 - 2))?,
            ),
            ("counterclockwise", encode(&layout.inward)?),
        ] {
            params.set(field, Some(&raw));
        }
        let after = encode(&params)?;
        if before.is_some_and(|b| b.get() == after.get()) {
            return Ok(false);
        }
        self.commit(vec![Change {
            id: id.into(),
            field: "params",
            before: before.map(ToOwned::to_owned),
            after,
        }])
    }
}
#[path = "tidal_descriptor.rs"]
mod descriptor;
pub use descriptor::TidalDescriptor;
#[cfg(test)]
#[path = "tidal_tests.rs"]
mod tests;
