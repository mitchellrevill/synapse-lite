//! Authenticate a bounded, explicitly selected local Ornata source manifest.
//!
//! SHA256 covers the complete bytes before selective JSON parsing. No hardware
//! discovery, locale fallback, vendor execution or runtime integration exists.
//! See docs/architecture.md for memory and asset-provisioning boundaries.

use crate::ornata_spatial::{
    LayoutIdentity, MappingEntry, OrnataSpatialMap, SourceLayoutRecord, SpatialError,
    MAX_SOURCE_ENTRIES, SOURCE_MANIFEST_SHA256,
};
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};

/// The pinned artifact is 2,895,966 bytes. This ceiling permits that exact file
/// while bounding reads before JSON parsing; it is not an application RAM claim.
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub enum ManifestError {
    Io(io::Error),
    NotRegularFile,
    TooLarge { limit: usize },
    Crypto(i32),
    DigestMismatch { actual: [u8; 32] },
    Json(serde_json::Error),
    MissingLayout(LayoutIdentity),
    WrongArtifactIdentity { vendor_id: u16, product_id: u16 },
    Spatial(SpatialError),
}
impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "cannot read manifest: {e}"),
            Self::NotRegularFile => f.write_str("manifest path is not a regular file"),
            Self::TooLarge { limit } => write!(f, "manifest exceeds the {limit}-byte read limit"),
            Self::Crypto(status) => write!(
                f,
                "Windows SHA256 failed with status {:#010x}",
                *status as u32
            ),
            Self::DigestMismatch { actual } => write!(
                f,
                "manifest SHA256 does not match the pinned artifact (received {})",
                digest_hex(actual)
            ),
            Self::Json(e) => write!(f, "invalid source manifest: {e}"),
            Self::MissingLayout(id) => write!(
                f,
                "source manifest has no exact edition/layout record for {id:?}"
            ),
            Self::WrongArtifactIdentity {
                vendor_id,
                product_id,
            } => write!(
                f,
                "source manifest identifies unexpected VID {vendor_id:#06x}/PID {product_id:#06x}"
            ),
            Self::Spatial(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ManifestError {}
impl From<io::Error> for ManifestError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for ManifestError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<SpatialError> for ManifestError {
    fn from(e: SpatialError) -> Self {
        Self::Spatial(e)
    }
}

/// Only the loader can construct this authenticated selection. It retains no
/// source JSON, SVG, file path or historical device observation.
#[derive(Debug, Clone)]
pub struct AuthenticatedLayout {
    map: OrnataSpatialMap,
    sha256: [u8; 32],
    source_byte_len: usize,
}
impl AuthenticatedLayout {
    pub fn map(&self) -> &OrnataSpatialMap {
        &self.map
    }
    pub fn into_map(self) -> OrnataSpatialMap {
        self.map
    }
    pub fn authenticated_sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn source_byte_len(&self) -> usize {
        self.source_byte_len
    }
}

/// Read the caller's explicit local path and select only its exact identity.
/// The same in-memory bytes are hashed and parsed; the file is not reread.
pub fn load_manifest(
    path: &Path,
    expected: LayoutIdentity,
) -> Result<AuthenticatedLayout, ManifestError> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(ManifestError::NotRegularFile);
    }
    if metadata.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(ManifestError::TooLarge {
            limit: MAX_MANIFEST_BYTES,
        });
    }
    let bytes = read_bounded(file, MAX_MANIFEST_BYTES)?;
    authenticate_and_select(&bytes, expected)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, ManifestError> {
    // Fixed storage, including one lookahead byte for growth after metadata.
    // No Vec growth depends on the reader's size hint or EOF behavior.
    let mut bytes = vec![0_u8; limit + 1];
    let mut used = 0;
    loop {
        if used == bytes.len() {
            return Err(ManifestError::TooLarge { limit });
        }
        match reader.read(&mut bytes[used..]) {
            Ok(0) => {
                bytes.truncate(used);
                return Ok(bytes);
            }
            Ok(count) => used += count,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ManifestError::Io(e)),
        }
    }
}

fn authenticate_and_select(
    bytes: &[u8],
    expected: LayoutIdentity,
) -> Result<AuthenticatedLayout, ManifestError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooLarge {
            limit: MAX_MANIFEST_BYTES,
        });
    }
    let sha256 = sha256(bytes)?;
    if digest_hex(&sha256) != SOURCE_MANIFEST_SHA256 {
        return Err(ManifestError::DigestMismatch { actual: sha256 });
    }
    // Unknown fields/other layouts are skipped only after complete artifact
    // authentication. The returned type cannot bypass this check publicly.
    let map = parse_selected_record(bytes, expected)?;
    Ok(AuthenticatedLayout {
        map,
        sha256,
        source_byte_len: bytes.len(),
    })
}

