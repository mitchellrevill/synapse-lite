//! Saved source macro inspection, routed before device discovery in main.
use secbird_core::{
    profile_compat::{
        self,
        macro_library::planning::{self, CacheStatus},
    },
    Error,
};
use serde_json::{json, Value};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::InvalidArg(format!("Source macro file: {message}"))
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let file = File::open(path).map_err(invalid)?;
    if !file.metadata().map_err(invalid)?.is_file() {
        return Err(invalid("choose a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() > limit {
        return Err(invalid("input exceeds its byte bound"));
    }
    Ok(bytes)
}
pub(super) fn run(args: &[String]) -> Result<(), Error> {
    if args.len() == 2 && args[0] == "list" {
        let report = listing(Path::new(&args[1]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(invalid)?
        );
        return Ok(());
    }
    if args.len() != 4 || args[0] != "plan" {
        return Err(invalid(
            "use source-macro list PROFILE_DOCUMENT or source-macro plan PROFILE_DOCUMENT BINDING_JSON NEW_OUTPUT",
        ));
    }
    let count = plan(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
    )?;
    println!(
        "{}",
        json!({"native_events":count,"input_sent":false,"normal_store_changed":false,"saved_plan":args[3]})
    );
    Ok(())
}
fn read_store(input: &Path) -> Result<secbird_core::Store, Error> {
    let document = read(input, 2 * 1024 * 1024)?;
    Ok(profile_compat::import_document(&document)?.store)
}
fn listing(input: &Path) -> Result<Value, Error> {
    let library = profile_compat::macro_library::read(&read_store(input)?)?;
    let macros: Vec<_> = library
        .entries
        .iter()
        .map(|entry| {
            json!({
                "guid":entry.guid,"name":entry.name,"source_row_count":entry.events.len(),
                "referenced":entry.referenced,"record_revision":entry.revision
            })
        })
        .collect();
    Ok(json!({"macros":macros,"missing_references":library.missing,
        "dependency_issues":library.dependency_issues,"hardware_discovery_performed":false,
        "input_sent":false,"normal_store_changed":false,"live_playback_available":false}))
}
fn plan(input: &Path, binding: &Path, output: &Path) -> Result<usize, Error> {
    let store = read_store(input)?;
    let binding = read(binding, 4096)?;
    let binding = binding
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&binding);
    let binding = std::str::from_utf8(binding).map_err(invalid)?;
    let prepared = planning::prepare(&store, binding)?;
    let events: Value = serde_json::from_str(prepared.native_json()).map_err(invalid)?;
    let count = prepared.plan().events().len();
    let report = json!({"version":1,"guid":prepared.guid(),"name":prepared.name(),"record_revision":prepared.revision(),
        "cache":match prepared.cache_status(){CacheStatus::Generated=>"generated",CacheStatus::Verified=>"verified"},
        "source_row_count":prepared.source_row_count(),"source_rows_zero_based":prepared.source_rows(),"native_events":events,
        "repeats":prepared.plan().repeats(),"explicit_delay_ms":prepared.plan().explicit_delay_ms(),
        "runtime_gaps":prepared.plan().runtime_gaps().iter().map(|gap|format!("{gap:?}")).collect::<Vec<_>>(),
        "hardware_discovery_performed":false,"input_sent":false,"normal_store_changed":false,"live_playback_available":false});
    let bytes = serde_json::to_vec_pretty(&report).map_err(invalid)?;
    // All validation completes before the only mutation. Existing files and
    // aliases of either input cannot be truncated or replaced.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(invalid)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| invalid(format!("new report may be incomplete: {e}")))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        profile_compat::macro_library::{self, Change},
        Store,
    };
    use std::path::PathBuf;
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "source-macro-plan-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn inputs(&self) -> (PathBuf, PathBuf) {
            let store = macro_library::edit(
                &Store::default(),
                Change::Create {
                    guid: "test".into(),
                    name: "File test".into(),
                },
            )
            .unwrap();
            let store = macro_library::edit(
                &store,
                Change::InsertDelay {
                    guid: "test".into(),
                    index: 1,
                    milliseconds: 2,
                },
            )
            .unwrap();
            let input = self.path("profiles.json");
            let binding = self.path("binding.json");
            std::fs::write(&input, store.native_bytes().unwrap()).unwrap();
            std::fs::write(&binding, br#"{"guid":"test","macroPlaybackOption":"Once"}"#).unwrap();
            (input, binding)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let target = self.0.canonicalize().unwrap();
            let temp = std::env::temp_dir().canonicalize().unwrap();
            assert!(
                target.starts_with(&temp)
                    && target != temp
                    && target
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("source-macro-plan-")
            );
            std::fs::remove_dir_all(target).unwrap();
        }
    }
    #[test]
    fn read_only_command_writes_a_bound_plan_before_hardware_discovery() {
        let folder = Folder::new();
        let (input, binding) = folder.inputs();
        let output = folder.path("plan.json");
        let original = std::fs::read(&input).unwrap();
        let original_binding = std::fs::read(&binding).unwrap();
        super::super::run(vec![
            "source-macro".into(),
            "plan".into(),
            input.to_string_lossy().into(),
            binding.to_string_lossy().into(),
            output.to_string_lossy().into(),
        ])
        .unwrap();
        let report: Value = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(report["native_events"], json!([{"type":"delay","ms":2}]));
        assert_eq!(report["source_rows_zero_based"], json!([1]));
        assert_eq!(report["hardware_discovery_performed"], false);
        assert_eq!(report["live_playback_available"], false);
        assert!(!report["runtime_gaps"].as_array().unwrap().is_empty());
        assert_eq!(std::fs::read(input).unwrap(), original);
        assert_eq!(std::fs::read(binding).unwrap(), original_binding);
    }
    #[test]
    fn local_listing_preserves_the_document_and_does_not_claim_execution() {
        let folder = Folder::new();
        let (input, _) = folder.inputs();
        let before = std::fs::read(&input).unwrap();
        let report = listing(&input).unwrap();
        assert_eq!(report["macros"][0]["guid"], "test");
        assert_eq!(report["macros"][0]["source_row_count"], 2);
        assert_eq!(report["live_playback_available"], false);
        assert_eq!(report["hardware_discovery_performed"], false);
        super::super::run(vec![
            "source-macro".into(),
            "list".into(),
            input.to_string_lossy().into(),
        ])
        .unwrap();
        assert_eq!(std::fs::read(input).unwrap(), before);
    }
    #[test]
    fn existing_outputs_and_aliases_are_never_overwritten() {
        let folder = Folder::new();
        let (input, binding) = folder.inputs();
        let output = folder.path("keep.json");
        std::fs::write(&output, b"keep").unwrap();
        let original = std::fs::read(&input).unwrap();
        let original_binding = std::fs::read(&binding).unwrap();
        for path in [&output, &input, &binding] {
            assert!(plan(&input, &binding, path).is_err());
        }
        assert_eq!(std::fs::read(output).unwrap(), b"keep");
        assert_eq!(std::fs::read(input).unwrap(), original);
        assert_eq!(std::fs::read(binding).unwrap(), original_binding);
    }
    #[test]
    fn invalid_documents_bindings_limits_and_device_selectors_create_no_report() {
        let folder = Folder::new();
        let (input, binding) = folder.inputs();
        let output = folder.path("absent.json");
        std::fs::write(
            &binding,
            br#"{"guid":"wrong","guid":"test","macroPlaybackOption":"Once"}"#,
        )
        .unwrap();
        assert!(plan(&input, &binding, &output).is_err());
        std::fs::write(&binding, vec![b' '; 4097]).unwrap();
        assert!(plan(&input, &binding, &output).is_err());
        std::fs::write(&input, b"{}").unwrap();
        assert!(plan(&input, &binding, &output).is_err());
        std::fs::write(&input, vec![b' '; 2 * 1024 * 1024 + 1]).unwrap();
        assert!(plan(&input, &binding, &output).is_err());
        assert!(run(&[]).is_err());
        assert!(super::super::run(vec![
            "-d".into(),
            "00C4".into(),
            "source-macro".into(),
            "plan".into(),
            input.to_string_lossy().into(),
            binding.to_string_lossy().into(),
            output.to_string_lossy().into()
        ])
        .is_err());
        assert!(!output.exists());
    }
}
