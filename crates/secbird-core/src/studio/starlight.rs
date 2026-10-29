//! Source Studio Starlight settings; playback admission remains separate.
use super::*;

pub const STARLIGHT_DURATIONS: [u32; 3] = [3000, 1700, 500];
/// Prepared source AddEffect7 data. This is not a playable animation or a
/// vendor ABI, and producing it never admits preview or physical output.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct StarlightDescriptor {
    pub mode: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_stops: Option<Vec<Box<RawValue>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    pub max_star: Option<u32>,
    pub regen: u8,
    pub cycles: u8,
    pub pause: u8,
}
impl StarlightDescriptor {
    fn prepare(params: &Object) -> Result<Self> {
        let random = params
            .get("randomColor")
            .map(|raw| serde_json::from_str::<bool>(raw.get()).map_err(invalid))
            .transpose()?
            .unwrap_or(false);
        let duration = params
            .get("duration")
            .map(|raw| serde_json::from_str::<u32>(raw.get()).map_err(invalid))
            .transpose()?;
        let density = params
            .get("density")
            .map(|raw| serde_json::from_str::<u32>(raw.get()).map_err(invalid))
            .transpose()?;
        if density.is_some_and(|value| !(1..=10).contains(&value)) {
            return Err(invalid(
                "Starlight preparation supports integer density1..10",
            ));
        }
        // Source normalizes the saved gradient before Random Colour replaces
        // the outgoing stops. Retain order: the converter's sorted copy is
        // discarded when case7 assigns its descriptor.
        let mut stops = params.get("colorStops").map(array).transpose()?;
        if let Some(stops) = &mut stops {
            if stops.len() > 10 {
                return Err(invalid(
                    "Starlight preparation supports at most ten saved stops",
                ));
            }
            let mut has_zero = false;
            let mut has_hundred = false;
            for raw in stops.iter_mut() {
                let mut stop = Object::parse(raw)?;
                let position: u8 = serde_json::from_str(
                    stop.get("Stop")
                        .ok_or_else(|| invalid("missing Starlight Stop"))?
                        .get(),
                )
                .map_err(invalid)?;
                if position > 100 {
                    return Err(invalid("Starlight Stop must be0..100"));
                }
                has_zero |= position == 0;
                has_hundred |= position == 100;
                if let Some(raw) = stop.get("Color") {
                    let color: u32 = serde_json::from_str(raw.get()).map_err(invalid)?;
                    if color > 0xffffff && color != 0x64000000 {
                        return Err(invalid("Starlight Color must be RGB24 or None"));
                    }
                } else {
                    stop.set("Color", Some(&encode(&0x64000000u32)?));
                }
                *raw = encode(&stop)?;
            }
            if !stops.is_empty() {
                if !has_zero {
                    let first = Object::parse(&stops[0])?;
                    let mut anchor = Object(vec![]);
                    anchor.set("Stop", Some(&encode(&0)?));
                    anchor.set("Color", first.get("Color"));
                    stops.insert(0, encode(&anchor)?);
                }
                if !has_hundred {
                    let last = Object::parse(stops.last().unwrap())?;
                    let mut anchor = Object(vec![]);
                    anchor.set("Stop", Some(&encode(&100)?));
                    anchor.set("Color", last.get("Color"));
                    stops.push(encode(&anchor)?);
                }
            }
        }
        Ok(Self {
            mode: u8::from(random),
            color_stops: if random { Some(vec![]) } else { stops },
            duration,
            // Missing density becomes NaN in JS and null at its JSON boundary.
            max_star: density.map(|density| density * 10),
            regen: 10,
            cycles: 1,
            pause: 0,
        })
    }
}