fn sha256(bytes: &[u8]) -> Result<[u8; 32], ManifestError> {
    let length = u32::try_from(bytes.len()).map_err(|_| ManifestError::TooLarge {
        limit: MAX_MANIFEST_BYTES,
    })?;
    let mut digest = [0_u8; 32];
    // The Windows SHA256 pseudo-handle is OS-owned and needs no provider
    // allocation/close. Input/output pointers remain valid for this synchronous
    // one-shot call; no target/vendor DLL is loaded.
    let status = unsafe {
        BCryptHash(
            BCRYPT_SHA256_ALG_HANDLE,
            std::ptr::null(),
            0,
            bytes.as_ptr(),
            length,
            digest.as_mut_ptr(),
            32,
        )
    };
    if status < 0 {
        return Err(ManifestError::Crypto(status));
    }
    Ok(digest)
}
fn digest_hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

// These borrowed/selective visitors retain only one bounded coordinate record.
// serde_json's default recursion limit remains enabled (128). No Value tree,
// full manifest struct, SVG or other-layout copy is constructed.
struct ManifestSeed<'a> {
    key: &'a str,
}
struct Selection {
    vendor_id: u16,
    product_id: u16,
    record: Option<RawLayout>,
}
impl<'de> DeserializeSeed<'de> for ManifestSeed<'_> {
    type Value = Selection;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Selection, D::Error> {
        d.deserialize_map(ManifestVisitor { key: self.key })
    }
}
struct ManifestVisitor<'a> {
    key: &'a str,
}
impl<'de> Visitor<'de> for ManifestVisitor<'_> {
    type Value = Selection;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a source manifest object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Selection, A::Error> {
        let mut vendor = None;
        let mut product = None;
        let mut layouts = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "vendorId" => {
                    if vendor.is_some() {
                        return Err(de::Error::duplicate_field("vendorId"));
                    }
                    vendor = Some(map.next_value::<u16>()?);
                }
                "productId" => {
                    if product.is_some() {
                        return Err(de::Error::duplicate_field("productId"));
                    }
                    product = Some(map.next_value::<u16>()?);
                }
                "ledConfig" => {
                    if layouts.is_some() {
                        return Err(de::Error::duplicate_field("ledConfig"));
                    }
                    layouts = Some(map.next_value_seed(LayoutsSeed { key: self.key })?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(Selection {
            vendor_id: vendor.ok_or_else(|| de::Error::missing_field("vendorId"))?,
            product_id: product.ok_or_else(|| de::Error::missing_field("productId"))?,
            record: layouts.ok_or_else(|| de::Error::missing_field("ledConfig"))?,
        })
    }
}
struct LayoutsSeed<'a> {
    key: &'a str,
}
impl<'de> DeserializeSeed<'de> for LayoutsSeed<'_> {
    type Value = Option<RawLayout>;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(LayoutsVisitor { key: self.key })
    }
}
struct LayoutsVisitor<'a> {
    key: &'a str,
}
impl<'de> Visitor<'de> for LayoutsVisitor<'_> {
    type Value = Option<RawLayout>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a layout record object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut selected = None;
        while let Some(key) = map.next_key::<String>()? {
            if key == self.key {
                if selected.is_some() {
                    return Err(de::Error::custom("duplicate exact selected layout key"));
                }
                selected = Some(map.next_value::<RawLayout>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(selected)
    }
}
#[derive(Deserialize)]
struct RawLayout {
    #[serde(rename = "VID")]
    vendor_id: u16,
    #[serde(rename = "PID")]
    product_id: u16,
    #[serde(rename = "EID")]
    edition_id: u16,
    #[serde(rename = "Layout")]
    layout_id: u16,
    #[serde(rename = "MatrixMaxRow")]
    logical_rows: u16,
    #[serde(rename = "MatrixMaxCol")]
    logical_cols: u16,
    #[serde(rename = "DeviceMaxRow")]
    physical_rows: u16,
    #[serde(rename = "DeviceMaxCol")]
    physical_cols: u16,
    #[serde(rename = "LedMatrix")]
    entries: BoundedEntries,
}
#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "MatrixPos")]
    logical: [u16; 2],
    #[serde(rename = "DevicePos")]
    physical: [u16; 2],
}
struct BoundedEntries(Vec<MappingEntry>);
impl<'de> Deserialize<'de> for BoundedEntries {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_seq(EntriesVisitor)
    }
}
struct EntriesVisitor;
impl<'de> Visitor<'de> for EntriesVisitor {
    type Value = BoundedEntries;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("at most 256 coordinate pairs")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        if seq.size_hint().is_some_and(|n| n > MAX_SOURCE_ENTRIES) {
            return Err(de::Error::custom("layout entry budget exceeded"));
        }
        let mut entries = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_SOURCE_ENTRIES));
        while entries.len() < MAX_SOURCE_ENTRIES {
            match seq.next_element::<RawEntry>()? {
                Some(e) => entries.push(MappingEntry {
                    logical: e.logical,
                    physical: e.physical,
                }),
                None => return Ok(BoundedEntries(entries)),
            }
        }
        if seq.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom("layout entry budget exceeded"));
        }
        Ok(BoundedEntries(entries))
    }
}

