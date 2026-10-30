//! Local compatibility recovery. Does not discover devices or install a store.
use secbird_core::{profile_compat, Error, Store};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn invalid(value: impl std::fmt::Display) -> Error {
    Error::InvalidArg(format!("Profile recovery: {value}"))
}
fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![];
    File::open(path)
        .map_err(invalid)?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(invalid("document exceeds2MiB"));
    }
    Ok(bytes)
}
pub(super) fn run(args: &[String]) -> Result<(), Error> {
    if args.len() == 2 && args[0] == "inspect-archive" {
        let bytes = read(Path::new(&args[1]))?;
        let report = profile_compat::legacy_archive::preview_archive(&bytes)?;
        println!("{}", serde_json::to_string_pretty(&report).map_err(invalid)?);
        return Ok(());
    }
    if args.len() != 5 || args[0] != "recover-macros" {
        return Err(invalid(
            "use profiles recover-macros NATIVE_BACKUP SOURCE_EXPORT EXACT_DEVICE_KEY NEW_OUTPUT",
        ));
    }
    let (added, unchanged, missing) = recover(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &args[3],
        Path::new(&args[4]),
    )?;
    println!(
        "{}",
        serde_json::json!({"added_definitions":added,"unchanged_definitions":unchanged,"remaining_direct_references":missing,"profile_settings_or_activation_changed":false,"macro_execution_implemented":false,"normal_store_installed":false})
    );
    Ok(())
}
fn recover(
    backup: &Path,
    source: &Path,
    target: &str,
    output: &Path,
) -> Result<(usize, usize, usize), Error> {
    let backup = read(backup)?;
    let backup = backup.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&backup);
    let value: serde_json::Value = serde_json::from_slice(backup).map_err(invalid)?;
    if !value
        .get("devices")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid("first input must be a native backup"));
    }
    let current: Store = serde_json::from_value(value).map_err(invalid)?;
    let source = read(source)?;
    let report = profile_compat::recover_macro_dependencies(&current, target, &source)?;
    let bytes = report.store.native_bytes()?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(invalid)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| invalid(format!("new candidate may be incomplete: {error}")))?;
    Ok((
        report.added.len(),
        report.unchanged.len(),
        report.missing.len(),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_sources_and_existing_outputs_never_overwrite_files() {
        let folder = std::env::temp_dir().join(format!(
            "macro-file-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let backup = folder.join("native.json");
        let bad = folder.join("bad.json");
        let output = folder.join("output.json");
        std::fs::write(&backup, Store::default().native_bytes().unwrap()).unwrap();
        std::fs::write(&bad, b"{}").unwrap();
        std::fs::write(&output, b"keep this").unwrap();
        let original = std::fs::read(&backup).unwrap();
        assert!(recover(&backup, &bad, "missing-device", &output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"keep this");
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        let source = include_bytes!("../../secbird-core/tests/fixtures/synthetic.synapse4");
        let current = profile_compat::import_document(source).unwrap().store;
        let key = current.devices.keys().next().unwrap();
        std::fs::write(&backup, current.native_bytes().unwrap()).unwrap();
        std::fs::write(&bad, source).unwrap();
        let before = std::fs::read(&backup).unwrap();
        assert!(recover(&backup, &bad, key, &output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"keep this");
        let candidate = folder.join("new.json");
        recover(&backup, &bad, key, &candidate).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), before);
        let decoded: Store = serde_json::from_slice(&std::fs::read(&candidate).unwrap()).unwrap();
        assert_eq!(decoded.devices[key].active, current.devices[key].active);
        std::fs::remove_file(candidate).unwrap();
        assert!(run(&[]).is_err());
        std::fs::remove_file(backup).unwrap();
        std::fs::remove_file(bad).unwrap();
        std::fs::remove_file(output).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
