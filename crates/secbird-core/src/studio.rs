//! Studio v1 JSON container interchange, separate from device profiles.
//!
//! Profile bodies and unknown envelope values remain raw JSON: no numeric
//! conversion, ID replacement, LED remapping, effect application or code execution.
use crate::{Error, Result};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::value::RawValue;
use std::{
    collections::BTreeSet,
    fmt,
    io::{Read, Write},
};

pub mod document;
pub mod inventory;

pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROFILES: usize = 128;
const MAX_FIELDS: usize = 64;
const MAX_DEPTH: usize = 64;
const MAX_MARKERS: usize = 262_144;

fn invalid(message: impl fmt::Display) -> Error {
    Error::InvalidArg(format!("Studio file: {message}"))
}

#[derive(Debug)]
enum Field {
    Profiles,
    Raw(Box<RawValue>),
}

/// An interchange document, not an installed or executable composition.
#[derive(Debug)]
pub struct Archive {
    fields: Vec<(String, Field)>,
    profiles: Vec<Box<RawValue>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProfileHeader {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

impl Archive {
    pub fn read(reader: impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_DOCUMENT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(invalid)?;
        Self::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(invalid("exceeds the 8 MiB interchange budget"));
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        if bytes.starts_with(b"PK") {
            return Err(Error::Unsupported(
                "Legacy ZIP/XML ChromaEffects conversion is not implemented; original file retained".into(),
            ));
        }
        check_complexity(bytes)?;
        serde_json::from_slice::<WireArchive>(bytes)
            .map(|wire| wire.0)
            .map_err(invalid)
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// The exact JSON value as supplied. It may include unsupported effects,
    /// foreign devices or future fields; returning it does not admit execution.
    pub fn profile_json(&self, index: usize) -> Option<&str> {
        self.profiles.get(index).map(|profile| profile.get())
    }

    /// Display metadata only. Unknown profile content is skipped, not decoded
    /// into an allocating tree or admitted as a compatible runtime composition.
    pub fn profile_header(&self, index: usize) -> Result<ProfileHeader> {
        let raw = self
            .profile_json(index)
            .ok_or_else(|| invalid("composition index out of range"))?;
        let header: ProfileHeader = serde_json::from_str(raw).map_err(invalid)?;
        if [&header.id, &header.title]
            .into_iter()
            .flatten()
            .any(|value| value.len() > 512)
        {
            return Err(invalid(
                "display metadata exceeds 512 bytes; raw composition retained",
            ));
        }
        Ok(header)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.encode(None)
    }

    /// Prepare a replacement without mutating this archive or saving a file.
    pub fn replace_profile_bytes(&self, index: usize, profile_json: &str) -> Result<Vec<u8>> {
        if index >= self.profiles.len() || profile_json.len() > MAX_DOCUMENT_BYTES {
            return Err(invalid("replacement index or composition size is invalid"));
        }
        check_complexity(profile_json.as_bytes())?;
        let raw = RawValue::from_string(profile_json.into()).map_err(invalid)?;
        if !raw.get().trim_start().starts_with('{') {
            return Err(invalid("replacement composition must be an object"));
        }
        let mut output = BoundedOutput(Vec::new());
        serde_json::to_writer(
            &mut output,
            &Export {
                archive: self,
                selected: None,
                replacement: Some((index, &raw)),
            },
        )
        .map_err(invalid)?;
        Ok(output.0)
    }

    /// Zero-based selection, emitted in source order as in the stock exporter.
    /// Duplicate/out-of-range indices or empty selection are rejected.
    pub fn export_selected(&self, indices: &[usize]) -> Result<Vec<u8>> {
        let mut selected = BTreeSet::new();
        if indices.is_empty() {
            return Err(invalid("select at least one composition"));
        }
        for &index in indices {
            if index >= self.profiles.len() || !selected.insert(index) {
                return Err(invalid("invalid or duplicate composition selection"));
            }
        }
        self.encode(Some(&selected))
    }

    fn encode(&self, selected: Option<&BTreeSet<usize>>) -> Result<Vec<u8>> {
        let mut output = BoundedOutput(Vec::new());
        serde_json::to_writer(
            &mut output,
            &Export {
                archive: self,
                selected,
                replacement: None,
            },
        )
        .map_err(invalid)?;
        Ok(output.0)
    }
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_DOCUMENT_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("Studio export exceeds 8 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// RawValue handles syntax; this scan bounds structural work even inside opaque
// raw profile values. Brackets inside strings do not count as structure.
fn check_complexity(bytes: &[u8]) -> Result<()> {
    let (mut depth, mut markers, mut quoted, mut escaped) = (0usize, 0usize, false, false);
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth += 1;
                markers += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                markers += 1;
            }
            b',' | b':' => markers += 1,
            _ => {}
        }
        if depth > MAX_DEPTH || markers > MAX_MARKERS {
            return Err(invalid("structural complexity budget exceeded"));
        }
    }
    Ok(())
}

struct Profiles(Vec<Box<RawValue>>);
impl<'de> Deserialize<'de> for Profiles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct ProfileVisitor;
        impl<'de> Visitor<'de> for ProfileVisitor {
            type Value = Profiles;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of Studio profile objects")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut array: A,
            ) -> std::result::Result<Profiles, A::Error> {
                let mut profiles = Vec::new();
                while let Some(profile) = array.next_element::<Box<RawValue>>()? {
                    if profiles.len() == MAX_PROFILES {
                        return Err(de::Error::custom("more than 128 Studio profiles"));
                    }
                    if !profile.get().trim_start().starts_with('{') {
                        return Err(de::Error::custom("Studio profiles must be objects"));
                    }
                    profiles.push(profile);
                }
                Ok(Profiles(profiles))
            }
        }
        deserializer.deserialize_seq(ProfileVisitor)
    }
}

