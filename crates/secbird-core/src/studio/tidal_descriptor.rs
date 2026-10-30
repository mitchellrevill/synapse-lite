//! Recovered AddEffect19 preparation only. This does not render radial frames.
use super::*;
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TidalDescriptor {
    pub mode: u16,
    pub color_stops: Vec<Box<RawValue>>,
    pub duration: u8,
    pub width: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u32>,
    pub pause: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<u32>,
    pub direction: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_col: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_row: Option<i32>,
}
fn number(params: &Object, key: &str, min: i64, max: i64) -> Result<Option<i64>> {
    let value = params
        .get(key)
        .map(|r| serde_json::from_str::<i64>(r.get()).map_err(invalid))
        .transpose()?;
    if value.is_some_and(|v| v < min || v > max) {
        return Err(invalid(format!(
            "Tidal {key} is outside the bounded descriptor range"
        )));
    }
    Ok(value)
}
fn flag(params: &Object, key: &str) -> Result<bool> {
    params
        .get(key)
        .map(|r| serde_json::from_str(r.get()).map_err(invalid))
        .transpose()
        .map(|v| v.unwrap_or(false))
}
fn color(params: &Object, key: &str) -> Result<Option<u32>> {
    let value = params
        .get(key)
        .map(|r| serde_json::from_str::<u32>(r.get()).map_err(invalid))
        .transpose()?;
    if value.is_some_and(|v| v > 0xffffff && v != TIDAL_NONE) {
        return Err(invalid("Tidal needs RGB24 or None"));
    }
    Ok(value)
}
fn stop(position: u8, color: Option<u32>) -> Result<Box<RawValue>> {
    let mut object = Object(vec![]);
    object.set("Stop", Some(&encode(&position)?));
    if let Some(color) = color {
        object.set("Color", Some(&encode(&color)?));
    }
    encode(&object)
}
impl TidalDescriptor {
    fn prepare(params: &Object) -> Result<Self> {
        let mut mode = 0;
        let color2 = color(params, "color2")?;
        let first_color = color(params, "color")?.or(color2).unwrap_or(TIDAL_NONE);
        let stops = if let Some(raw) = params.get("colorStops") {
            let mut stops = array(raw)?;
            if stops.len() > 10 {
                return Err(invalid("Tidal supports at most ten imported stops"));
            }
            for raw in &mut stops {
                let mut s = Object::parse(raw)?;
                number(&s, "Stop", 0, 100)?.ok_or_else(|| invalid("missing Tidal Stop"))?;
                if s.get("Color").is_none() {
                    s.set("Color", Some(&encode(&TIDAL_NONE)?));
                } else {
                    color_value(&s)?;
                }
                *raw = encode(&s)?;
            }
            if let (Some(first), Some(last)) = (stops.first(), stops.last()) {
                let first = color(&Object::parse(first)?, "Color")?;
                let last = color(&Object::parse(last)?, "Color")?;
                if !stops.iter().any(|r| {
                    Object::parse(r)
                        .ok()
                        .and_then(|s| number(&s, "Stop", 0, 100).ok().flatten())
                        == Some(0)
                }) {
                    stops.insert(0, stop(0, first)?);
                }
                if !stops.iter().any(|r| {
                    Object::parse(r)
                        .ok()
                        .and_then(|s| number(&s, "Stop", 0, 100).ok().flatten())
                        == Some(100)
                }) {
                    stops.push(stop(100, last)?);
                }
            }
            // The converter sorts a temporary copy, then overwrites it with c in case19.
            stops
        } else {
            let random = flag(params, "randomColor")?;
            if random {
                mode = 1;
            }
            let (positions, a, b) = if random {
                ([0, 15, 30, 40, 65, 70, 90, 100], 1, Some(2))
            } else {
                ([0, 10, 25, 40, 60, 75, 90, 100], first_color, color2)
            };
            positions
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    stop(
                        p,
                        if i == 2 {
                            Some(a)
                        } else if i == 5 {
                            b
                        } else {
                            Some(0)
                        },
                    )
                })
                .collect::<Result<Vec<_>>>()?
        };
        if flag(params, "split")? {
            mode += 2;
        }
        let start = params
            .get("playbackStart")
            .map(|r| serde_json::from_str::<String>(r.get()).map_err(invalid))
            .transpose()?;
        mode += match start.as_deref() {
            None | Some("random") => 0,
            Some("onPress") => 4096,
            Some("onSelectedKeys") => 12288,
            _ => return Err(invalid("unknown Tidal start")),
        };
        let end = params
            .get("playbackEnd")
            .map(|r| serde_json::from_str::<String>(r.get()).map_err(invalid))
            .transpose()?;
        match end.as_deref() {
            None | Some("never") | Some("after") => {}
            Some("toggle") => mode += 4,
            _ => return Err(invalid("unknown Tidal end")),
        };
        let cycles = number(params, "cycles", -1, 100)?.map(|v| v as i32);
        if cycles == Some(0) {
            return Err(invalid("Tidal cycles must be -1 or 1-100"));
        }
        Ok(Self {
            mode,
            color_stops: stops,
            duration: 0,
            width: number(params, "width", 10, 400)?.unwrap_or(100) as u32,
            speed: number(params, "speed", 0, 50)?.map(|v| v as u32),
            pause: number(params, "pause", 0, 60000)?.unwrap_or(0) as u32,
            angle: number(params, "angle", 0, 360)?.map(|v| v as u32),
            direction: if flag(params, "counterclockwise")? {
                0
            } else {
                1
            },
            cycles,
            center_col: number(params, "centerPointX", -2, 250)?.map(|v| v as i32),
            center_row: number(params, "centerPointY", -2, 250)?.map(|v| v as i32),
        })
    }
}
fn color_value(stop: &Object) -> Result<()> {
    color(stop, "Color")?;
    Ok(())
}
impl Draft {
    /// Prepare each declared region after inheritance. No defaults for missing
    /// speed, angle, cycles or center are invented. No playback is admitted.
    pub fn tidal_region_descriptors(&self, id: &str) -> Result<Vec<TidalDescriptor>> {
        let item = layer(self, id, None)?;
        validate_layer(&item)?;
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut params = merged(&item, Some(&region), false)?;
            if let Some(raw) = item.get("params") {
                let layout = Object::parse(raw)?;
                for key in ["centerPointX", "centerPointY", "counterclockwise"] {
                    params.set(key, layout.get(key));
                }
            }
            let descriptor = TidalDescriptor::prepare(&params)?;
            bytes = bytes
                .checked_add(encode(&descriptor)?.get().len())
                .ok_or_else(|| invalid("Tidal descriptor size overflow"))?;
            if bytes > MAX_DOCUMENT_BYTES {
                return Err(invalid("Tidal descriptors exceed 8 MiB"));
            }
            result.push(descriptor);
        }
        Ok(result)
    }
}
