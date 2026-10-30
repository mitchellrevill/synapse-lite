//! Source-compatible Ripple regional authoring. This module does not admit playback.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RippleProperty {
    Speed,
    Width,
}
impl RippleProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Speed => "speed",
            Self::Width => "width",
        }
    }
    fn validate(self, value: u32) -> Result<()> {
        let valid = match self {
            Self::Speed => (1..=50).contains(&value),
            Self::Width => (100..=400).contains(&value) && value % 100 == 0,
        };
        if valid {
            Ok(())
        } else {
            Err(invalid(
                "Ripple needs speed1..50 and width100/200/300/400 percent",
            ))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedRippleValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RippleStart {
    #[serde(rename = "random")]
    Random,
    #[serde(rename = "onPress")]
    OnPress,
    #[serde(rename = "onSelectedKeys")]
    OnSelectedKeys,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RippleEnd {
    #[serde(rename = "never")]
    Never,
    #[serde(rename = "after")]
    After,
    #[serde(rename = "toggle")]
    Toggle,
}
/// The three fields are edited together so a mode switch cannot leave a stale count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RipplePlayback {
    #[serde(rename = "playbackStart")]
    pub start: RippleStart,
    #[serde(rename = "playbackEnd")]
    pub end: RippleEnd,
    pub cycles: i32,
}
impl RipplePlayback {
    pub fn validate(self) -> Result<()> {
        if (self.start == RippleStart::Random && self.end == RippleEnd::Toggle)
            || (self.end == RippleEnd::After && !(1..=100).contains(&self.cycles))
            || (self.end != RippleEnd::After && self.cycles != -1)
        {
            return Err(invalid(
                "unsupported Ripple playback combination or repeat count",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedRipplePlayback {
    Empty,
    Uniform(Option<RipplePlayback>),
    Mixed,
}

/// Module6257.dp uses the same five patterns for Ripple and Wave.
pub fn ripple_preset(index: usize) -> Option<Vec<GradientStop>> {
    wave_preset(index)
}

pub(super) fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "ripple" || item.get("value").is_some_and(|v| v.get() != "1") {
        return Err(invalid("selected layer is not Studio Ripple1"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut params = if defaults {
        Object::parse(&encode(&serde_json::json!({"speed":10,"width":300,
            "playbackStart":"onPress","playbackEnd":"after","cycles":1,
            "colorStopsCustom":false,"colorStops":ripple_preset(0).unwrap()}))?)?
    } else {
        Object(vec![])
    };
    if let Some(raw) = item.get("params") {
        for (name, value) in Object::parse(raw)?.0 {
            params.set(&name, Some(&value));
        }
    }
    if let Some(region) = region {
        for (name, value) in Object::parse(
            region
                .get("params")
                .ok_or_else(|| invalid("region has no params"))?,
        )?
        .0
        {
            params.set(&name, Some(&value));
        }
    }
    Ok(params)
}
#[derive(Clone, Copy)]
pub(super) enum Edit {
    Property(RippleProperty, u32),
    Motion(u32, u32),
    Preset(usize),
    Stop(StopEdit),
    Playback(RipplePlayback),
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (name, value) in &projection.0 {
                params.set(name, Some(value));
            }
        }
        match self {
            Self::Property(property, value) => params.set(property.field(), Some(&encode(&value)?)),
            Self::Motion(speed, width) => {
                params.set("speed", Some(&encode(&speed)?));
                params.set("width", Some(&encode(&width)?));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &ripple_preset(index).ok_or_else(|| invalid("unknown Ripple preset"))?,
                    )?),
                );
                params.set("colorStopsCustom", Some(&encode(&false)?));
            }
            Self::Stop(edit) => {
                if params.get("colorStops").is_none() {
                    params.set("colorStops", projection.get("colorStops"));
                }
                spectrum::edit_stop(params, edit, 7)?;
            }
            Self::Playback(value) => {
                params.set("playbackStart", Some(&encode(&value.start)?));
                params.set("playbackEnd", Some(&encode(&value.end)?));
                params.set("cycles", Some(&encode(&value.cycles)?));
            }
        }
        Ok(())
    }
}
impl Draft {
    fn ripple_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                let device_id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &device_id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected region memberships"));
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
    pub fn ripple_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: RippleProperty,
    ) -> Result<SelectedRippleValue> {
        let params = self.ripple_params(id, selection)?;
        let value = |p: &Object| -> Result<Option<u32>> {
            p.get(property.field())
                .map(|v| serde_json::from_str(v.get()).map_err(invalid))
                .transpose()
        };
        let Some(first) = params.first() else {
            return Ok(SelectedRippleValue::Empty);
        };
        let first = value(first)?;
        for params in &params[1..] {
            if value(params)? != first {
                return Ok(SelectedRippleValue::Mixed);
            }
        }
        Ok(SelectedRippleValue::Uniform(first))
    }
    fn edit_ripple(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        // Detect invalid inventory and overlapping selected memberships before any edit.
        self.ripple_params(id, selection)?;
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Ripple(edit, &projection))
    }
    pub fn ripple_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let params = self.ripple_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let first = spectrum::gradient(first)?;
        for params in &params[1..] {
            if spectrum::gradient(params)? != first {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(first))
    }
    pub fn edit_ripple_stop(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        edit: StopEdit,
    ) -> Result<bool> {
        match edit {
            StopEdit::Set {
                position, color, ..
            }
            | StopEdit::Insert { position, color } => {
                spectrum::validate_stop(&GradientStop { position, color })?
            }
            StopEdit::Move { position, .. } => spectrum::validate_stop(&GradientStop { position, color: None })?,
            StopEdit::Delete { .. } => {}
        }
        if !matches!(
            self.ripple_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid(
                "choose one uniform Ripple gradient before editing stops",
            ));
        }
        self.edit_ripple(id, selection, Edit::Stop(edit))
    }
    pub fn set_ripple_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: RippleProperty,
        value: u32,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_ripple(id, selection, Edit::Property(property, value))
    }
    pub fn set_ripple_motion(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        speed: u32,
        width: u32,
    ) -> Result<bool> {
        RippleProperty::Speed.validate(speed)?;
        RippleProperty::Width.validate(width)?;
        self.edit_ripple(id, selection, Edit::Motion(speed, width))
    }
    pub fn set_ripple_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        ripple_preset(index).ok_or_else(|| invalid("unknown Ripple preset"))?;
        self.edit_ripple(id, selection, Edit::Preset(index))
    }
    pub fn set_ripple_playback(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        value: RipplePlayback,
    ) -> Result<bool> {
        value.validate()?;
        self.edit_ripple(id, selection, Edit::Playback(value))
    }
    pub fn ripple_selection_playback(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedRipplePlayback> {
        let params = self.ripple_params(id, selection)?;
        let value = |p: &Object| -> Result<Option<RipplePlayback>> {
            if ["playbackStart", "playbackEnd", "cycles"]
                .iter()
                .any(|field| p.get(field).is_none())
            {
                return Ok(None);
            }
            serde_json::from_str(&serde_json::to_string(p).map_err(invalid)?)
                .map(Some)
                .map_err(invalid)
        };
        let Some(first) = params.first() else {
            return Ok(SelectedRipplePlayback::Empty);
        };
        let first = value(first)?;
        for p in &params[1..] {
            if value(p)? != first {
                return Ok(SelectedRipplePlayback::Mixed);
            }
        }
        Ok(SelectedRipplePlayback::Uniform(first))
    }
}
#[cfg(test)]
#[path = "ripple_tests.rs"]
mod tests;