// Keep Deserialize private: callers must use the byte/complexity-limited public
// parse/read entry points, rather than serde directly on an unbounded reader.
struct WireArchive(Archive);
impl<'de> Deserialize<'de> for WireArchive {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct ArchiveVisitor;
        impl<'de> Visitor<'de> for ArchiveVisitor {
            type Value = WireArchive;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a version1 Studio export object")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<WireArchive, A::Error> {
                let (mut fields, mut names, mut profiles, mut version) =
                    (Vec::new(), BTreeSet::new(), None, false);
                while let Some(name) = map.next_key::<String>()? {
                    if names.len() == MAX_FIELDS || !names.insert(name.clone()) {
                        return Err(de::Error::custom("duplicate or excessive envelope fields"));
                    }
                    let field = if name == "profiles" {
                        profiles = Some(map.next_value::<Profiles>()?.0);
                        Field::Profiles
                    } else {
                        let raw = map.next_value::<Box<RawValue>>()?;
                        if name == "version" {
                            let number: serde_json::Number =
                                serde_json::from_str(raw.get()).map_err(de::Error::custom)?;
                            if number.as_f64() != Some(1.0) {
                                return Err(de::Error::custom("only Studio version1 is supported"));
                            }
                            version = true;
                        }
                        Field::Raw(raw)
                    };
                    fields.push((name, field));
                }
                if !version {
                    return Err(de::Error::missing_field("version"));
                }
                let profiles = profiles.ok_or_else(|| de::Error::missing_field("profiles"))?;
                Ok(WireArchive(Archive { fields, profiles }))
            }
        }
        deserializer.deserialize_map(ArchiveVisitor)
    }
}

