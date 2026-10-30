//! Read the current Game Manager 3 catalogue without starting its service,
//! scanning other accounts, launching games, or modifying vendor data.
use super::{Catalog, Game, KEY};
use crate::{profile::normalize_app_path, Store};
use quick_xml::{events::Event, Reader};
use std::{collections::BTreeMap, io::Read, path::Path};

const LIMIT: u64 = 8 * 1024 * 1024;
const HIDDEN: &str = "_synapseLiteHiddenStockGames";

fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Game catalogue must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Game catalogue exceeds 8 MiB".into());
    }
    Ok(bytes)
}

pub fn read(root: &Path) -> Result<Vec<Game>, String> {
    let selector_file = root.join("LiveUser");
    if !selector_file.exists() {
        return Ok(vec![]);
    }
    let selector = String::from_utf8(bounded_read(&selector_file)?).map_err(|e| e.to_string())?;
    let selector = selector.trim();
    if selector.is_empty()
        || selector.len() > 128
        || !selector
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Unrecognized Game Manager account selector".into());
    }
    let folder = root.join(selector);
    if !folder.is_dir() {
        return Ok(vec![]);
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&folder).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = name
            .strip_prefix("LocalGameListEnc_")
            .and_then(|s| s.strip_suffix(".xml"))
        {
            if id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
                files.push(entry.path());
            }
        }
    }
    if files.is_empty() {
        return Ok(vec![]);
    }
    if files.len() != 1 {
        return Err("Several machine catalogues exist; no catalogue was guessed".into());
    }
    let mut games = parse(&decrypt(&bounded_read(&files[0])?)?)?;
    // Source combines detected games with the user's manually retained list.
    // Use the same machine suffix; never select another account or machine.
    let user_file = folder.join(files[0].file_name().unwrap().to_string_lossy().replacen(
        "LocalGameListEnc_",
        "UserGameListEnc_",
        1,
    ));
    if user_file.is_file() {
        games.extend(parse(&decrypt(&bounded_read(&user_file)?)?)?);
    }
    let merged = merge(&Store::default(), games)?;
    Ok(Catalog::read(&merged).map_err(|e| e.to_string())?.games)
}

fn parse(bytes: &[u8]) -> Result<Vec<Game>, String> {
    let mut reader = Reader::from_reader(bytes);
    let mut stack = Vec::<String>::new();
    let mut row = None::<BTreeMap<String, String>>;
    let mut games = Vec::new();
    let mut seen_root = false;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_string();
                if stack.len() >= 16 {
                    return Err("Game catalogue nesting exceeds limit".into());
                }
                if stack.is_empty() {
                    if seen_root || name != "GameSignatureList" {
                        return Err("Unrecognized game catalogue root".into());
                    }
                    seen_root = true;
                }
                if name == "GameSignature" && stack == ["GameSignatureList", "GameSignatureList"] {
                    row = Some(BTreeMap::new());
                }
                stack.push(name);
            }
            Event::Text(e) => {
                if let (Some(row), Some(name)) =
                    (&mut row, stack.last().filter(|_| stack.len() == 4))
                {
                    let text = e.xml10_content();
                    row.entry(name.clone()).or_default().push_str(&text);
                }
            }
            Event::GeneralRef(e) => {
                let escaped = format!("&{};", e.as_ref());
                let value = quick_xml::escape::unescape(&escaped).map_err(|e| e.to_string())?;
                if let (Some(row), Some(name)) =
                    (&mut row, stack.last().filter(|_| stack.len() == 4))
                {
                    row.entry(name.clone()).or_default().push_str(&value);
                }
            }
            Event::Empty(e) if stack.is_empty() => {
                if seen_root || e.local_name().as_ref() != "GameSignatureList" {
                    return Err("Unrecognized game catalogue root".into());
                }
                seen_root = true;
            }
            Event::End(e) => {
                if e.local_name().as_ref() == "GameSignature" {
                    if let Some(row) = row.take() {
                        let field = |key: &str| row.get(key).cloned().unwrap_or_default();
                        let id = field("EntryGUID");
                        let name = field("DisplayName");
                        let path = field("LaunchExe");
                        if !id.is_empty() && !name.is_empty() && normalize_app_path(&path).is_ok() {
                            let mut extra = BTreeMap::new();
                            extra.insert("stockIconPath".into(), field("DisplayIconPath").into());
                            extra.insert("stockCoverArt".into(), field("CoverArt").into());
                            games.push(Game {
                                id,
                                name,
                                path,
                                aliases: vec![],
                                extra,
                            });
                            if games.len() > 512 {
                                return Err("Game catalogue exceeds 512 entries".into());
                            }
                        }
                    }
                }
                stack.pop();
            }
            Event::DocType(_) => return Err("Game catalogue DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root || !stack.is_empty() {
        return Err("Incomplete game catalogue".into());
    }
    Ok(games)
}

