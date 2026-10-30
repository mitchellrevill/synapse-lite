//! Fixed Breathing gradient authoring over the recovered nine-stop converter.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathingEdit {
    Random(bool),
    ResetColors,
    Start(&'static str),
    End(&'static str),
    Cycles(i64),
    Playback {
        start: Option<&'static str>,
        end: &'static str,
        cycles: i64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreathingSettings {
    pub random: bool,
    pub start: String,
    pub end: String,
    pub cycles: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectedBreathing {
    Empty,
    Mixed,
    Uniform(BreathingSettings),
}
fn settings(params: &Object) -> Result<BreathingSettings> {
    fn value<T: serde::de::DeserializeOwned>(params: &Object, key: &str, default: T) -> Result<T> {
        params
            .get(key)
            .map(|raw| serde_json::from_str(raw.get()).map_err(invalid))
            .transpose()
            .map(|v| v.unwrap_or(default))
    }
    Ok(BreathingSettings {
        random: value(params, "randomColor", false)?,
        start: value(params, "playbackStart", "random".into())?,
        end: value(params, "playbackEnd", "never".into())?,
        cycles: value(params, "cycles", -1)?,
    })
}
pub(super) fn apply_settings(
    params: &mut Object,
    defaults: &Object,
    edit: BreathingEdit,
    new: bool,
) -> Result<()> {
    if new {
        for (name, value) in &defaults.0 {
            params.set(name, Some(value));
        }
    }
    match edit {
        BreathingEdit::Random(random) => params.set("randomColor", Some(&encode(&random)?)),
        BreathingEdit::ResetColors => {
            // null is the converter's non-array branch. It masks an inherited
            // gradient without deleting that layer's original opaque stop data.
            params.set("colorStops", Some(&encode(&Option::<u32>::None)?));
            params.set("colorStopsCustom", Some(&encode(&false)?));
            params.set("randomColor", Some(&encode(&false)?));
        }
        BreathingEdit::Start(value) => {
            params.set("playbackStart", Some(&encode(&value)?));
            let automatic = value == "random";
            let end = if automatic { "never" } else { "after" };
            let cycles = if automatic { -1 } else { 1 };
            params.set("playbackEnd", Some(&encode(&end)?));
            params.set("cycles", Some(&encode(&cycles)?));
        }
        BreathingEdit::End(value) => {
            params.set("playbackEnd", Some(&encode(&value)?));
            let cycles = if value == "after" { 1 } else { -1 };
            params.set("cycles", Some(&encode(&cycles)?));
        }
        BreathingEdit::Cycles(value) => {
            params.set("cycles", Some(&encode(&value)?));
            let end = if value > 0 { "after" } else { "never" };
            params.set("playbackEnd", Some(&encode(&end)?));
        }
        BreathingEdit::Playback { start, end, cycles } => {
            if let Some(start) = start {
                params.set("playbackStart", Some(&encode(&start)?));
            }
            params.set("playbackEnd", Some(&encode(&end)?));
            params.set("cycles", Some(&encode(&cycles)?));
        }
    }
    Ok(())
}

pub(super) fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "breathing" || item.get("value").is_some_and(|v| v.get() != "5") {
        return Err(invalid("selected layer is not Studio Breathing5"));
    }
    Ok(())
}
fn authorable(params: &Object) -> Result<()> {
    let random = params
        .get("randomColor")
        .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
        .transpose()?
        .unwrap_or(false);
    if random {
        return Err(invalid(
            "random Breathing gradient editing is unavailable; settings were preserved",
        ));
    }
    Ok(())
}
fn stops(params: &Object) -> Result<Vec<GradientStop>> {
    authorable(params)?;
    if params
        .get("colorStops")
        .is_some_and(|raw| raw.get() != "null")
    {
        if let Some(stops) = spectrum::gradient(params)? {
            return Ok(stops);
        }
    }
    let color = |field: &str| -> Result<Option<u32>> {
        params
            .get(field)
            .map(|v| serde_json::from_str(v.get()).map_err(invalid))
            .transpose()
    };
    let second = color("color2")?;
    let first = color("color")?.or(second);
    Ok([
        (0, Some(0)),
        (12, Some(0)),
        (25, first),
        (38, first),
        (50, Some(0)),
        (62, Some(0)),
        (75, second),
        (88, second),
        (100, Some(0)),
    ]
    .into_iter()
    .map(|(position, color)| GradientStop {
        position,
        color: color.filter(|c| *c != 0x64000000),
    })
    .collect())
}
pub(super) fn merged(item: &Object, region: Option<&Object>) -> Result<Object> {
    let mut params = Object(vec![]);
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
fn validate(stops: &[GradientStop]) -> Result<()> {
    if stops.len() < 2
        || stops.len() > 10
        || stops.windows(2).any(|p| p[0].position >= p[1].position)
    {
        return Err(invalid("Breathing needs two to ten strictly ordered stops"));
    }
    for stop in stops {
        spectrum::validate_stop(stop)?;
    }
    // Endpoint expansion and native palette admission must succeed before commit.
    crate::breathing_lighting::BreathingCycle::studio(None, None, Some(stops), Some(6000))?;
    Ok(())
}
pub(super) fn apply(params: &mut Object, defaults: &Object, edit: StopEdit) -> Result<()> {
    let mut combined = Object::parse(&encode(defaults)?)?;
    for (name, value) in &params.0 {
        combined.set(name, Some(value));
    }
    let effective = stops(&combined)?;
    validate(&effective)?;
    if params
        .get("colorStops")
        .is_none_or(|raw| raw.get() == "null")
    {
        if let Some(raw) = combined.get("colorStops").filter(|raw| raw.get() != "null") {
            params.set("colorStops", Some(raw));
        } else {
            params.set("colorStops", Some(&encode(&effective)?));
        }
    }
    spectrum::edit_stop(params, edit, 10)?;
    validate(&stops(params)?)
}
impl Draft {
    pub fn breathing_selection_settings(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedBreathing> {
        let params = self.breathing_gradient_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedBreathing::Empty);
        };
        let first = settings(first)?;
        for params in &params[1..] {
            if settings(params)? != first {
                return Ok(SelectedBreathing::Mixed);
            }
        }
        Ok(SelectedBreathing::Uniform(first))
    }
    pub fn edit_breathing(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        edit: BreathingEdit,
    ) -> Result<bool> {
        match edit {
            BreathingEdit::Start(value)
                if !["random", "onPress", "onSelectedKeys"].contains(&value) =>
            {
                return Err(invalid("unknown Breathing start mode"))
            }
            BreathingEdit::End(value) if !["never", "after", "toggle"].contains(&value) => {
                return Err(invalid("unknown Breathing end mode"))
            }
            BreathingEdit::Cycles(value) if value != -1 && !(1..=100).contains(&value) => {
                return Err(invalid("invalid Breathing repeat count"))
            }
            BreathingEdit::Playback { start, end, cycles }
                if start.is_some_and(|v| !["random", "onPress", "onSelectedKeys"].contains(&v))
                    || !["never", "after", "toggle"].contains(&end)
                    || (end == "after" && !(1..=100).contains(&cycles))
                    || (end != "after" && cycles != -1)
                    || (start == Some("random") && end == "toggle") =>
            {
                return Err(invalid("invalid Breathing playback settings"))
            }
            _ => {}
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        if matches!(
            edit,
            BreathingEdit::End("toggle")
                | BreathingEdit::Playback {
                    start: None,
                    end: "toggle",
                    ..
                }
        ) {
            for params in self.breathing_gradient_params(id, selection)? {
                if settings(&params)?.start == "random" {
                    return Err(invalid("Toggle needs an input start mode"));
                }
            }
        }
        let defaults = merged(&item, None)?;
        self.edit_regions(id, selection, RegionEdit::Breathing(edit, &defaults))
    }
    fn breathing_gradient_params(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<Vec<Object>> {
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
                result.push(merged(&item, Some(&region))?);
            }
        }
        if seen.len() < selection.len() {
            result.push(merged(&item, None)?);
        }
        Ok(result)
    }
    pub fn breathing_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let params = self.breathing_gradient_params(id, selection)?;
        let Some(first) = params.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let first = stops(first)?;
        for p in &params[1..] {
            if stops(p)? != first {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(Some(first)))
    }
    pub fn edit_breathing_stop(
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
            StopEdit::Delete { .. } => {}
        }
        if !matches!(
            self.breathing_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid(
                "select one uniform Breathing gradient before editing",
            ));
        }
        // Materialize inherited stops per selected membership without changing
        // duration, playback flags, untouched regions or unknown raw leaves.
        let item = layer(self, id, None)?;
        let defaults = merged(&item, None)?;
        self.edit_regions(
            id,
            selection,
            RegionEdit::BreathingGradient(edit, &defaults),
        )
    }
}
