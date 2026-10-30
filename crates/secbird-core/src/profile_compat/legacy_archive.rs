//! Bounded ZIP structural inspection only. No legacy profile schema is inferred.
use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::BTreeSet;

pub const MAX_ARCHIVE_BYTES: usize = 2 * 1024 * 1024;
const MAX_MEMBERS: usize = 256;
const MAX_EXPANDED: usize = 8 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct ArchivePreview {
    pub members: Vec<ArchiveMember>,
    pub expanded_bytes: usize,
    pub profile_conversion_supported: bool,
}
#[derive(Debug, Serialize)]
pub struct ArchiveMember {
    pub name: String,
    pub bytes: usize,
}
fn bad() -> Error {
    Error::InvalidArg("Unsafe, malformed or unsupported ZIP archive (stored UTF-8 members only; no ZIP64/encryption)".into())
}
fn u16at(b: &[u8], p: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(b.get(p..p + 2).ok_or_else(bad)?.try_into().unwrap()) as usize)
}
fn u32at(b: &[u8], p: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(b.get(p..p + 4).ok_or_else(bad)?.try_into().unwrap()) as usize)
}
fn crc32(b: &[u8]) -> usize {
    let mut crc = !0u32;
    for byte in b {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320u32 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    (!crc) as usize
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 512
        && !name.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && !name.chars().any(|c| c.is_control() || !c.is_ascii())
        && {
            let path = name.strip_suffix('/').unwrap_or(name);
            path.split('/').all(|c| {
                !c.is_empty() && c != "." && c != ".." && !c.ends_with(['.', ' ']) && {
                    let stem = c.split('.').next().unwrap().to_ascii_uppercase();
                    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        && !(stem.len() == 4
                            && (stem.starts_with("COM") || stem.starts_with("LPT"))
                            && stem.as_bytes()[3].is_ascii_digit())
                }
            })
        }
}
/// Inspects bytes without extraction, record decoding, filesystem writes or store mutation.
/// Deliberately accepts a small ZIP subset until real legacy fixtures establish a codec.
pub fn preview_archive(b: &[u8]) -> Result<ArchivePreview> {
    if b.len() > MAX_ARCHIVE_BYTES || b.len() < 22 {
        return Err(bad());
    }
    // Comments are not needed for inspection; requiring EOCD at the end avoids ambiguous signatures.
    let end = b.len() - 22;
    if b.get(end..end + 4) != Some(b"PK\x05\x06")
        || u16at(b, end + 4)? != 0
        || u16at(b, end + 6)? != 0
        || u16at(b, end + 20)? != 0
    {
        return Err(bad());
    }
    let count = u16at(b, end + 10)?;
    if count == 0 || count > MAX_MEMBERS || u16at(b, end + 8)? != count {
        return Err(bad());
    }
    let start = u32at(b, end + 16)?;
    if start.checked_add(u32at(b, end + 12)?) != Some(end) {
        return Err(bad());
    }
    let mut p = start;
    let mut local = 0;
    let mut total = 0usize;
    let mut seen = BTreeSet::new();
    let mut members = Vec::new();
    for _ in 0..count {
        if b.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err(bad());
        }
        let flags = u16at(b, p + 8)?;
        let method = u16at(b, p + 10)?;
        let packed = u32at(b, p + 20)?;
        let size = u32at(b, p + 24)?;
        total = total.checked_add(size).ok_or_else(bad)?;
        if total > MAX_EXPANDED
            || size > packed.saturating_mul(100)
            || method != 0
            || size != packed
            || flags & !0x800 != 0
        {
            return Err(bad());
        }
        let n = u16at(b, p + 28)?;
        let name_bytes = b.get(p + 46..p + 46 + n).ok_or_else(bad)?;
        let name = std::str::from_utf8(name_bytes).map_err(|_| bad())?;
        if !safe_name(name)
            || !seen.insert(name.trim_end_matches('/').to_ascii_lowercase())
            || u16at(b, p + 34)? != 0
            || u32at(b, p + 42)? != local
        {
            return Err(bad());
        }
        // Reject links and special Unix files, regardless of creator platform.
        let mode = (u32at(b, p + 38)? >> 16) & 0xf000;
        if !matches!(mode, 0 | 0x4000 | 0x8000) {
            return Err(bad());
        }
        if b.get(local..local + 4) != Some(b"PK\x03\x04")
            || u16at(b, local + 6)? != flags
            || u16at(b, local + 8)? != method
            || u32at(b, local + 14)? != u32at(b, p + 16)?
            || u32at(b, local + 18)? != packed
            || u32at(b, local + 22)? != size
            || u16at(b, local + 26)? != n
        {
            return Err(bad());
        }
        if b.get(local + 30..local + 30 + n) != Some(name_bytes) {
            return Err(bad());
        }
        let data = local + 30 + n + u16at(b, local + 28)?;
        local = data.checked_add(size).ok_or_else(bad)?;
        if local > start
            || crc32(b.get(data..local).ok_or_else(bad)?) != u32at(b, p + 16)?
            || (name.ends_with('/') && size != 0)
        {
            return Err(bad());
        }
        members.push(ArchiveMember {
            name: name.into(),
            bytes: size,
        });
        p = p
            .checked_add(46 + n + u16at(b, p + 30)? + u16at(b, p + 32)?)
            .ok_or_else(bad)?;
        if p > end {
            return Err(bad());
        }
    }
    if p != end || local != start {
        return Err(bad());
    }
    Ok(ArchivePreview {
        members,
        expanded_bytes: total,
        profile_conversion_supported: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic ZIP structure tests, not Synapse fixtures or compatibility evidence.
    fn archive(names: &[&str]) -> Vec<u8> {
        let mut b = Vec::new();
        let mut central = Vec::new();
        for name in names {
            let offset = b.len();
            let n = name.len();
            let mut l = vec![0u8; 30];
            l[..4].copy_from_slice(b"PK\x03\x04");
            l[26..28].copy_from_slice(&(n as u16).to_le_bytes());
            b.extend(l);
            b.extend(name.as_bytes());
            let mut c = vec![0u8; 46];
            c[..4].copy_from_slice(b"PK\x01\x02");
            c[28..30].copy_from_slice(&(n as u16).to_le_bytes());
            c[42..46].copy_from_slice(&(offset as u32).to_le_bytes());
            central.extend(c);
            central.extend(name.as_bytes());
        }
        let offset = b.len();
        let size = central.len();
        b.extend(central);
        let mut e = vec![0u8; 22];
        e[..4].copy_from_slice(b"PK\x05\x06");
        e[8..10].copy_from_slice(&(names.len() as u16).to_le_bytes());
        e[10..12].copy_from_slice(&(names.len() as u16).to_le_bytes());
        e[12..16].copy_from_slice(&(size as u32).to_le_bytes());
        e[16..20].copy_from_slice(&(offset as u32).to_le_bytes());
        b.extend(e);
        b
    }
    #[test]
    fn preview_is_explicitly_non_converting() {
        let b = archive(&["profiles/a.xml", "macros/b.xml"]);
        let r = preview_archive(&b).unwrap();
        assert_eq!(r.members.len(), 2);
        assert!(!r.profile_conversion_supported);
    }
    #[test]
    fn rejects_path_aliases_and_duplicates() {
        for names in [
            vec!["../x"],
            vec!["/x"],
            vec!["C:x"],
            vec!["a\\b"],
            vec!["a", "A"],
            vec!["a/", "a"],
            vec!["CON.xml"],
            vec!["a."],
            vec!["a?b"],
            vec!["a|b"],
        ] {
            assert!(preview_archive(&archive(&names)).is_err(), "{names:?}");
        }
    }
    #[test]
    fn every_truncation_fails() {
        let b = archive(&["a"]);
        for n in 0..b.len() {
            assert!(preview_archive(&b[..n]).is_err());
        }
    }
    #[test]
    fn rejects_limits_and_unsupported_features() {
        assert!(preview_archive(&archive(&vec!["a"; 257])).is_err());
        let b = archive(&["a"]);
        for (offset, value) in [(6, 1), (8, 8), (18, 1)] {
            let mut c = b.clone();
            c[offset] = value;
            assert!(preview_archive(&c).is_err());
        }
        assert!(preview_archive(&vec![0; MAX_ARCHIVE_BYTES + 1]).is_err());
    }
    #[test]
    fn validates_nonempty_payload_crc_and_header_agreement() {
        let mut b = archive(&["a"]);
        b.splice(31..31, [b'x']);
        let crc = crc32(b"x") as u32;
        for p in [14, 32 + 16] {
            b[p..p + 4].copy_from_slice(&crc.to_le_bytes());
        }
        for p in [18, 22, 32 + 20, 32 + 24] {
            b[p..p + 4].copy_from_slice(&1u32.to_le_bytes());
        }
        let end = b.len() - 22;
        b[end + 16..end + 20].copy_from_slice(&32u32.to_le_bytes());
        assert_eq!(preview_archive(&b).unwrap().expanded_bytes, 1);
        b[31] = b'y';
        assert!(preview_archive(&b).is_err());
    }
    #[test]
    fn hostile_central_metadata_never_bypasses_limits() {
        let original = archive(&["a"]);
        let c = 31;
        for (offset, value) in [
            (c + 24, 9 * 1024 * 1024u32),
            (c + 20, 0xffffffff),
            (c + 42, 1),
            (c + 38, 0xa0000000),
        ] {
            let mut b = original.clone();
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(preview_archive(&b).is_err());
        }
        for index in 0..original.len() {
            let mut b = original.clone();
            b[index] ^= 0xff;
            let _ = preview_archive(&b); // malformed bytes must never panic
        }
    }
}
