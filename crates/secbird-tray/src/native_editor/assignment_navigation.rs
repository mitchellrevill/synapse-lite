//! Profile Switch draft state and current reference catalogue. No activation.
use super::{
    assignment_model::{Category, Draft},
    *,
};
use secbird_core::{profile::DeviceProfiles, profile_compat::profile_navigation as source};

#[derive(Clone, Debug)]
pub(super) struct Catalog {
    pub profiles: DeviceProfiles,
    pub effects: Vec<(String, String)>,
    pub path: Option<PathBuf>,
}
impl Catalog {
    pub fn from_device(device: &Value) -> Option<Self> {
        let values = device["profilesData"].as_object()?;
        if values.values().any(|profile| !profile.is_object()) {
            return None;
        }
        let profiles = DeviceProfiles {
            active: device["active"].as_str()?.into(),
            profiles: values
                .iter()
                .map(|(name, profile)| {
                    (
                        name.clone(),
                        reference(profile["_synapseLiteSynapse4"]["decoded"]["guid"].as_str()),
                    )
                })
                .collect(),
            ..DeviceProfiles::default()
        };
        profiles
            .profiles
            .contains_key(&profiles.active)
            .then_some(Self {
                profiles,
                effects: vec![],
                path: None,
            })
    }
    pub fn targets(&self) -> Vec<(Option<String>, String)> {
        self.profiles
            .profiles
            .iter()
            .filter(|(name, _)| *name != &self.profiles.active)
            .map(|(name, profile)| {
                (
                    profile
                        .extra
                        .get("_synapseLiteSynapse4")
                        .and_then(|v| v["decoded"]["guid"].as_str())
                        .filter(|id| !id.is_empty())
                        .map(str::to_string),
                    name.clone(),
                )
            })
            .collect()
    }
    pub fn refresh(&mut self, profiles: &DeviceProfiles, linked: bool) -> Result<(), String> {
        self.profiles = DeviceProfiles {
            active: profiles.active.clone(),
            profiles: profiles
                .profiles
                .iter()
                .map(|(name, profile)| {
                    (
                        name.clone(),
                        reference(
                            profile
                                .extra
                                .get("_synapseLiteSynapse4")
                                .and_then(|v| v["decoded"]["guid"].as_str()),
                        ),
                    )
                })
                .collect(),
            ..DeviceProfiles::default()
        };
        if linked {
            let path = self
                .path
                .as_ref()
                .ok_or("Open a saved Studio library to link an effect")?;
            let archive = secbird_core::studio::Archive::read(
                std::fs::File::open(path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            self.effects = effect_choices(&archive);
        }
        Ok(())
    }
}
// Reference validation needs names and retained identities, not duplicate DPI,
// mappings, macro and lighting payloads for every profile in the editor.
fn reference(guid: Option<&str>) -> secbird_core::Profile {
    let mut profile = secbird_core::Profile::default();
    if let Some(guid) = guid {
        profile.extra.insert(
            "_synapseLiteSynapse4".into(),
            json!({"decoded":{"guid":guid}}),
        );
    }
    profile
}
pub(super) fn effect_choices(archive: &secbird_core::studio::Archive) -> Vec<(String, String)> {
    let entries: Vec<_> = (0..archive.profile_count())
        .filter_map(|i| {
            let header = archive.profile_header(i).ok()?;
            let id = header.id.filter(|id| !id.is_empty())?;
            Some((
                id,
                header
                    .title
                    .unwrap_or_else(|| format!("Composition {}", i + 1)),
            ))
        })
        .collect();
    entries
        .iter()
        .filter(|(id, _)| entries.iter().filter(|(other, _)| id == other).count() == 1)
        .cloned()
        .collect()
}
#[derive(Clone, Debug)]
pub(super) struct Navigation {
    pub action: Option<usize>,
    pub guid: Option<String>,
    pub linked: bool,
    pub effect: Option<String>,
    pub effects_menu: bool,
}
impl Navigation {
    pub fn read(group: Option<&Value>) -> Self {
        let action = group.map_or(Some(0), |group| {
            if group.get("Name").is_some() {
                Some(4)
            } else {
                source::CHOICES
                    .iter()
                    .position(|(id, _)| group["profileNavigationAssignment"] == *id)
            }
        });
        Self {
            action,
            guid: group
                .and_then(|v| v.get("guid").or_else(|| v.get("ProfileId")))
                .and_then(Value::as_str)
                .map(str::to_string),
            linked: group.is_some_and(|g| g.get("lightpacId").is_some()),
            effect: group
                .and_then(|g| g["lightpacId"].as_str())
                .map(str::to_string),
            effects_menu: false,
        }
    }
}
impl Draft {
    pub fn navigation_edit(&self) -> Result<source::Edit, String> {
        if self.category != Some(Category::Profile) {
            return Err("Select Profile Switch first".into());
        }
        let action = source::CHOICES
            .get(
                self.navigation
                    .action
                    .ok_or("Choose a profile navigation mode")?,
            )
            .ok_or("Unknown profile navigation mode")?
            .0;
        Ok(source::Edit {
            input_id: self.input.input_id.clone(),
            input_type: self.input.input_type.clone(),
            hyper: self.hyper,
            action: action.into(),
            guid: (action == "Specific")
                .then(|| self.navigation.guid.clone())
                .flatten(),
            lightpac_id: if action == "Specific" && self.navigation.linked {
                Some(
                    self.navigation
                        .effect
                        .clone()
                        .ok_or("Choose a Chroma effect")?,
                )
            } else {
                None
            },
        })
    }
    pub fn navigation_profile(&self) -> Result<secbird_core::Profile, String> {
        let catalog = self
            .context
            .navigation
            .as_ref()
            .ok_or("Current device profiles are unavailable")?;
        let profile = serde_json::from_value(self.context.original_profile.clone())
            .map_err(|e| e.to_string())?;
        let ids: Vec<_> = catalog.effects.iter().map(|(id, _)| id.as_str()).collect();
        source::candidate(
            &profile,
            self.context.owner.pid,
            &catalog.profiles,
            &ids,
            &self.navigation_edit()?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn navigation_action(&mut self, index: usize) {
        if index >= source::CHOICES.len() {
            return;
        }
        self.navigation.action = Some(index);
        if index == 4 && self.navigation.guid.is_none() {
            self.navigation.guid = self
                .context
                .navigation
                .as_ref()
                .and_then(|c| c.targets().first().and_then(|t| t.0.clone()));
        }
        self.dirty = true;
        self.options_open = false;
    }
    pub fn navigation_option(&mut self, index: usize) {
        let Some(catalog) = &self.context.navigation else {
            return;
        };
        if self.navigation.effects_menu {
            let Some((id, _)) = catalog.effects.get(index) else {
                return;
            };
            self.navigation.effect = Some(id.clone());
        } else {
            let targets = catalog.targets();
            let Some((guid, _)) = targets.get(index) else {
                return;
            };
            self.navigation.guid = guid.clone();
        }
        self.dirty = true;
        self.options_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::super::assignment_model::{Anchor, Context, Input};

    #[test]
    fn navigation_catalogue_retains_only_reference_metadata() {
        let mut target = secbird_core::profile_compat::new_native_profile(605, "Target").unwrap();
        target
            .extra
            .insert("large-opaque-payload".into(), json!("x".repeat(128 * 1024)));
        let device = json!({"active":"Current","profilesData":{"Current":target,"Target":target}});
        let catalog = Catalog::from_device(&device).unwrap();
        assert!(serde_json::to_vec(&catalog.profiles).unwrap().len() < 4096);
        assert!(!catalog.profiles.profiles["Target"].extra.contains_key("large-opaque-payload"));
        assert_eq!(
            catalog.targets()[0].0,
            device["profilesData"]["Target"]["_synapseLiteSynapse4"]["decoded"]["guid"]
                .as_str()
                .map(str::to_string)
        );
    }
    use super::*;

    fn draft(pid: u16, hyper: bool) -> Draft {
        let profile = secbird_core::profile_compat::new_native_profile(pid, "Current").unwrap();
        let target = secbird_core::profile_compat::new_native_profile(pid, "Target").unwrap();
        let context = Context::from_device(
            &json!({"id":"fixture","pid":format!("{pid:04X}"),"active":"Current",
            "profile":profile,"profilesData":{"Current":profile,"Target":target}}),
        )
        .unwrap();
        let input = if pid == 605 {
            Input {
                key: "KEY_A".into(),
                input_id: "KEY_A".into(),
                input_type: "KeyInput".into(),
                label: "A".into(),
                default_label: "A".into(),
                functions: vec!["SWITCH_PROFILE".into()],
                enabled: true,
            }
        } else {
            super::super::assignment_model::mouse_inputs()
                .into_iter()
                .find(|i| i.input_id == "RightClick")
                .unwrap()
        };
        let mut draft = Draft::new(
            Arc::new(context),
            input,
            Anchor::Key("fixture".into()),
            hyper,
        );
        draft.select(Category::Profile);
        draft
    }
    #[test]
    fn navigation_categories_save_and_reopen_all_modes_for_keyboard_and_mouse_layers() {
        for pid in [605, 196, 197] {
            for hyper in [false, true] {
                for index in 0..source::CHOICES.len() {
                    let mut draft = draft(pid, hyper);
                    draft.navigation_action(index);
                    assert!(
                        draft.can_save(),
                        "{pid}/{hyper}/{index}: {:?}",
                        draft.navigation_profile().err()
                    );
                    let next = super::super::assignment_backend::candidate(&draft).unwrap();
                    let row = &next.extra["_synapseLiteSynapse4"]["decoded"]["mappings"][0];
                    assert_eq!(
                        row["profileNavigationGroup"]["profileNavigationAssignment"],
                        source::CHOICES[index].0
                    );
                    assert_eq!(row["isHyperShift"], hyper);
                    let mut context = (*draft.context).clone();
                    context.original_profile = serde_json::to_value(next).unwrap();
                    context.mappings = Some(
                        context.original_profile["_synapseLiteSynapse4"]["decoded"]["mappings"]
                            .as_array()
                            .unwrap()
                            .clone(),
                    );
                    let mut reopened = Draft::new(
                        Arc::new(context),
                        draft.input.clone(),
                        draft.anchor.clone(),
                        hyper,
                    );
                    assert_eq!(reopened.category, Some(Category::Profile));
                    assert_eq!(reopened.navigation.action, Some(index));
                    assert_eq!(
                        reopened.navigation.guid,
                        draft.navigation.guid.filter(|_| index == 4)
                    );
                    assert!(!reopened.dirty);
                    reopened.navigation_action((index + 1) % 5);
                    reopened.undo();
                    assert_eq!(reopened.navigation.action, Some(index));
                    assert!(!reopened.dirty);
                }
            }
        }
    }
    #[test]
    fn navigation_retains_broken_and_legacy_references_until_explicit_edit() {
        let unknown = json!({"profileNavigationAssignment":"Future","guid":"missing","lightpacId":"missing","opaque":[9]});
        let state = Navigation::read(Some(&unknown));
        assert_eq!(state.action, None);
        assert_eq!(state.guid.as_deref(), Some("missing"));
        assert!(state.linked);
        let legacy = Navigation::read(Some(&json!({"Name":"old label","ProfileId":"stable-guid"})));
        assert_eq!(legacy.action, Some(4));
        assert_eq!(legacy.guid.as_deref(), Some("stable-guid"));
        let mut draft = draft(605, false);
        draft.navigation = state;
        assert!(draft.navigation_profile().is_err());
        draft.navigation_action(0);
        assert!(draft.navigation_profile().is_ok());
    }
    #[test]
    fn navigation_effect_catalogue_refuses_ambiguous_ids_and_refreshes_saved_files() {
        let archive=secbird_core::studio::Archive::parse(br#"{"version":1,"profiles":[{"id":"same","title":"First"},{"id":"same","title":"Second"},{"id":"unique","title":"Desk"}]}"#).unwrap();
        assert_eq!(
            effect_choices(&archive),
            vec![("unique".into(), "Desk".into())]
        );
        let path = std::env::temp_dir().join(format!(
            "nav-library-{}-{}.ChromaEffects",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, archive.to_bytes().unwrap()).unwrap();
        let mut catalog = draft(605, false).context.navigation.clone().unwrap();
        catalog.path = Some(path.clone());
        let profiles = catalog.profiles.clone();
        catalog.refresh(&profiles, true).unwrap();
        assert_eq!(catalog.effects.len(), 1);
        std::fs::write(&path, br#"{"version":1,"profiles":[]}"#).unwrap();
        catalog.refresh(&profiles, true).unwrap();
        assert!(catalog.effects.is_empty());
        std::fs::write(&path, b"invalid").unwrap();
        assert!(catalog.refresh(&profiles, true).is_err());
        assert!(catalog.refresh(&profiles, false).is_ok());
    }
}
