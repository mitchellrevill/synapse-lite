//! Layer-scoped, in-process property clipboard. Never reads the system clipboard.
use super::*;

#[derive(Clone, Debug)]
pub struct PropertyClipboard {
    owner: u64,
    layer: String,
    effect: String,
    params: Box<RawValue>,
    base: Box<RawValue>,
}

fn defaults(name: &str, reset: bool) -> Result<Object> {
    let all: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(include_str!("property_defaults.json")).map_err(invalid)?;
    let effect = all
        .get(name)
        .ok_or_else(|| invalid("unknown effect has no recovered property contract"))?;
    let effect = Object::parse(effect)?;
    Object::parse(
        effect
            .get(if reset { "reset" } else { "selected" })
            .unwrap(),
    )
}
fn effect_layer(draft: &Draft, id: &str) -> Result<Object> {
    let item = layer(draft, id, None)?;
    let name = item.text("name")?;
    let value = match name.as_str() {
        "ripple" => 1,
        "wave" => 2,
        "reactive" => 3,
        "spectrum" => 4,
        "breathing" => 5,
        "static" => 6,
        "starlight" => 7,
        "fire" => 8,
        "ambient" => 11,
        "audio" => 12,
        "wheel" => 13,
        "chroma-generate" => 18,
        "tidal" => 19,
        _ => return Err(invalid("unknown effect property clipboard is unavailable")),
    };
    if item
        .get("value")
        .is_some_and(|v| serde_json::from_str::<u16>(v.get()).ok() != Some(value))
    {
        return Err(invalid("conflicting Studio effect ID"));
    }
    Ok(item)
}
fn one_device(selection: &BTreeSet<LedSelection>) -> Result<()> {
    if selection.is_empty()
        || selection
            .iter()
            .any(|s| s.device != selection.first().unwrap().device)
    {
        return Err(invalid("select LEDs on exactly one device"));
    }
    Ok(())
}
fn overlay(target: &mut Object, source: &Object) {
    for (name, value) in &source.0 {
        target.set(name, Some(value));
    }
}
// Deliberately retain raw numeric spellings; unknown large integers must not be
// rounded through a floating point representation to make a mixed selection equal.
fn signature(params: &Object) -> BTreeMap<&str, &str> {
    params
        .0
        .iter()
        .map(|(k, v)| (k.as_str(), v.get()))
        .collect()
}
impl Draft {
    /// The source Copy menu requires one device; the thunk also rejects mixed
    /// regional properties. A failed copy leaves the caller's previous clip intact.
    pub fn copy_selection_properties(
        &self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<PropertyClipboard> {
        let selection = selected(selection)?;
        one_device(&selection)?;
        inventory(self, &selection)?;
        let item = effect_layer(self, id)?;
        let name = item.text("name")?;
        let mut seen = BTreeSet::new();
        let mut uniform: Option<Object> = None;
        for raw in entries(&item, "regions")? {
            let region = Object::parse(&raw)?;
            let mut chosen = false;
            for raw in entries(&region, "devices")? {
                let device = Object::parse(&raw)?;
                let id = device.text("id")?;
                for raw in entries(&device, "leds")? {
                    let key = led(&raw, &id)?;
                    if selection.contains(&key) {
                        if !seen.insert(key) {
                            return Err(invalid("overlapping selected region memberships"));
                        }
                        chosen = true;
                    }
                }
            }
            if chosen {
                let mut params = defaults(&name, false)?;
                overlay(
                    &mut params,
                    &Object::parse(
                        region
                            .get("params")
                            .ok_or_else(|| invalid("missing regional params"))?,
                    )?,
                );
                if uniform
                    .as_ref()
                    .is_some_and(|old| signature(old) != signature(&params))
                {
                    return Err(invalid("cannot copy mixed effect properties"));
                }
                uniform = Some(params);
            }
        }
        if seen.len() != selection.len() {
            let params = defaults(&name, false)?;
            if uniform
                .as_ref()
                .is_some_and(|old| signature(old) != signature(&params))
            {
                return Err(invalid(
                    "cannot copy mixed assigned and unassigned properties",
                ));
            }
            uniform = Some(params);
        }
        Ok(PropertyClipboard {
            owner: self.owner,
            layer: id.into(),
            effect: name,
            params: encode(&uniform.unwrap())?,
            base: item
                .get("params")
                .map(RawValue::to_owned)
                .unwrap_or(encode(&Object(vec![]))?),
        })
    }
    pub fn can_paste_properties(&self, id: &str, clip: &PropertyClipboard) -> bool {
        self.owner == clip.owner
            && id == clip.layer
            && effect_layer(self, id)
                .ok()
                .and_then(|item| item.text("name").ok())
                .as_deref()
                == Some(clip.effect.as_str())
    }
    /// Paste may target multiple devices, but only in the copied layer/session.
    /// All changes, including layer-wide parameters, share a single history entry.
    pub fn paste_selection_properties(
        &mut self,
        id: &str,
        selection: &[LedSelection],
        clip: &PropertyClipboard,
    ) -> Result<bool> {
        if !self.can_paste_properties(id, clip) {
            return Err(invalid(
                "property clipboard belongs to another layer or composition",
            ));
        }
        let params = Object::parse(&clip.params)?;
        let base = Object::parse(&clip.base)?;
        let known = defaults(&clip.effect, true)?;
        let clear: Vec<_> = known.0.iter().map(|(key, _)| key.as_str()).collect();
        self.edit_regions(
            id,
            selection,
            RegionEdit::Properties(&params, &clear, Some(&base)),
        )
    }
    /// Reset merges recovered default fields; opaque regional data and layer-wide
    /// center points are retained, just as the source reset action retains params2.
    pub fn reset_selection_properties(
        &mut self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<bool> {
        one_device(&selected(selection)?)?;
        let item = effect_layer(self, id)?;
        let params = defaults(&item.text("name")?, true)?;
        self.edit_regions(id, selection, RegionEdit::Properties(&params, &[], None))
    }
    /// Context-menu eligibility is narrower than the existing erase tool.
    pub fn remove_selection_effect(
        &mut self,
        id: &str,
        selection: &[LedSelection],
    ) -> Result<bool> {
        one_device(&selected(selection)?)?;
        effect_layer(self, id)?;
        self.erase_leds(id, selection)
    }
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
