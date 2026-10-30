//! Explicit, offline migration of retained global Linked Games state.
//! No scanning, execution, device access, or normal-store writes.
use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Selection {
    pub game_id: String,
    pub device_key: String,
    pub profile_guid: String,
    /// Explicitly selected executable on this computer.
    pub local_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

pub struct Preview {
    pub candidate: Store,
    pub associations: Vec<(String, String, String)>,
}

/// Accept the source reducer's {gameList, linkedGames} snapshot. Resolve GUIDs
/// only within an explicitly selected native device; never fall back to a name
/// or a PID-only match. Preserve the entire source snapshot for later recovery.
pub fn preview(store: &Store, source: &Value, selections: &[Selection]) -> Result<Preview> {
    crate::profile_compat::validate_store_budget(store)?;
    let games = source
        .get("gameList")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source gameList must be an array"))?;
    let links = source
        .get("linkedGames")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source linkedGames must be an array"))?;
    if games.len() > 512 || selections.len() > 512 {
        return Err(invalid("source exceeds migration limits"));
    }
    let mut next = store.clone();
    let mut associations = Vec::new();
    let mut selected = BTreeSet::new();
    for choice in selections {
        if !selected.insert((&choice.game_id, &choice.device_key)) {
            return Err(invalid("duplicate game/device selection"));
        }
        let matching: Vec<_> = games
            .iter()
            .filter(|g| g["id"].as_str() == Some(&choice.game_id))
            .collect();
        if matching.len() != 1 {
            return Err(invalid("missing or ambiguous source game ID"));
        }
        let game = matching[0];
        let linked: Vec<_> = links
            .iter()
            .filter(|g| g["gameID"].as_str() == Some(&choice.game_id))
            .collect();
        if linked.len() != 1 {
            return Err(invalid("missing or ambiguous source association"));
        }
        let devices = linked[0]["devices"]
            .as_array()
            .ok_or_else(|| invalid("source devices must be an array"))?;
        // Serial identity is required: container-only and PID-only identities
        // cannot establish a safe correspondence with a native store key.
        let (pid, serial) = choice
            .device_key
            .split_once(':')
            .ok_or_else(|| invalid("exact serial device key required"))?;
        let pid = u64::from_str_radix(pid, 16).map_err(|_| invalid("invalid native PID"))?;
        if serial.is_empty()
            || !devices.iter().any(|d| {
                d["pid"].as_u64() == Some(pid)
                    && d["serialNumber"].as_str() == Some(serial)
                    && d["profileID"].as_str() == Some(&choice.profile_guid)
            })
        {
            return Err(invalid(
                "source device/profile association does not match selection",
            ));
        }
        let profiles = store
            .devices
            .get(&choice.device_key)
            .ok_or_else(|| invalid("device no longer exists"))?;
        let names: Vec<_> = profiles
            .profiles
            .iter()
            .filter(|(_, p)| {
                p.extra
                    .get("_synapseLiteSynapse4")
                    .and_then(|v| v.get("decoded"))
                    .and_then(|v| v.get("guid"))
                    .and_then(Value::as_str)
                    == Some(&choice.profile_guid)
            })
            .map(|(name, _)| name.clone())
            .collect();
        if names.len() != 1 {
            return Err(invalid("missing or ambiguous saved profile GUID"));
        }
        let catalog = Catalog::read(&next)?;
        if let Some(existing) = catalog.games.iter().find(|g| g.id == choice.game_id) {
            if normalize_app_path(&existing.path)? != normalize_app_path(&choice.local_path)? {
                return Err(invalid("existing game has a different local executable"));
            }
        } else {
            next = edit(
                &next,
                Change::Add {
                    id: choice.game_id.clone(),
                    path: choice.local_path.clone(),
                },
            )?;
        }
        next = edit(
            &next,
            Change::Aliases {
                game: choice.game_id.clone(),
                paths: choice.aliases.clone(),
            },
        )?;
        next = edit(
            &next,
            Change::Bind {
                game: choice.game_id.clone(),
                device: choice.device_key.clone(),
                profile: Some(names[0].clone()),
            },
        )?;
        let mut catalog = Catalog::read(&next)?;
        let entry = catalog
            .games
            .iter_mut()
            .find(|g| g.id == choice.game_id)
            .unwrap();
        if let Some(name) = game["name"].as_str() {
            entry.name = name.into();
        }
        entry.extra.insert("sourceGame".into(), game.clone());
        next.extra.insert(
            KEY.into(),
            serde_json::to_value(catalog).map_err(|e| invalid(&e.to_string()))?,
        );
        associations.push((
            choice.game_id.clone(),
            choice.device_key.clone(),
            names[0].clone(),
        ));
    }
    next.extra
        .insert("_synapseLiteLinkedGamesSource".into(), source.clone());
    Catalog::read(&next)?;
    crate::profile_compat::validate_store_budget(&next)?;
    Ok(Preview {
        candidate: next,
        associations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_guid_import_preserves_source_and_refuses_stale_or_ambiguous_identity() {
        let mut store = Store::default();
        let profiles = store.devices.entry("00C4:SERIAL".into()).or_default();
        profiles.active_mut().extra.insert(
            "_synapseLiteSynapse4".into(),
            serde_json::json!({"decoded":{"guid":"GUID"}}),
        );
        let source = serde_json::json!({"gameList":[{"id":"GAME","name":"Source name","location":"old","cover":"opaque"}],"linkedGames":[{"gameID":"GAME","devices":[{"pid":196,"serialNumber":"SERIAL","profileID":"GUID"}]}],"future":true});
        let mut choices = vec![Selection {
            game_id: "GAME".into(),
            device_key: "00C4:SERIAL".into(),
            profile_guid: "GUID".into(),
            local_path: r"C:\Games\Game.exe".into(),
            aliases: vec![r"D:\Alias.exe".into()],
        }];
        let before = store.native_bytes().unwrap();
        let report = preview(&store, &source, &choices).unwrap();
        assert_eq!(store.native_bytes().unwrap(), before);
        assert_eq!(report.candidate.devices["00C4:SERIAL"].active, "Default");
        assert_eq!(
            report.candidate.extra["_synapseLiteLinkedGamesSource"],
            source
        );
        assert_eq!(
            Catalog::read(&report.candidate).unwrap().games[0].name,
            "Source name"
        );
        assert_eq!(report.associations[0].2, "Default");
        assert_eq!(
            report.candidate.devices["00C4:SERIAL"].profiles["Default"]
                .app_paths
                .len(),
            2
        );
        let removed = edit(
            &report.candidate,
            Change::Remove {
                game: "GAME".into(),
            },
        )
        .unwrap();
        assert!(removed.devices["00C4:SERIAL"].profiles["Default"]
            .app_paths
            .is_empty());
        choices[0].profile_guid = "stale".into();
        assert!(preview(&store, &source, &choices).is_err());
        choices[0].profile_guid = "GUID".into();
        let p = store.devices["00C4:SERIAL"].profiles["Default"].clone();
        store
            .devices
            .get_mut("00C4:SERIAL")
            .unwrap()
            .profiles
            .insert("Duplicate".into(), p);
        assert!(preview(&store, &source, &choices).is_err());
    }
}