struct Export<'a> {
    archive: &'a Archive,
    selected: Option<&'a BTreeSet<usize>>,
    replacement: Option<(usize, &'a RawValue)>,
}
impl Serialize for Export<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.archive.fields.len()))?;
        for (name, field) in &self.archive.fields {
            match field {
                Field::Raw(raw) => map.serialize_entry(name, raw)?,
                Field::Profiles => map.serialize_entry(name, &SelectedProfiles(self))?,
            }
        }
        map.end()
    }
}
struct SelectedProfiles<'a, 'b>(&'a Export<'b>);
impl Serialize for SelectedProfiles<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let count = self
            .0
            .selected
            .map_or(self.0.archive.profiles.len(), BTreeSet::len);
        let mut sequence = serializer.serialize_seq(Some(count))?;
        for (index, profile) in self.0.archive.profiles.iter().enumerate() {
            if self
                .0
                .selected
                .is_none_or(|selected| selected.contains(&index))
            {
                match self.0.replacement {
                    Some((target, replacement)) if target == index => {
                        sequence.serialize_element(replacement)?
                    }
                    _ => sequence.serialize_element(profile)?,
                }
            }
        }
        sequence.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROFILE: &str = r#"{ "id":"studio-one", "title":"Desk", "layers":[{"id":"layer-one","type":"layer","name":"static","value":6,"title":"STATIC","visible":true,"params":{},"regions":[{"id":"region-one","params":{"color":65280},"devices":[{"id":"device-one","deviceId":"physical","instanceId":"instance","productId":605,"leds":[{"row":0,"col":1,"regionId":"source-region","regionIndex":3,"inputType":1,"inputData":"{\"Key\":\"A\"}"}]}]}]}],"unknown":{"large":9007199254740993123456789,"float":1.2300e-7,"nested":[null,true,"[{}]\\\""]} }"#;
    fn document() -> Vec<u8> {
        format!(r#"{{"version":1,"profiles":[{PROFILE},{{"id":"two","title":"Second"}}],"future":{{"literal":9.9900e+100}}}}"#).into_bytes()
    }
    #[test]
    fn source_shaped_layers_devices_and_opaque_numbers_remain_exact_raw_json() {
        let archive = Archive::parse(&document()).unwrap();
        assert_eq!(archive.profile_count(), 2);
        assert_eq!(archive.profile_json(0), Some(PROFILE));
        let encoded = archive.to_bytes().unwrap();
        assert!(String::from_utf8_lossy(&encoded).contains(r#""future":{"literal":9.9900e+100}"#));
        let again = Archive::parse(&encoded).unwrap();
        assert_eq!(again.profile_json(0), Some(PROFILE));
        assert!(again.profile_json(2).is_none());
    }
    #[test]
    fn selections_preserve_source_order_and_refuse_ambiguous_indices() {
        let archive = Archive::parse(&document()).unwrap();
        let selected = Archive::parse(&archive.export_selected(&[1]).unwrap()).unwrap();
        assert_eq!(selected.profile_count(), 1);
        assert_eq!(selected.profile_json(0), archive.profile_json(1));
        let both = Archive::parse(&archive.export_selected(&[1, 0]).unwrap()).unwrap();
        assert_eq!(both.profile_json(0), Some(PROFILE));
        for indices in [vec![], vec![0, 0], vec![2]] {
            assert!(archive.export_selected(&indices).is_err());
        }
        assert_eq!(archive.profile_count(), 2);
    }
    #[test]
    fn envelope_duplicates_versions_and_wrong_shapes_are_refused() {
        for invalid in [
            r#"{"version":1,"version":1,"profiles":[]}"#,
            r#"{"version":1,"profiles":[],"future":1,"future":2}"#,
            r#"{"version":2,"profiles":[]}"#,
            r#"{"version":"1","profiles":[]}"#,
            r#"{"profiles":[]}"#,
            r#"{"version":1}"#,
            r#"{"version":1,"profiles":{}}"#,
            r#"{"version":1,"profiles":[null]}"#,
            r#"{"version":1,"profiles":[]} trailing"#,
            r#"{"version":1,"profiles":[{"invalid":NaN}]}"#,
        ] {
            assert!(Archive::parse(invalid.as_bytes()).is_err(), "{invalid}");
        }
        let empty = Archive::parse(br#"{"version":1,"profiles":[]}"#).unwrap();
        assert_eq!(empty.profile_count(), 0);
        assert!(Archive::parse(&empty.to_bytes().unwrap()).is_ok());
        assert!(Archive::parse(b"PK\x03\x04")
            .unwrap_err()
            .to_string()
            .contains("Legacy ZIP/XML"));
    }
    #[test]
    fn display_metadata_does_not_narrow_or_rewrite_opaque_compositions() {
        for profile in [
            format!(r#"{{"title":"{}"}}"#, "x".repeat(513)),
            r#"{"title":7,"unknown":{"same":1,"same":2}}"#.into(),
        ] {
            let bytes = format!(r#"{{"version":1,"profiles":[{profile}]}}"#);
            let archive = Archive::parse(bytes.as_bytes()).unwrap();
            assert!(archive.profile_header(0).is_err());
            assert!(archive.profile_header(1).is_err());
            let exported = Archive::parse(&archive.export_selected(&[0]).unwrap()).unwrap();
            assert_eq!(exported.profile_json(0), Some(profile.as_str()));
        }
    }

    #[test]
    fn bom_quotes_and_encoded_duplicate_keys_have_unambiguous_envelope_behavior() {
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend(document());
        let archive = Archive::read(bytes.as_slice()).unwrap();
        assert_eq!(archive.profile_json(0), Some(PROFILE));
        assert!(Archive::parse(br#"{"version":1,"ver\u0073ion":1,"profiles":[]}"#).is_err());
        let many = std::iter::repeat_n("0", MAX_MARKERS)
            .collect::<Vec<_>>()
            .join(",");
        assert!(Archive::parse(
            format!(r#"{{"version":1,"profiles":[{{"unknown":[{many}]}}]}}"#).as_bytes()
        )
        .is_err());
    }

    #[test]
    fn reader_bytes_depth_field_and_profile_counts_are_bounded() {
        assert!(Archive::read(std::io::repeat(b' ').take(MAX_DOCUMENT_BYTES as u64 + 1)).is_err());
        let profiles = std::iter::repeat_n("{}", MAX_PROFILES + 1)
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            Archive::parse(format!(r#"{{"version":1,"profiles":[{profiles}]}}"#).as_bytes())
                .is_err()
        );
        let deep = format!(
            "{}0{}",
            "[".repeat(MAX_DEPTH + 1),
            "]".repeat(MAX_DEPTH + 1)
        );
        assert!(Archive::parse(deep.as_bytes()).is_err());
        let fields = (0..MAX_FIELDS)
            .map(|i| format!(r#""field{i}":0"#))
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            Archive::parse(format!(r#"{{"version":1,"profiles":[],{fields}}}"#).as_bytes())
                .is_err()
        );
    }
}
