//! Explicit exports create a new regular file; they never replace a store/file.
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Component, Path, Prefix},
};

fn validate(path: &Path) -> Result<(), String> {
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Choose a regular export filename")?;
    let stem = filename
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
    }) {
        return Err("Choose a regular filename, not a Windows device".into());
    }
    for component in path.components() {
        match component {
            Component::Prefix(prefix)
                if !matches!(
                    prefix.kind(),
                    Prefix::Disk(_)
                        | Prefix::UNC(..)
                        | Prefix::VerbatimDisk(_)
                        | Prefix::VerbatimUNC(..)
                ) =>
            {
                return Err("Device namespace export paths are unsupported".into())
            }
            Component::Normal(part) if part.to_string_lossy().contains(':') => {
                return Err("Alternate data stream export paths are unsupported".into())
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    validate(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Choose a new export filename: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Export destination is not a regular file".into());
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("XML export failed; the new file may be incomplete: {e}"))
}

pub(super) fn suggested_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let stem = cleaned.trim_end_matches(['.', ' ']);
    let filename = format!("{stem}.xml");
    if stem.is_empty() || validate(Path::new(&filename)).is_err() {
        "Macro.xml".into()
    } else {
        filename
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_export_never_replaces_existing_files_or_opens_devices_and_streams() {
        let folder = std::env::temp_dir().join(format!(
            "xml-output-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let original = folder.join("profiles.json");
        std::fs::write(&original, b"preserve profile bytes").unwrap();
        assert!(write_new(&original, b"replacement").is_err());
        assert!(write_new(&folder.join("profiles.json:alternate"), b"stream").is_err());
        assert_eq!(std::fs::read(&original).unwrap(), b"preserve profile bytes");
        for path in [
            "NUL",
            "con.xml",
            "COM1.xml",
            "LPT9",
            r"\\.\NUL",
            r"\\?\GLOBALROOT\Device\Null",
        ] {
            assert!(validate(Path::new(path)).is_err());
        }
        let output = folder.join("new.xml");
        write_new(&output, b"<Macro/>").unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"<Macro/>");
        assert!(write_new(&output, b"overwrite").is_err());
        std::fs::remove_file(&output).unwrap();
        std::fs::remove_file(&original).unwrap();
        std::fs::remove_dir(&folder).unwrap();
    }
    #[test]
    fn xml_export_suggests_usable_names_without_losing_unicode() {
        for name in ["...", "CON", "nul", "LPT1"] {
            assert_eq!(suggested_name(name), "Macro.xml");
        }
        assert_eq!(suggested_name("My: macro? 😃. "), "My_ macro_ 😃.xml");
    }
}