/// Preserve local names, IDs, links and metadata; only append distinct source
/// executables. Reading a page never saves this merged store.
pub fn merge(store: &Store, discovered: Vec<Game>) -> Result<Store, String> {
    let mut next = store.clone();
    let mut catalog = Catalog::read(store).map_err(|e| e.to_string())?;
    let hidden = store.extra.get(HIDDEN).and_then(|v| v.as_array());
    for game in discovered {
        let path = normalize_app_path(&game.path).map_err(|e| e.to_string())?;
        let hidden_path = format!("path:{path}");
        if hidden.is_some_and(|ids| {
            ids.iter()
                .any(|id| id.as_str() == Some(&game.id) || id.as_str() == Some(&hidden_path))
        }) {
            continue;
        }
        if let Some(existing) = catalog
            .games
            .iter_mut()
            .find(|g| normalize_app_path(&g.path).ok().as_ref() == Some(&path))
        {
            for (key, value) in game.extra {
                existing.extra.entry(key).or_insert(value);
            }
        } else if catalog.games.len() < 512 && !catalog.games.iter().any(|g| g.id == game.id) {
            catalog.games.push(game);
        }
    }
    next.extra.insert(
        KEY.into(),
        serde_json::to_value(catalog).map_err(|e| e.to_string())?,
    );
    Catalog::read(&next).map_err(|e| e.to_string())?;
    Ok(next)
}

pub fn hide(store: &mut Store, id: &str) {
    let path = Catalog::read(store)
        .ok()
        .and_then(|catalog| catalog.games.into_iter().find(|game| game.id == id))
        .and_then(|game| normalize_app_path(&game.path).ok())
        .map(|path| format!("path:{path}"));
    let ids = store
        .extra
        .entry(HIDDEN.into())
        .or_insert_with(|| serde_json::json!([]));
    if let Some(ids) = ids.as_array_mut() {
        if !ids.iter().any(|value| value.as_str() == Some(id)) {
            ids.push(id.into());
        }
        if let Some(path) = path {
            if !ids.iter().any(|value| value.as_str() == Some(&path)) {
                ids.push(path.into());
            }
        }
    }
}

