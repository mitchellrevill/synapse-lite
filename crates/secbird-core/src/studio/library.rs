//! Pure composition management over the existing interchange codec. No persistence.
use super::*;
use std::collections::BTreeMap;

fn title(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.encode_utf16().count() > 32
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "composition title must contain 1–32 UTF16 units without control characters",
        ));
    }
    Ok(())
}

fn fresh(archive: &Archive, id: &str) -> Result<()> {
    if id.len() != 12 || !id.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err(invalid(
            "new composition identity must contain 12 ASCII letters or digits",
        ));
    }
    for index in 0..archive.profile_count() {
        if archive.profile_header(index)?.id.as_deref() == Some(id) {
            return Err(invalid("composition identity already exists"));
        }
    }
    Ok(())
}

fn assemble(archive: &Archive, profiles: Vec<Box<RawValue>>) -> Result<Archive> {
    // Reuse the codec's bounded serializer and admission, retaining raw envelope values.
    let next = Archive {
        fields: archive
            .fields
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    match value {
                        Field::Profiles => Field::Profiles,
                        Field::Raw(raw) => Field::Raw(raw.clone()),
                    },
                )
            })
            .collect(),
        profiles,
    };
    Archive::parse(&next.to_bytes()?)
}

fn patch(raw: &str, changes: &[(&str, &str)]) -> Result<Box<RawValue>> {
    // Reject ambiguous duplicate root keys before editing. Opaque descendants stay raw.
    #[derive(Deserialize)]
    struct Unique(#[serde(deserialize_with = "unique")] BTreeMap<String, Box<RawValue>>);
    fn unique<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<String, Box<RawValue>>, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = BTreeMap<String, Box<RawValue>>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("composition object with unique keys")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut fields = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if fields.len() >= 256 || fields.insert(key, value).is_some() {
                        return Err(de::Error::custom(
                            "ambiguous or excessive composition fields",
                        ));
                    }
                }
                Ok(fields)
            }
        }
        deserializer.deserialize_map(Fields)
    }
    let Unique(mut fields) = serde_json::from_str(raw).map_err(invalid)?;
    for (key, value) in changes {
        fields.insert(
            (*key).into(),
            serde_json::value::to_raw_value(value).map_err(invalid)?,
        );
    }
    serde_json::value::to_raw_value(&fields).map_err(invalid)
}

pub fn empty() -> Result<Archive> {
    Archive::parse(br#"{"version":1,"profiles":[]}"#)
}

pub fn create(archive: &Archive, id: &str, layer_id: &str, name: &str) -> Result<Archive> {
    fresh(archive, id)?;
    title(name)?;
    if layer_id.len() != 12 || !layer_id.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err(invalid("invalid layer identity"));
    }
    let profile = serde_json::json!({"id":id,"title":name,"devices":[],"layers":[{
        "id":layer_id,"type":"layer","name":"spectrum","value":4,"title":"SPECTRUM_CYCLING",
        "params":{},"regions":[],"visible":true
    }]});
    let mut profiles = archive.profiles.clone();
    profiles.push(serde_json::value::to_raw_value(&profile).map_err(invalid)?);
    assemble(archive, profiles)
}

pub fn duplicate(archive: &Archive, index: usize, id: &str, name: &str) -> Result<Archive> {
    fresh(archive, id)?;
    title(name)?;
    let raw = archive
        .profile_json(index)
        .ok_or_else(|| invalid("composition index out of range"))?;
    let mut profiles = archive.profiles.clone();
    profiles.push(patch(raw, &[("id", id), ("title", name)])?);
    assemble(archive, profiles)
}

pub fn rename(archive: &Archive, index: usize, name: &str) -> Result<Archive> {
    title(name)?;
    let raw = archive
        .profile_json(index)
        .ok_or_else(|| invalid("composition index out of range"))?;
    Archive::parse(&archive.replace_profile_bytes(index, patch(raw, &[("title", name)])?.get())?)
}

