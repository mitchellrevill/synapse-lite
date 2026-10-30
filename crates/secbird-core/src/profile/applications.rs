//! Local executable identities. No process launching, file lookup or link activation.
use super::*;

/// Normalize an unambiguous absolute Windows executable path without resolving it.
/// ASCII case/slashes are insignificant. Unicode spelling is retained: non-ASCII
/// case aliases, junctions and short names are not guessed.
pub fn normalize_app_path(input: &str) -> Result<String> {
    let mut path = input.trim().replace('/', "\\");
    if path.encode_utf16().count() > 32767 {
        return Err(invalid("executable path exceeds the Windows path limit"));
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        path = if rest
            .get(..4)
            .is_some_and(|s| s.eq_ignore_ascii_case("UNC\\"))
        {
            format!(r"\\{}", &rest[4..])
        } else {
            rest.to_string()
        };
    }
    let (parts, minimum) = if let Some(rest) = path.strip_prefix(r"\\") {
        (rest.split('\\').collect::<Vec<_>>(), 3)
    } else {
        let bytes = path.as_bytes();
        if bytes.len() < 3
            || !bytes[0].is_ascii_alphabetic()
            || bytes[1] != b':'
            || bytes[2] != b'\\'
        {
            return Err(invalid("choose an absolute drive or UNC executable path"));
        }
        (path[3..].split('\\').collect::<Vec<_>>(), 1)
    };
    if parts.len() < minimum
        || parts.iter().any(|part| {
            part.is_empty()
                || matches!(*part, "." | "..")
                || part.ends_with(['.', ' '])
                || part
                    .chars()
                    .any(|c| c.is_control() || matches!(c, ':' | '*' | '?' | '<' | '>' | '|' | '"'))
                || reserved(part)
        })
        || !parts
            .last()
            .is_some_and(|part| part.to_ascii_lowercase().ends_with(".exe"))
    {
        return Err(invalid(
            "executable path contains ambiguous or unsupported Windows components",
        ));
    }
    Ok(path.to_ascii_lowercase())
}
fn reserved(part: &str) -> bool {
    let name = part.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(
        name.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || name
        .strip_prefix("COM")
        .or_else(|| name.strip_prefix("LPT"))
        .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}
fn invalid(message: &str) -> Error {
    Error::InvalidArg(format!("Application link: {message}"))
}
fn path_like(value: &str) -> bool {
    value.contains(['\\', '/', ':'])
}

/// Foreground observations may be an absolute path or a legacy executable name.
pub fn foreground_app_identity(value: &str) -> Option<String> {
    if path_like(value) {
        normalize_app_path(value).ok()
    } else {
        let name = normalize_app(value);
        (!name.is_empty()).then_some(name)
    }
}
pub(super) fn profile_for<'a>(profiles: &'a DeviceProfiles, foreground: &str) -> Option<&'a str> {
    if path_like(foreground) {
        let path = normalize_app_path(foreground).ok()?;
        let mut matches = profiles.profiles.iter().filter(|(_, p)| {
            p.app_paths
                .iter()
                .any(|s| normalize_app_path(s).ok().as_ref() == Some(&path))
        });
        let first = matches.next();
        // A hand-edited ambiguous exact binding must not choose a profile by name order.
        if matches.next().is_some() {
            return None;
        }
        if let Some((name, _)) = first {
            return Some(name);
        }
    }
    let name = normalize_app(foreground);
    profiles
        .profiles
        .iter()
        .find(|(_, p)| p.apps.iter().any(|app| normalize_app(app) == name))
        .map(|(name, _)| name.as_str())
}
impl DeviceProfiles {
    /// Set exact paths without changing active settings or name-based bindings.
    /// A path can belong to one profile per device; other devices are independent.
    pub fn set_app_paths(&mut self, name: &str, paths: &[String]) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(Error::NotFound(format!("profile '{name}'")));
        }
        if paths.len() > 128 {
            return Err(invalid("a profile supports at most128 executable paths"));
        }
        let mut paths = paths
            .iter()
            .map(|path| normalize_app_path(path))
            .collect::<Result<Vec<_>>>()?;
        paths.sort();
        paths.dedup();
        for (other, profile) in &mut self.profiles {
            if other != name {
                profile.app_paths.retain(|old| {
                    normalize_app_path(old)
                        .ok()
                        .is_none_or(|old| !paths.contains(&old))
                });
            }
        }
        self.profiles.get_mut(name).unwrap().app_paths = paths;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_path_identity_is_explicit_and_never_expands_or_collapses_ambiguous_paths() {
        for (input, expected) in [
            (r"C:/Games/Title/Game.EXE", r"c:\games\title\game.exe"),
            (r"\\?\C:\Games\Game.exe", r"c:\games\game.exe"),
            (r"\\?\UNC\Server\Share\Game.exe", r"\\server\share\game.exe"),
            (r"C:\日本語\Game.exe", r"c:\日本語\game.exe"),
        ] {
            assert_eq!(normalize_app_path(input).unwrap(), expected);
        }
        for input in [
            "game.exe",
            r"C:game.exe",
            r"\game.exe",
            r"C:\Games\..\Game.exe",
            r"C:\Games\\Game.exe",
            r"C:\Games.\Game.exe",
            r"C:\NUL\Game.exe",
            r"\\.\pipe\Game.exe",
            r"C:\Game.exe:other",
            r"C:\Game",
            r"%GAME%\Game.exe",
            r"https://example/Game.exe",
        ] {
            assert!(normalize_app_path(input).is_err(), "{input}");
        }
        assert!(foreground_app_identity(r"C:game.exe").is_none());
        assert_eq!(foreground_app_identity(" GAME "), Some("game.exe".into()));
        assert!(normalize_app_path(&format!("C:\\{}\\game.exe", "a".repeat(32768))).is_err());
    }
    #[test]
    fn exact_paths_take_precedence_and_do_not_confuse_same_named_games() {
        let mut p = DeviceProfiles::default();
        p.create("First").unwrap();
        p.create("Second").unwrap();
        p.create("Legacy").unwrap();
        p.set_apps("Legacy", &["game.exe".into()]).unwrap();
        p.set_app_paths("First", &[r"C:\One\Game.exe".into()])
            .unwrap();
        p.set_app_paths("Second", &[r"D:\Two\Game.exe".into()])
            .unwrap();
        assert_eq!(p.effective(Some(r"C:/ONE/game.EXE")), "First");
        assert_eq!(p.effective(Some(r"d:\Two\Game.exe")), "Second");
        assert_eq!(p.effective(Some(r"E:\Other\Game.exe")), "Legacy");
        assert_eq!(p.effective(Some("game.exe")), "Legacy");
        p.set_apps("Legacy", &[]).unwrap();
        assert_eq!(p.effective(Some(r"E:\Other\Game.exe")), DEFAULT_PROFILE);
        assert_eq!(p.effective(None), DEFAULT_PROFILE);
        p.profiles
            .get_mut("Second")
            .unwrap()
            .app_paths
            .push(r"C:\One\Game.exe".into());
        assert_eq!(p.effective(Some(r"C:\One\Game.exe")), DEFAULT_PROFILE);
    }
    #[test]
    fn path_changes_are_atomic_and_device_local_and_profile_copies_clear_links() {
        let mut p = DeviceProfiles::default();
        p.create("First").unwrap();
        p.create("Second").unwrap();
        p.set_app_paths(
            "First",
            &[r"C:\One\Game.exe".into(), r"c:/one/game.exe".into()],
        )
        .unwrap();
        assert_eq!(p.profiles["First"].app_paths.len(), 1);
        let other = p.clone();
        let before = serde_json::to_value(&p).unwrap();
        assert!(p
            .set_app_paths(
                "Second",
                &[r"C:\Okay\Game.exe".into(), "relative.exe".into()]
            )
            .is_err());
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
        assert!(p
            .set_app_paths("First", &vec![r"C:\One\Game.exe".into(); 129])
            .is_err());
        p.set_app_paths("Second", &[r"C:\One\Game.exe".into()])
            .unwrap();
        assert!(p.profiles["First"].app_paths.is_empty());
        assert_eq!(other.profiles["First"].app_paths.len(), 1);
        p.switch("Second").unwrap();
        p.create("Copy").unwrap();
        assert!(p.profiles["Copy"].app_paths.is_empty());
        assert_eq!(p.active, "Second");
        let encoded = serde_json::to_vec(&p).unwrap();
        let decoded: DeviceProfiles = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            decoded.profiles["Second"].app_paths,
            p.profiles["Second"].app_paths
        );
    }
    #[test]
    fn native_backups_retain_paths_but_unverified_synapse_export_refuses_them() {
        let mut report = crate::profile_compat::import_document(include_bytes!(
            "../../tests/fixtures/synthetic.synapse4"
        ))
        .unwrap();
        let key = report.store.devices.keys().next().unwrap().clone();
        let device = report.store.devices.get_mut(&key).unwrap();
        let name = device.profiles.keys().next().unwrap().clone();
        device
            .set_app_paths(&name, &[r"C:\Games\Game.exe".into()])
            .unwrap();
        assert!(crate::profile_compat::export_synapse4(&report.store, &key).is_err());
        let bytes = report.store.native_bytes().unwrap();
        let native: Store = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            native.devices[&key].profiles[&name].app_paths,
            [r"c:\games\game.exe"]
        );
    }
}
