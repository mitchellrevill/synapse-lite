//! Local Studio interchange. No device discovery, profile activation or network.
use secbird_core::{studio::Archive, Error};
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::InvalidArg(format!("Studio file: {message}"))
}

pub(super) fn run(args: &[String]) -> Result<(), Error> {
    match args.first().map(String::as_str) {
        Some("inspect") if args.len() == 2 => {
            let archive = Archive::read(File::open(&args[1]).map_err(invalid)?)?;
            let compositions: Vec<_> = (0..archive.profile_count())
                .map(|index| match archive.profile_header(index) {
                    Ok(header) => json!({"number":index+1,"id":header.id,"title":header.title}),
                    Err(error) => json!({"number":index+1,"metadata_notice":error.to_string()}),
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "format":"ChromaEffects JSON version1","compositions":compositions,
                    "device_compatibility_checked":false,"lighting_applied":false
                }))
                .map_err(invalid)?
            );
            Ok(())
        }
        Some("starlight") if args.len() == 4 => {
            let archive = Archive::read(File::open(&args[1]).map_err(invalid)?)?;
            let index = args[2]
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .ok_or_else(|| invalid("select a positive composition number"))?;
            let draft = secbird_core::studio::document::Draft::open(
                archive.profile_json(index)
                    .ok_or_else(|| invalid("unknown composition number"))?,
            )?;
            let descriptors = draft.starlight_region_descriptors(&args[3])?;
            // Serialize RawValue stops directly; routing through json!/Value
            // would round opaque imported numeric fields in their metadata.
            let descriptors_json = serde_json::to_string(&descriptors).map_err(invalid)?;
            println!(
                "{{\n  \"effect_id\":7,\n  \"region_descriptors\":{descriptors_json},\n  \"animation_implemented\":false,\n  \"lighting_applied\":false,\n  \"device_compatibility_checked\":false\n}}"
            );
            Ok(())
        }
        Some("export") if args.len() >= 4 => {
            let selection = if args[3..] == ["all"] {
                None
            } else {
                Some(
                    args[3..]
                        .iter()
                        .map(|text| {
                            text.parse::<usize>()
                                .ok()
                                .and_then(|value| value.checked_sub(1))
                                .ok_or_else(|| {
                                    invalid("select all or positive composition numbers")
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            };
            export(
                Path::new(&args[1]),
                Path::new(&args[2]),
                selection.as_deref(),
            )?;
            println!("Studio file saved. No lighting was applied.");
            Ok(())
        }
        _ => Err(invalid(
            "use studio inspect FILE, studio starlight FILE NUMBER LAYER_ID or studio export INPUT NEW_OUTPUT all|NUMBER...",
        )),
    }
}

fn export(input: &Path, output: &Path, selected: Option<&[usize]>) -> Result<(), Error> {
    let archive = Archive::read(File::open(input).map_err(invalid)?)?;
    let bytes = match selected {
        Some(indices) => archive.export_selected(indices)?,
        None => archive.to_bytes()?,
    };
    // Validation/encoding completes before the only file mutation. Existing
    // files (including aliases of the input) are never truncated or replaced.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| invalid(format!("cannot create {}: {error}", output.display())))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            invalid(format!(
                "new export {} may be incomplete: {error}",
                output.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let folder = std::env::temp_dir().join(format!(
                "studio-files-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&folder).unwrap();
            Self(folder)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn starlight_inspection_keeps_archive_bytes_and_rejects_invalid_selection() {
        let fixture = Fixture::new();
        let input = fixture.0.join("source.ChromaEffects");
        let original = br#"{"version":1,"profiles":[{"id":"p","title":"Stars","devices":[],"layers":[{"id":"s","title":"Starlight","type":"layer","name":"starlight","value":7,"params":{"density":7,"duration":500,"colorStops":[{"Stop":0,"Color":1},{"Stop":100}]},"regions":[{"devices":[]}]}]}]}"#;
        std::fs::write(&input, original).unwrap();
        for (number, layer, accepted) in [
            ("1", "s", true), ("0", "s", false), ("2", "s", false),
            ("1", "missing", false), ("garbage", "s", false),
        ] {
            let args = ["starlight", input.to_str().unwrap(), number, layer]
                .into_iter().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(run(&args).is_ok(), accepted);
            assert_eq!(std::fs::read(&input).unwrap(), original);
        }
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn file_export_preserves_input_and_never_overwrites_an_existing_target() {
        let fixture = Fixture::new();
        let input = fixture.0.join("source.ChromaEffects");
        let output = fixture.0.join("selected.ChromaEffects");
        let original = br#"{"version":1,"profiles":[{"id":"one","title":"Desk","future":9.990e+20},{"id":"two"}],"extra":{"keep":true}}"#;
        std::fs::write(&input, original).unwrap();
        export(&input, &output, Some(&[0])).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("9.990e+20"));
        let selected = Archive::parse(&bytes).unwrap();
        assert_eq!(selected.profile_count(), 1);
        assert_eq!(
            selected.profile_header(0).unwrap().title.as_deref(),
            Some("Desk")
        );
        assert!(export(&input, &output, None).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
        assert!(export(&input, &input, None).is_err());
        assert_eq!(std::fs::read(&input).unwrap(), original);
    }
    #[test]
    fn rejected_data_or_selection_creates_no_export_file() {
        let fixture = Fixture::new();
        let input = fixture.0.join("source.ChromaEffects");
        let output = fixture.0.join("new.ChromaEffects");
        std::fs::write(&input, br#"{"version":1,"profiles":[{}]}"#).unwrap();
        for indices in [vec![], vec![1], vec![0, 0]] {
            assert!(export(&input, &output, Some(&indices)).is_err());
            assert!(!output.exists());
        }
        std::fs::write(&input, b"PK\x03\x04").unwrap();
        assert!(export(&input, &output, None).is_err());
        assert!(!output.exists());
        for args in [
            vec![],
            vec!["export"],
            vec!["inspect"],
            vec!["export", "missing", "new", "0"],
            vec!["export", "missing", "new", "all", "1"],
        ] {
            assert!(run(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
}