/// Private parser boundary; production calls it only after the SHA256 check.
/// Tests use synthetic documents here to exercise format rejection separately.
fn parse_selected_record(
    bytes: &[u8],
    expected: LayoutIdentity,
) -> Result<OrnataSpatialMap, ManifestError> {
    let key = format!("ledConfig_{}_{}", expected.edition_id, expected.layout_id);
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let selection = ManifestSeed { key: &key }.deserialize(&mut d)?;
    d.end()?;
    if selection.vendor_id != 0x1532 || selection.product_id != 0x025d {
        return Err(ManifestError::WrongArtifactIdentity {
            vendor_id: selection.vendor_id,
            product_id: selection.product_id,
        });
    }
    let record = selection
        .record
        .ok_or(ManifestError::MissingLayout(expected))?;
    let source = SourceLayoutRecord {
        identity: LayoutIdentity {
            vendor_id: record.vendor_id,
            product_id: record.product_id,
            edition_id: record.edition_id,
            layout_id: record.layout_id,
        },
        logical_dimensions: [record.logical_rows, record.logical_cols],
        physical_dimensions: [record.physical_rows, record.physical_cols],
        entries: &record.entries.0,
        manifest_sha256: SOURCE_MANIFEST_SHA256,
    };
    Ok(OrnataSpatialMap::from_record(expected, source)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn identity(layout: u16) -> LayoutIdentity {
        LayoutIdentity {
            vendor_id: 0x1532,
            product_id: 0x025d,
            edition_id: 0,
            layout_id: layout,
        }
    }
    fn record() -> Value {
        json!({"VID":5426,"PID":605,"EID":0,"Layout":6,"MatrixMaxRow":7,"MatrixMaxCol":23,"DeviceMaxRow":6,"DeviceMaxCol":22,"LedMatrix":[{"MatrixPos":[0,4],"DevicePos":[0,3]}]})
    }
    fn document(record: Value) -> Value {
        json!({"vendorId":5426,"productId":605,"ledConfig":{"ledConfig_0_6":record}})
    }
    fn decode(v: &Value) -> Result<OrnataSpatialMap, ManifestError> {
        parse_selected_record(&serde_json::to_vec(v).unwrap(), identity(6))
    }

    #[test]
    fn windows_sha256_matches_standard_vectors() {
        for (bytes, expected) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_slice(),
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            assert_eq!(digest_hex(&sha256(bytes).unwrap()), expected);
        }
    }

    #[test]
    fn bounded_read_accepts_limit_and_stops_after_one_lookahead_byte() {
        use std::io::Cursor;
        let mut reader = Cursor::new(vec![1; 100]);
        assert!(matches!(
            read_bounded(&mut reader, 8),
            Err(ManifestError::TooLarge { limit: 8 })
        ));
        assert_eq!(reader.position(), 9);
        assert_eq!(
            read_bounded(Cursor::new(vec![2; 8]), 8).unwrap(),
            vec![2; 8]
        );
        assert!(read_bounded(Cursor::new(Vec::<u8>::new()), 8)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn digest_failure_happens_before_json_parsing_or_layout_projection() {
        assert!(matches!(
            authenticate_and_select(b"{", identity(6)),
            Err(ManifestError::DigestMismatch { .. })
        ));
        assert!(matches!(
            authenticate_and_select(b"not JSON or the source artifact", identity(6)),
            Err(ManifestError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn selected_record_is_exact_and_unselected_data_is_not_projected() {
        let mut v = document(record());
        v["svg"] = json!({"bigUnusedString":"z".repeat(100_000)});
        v["ledConfig"]["ledConfig_0_1"] = json!({"invalidOtherLayout":true});
        let map = decode(&v).unwrap();
        assert_eq!(map.mapped_pixel_count(), 1);
        assert_eq!(map.provenance().identity, identity(6));
        assert!(matches!(
            parse_selected_record(&serde_json::to_vec(&v).unwrap(), identity(1)),
            Err(ManifestError::Json(_))
        ));
        assert!(matches!(
            parse_selected_record(&serde_json::to_vec(&v).unwrap(), identity(2)),
            Err(ManifestError::MissingLayout(_))
        ));
    }

    #[test]
    fn malformed_selected_types_dimensions_and_identity_fail() {
        for value in [json!(-1), json!(1.5), json!(65536), json!("6"), json!(null)] {
            let mut r = record();
            r["Layout"] = value;
            assert!(matches!(decode(&document(r)), Err(ManifestError::Json(_))));
        }
        let mut r = record();
        r["MatrixMaxRow"] = json!(6);
        assert!(matches!(
            decode(&document(r)),
            Err(ManifestError::Spatial(_))
        ));
        let mut r = record();
        r["EID"] = json!(1);
        assert!(matches!(
            decode(&document(r)),
            Err(ManifestError::Spatial(_))
        ));
        for pair in [
            json!([-1, 0]),
            json!([0, 23]),
            json!([0, 0, 0]),
            json!([0]),
            json!(["0", 0]),
        ] {
            let mut r = record();
            r["LedMatrix"][0]["MatrixPos"] = pair;
            assert!(decode(&document(r)).is_err());
        }
        let mut v = document(record());
        v["vendorId"] = json!(0);
        assert!(matches!(
            decode(&v),
            Err(ManifestError::WrongArtifactIdentity { .. })
        ));
    }

    #[test]
    fn entry_budget_duplicate_keys_and_trailing_json_are_rejected() {
        let mut r = record();
        r["LedMatrix"] = json!(vec![json!({"MatrixPos":[0,0],"DevicePos":[0,0]}); 257]);
        assert!(matches!(decode(&document(r)), Err(ManifestError::Json(_))));
        let body = serde_json::to_string(&record()).unwrap();
        let duplicate=format!("{{\"vendorId\":5426,\"productId\":605,\"ledConfig\":{{\"ledConfig_0_6\":{body},\"ledConfig_0_6\":{body}}}}}");
        assert!(parse_selected_record(duplicate.as_bytes(), identity(6)).is_err());
        let mut bytes = serde_json::to_vec(&document(record())).unwrap();
        bytes.extend_from_slice(b"{}");
        assert!(parse_selected_record(&bytes, identity(6)).is_err());
        let deep = format!(
            "{{\"vendorId\":5426,\"productId\":605,\"unknown\":{}0{},\"ledConfig\":{{}}}}",
            "[".repeat(160),
            "]".repeat(160)
        );
        assert!(parse_selected_record(deep.as_bytes(), identity(6)).is_err());
    }

    #[test]
    fn file_metadata_oversize_and_untrusted_file_never_return_authenticated_map() {
        let path = std::env::temp_dir().join(format!(
            "secbird-manifest-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        {
            let file = File::create(&path).unwrap();
            file.set_len(MAX_MANIFEST_BYTES as u64 + 1).unwrap();
        }
        assert!(matches!(
            load_manifest(&path, identity(6)),
            Err(ManifestError::TooLarge { .. })
        ));
        std::fs::write(&path, b"{").unwrap();
        assert!(matches!(
            load_manifest(&path, identity(6)),
            Err(ManifestError::DigestMismatch { .. })
        ));
    }

    #[test]
    #[ignore = "requires SECBIRD_ORNATA_MANIFEST_TEST_PATH pointing to the exact local pinned vendor manifest"]
    fn explicitly_selected_private_manifest_authenticates_all17_layouts() {
        let path = std::env::var_os("SECBIRD_ORNATA_MANIFEST_TEST_PATH")
            .expect("explicit private manifest path");
        for layout in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18, 20] {
            let loaded = load_manifest(Path::new(&path), identity(layout)).unwrap();
            assert_eq!(loaded.source_byte_len(), 2_895_966);
            assert_eq!(
                digest_hex(&loaded.authenticated_sha256()),
                SOURCE_MANIFEST_SHA256
            );
            assert_eq!(loaded.map().provenance().identity, identity(layout));
            assert_eq!(
                loaded.map().provenance().source_entry_count,
                if layout == 12 {
                    119
                } else if [1, 2, 5, 8, 9, 11, 20].contains(&layout) {
                    113
                } else {
                    114
                }
            );
            if layout == 6 {
                assert_eq!(
                    (
                        loaded.map().mapped_pixel_count(),
                        loaded.map().unmapped_pixel_count()
                    ),
                    (114, 18)
                );
            }
        }
    }
}