impl Draft {
    /// Inspect each declared region after layer/region parameter inheritance.
    /// No UI defaults are invented for missing runtime parameters. Hidden/offline
    /// regions are inspected too; this is not playback or compatibility admission.
    /// The draft and its history remain untouched, including opaque numeric text.
    pub fn starlight_region_descriptors(&self, id: &str) -> Result<Vec<StarlightDescriptor>> {
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let descriptor = StarlightDescriptor::prepare(&merged(&item, Some(&region), false)?)?;
            bytes = bytes
                .checked_add(encode(&descriptor)?.get().len())
                .ok_or_else(|| invalid("Starlight descriptor budget overflow"))?;
            if bytes > MAX_DOCUMENT_BYTES {
                return Err(invalid("Starlight descriptors exceed8MiB"));
            }
            result.push(descriptor);
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarlightProperty {
    Duration,
    Density,
    Random,
}
impl StarlightProperty {
    fn field(self) -> &'static str {
        match self {
            Self::Duration => "duration",
            Self::Density => "density",
            Self::Random => "randomColor",
        }
    }
    fn validate(self, value: u32) -> Result<()> {
        if match self {
            Self::Duration => STARLIGHT_DURATIONS.contains(&value),
            Self::Density => (1..=10).contains(&value),
            Self::Random => value <= 1,
        } {
            Ok(())
        } else {
            Err(invalid(
                "Starlight requires a source duration, density1..10, or a boolean random setting",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedStarlightValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "starlight" || item.get("value").is_some_and(|v| v.get() != "7") {
        return Err(invalid("selected layer is not Studio Starlight7"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut result = if defaults {
        Object::parse(&encode(
            &serde_json::json!({"duration":1700,"density":1,"randomColor":false,"colorStopsCustom":false,"colorStops":wave_preset(0).unwrap()}),
        )?)?
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
#[derive(Clone, Copy)]
pub(super) enum Edit {
    Property(StarlightProperty, u32),
    Preset(usize),
    Stop(StopEdit),
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object, projection: &Object, new: bool) -> Result<()> {
        if new {
            for (key, value) in &projection.0 {
                params.set(key, Some(value));
            }
        }
        match self {
            Self::Property(property, value) => {
                let raw = if property == StarlightProperty::Random {
                    encode(&(value != 0))?
                } else {
                    encode(&value)?
                };
                params.set(property.field(), Some(&raw));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &wave_preset(index).ok_or_else(|| invalid("unknown Starlight preset"))?,
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
        }
        Ok(())
    }
}
impl Draft {
    fn starlight_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
                            return Err(invalid("overlapping selected Starlight memberships"));
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
    pub fn starlight_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: StarlightProperty,
    ) -> Result<SelectedStarlightValue> {
        let values = self.starlight_params(id, selection)?;
        let read = |params: &Object| -> Result<Option<u32>> {
            let raw = params.get(property.field());
            if property == StarlightProperty::Random {
                return raw
                    .map(|v| {
                        serde_json::from_str::<bool>(v.get())
                            .map(u32::from)
                            .map_err(invalid)
                    })
                    .transpose();
            }
            let value = raw
                .map(|v| serde_json::from_str::<u32>(v.get()).map_err(invalid))
                .transpose()?;
            Ok(if property == StarlightProperty::Duration {
                Some(
                    value
                        .filter(|v| STARLIGHT_DURATIONS.contains(v))
                        .unwrap_or(1700),
                )
            } else {
                value
            })
        };
        let Some(first) = values.first() else {
            return Ok(SelectedStarlightValue::Empty);
        };
        let first = read(first)?;
        for params in &values[1..] {
            if read(params)? != first {
                return Ok(SelectedStarlightValue::Mixed);
            }
        }
        Ok(SelectedStarlightValue::Uniform(first))
    }
    pub fn starlight_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let values = self.starlight_params(id, selection)?;
        let Some(first) = values.first() else {
            return Ok(SelectedGradient::Empty);
        };
        let first = spectrum::gradient(first)?;
        for params in &values[1..] {
            if spectrum::gradient(params)? != first {
                return Ok(SelectedGradient::Mixed);
            }
        }
        Ok(SelectedGradient::Uniform(first))
    }
    fn edit_starlight(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        let params = self.starlight_params(id, selection)?;
        if !matches!(edit, Edit::Property(..)) {
            for params in &params {
                if params
                    .get("randomColor")
                    .map(|v| serde_json::from_str::<bool>(v.get()).map_err(invalid))
                    .transpose()?
                    .unwrap_or(false)
                {
                    return Err(invalid(
                        "turn off Random Colour before editing the Starlight gradient",
                    ));
                }
            }
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Starlight(edit, &projection))
    }
    pub fn set_starlight_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: StarlightProperty,
        value: u32,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_starlight(id, selection, Edit::Property(property, value))
    }
    pub fn set_starlight_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        wave_preset(index).ok_or_else(|| invalid("unknown Starlight preset"))?;
        self.edit_starlight(id, selection, Edit::Preset(index))
    }
    pub fn edit_starlight_stop(
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
            self.starlight_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid("choose one uniform Starlight gradient first"));
        }
        self.edit_starlight(id, selection, Edit::Stop(edit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Starlight","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"s","type":"layer","name":"starlight","value":7,"title":"STARLIGHT","params":{"future":9.9900e+100},"regions":[{"params":{"duration":9999,"density":4,"randomColor":false,"colorStops":[{"Stop":0,"Color":123,"opaque":1.2300e-7},{"Stop":100}],"future":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
    fn keys(cols: &[u16]) -> Vec<LedSelection> {
        cols.iter()
            .map(|&col| LedSelection {
                device: "k".into(),
                row: 0,
                col,
            })
            .collect()
    }
    #[test]
    fn region_descriptors_match69_independent_shipped_converter_replays() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-starlight-descriptors.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["source_module_sha256"],
            "008bf7b8aac4e86f47dd0c4e498402949c384d640553ca08d9c2424ad3527bda"
        );
        assert_eq!(fixture["native_calls"], 0);
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 69);
        for case in fixture["cases"].as_array().unwrap() {
            let mut profile: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
            profile["layers"][0]["params"] = serde_json::json!({});
            profile["layers"][0]["regions"][0]["params"] = case["params"].clone();
            let raw = profile.to_string();
            let draft = Draft::open(&raw).unwrap();
            let snapshot = draft.read_snapshot();
            let descriptors = draft.starlight_region_descriptors("s").unwrap();
            assert_eq!(descriptors.len(), 1);
            assert_eq!(
                serde_json::to_value(&descriptors[0]).unwrap(),
                case["descriptor"],
                "{}",
                case["name"]
            );
            assert_eq!(draft.json(), raw);
            assert!(draft.matches_snapshot(&snapshot));
            assert!(!draft.is_dirty());
        }
    }
    #[test]
    fn descriptor_inheritance_preserves_missing_values_order_and_opaque_stop_text() {
        let raw = r#"{"id":"p","title":"Test","devices":[],"layers":[{"id":"s","title":"Stars","type":"layer","name":"starlight","value":7,"visible":false,"params":{"duration":9999,"density":3,"randomColor":true,"colorStops":[{"Stop":80,"Color":1,"opaque":9.9900e+100},{"Stop":20}]},"regions":[{"devices":[{"id":"offline","leds":[{"row":0,"col":1}]}],"params":{"randomColor":false}},{"devices":[],"params":{"density":10,"duration":500}},{"devices":[]}]}]}"#;
        let draft = Draft::open(raw).unwrap();
        let descriptors = draft.starlight_region_descriptors("s").unwrap();
        assert_eq!(descriptors.len(), 3);
        assert_eq!(descriptors[0].max_star, Some(30));
        assert_eq!(descriptors[0].duration, Some(9999));
        let stops = descriptors[0].color_stops.as_ref().unwrap();
        assert_eq!(stops.len(), 4);
        assert!(stops[1].get().contains("9.9900e+100"));
        assert_eq!(
            Object::parse(&stops[1]).unwrap().get("Stop").unwrap().get(),
            "80"
        );
        assert_eq!(
            Object::parse(&stops[2]).unwrap().get("Stop").unwrap().get(),
            "20"
        );
        assert_eq!(descriptors[1].max_star, Some(100));
        assert_eq!(descriptors[1].duration, Some(500));
        assert_eq!(descriptors[1].mode, 1);
        assert!(descriptors[1].color_stops.as_ref().unwrap().is_empty());
        assert_eq!(descriptors[2].max_star, Some(30));
        assert_eq!(draft.json(), raw);
        assert!(!draft.is_dirty());
        assert!(draft.starlight_region_descriptors("missing").is_err());
        let conflict = Draft::open(&raw.replace("\"value\":7", "\"value\":8")).unwrap();
        assert!(conflict.starlight_region_descriptors("s").is_err());
    }
    #[test]
    fn descriptor_bounds_and_malformed_fields_refuse_without_editing() {
        for params in [
            serde_json::json!({"density":0}),
            serde_json::json!({"density":11}),
            serde_json::json!({"density":1.5}),
            serde_json::json!({"duration":-1}),
            serde_json::json!({"randomColor":1}),
            serde_json::json!({"colorStops":[{"Stop":101}]}),
            serde_json::json!({"colorStops":[{"Stop":0,"Color":-1}]}),
            serde_json::json!({"colorStops":[{"Stop":0,"Color":0xffffffffu32}]}),
            serde_json::json!({"colorStops":[{"Color":0}]}),
            serde_json::json!({"colorStops":vec![serde_json::json!({"Stop":0});11]}),
        ] {
            let mut profile: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
            profile["layers"][0]["regions"][0]["params"] = params;
            let raw = profile.to_string();
            let draft = Draft::open(&raw).unwrap();
            assert!(draft.starlight_region_descriptors("s").is_err());
            assert_eq!(draft.json(), raw);
            assert!(!draft.is_dirty());
        }
        let profile = serde_json::json!({"id":"p","title":"Test","devices":[],"layers":[{
            "id":"s","title":"Stars","type":"layer","name":"starlight","value":7,
            "params":{"colorStops":[{"Stop":0,"future":"x".repeat(32768)},{"Stop":100}]},
            "regions": vec![serde_json::json!({"devices":[]}); 300]
        }]});
        let draft = Draft::open(&profile.to_string()).unwrap();
        assert!(draft
            .starlight_region_descriptors("s")
            .unwrap_err()
            .to_string()
            .contains("8MiB"));
        assert!(!draft.is_dirty());
    }
    #[test]
    fn defaults_match_independently_extracted_source_constants() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/studio-starlight-settings.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["source_sha256"],
            "109b483f129044e1c7c17b9fcae0e30fa775885667976807cf2c94bd1c696d19"
        );
        assert_eq!(
            serde_json::to_value(STARLIGHT_DURATIONS).unwrap(),
            fixture["durations"]
        );
        let mut draft = Draft::open(PROFILE).unwrap();
        draft
            .set_starlight_property(
                "s",
                &keys(&[3]),
                StarlightProperty::Density,
                fixture["default_density"].as_u64().unwrap() as u32,
            )
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[3]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(
                fixture["default_duration"].as_u64().unwrap() as u32
            ))
        );
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[3]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            serde_json::to_value(stops).unwrap(),
            fixture["default_gradient"]
        );
    }
    #[test]
    fn source_settings_partition_and_history_keep_untouched_raw_data() {
        let mut draft = Draft::open(PROFILE).unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(1700))
        );
        draft
            .set_starlight_property("s", &keys(&[1, 3]), StarlightProperty::Density, 10)
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1, 3]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(10))
        );
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[2]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(4))
        );
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1, 2]), StarlightProperty::Density)
                .unwrap(),
            SelectedStarlightValue::Mixed
        );
        assert!(draft.json().contains("9.9900e+100"));
        assert!(draft.json().contains("1.2300e-7"));
        assert!(draft.json().contains("9007199254740993123456789"));
        assert!(draft.json().contains("9999")); // an unrelated edit preserves unsupported imported duration
        assert_eq!(
            draft
                .starlight_selection_gradient("s", &keys(&[3]))
                .unwrap(),
            SelectedGradient::Uniform(Some(wave_preset(0).unwrap()))
        );
        draft.undo().unwrap();
        assert_eq!(draft.json(), PROFILE);
        draft.redo().unwrap();
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Random, 1)
            .unwrap();
        let before = draft.json().to_owned();
        assert!(draft.set_starlight_preset("s", &keys(&[1]), 4).is_err());
        assert!(draft
            .edit_starlight_stop("s", &keys(&[1]), StopEdit::Delete { index: 0 })
            .is_err());
        assert_eq!(draft.json(), before);
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Random, 0)
            .unwrap();
        draft
            .edit_starlight_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 20,
                    color: None,
                },
            )
            .unwrap();
        assert!(draft.json().contains("1.2300e-7"));
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[1]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            stops[0],
            GradientStop {
                position: 20,
                color: None
            }
        );
    }
    #[test]
    fn invalid_properties_id_and_inventory_refuse_without_dirtying() {
        let mut draft = Draft::open(PROFILE).unwrap();
        for (p, v) in [
            (StarlightProperty::Density, 0),
            (StarlightProperty::Density, 11),
            (StarlightProperty::Duration, 2000),
            (StarlightProperty::Random, 2),
        ] {
            assert!(draft
                .set_starlight_property("s", &keys(&[1]), p, v)
                .is_err());
        }
        assert!(draft
            .set_starlight_property("s", &keys(&[99]), StarlightProperty::Density, 1)
            .is_err());
        assert!(draft.set_starlight_preset("s", &keys(&[1]), 5).is_err());
        assert_eq!(draft.json(), PROFILE);
        assert!(!draft.is_dirty());
        let before = PROFILE.replace("\"value\":7", "\"value\":8");
        let mut draft = Draft::open(&before).unwrap();
        assert!(draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Density, 1)
            .is_err());
        assert_eq!(draft.json(), before);
    }
    #[test]
    fn inherited_gradient_and_scalar_edits_remain_independent() {
        let mut value: serde_json::Value = serde_json::from_str(PROFILE).unwrap();
        value["layers"][0]["params"]["colorStops"] =
            serde_json::json!([{"Stop":0,"Color":1},{"Stop":100,"Color":2}]);
        value["layers"][0]["regions"][0]["params"]
            .as_object_mut()
            .unwrap()
            .remove("colorStops");
        let mut draft = Draft::open(&value.to_string()).unwrap();
        draft
            .edit_starlight_stop(
                "s",
                &keys(&[1]),
                StopEdit::Set {
                    index: 0,
                    position: 0,
                    color: Some(0x112233),
                },
            )
            .unwrap();
        let out: serde_json::Value = serde_json::from_str(draft.json()).unwrap();
        assert_eq!(out["layers"][0]["params"], value["layers"][0]["params"]);
        draft
            .set_starlight_property("s", &keys(&[1]), StarlightProperty::Duration, 500)
            .unwrap();
        assert_eq!(
            draft
                .starlight_selection_value("s", &keys(&[1]), StarlightProperty::Duration)
                .unwrap(),
            SelectedStarlightValue::Uniform(Some(500))
        );
        let SelectedGradient::Uniform(Some(stops)) = draft
            .starlight_selection_gradient("s", &keys(&[1]))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(stops[0].color, Some(0x112233));
    }
}
