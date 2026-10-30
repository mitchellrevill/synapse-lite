//! Regional Wheel authoring. Existing leaves remain raw until explicitly edited.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelProperty {
    Speed,
}
impl WheelProperty {
    fn field(self) -> &'static str {
        "speed"
    }
    fn validate(self, value: u32) -> Result<()> {
        if value <= 360 {
            Ok(())
        } else {
            Err(invalid("Wheel speed must be0..360"))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedWheelValue {
    Empty,
    Uniform(Option<u32>),
    Mixed,
}

/// Historical Studio module6257.k shared by Wheel and Wave.
pub fn wheel_preset(index: usize) -> Option<Vec<GradientStop>> {
    let rows: &[(u8, u32)] = match index {
        0 => &[
            (0, 0xff0000),
            (26, 0xfff300),
            (42, 0x36ff48),
            (55, 0x62f1ff),
            (68, 0x000cff),
            (81, 0x8000ff),
            (100, 0xff0000),
        ],
        1 => &[(0, 0), (25, 0xffff00), (50, 0), (75, 0x00ff00), (100, 0)],
        2 => &[(0, 0), (50, 0x00ff00), (100, 0)],
        3 => &[(0, 0xff0000), (100, 0xffff00)],
        4 => &[(0, 0x00ff00), (100, 0x00ff00)],
        _ => return None,
    };
    Some(
        rows.iter()
            .map(|&(position, color)| GradientStop {
                position,
                color: Some(color),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{"id":"p","title":"Wheel","devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2},{"row":0,"col":3}]}],"layers":[{"id":"w","type":"layer","name":"wheel","value":13,"title":"Wheel","params":{"centerPointX":125,"centerPointX2":5000,"centerPointY":123,"centerPointY2":5000,"counterclockwise":false,"future":9.9900e+100},"regions":[{"params":{"speed":60,"cycles":-1,"colorStops":[{"Stop":0,"Color":1,"futureStop":1.2300e-7},{"Stop":100,"Color":2}],"opaque":9007199254740993123456789},"devices":[{"id":"k","leds":[{"row":0,"col":1},{"row":0,"col":2}]}]}]}]}"#;
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
    fn wheel_partition_geometry_gradient_history_and_raw_unknowns() {
        let mut d = Draft::open(PROFILE).unwrap();
        let snapshot = d.read_snapshot();
        assert!(d
            .set_wheel_property("w", &keys(&[1, 3]), WheelProperty::Speed, 360)
            .unwrap());
        assert_eq!(
            d.wheel_selection_value("w", &keys(&[2]), WheelProperty::Speed)
                .unwrap(),
            SelectedWheelValue::Uniform(Some(60))
        );
        assert_eq!(
            d.wheel_selection_value("w", &keys(&[1, 2]), WheelProperty::Speed)
                .unwrap(),
            SelectedWheelValue::Mixed
        );
        assert!(d.json().contains("9007199254740993123456789"));
        assert!(d.json().contains("1.2300e-7"));
        d.set_wheel_geometry(
            "w",
            WheelGeometry {
                x: 39,
                y: 0,
                counterclockwise: true,
            },
        )
        .unwrap();
        let raw: serde_json::Value = serde_json::from_str(d.json()).unwrap();
        let p = &raw["layers"][0]["params"];
        assert_eq!(p["centerPointX"], 1);
        assert_eq!(p["centerPointY"], -2);
        assert_eq!(p["centerPointX2"], 39);
        assert_eq!(
            d.wheel_geometry("w").unwrap(),
            WheelGeometry {
                x: 39,
                y: 0,
                counterclockwise: true
            }
        );
        assert!(d.json().contains("9.9900e+100"));
        d.set_wheel_preset("w", &keys(&[1, 3]), 2).unwrap();
        d.edit_wheel_stop(
            "w",
            &keys(&[1, 3]),
            StopEdit::Set {
                index: 0,
                position: 0,
                color: None,
            },
        )
        .unwrap();
        assert!(
            matches!(d.wheel_selection_gradient("w",&keys(&[1,3])).unwrap(),SelectedGradient::Uniform(Some(ref s)) if s[0].color.is_none())
        );
        let after = d.json().to_owned();
        for _ in 0..4 {
            assert!(d.undo().unwrap());
        }
        assert_eq!(d.json(), PROFILE);
        // Undo restores bytes but intentionally advances the session revision.
        assert!(!d.matches_snapshot(&snapshot));
        for _ in 0..4 {
            assert!(d.redo().unwrap());
        }
        assert_eq!(d.json(), after);
        let reopened = Draft::open(d.json()).unwrap();
        assert_eq!(
            reopened.wheel_geometry("w").unwrap(),
            d.wheel_geometry("w").unwrap()
        );
    }
    #[test]
    fn wheel_invalid_edits_and_unverified_playback_modes_are_atomic() {
        let mut d = Draft::open(PROFILE).unwrap();
        assert!(!d
            .set_wheel_property("w", &[], WheelProperty::Speed, 60)
            .unwrap());
        assert!(d
            .set_wheel_property("w", &keys(&[1]), WheelProperty::Speed, 361)
            .is_err());
        assert!(d
            .set_wheel_property("w", &keys(&[99]), WheelProperty::Speed, 60)
            .is_err());
        assert!(d
            .set_wheel_geometry(
                "w",
                WheelGeometry {
                    x: 10001,
                    y: 0,
                    counterclockwise: false
                }
            )
            .is_err());
        assert!(d.set_wheel_preset("w", &keys(&[1]), 5).is_err());
        assert_eq!(d.json(), PROFILE);
        let before = PROFILE.replace("\"cycles\":-1", "\"cycles\":2");
        let mut d = Draft::open(&before).unwrap();
        assert!(d
            .set_wheel_property("w", &keys(&[1]), WheelProperty::Speed, 60)
            .is_err());
        assert_eq!(d.json(), before);
    }
}
pub(super) fn validate_layer(item: &Object) -> Result<()> {
    if item.text("name")? != "wheel" || item.get("value").is_some_and(|v| v.get() != "13") {
        return Err(invalid("selected layer is not Studio Wheel13"));
    }
    Ok(())
}
fn merged(item: &Object, region: Option<&Object>, defaults: bool) -> Result<Object> {
    let mut params = if defaults {
        Object::parse(&encode(
            &serde_json::json!({"speed":60,"playbackStart":"random","playbackEnd":"never","cycles":-1,"colorStopsCustom":false,"colorStops":wheel_preset(0).unwrap()}),
        )?)?
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
    // Studio params2 belongs to the layer; never freeze a center/direction into
    // a newly assigned regional params object.
    for key in [
        "centerPointX",
        "centerPointY",
        "centerPointX2",
        "centerPointY2",
        "counterclockwise",
    ] {
        params.set(key, None);
    }
    Ok(params)
}
fn authorable(params: &Object) -> Result<()> {
    let start = params
        .get("playbackStart")
        .map(|v| serde_json::from_str::<String>(v.get()).map_err(invalid))
        .transpose()?;
    let end = params
        .get("playbackEnd")
        .map(|v| serde_json::from_str::<String>(v.get()).map_err(invalid))
        .transpose()?;
    let cycles = params
        .get("cycles")
        .map(|v| serde_json::from_str::<i64>(v.get()).map_err(invalid))
        .transpose()?;
    if cycles != Some(-1)
        || start.as_deref().is_some_and(|s| s != "random")
        || end.as_deref().is_some_and(|s| s != "never")
    {
        return Err(invalid("Wheel finite and input-triggered editing is not implemented; imported settings were preserved"));
    }
    Ok(())
}
#[derive(Clone, Copy)]
pub(super) enum Edit {
    Property(WheelProperty, u32),
    Preset(usize),
    Stop(StopEdit),
}
impl Edit {
    pub(super) fn apply(self, params: &mut Object) -> Result<()> {
        match self {
            Self::Stop(edit) => spectrum::edit_stop(params, edit, 7)?,
            Self::Property(property, value) => {
                params.set(property.field(), Some(&encode(&value)?));
            }
            Self::Preset(index) => {
                params.set(
                    "colorStops",
                    Some(&encode(
                        &wheel_preset(index).ok_or_else(|| invalid("unknown Wheel preset"))?,
                    )?),
                );
                params.set("colorStopsCustom", Some(&encode(&false)?));
            }
        }
        Ok(())
    }
}
impl Draft {
    fn wheel_params(&self, id: &str, selection: &[LedSelection]) -> Result<Vec<Object>> {
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
    pub fn wheel_selection_value(
        &self,
        id: &str,
        selection: &[LedSelection],
        property: WheelProperty,
    ) -> Result<SelectedWheelValue> {
        let params = self.wheel_params(id, selection)?;
        let value = |p: &Object| -> Result<Option<u32>> {
            p.get(property.field())
                .map(|v| serde_json::from_str(v.get()).map_err(invalid))
                .transpose()
        };
        let Some(first) = params.first() else {
            return Ok(SelectedWheelValue::Empty);
        };
        let first = value(first)?;
        for params in &params[1..] {
            if value(params)? != first {
                return Ok(SelectedWheelValue::Mixed);
            }
        }
        Ok(SelectedWheelValue::Uniform(first))
    }
    fn edit_wheel(&mut self, id: &str, selection: &[LedSelection], edit: Edit) -> Result<bool> {
        for params in self.wheel_params(id, selection)? {
            authorable(&params)?;
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let projection = merged(&item, None, true)?;
        self.edit_regions(id, selection, RegionEdit::Wheel(edit, &projection))
    }
    pub fn wheel_selection_gradient(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<SelectedGradient> {
        let params = self.wheel_params(id, selection)?;
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
    pub fn edit_wheel_stop(
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
            self.wheel_selection_gradient(id, selection)?,
            SelectedGradient::Uniform(Some(_))
        ) {
            return Err(invalid(
                "choose one uniform Wheel gradient before editing stops",
            ));
        }
        self.edit_wheel(id, selection, Edit::Stop(edit))
    }
    pub fn set_wheel_property(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        property: WheelProperty,
        value: u32,
    ) -> Result<bool> {
        property.validate(value)?;
        self.edit_wheel(id, selection, Edit::Property(property, value))
    }
    pub fn set_wheel_preset(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        index: usize,
    ) -> Result<bool> {
        wheel_preset(index).ok_or_else(|| invalid("unknown Wheel preset"))?;
        self.edit_wheel(id, selection, Edit::Preset(index))
    }
}

/// Layer-wide geometry, separate from regional speed and gradients (Studio params2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WheelGeometry {
    pub x: u16,
    pub y: u16,
    pub counterclockwise: bool,
}
impl Draft {
    pub fn wheel_geometry(&self, id: &str) -> Result<WheelGeometry> {
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let params = Object::parse(
            item.get("params")
                .ok_or_else(|| invalid("Wheel needs layer geometry"))?,
        )?;
        let number = |key: &str| -> Result<u16> {
            serde_json::from_str(
                params
                    .get(key)
                    .ok_or_else(|| invalid(format!("Wheel needs {key}")))?
                    .get(),
            )
            .map_err(invalid)
        };
        Ok(WheelGeometry {
            x: number("centerPointX2")?,
            y: number("centerPointY2")?,
            counterclockwise: serde_json::from_str(
                params
                    .get("counterclockwise")
                    .ok_or_else(|| invalid("Wheel needs direction"))?
                    .get(),
            )
            .map_err(invalid)?,
        })
    }
    pub fn set_wheel_geometry(&mut self, id: &str, geometry: WheelGeometry) -> Result<bool> {
        if geometry.x > 10000 || geometry.y > 10000 {
            return Err(invalid(
                "Wheel center must stay inside the10000 by10000 canvas",
            ));
        }
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let before = item
            .get("params")
            .ok_or_else(|| invalid("Wheel needs layer params"))?;
        let mut params = Object::parse(before)?;
        // Historical module1981.DO: round(x/40), round(y/40)-2.
        for (key, value) in [
            (
                "centerPointX",
                encode(&((u32::from(geometry.x) + 20) / 40))?,
            ),
            (
                "centerPointY",
                encode(&(((i32::from(geometry.y) + 20) / 40) - 2))?,
            ),
            ("centerPointX2", encode(&geometry.x)?),
            ("centerPointY2", encode(&geometry.y)?),
            ("counterclockwise", encode(&geometry.counterclockwise)?),
        ] {
            params.set(key, Some(&value));
        }
        let after = encode(&params)?;
        if after.get() == before.get() {
            return Ok(false);
        }
        self.commit_edit(super::super::Edit::Fields(vec![super::super::Change {
            id: id.into(),
            field: "params",
            before: Some(before.to_owned()),
            after,
        }]))
    }
}
