//! Local game catalogue and exact-path profile links. Pure, atomic store edits.
use crate::{
    profile::{normalize_app, normalize_app_path},
    Error, Result, Store,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
const KEY: &str = "_synapseLiteLinkedGames";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    pub games: Vec<Game>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            games: vec![],
            extra: BTreeMap::new(),
        }
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidArg(format!("Linked Games: {message}"))
}
impl Catalog {
    pub fn read(store: &Store) -> Result<Self> {
        let value = store.extra.get(KEY).cloned();
        let catalog: Self = value
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| invalid(&e.to_string()))?
            .unwrap_or_default();
        if catalog.version != 1 || catalog.games.len() > 512 {
            return Err(invalid(
                "unsupported or excessive catalogue; original retained",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for game in &catalog.games {
            if game.id.is_empty()
                || game.id.len() > 128
                || game.id.chars().any(char::is_control)
                || !ids.insert(&game.id)
                || game.name.trim().is_empty()
                || game.name.encode_utf16().count() > 256
                || game.name.chars().any(char::is_control)
                || !paths.insert(normalize_app_path(&game.path)?)
            {
                return Err(invalid(
                    "invalid or ambiguous catalogue identity; original retained",
                ));
            }
        }
        Ok(catalog)
    }
}
pub enum Change {
    Add {
        id: String,
        path: String,
    },
    Bind {
        game: String,
        device: String,
        profile: Option<String>,
    },
    Remove {
        game: String,
    },
}
pub fn edit(store: &Store, change: Change) -> Result<Store> {
    crate::profile_compat::validate_store_budget(store)?;
    let mut catalog = Catalog::read(store)?;
    let mut next = store.clone();
    match change {
        Change::Add { id, path } => {
            let path = normalize_app_path(&path)?;
            if catalog
                .games
                .iter()
                .any(|game| normalize_app_path(&game.path).ok().as_ref() == Some(&path))
            {
                return Err(invalid("this executable is already in Linked Games"));
            }
            let file = path.rsplit('\\').next().unwrap_or(&path);
            let name = file
                .strip_suffix(".exe")
                .filter(|s| !s.is_empty())
                .unwrap_or(file)
                .to_string();
            catalog.games.push(Game {
                id,
                path,
                name,
                extra: BTreeMap::new(),
            });
        }
        Change::Bind {
            game,
            device,
            profile,
        } => {
            let path = normalize_app_path(
                &catalog
                    .games
                    .iter()
                    .find(|g| g.id == game)
                    .ok_or_else(|| invalid("selected game no longer exists"))?
                    .path,
            )?;
            let profiles = next
                .devices
                .get_mut(&device)
                .ok_or_else(|| invalid("selected device has no saved profile store"))?;
            if profile
                .as_ref()
                .is_some_and(|name| !profiles.profiles.contains_key(name))
            {
                return Err(invalid("selected profile no longer exists"));
            }
            for entry in profiles.profiles.values_mut() {
                entry
                    .app_paths
                    .retain(|old| normalize_app_path(old).ok().as_ref() != Some(&path));
            }
            if let Some(name) = profile {
                let mut paths = profiles.profiles[&name].app_paths.clone();
                paths.push(path);
                profiles.set_app_paths(&name, &paths)?;
            }
        }
        Change::Remove { game } => {
            let index = catalog
                .games
                .iter()
                .position(|g| g.id == game)
                .ok_or_else(|| invalid("selected game no longer exists"))?;
            let path = normalize_app_path(&catalog.games.remove(index).path)?;
            for profiles in next.devices.values_mut() {
                for entry in profiles.profiles.values_mut() {
                    entry
                        .app_paths
                        .retain(|old| normalize_app_path(old).ok().as_ref() != Some(&path));
                }
            }
        }
    }
    if catalog.games.is_empty() && catalog.extra.is_empty() {
        next.extra.remove(KEY);
    } else {
        next.extra.insert(
            KEY.into(),
            serde_json::to_value(catalog).map_err(|e| invalid(&e.to_string()))?,
        );
    }
    Catalog::read(&next)?;
    crate::profile_compat::validate_store_budget(&next)?;
    Ok(next)
}
/// Explicit path bindings only. Legacy name rules are reported independently.
pub fn bindings(store: &Store, game: &Game, device: &str) -> Result<(Vec<String>, Vec<String>)> {
    let path = normalize_app_path(&game.path)?;
    let name = normalize_app(&path);
    let Some(profiles) = store.devices.get(device) else {
        return Ok((vec![], vec![]));
    };
    let exact = profiles
        .profiles
        .iter()
        .filter(|(_, p)| {
            p.app_paths
                .iter()
                .any(|old| normalize_app_path(old).ok().as_ref() == Some(&path))
        })
        .map(|(name, _)| name.clone())
        .collect();
    let legacy = profiles
        .profiles
        .iter()
        .filter(|(_, p)| p.apps.iter().any(|old| normalize_app(old) == name))
        .map(|(name, _)| name.clone())
        .collect();
    Ok((exact, legacy))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Store {
        let mut s = Store::default();
        let d = s.devices.entry("00C4:ONE".into()).or_default();
        d.active_mut().dpi = Some((800, 800));
        d.create("Gaming").unwrap();
        d.profiles.get_mut("Gaming").unwrap().dpi = Some((1600, 1600));
        s.extra
            .insert("future".into(), serde_json::json!({"preserve":[1,2,3]}));
        s
    }
    #[test]
    fn catalogue_and_links_are_one_reversible_metadata_transaction() {
        let original = fixture();
        let added = edit(
            &original,
            Change::Add {
                id: "game-id".into(),
                path: r"C:\Games\Game.exe".into(),
            },
        )
        .unwrap();
        let linked = edit(
            &added,
            Change::Bind {
                game: "game-id".into(),
                device: "00C4:ONE".into(),
                profile: Some("Gaming".into()),
            },
        )
        .unwrap();
        assert_eq!(linked.devices["00C4:ONE"].active, "Default");
        assert_eq!(
            linked.devices["00C4:ONE"].profiles["Gaming"].dpi,
            Some((1600, 1600))
        );
        let game = Catalog::read(&linked).unwrap().games.remove(0);
        assert_eq!(bindings(&linked, &game, "00C4:ONE").unwrap().0, ["Gaming"]);
        let removed = edit(
            &linked,
            Change::Remove {
                game: "game-id".into(),
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(removed).unwrap(),
            serde_json::to_value(original).unwrap()
        );
        let backup = linked.native_bytes().unwrap();
        let decoded: Store = serde_json::from_slice(&backup).unwrap();
        assert_eq!(Catalog::read(&decoded).unwrap().games[0].id, "game-id");
    }
    #[test]
    fn stale_conflicting_and_invalid_edits_preserve_the_original() {
        let store = edit(
            &fixture(),
            Change::Add {
                id: "id".into(),
                path: r"C:\Game.exe".into(),
            },
        )
        .unwrap();
        let before = store.native_bytes().unwrap();
        for change in [
            Change::Add {
                id: "other".into(),
                path: r"c:/game.exe".into(),
            },
            Change::Add {
                id: "id".into(),
                path: r"C:\Other.exe".into(),
            },
            Change::Bind {
                game: "id".into(),
                device: "00C4:ONE".into(),
                profile: Some("missing".into()),
            },
            Change::Remove {
                game: "missing".into(),
            },
        ] {
            assert!(edit(&store, change).is_err());
            assert_eq!(store.native_bytes().unwrap(), before);
        }
    }
    #[test]
    fn unknown_catalogue_fields_survive_and_unsupported_catalogues_are_inert() {
        let mut store = fixture();
        store.extra.insert(
            KEY.into(),
            serde_json::json!({
                "version": 1, "games": [], "futureCatalogue": {"enabled": true}
            }),
        );
        let added = edit(
            &store,
            Change::Add {
                id: "game".into(),
                path: r"C:\Games\Game.exe".into(),
            },
        )
        .unwrap();
        let mut imported = added.clone();
        imported.extra.get_mut(KEY).unwrap()["games"][0]["futureGame"] = serde_json::json!([1, 2]);
        let linked = edit(
            &imported,
            Change::Bind {
                game: "game".into(),
                device: "00C4:ONE".into(),
                profile: Some("Gaming".into()),
            },
        )
        .unwrap();
        assert_eq!(
            linked.extra[KEY]["games"][0]["futureGame"],
            serde_json::json!([1, 2])
        );
        let removed = edit(
            &linked,
            Change::Remove {
                game: "game".into(),
            },
        )
        .unwrap();
        assert_eq!(removed.extra[KEY], store.extra[KEY]);
        for value in [
            serde_json::json!({"version": 2, "games": []}),
            serde_json::json!({"version": 1, "games": "invalid"}),
        ] {
            store.extra.insert(KEY.into(), value);
            let original = store.native_bytes().unwrap();
            assert!(edit(
                &store,
                Change::Add {
                    id: "other".into(),
                    path: r"C:\Other.exe".into()
                }
            )
            .is_err());
            assert_eq!(store.native_bytes().unwrap(), original);
        }
    }
}
