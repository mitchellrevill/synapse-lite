//! Offline Linked Games migration, dispatched before hardware discovery.
use secbird_core::{linked_games::migration, Error, Store};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidArg(format!("Linked Games migration: {error}"))
}
fn read<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(invalid)?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(invalid("input exceeds 2 MiB"));
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    serde_json::from_slice(bytes).map_err(invalid)
}
pub(super) fn run(args: &[String]) -> Result<(), Error> {
    let prepare = args.first().is_some_and(|s| s == "prepare");
    if !(prepare && args.len() == 5
        || args.first().is_some_and(|s| s == "preview") && args.len() == 4)
    {
        return Err(invalid("use linked-games preview BACKUP SOURCE SELECTIONS or linked-games prepare BACKUP SOURCE SELECTIONS NEW_OUTPUT"));
    }
    let value: serde_json::Value = read(&args[1])?;
    if !value
        .get("devices")
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(invalid("native backup required"));
    }
    let store: Store = serde_json::from_value(value).map_err(invalid)?;
    let source = read(&args[2])?;
    let selections: Vec<migration::Selection> = read(&args[3])?;
    let report = migration::preview(&store, &source, &selections)?;
    if prepare {
        let bytes = report.candidate.native_bytes()?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[4])
            .map_err(invalid)?;
        output
            .write_all(&bytes)
            .and_then(|_| output.sync_all())
            .map_err(invalid)?;
    }
    println!(
        "{}",
        serde_json::json!({"associations":report.associations,"candidate_written":prepare,"normal_store_installed":false,"profiles_activated":false})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_and_prepare_never_replace_input_or_existing_output() {
        let folder = std::env::temp_dir().join(format!(
            "linked-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let backup = folder.join("backup.json");
        let source = folder.join("source.json");
        let choices = folder.join("choices.json");
        let output = folder.join("candidate.json");
        let bytes = Store::default().native_bytes().unwrap();
        std::fs::write(&backup, &bytes).unwrap();
        std::fs::write(
            &source,
            br#"{"gameList":[],"linkedGames":[],"foreign":true}"#,
        )
        .unwrap();
        std::fs::write(&choices, b"[]").unwrap();
        let mut args = vec![
            "preview".into(),
            backup.to_string_lossy().into_owned(),
            source.to_string_lossy().into_owned(),
            choices.to_string_lossy().into_owned(),
        ];
        run(&args).unwrap();
        assert!(!output.exists());
        args[0] = "prepare".into();
        args.push(output.to_string_lossy().into_owned());
        run(&args).unwrap();
        let candidate = std::fs::read(&output).unwrap();
        assert!(run(&args).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), candidate);
        assert_eq!(std::fs::read(&backup).unwrap(), bytes);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