pub fn delete(archive: &Archive, index: usize) -> Result<Archive> {
    if index >= archive.profile_count() {
        return Err(invalid("composition index out of range"));
    }
    let mut profiles = archive.profiles.clone();
    profiles.remove(index);
    assemble(archive, profiles)
}

/// Append selections in source order without remapping identities or LED data.
/// Conflicting identities or envelope metadata are refused, never silently lost.
pub fn import(archive: &Archive, incoming: &Archive, indices: &[usize]) -> Result<Archive> {
    let selected = Archive::parse(&incoming.export_selected(indices)?)?;
    let mut ids = BTreeSet::new();
    for source in [archive, &selected] {
        for i in 0..source.profile_count() {
            let id = source
                .profile_header(i)?
                .id
                .ok_or_else(|| invalid("import requires stable composition identities"))?;
            if id.is_empty() || !ids.insert(id) {
                return Err(invalid(
                    "duplicate composition identity; original libraries retained",
                ));
            }
        }
    }
    let mut merged = assemble(
        archive,
        archive
            .profiles
            .iter()
            .chain(&selected.profiles)
            .cloned()
            .collect(),
    )?;
    for (key, field) in &incoming.fields {
        let Field::Raw(raw) = field else {
            continue;
        };
        if key == "version" {
            continue;
        }
        if let Some((_, existing)) = merged.fields.iter().find(|(name, _)| name == key) {
            if !matches!(existing, Field::Raw(value) if value.get() == raw.get()) {
                return Err(invalid(format!(
                    "conflicting envelope field {key}; original libraries retained"
                )));
            }
        } else {
            merged.fields.push((key.clone(), Field::Raw(raw.clone())));
        }
    }
    Archive::parse(&merged.to_bytes()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn original() -> Archive {
        Archive::parse(br#"{"version":1,"opaque":9.9900e+100,"profiles":[{"id":"foreign-id","title":"Desk","devices":[],"layers":[],"future":{"n":9007199254740993123}}]}"#).unwrap()
    }
    #[test]
    fn management_preserves_unknown_values_and_original_identities() {
        let a = original();
        let created = create(&a, "0123456789ab", "abcdef012345", "New").unwrap();
        assert!(document::Draft::open(created.profile_json(1).unwrap()).is_ok());
        let b = duplicate(&created, 0, "0123456789ac", "Copy").unwrap();
        let c = rename(&b, 2, "Renamed").unwrap();
        let d = delete(&c, 1).unwrap();
        assert_eq!(d.profile_json(0), a.profile_json(0));
        assert!(d.profile_json(1).unwrap().contains("9007199254740993123"));
        assert!(String::from_utf8(d.to_bytes().unwrap())
            .unwrap()
            .contains("9.9900e+100"));
        assert_eq!(
            d.profile_header(1).unwrap().id.as_deref(),
            Some("0123456789ac")
        );
        assert_eq!(a.profile_count(), 1);
    }
    #[test]
    fn selected_import_retains_envelopes_and_refuses_collisions_atomically() {
        let a = original();
        let incoming = create(&empty().unwrap(), "abcdef012345", "111111111111", "Import").unwrap();
        let b = import(&a, &incoming, &[0]).unwrap();
        assert_eq!(b.profile_json(1), incoming.profile_json(0));
        assert!(import(&b, &incoming, &[0]).is_err());
        assert!(import(&a, &incoming, &[]).is_err());
        let conflict = Archive::parse(
            br#"{"version":1,"opaque":0,"profiles":[{"id":"different","title":"x"}]}"#,
        )
        .unwrap();
        assert!(import(&a, &conflict, &[0]).is_err());
        assert_eq!(delete(&a, 0).unwrap().profile_count(), 0);
        assert!(rename(&a, 0, "  ").is_err());
        assert!(rename(&a, 0, &"😀".repeat(17)).is_err());
        assert!(duplicate(&a, 0, "bad", "Copy").is_err());
        let ambiguous =
            Archive::parse(br#"{"version":1,"profiles":[{"id":"a","title":"a","title":"b"}]}"#)
                .unwrap();
        assert!(rename(&ambiguous, 0, "c").is_err());
    }
}