// AES-CBC/PKCS7 format from GameManagerServiceLib 3.15 IoUtil.CreateAes and
// GameScannerCommon.CryptoXmlSerializer. This is a public file-format key,
// unrelated to account authentication. Native CNG keeps the Rust app standalone.
fn decrypt(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Security::Cryptography::*;
    struct Algorithm(BCRYPT_ALG_HANDLE);
    impl Drop for Algorithm {
        fn drop(&mut self) {
            unsafe {
                BCryptCloseAlgorithmProvider(self.0, 0);
            }
        }
    }
    struct Key(BCRYPT_KEY_HANDLE);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                BCryptDestroyKey(self.0);
            }
        }
    }
    if bytes.is_empty() || bytes.len() % 16 != 0 {
        return Err("Invalid encrypted game catalogue length".into());
    }
    let key_bytes: [u8; 32] = [
        172, 56, 73, 102, 24, 204, 255, 167, 10, 79, 82, 42, 112, 81, 110, 246, 146, 93, 109, 65,
        77, 157, 250, 248, 17, 44, 167, 87, 215, 183, 161, 22,
    ];
    let mut iv: [u8; 16] = [
        142, 104, 123, 108, 253, 163, 236, 142, 239, 189, 185, 74, 27, 54, 209, 124,
    ];
    unsafe {
        let mut handle = null_mut();
        if BCryptOpenAlgorithmProvider(&mut handle, BCRYPT_AES_ALGORITHM, null(), 0) < 0 {
            return Err("Could not open AES provider".into());
        }
        let algorithm = Algorithm(handle);
        let mode: Vec<u16> = "ChainingModeCBC\0".encode_utf16().collect();
        if BCryptSetProperty(
            algorithm.0,
            BCRYPT_CHAINING_MODE,
            mode.as_ptr() as *const u8,
            (mode.len() * 2) as u32,
            0,
        ) < 0
        {
            return Err("Could not configure AES-CBC".into());
        }
        let mut key = null_mut();
        if BCryptGenerateSymmetricKey(
            algorithm.0,
            &mut key,
            null_mut(),
            0,
            key_bytes.as_ptr(),
            key_bytes.len() as u32,
            0,
        ) < 0
        {
            return Err("Could not open game catalogue key".into());
        }
        let key = Key(key);
        let mut plain = vec![0; bytes.len()];
        let mut length = 0;
        if BCryptDecrypt(
            key.0,
            bytes.as_ptr(),
            bytes.len() as u32,
            null(),
            iv.as_mut_ptr(),
            iv.len() as u32,
            plain.as_mut_ptr(),
            plain.len() as u32,
            &mut length,
            BCRYPT_BLOCK_PADDING,
        ) < 0
        {
            return Err("Unrecognized game catalogue encryption".into());
        }
        plain.truncate(length as usize);
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn xml(name: &str, path: &str) -> String {
        format!("<GameSignatureList><GameSignatureList><GameSignature><EntryGUID>source-game</EntryGUID><DisplayName>{name}</DisplayName><LaunchExe>{path}</LaunchExe></GameSignature></GameSignatureList></GameSignatureList>")
    }
    #[test]
    fn source_catalogue_preserves_names_and_exact_paths_without_launching() {
        let games = parse(xml("A &amp; B", r"D:\Games\Test.exe").as_bytes()).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "A & B");
        assert_eq!(games[0].path, r"D:\Games\Test.exe");
        assert!(parse(b"<unexpected />").is_err());
        assert!(parse(b"<GameSignatureList/>").unwrap().is_empty());
        assert!(parse(b"<unexpected>").is_err());
        assert!(parse(b"<!DOCTYPE x><GameSignatureList/>").is_err());
    }
    #[test]
    fn catalogue_merge_keeps_local_identity_and_hidden_source_removals() {
        let source = parse(xml("Source name", r"C:\Game\Play.exe").as_bytes()).unwrap();
        let local = super::super::edit(
            &Store::default(),
            super::super::Change::Add {
                id: "local-game".into(),
                path: r"C:\Game\Play.exe".into(),
            },
        )
        .unwrap();
        let merged = merge(&local, source.clone()).unwrap();
        let games = Catalog::read(&merged).unwrap().games;
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "local-game");
        assert_eq!(
            serde_json::to_value(&local.devices).unwrap(),
            serde_json::to_value(&merged.devices).unwrap()
        );
        let mut hidden = Store::default();
        hide(&mut hidden, "source-game");
        assert!(Catalog::read(&merge(&hidden, source).unwrap())
            .unwrap()
            .games
            .is_empty());
    }
    #[test]
    #[ignore = "reads the installed current-account Game Manager catalogue without changing files"]
    fn installed_catalogue_read_only() {
        let root = std::path::PathBuf::from(std::env::var_os("ProgramData").unwrap())
            .join("Razer/GameManager3/Data");
        let games = read(&root).unwrap();
        assert!(!games.is_empty(), "No installed catalogue was available");
        let merged = merge(&Store::default(), games.clone()).unwrap();
        assert_eq!(Catalog::read(&merged).unwrap().games.len(), games.len());
        eprintln!(
            "Read {} current-account games; {} local icon files; no saves or service calls",
            games.len(),
            games
                .iter()
                .filter(|game| game
                    .extra
                    .get("stockIconPath")
                    .and_then(|v| v.as_str())
                    .is_some_and(|path| Path::new(path).is_file()))
                .count()
        );
    }
}
