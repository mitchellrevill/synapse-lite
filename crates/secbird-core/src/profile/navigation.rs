//! Read-only source assignment eligibility. Never selects a profile or writes hardware.
use super::DeviceProfiles;
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum NavigationStatus<'a> {
    Ready {
        action: &'a str,
        target: Option<&'a str>,
    },
    Unavailable(&'static str),
}

impl DeviceProfiles {
    /// Resolve against this device only. Labels are never used as identity fallbacks.
    /// `supported` is explicit recovered device capability, not an inferred PID list.
    /// Chroma IDs must be supplied by the caller's current local library.
    pub fn navigation_status<'a>(
        &'a self,
        group: &'a Value,
        supported: &[&str],
        chroma_ids: &[&str],
    ) -> NavigationStatus<'a> {
        use NavigationStatus::*;
        let Some(action) = group["profileNavigationAssignment"].as_str() else {
            return Unavailable("Missing profile navigation action");
        };
        if ![
            "NextProfile",
            "PreviousProfile",
            "CycleUp",
            "CycleDown",
            "Specific",
        ]
        .contains(&action)
        {
            return Unavailable("Unknown profile navigation action");
        }
        if !supported.contains(&action) {
            return Unavailable("Action is not verified for this device");
        }
        if !self.profiles.contains_key(&self.active) {
            return Unavailable("Active profile is missing");
        }
        if self.profiles.len() < 2 {
            return Unavailable("Another profile is required");
        }
        if action != "Specific" {
            // Ordering and live delivery belong to the mapping runtime, not BTreeMap order.
            return Ready {
                action,
                target: None,
            };
        }
        let Some(guid) = group["guid"].as_str().filter(|id| !id.is_empty()) else {
            return Unavailable("Target profile identity is missing");
        };
        let mut matches = self.profiles.iter().filter(|(_, profile)| {
            profile
                .extra
                .get("_synapseLiteSynapse4")
                .and_then(|v| v.get("decoded"))
                .and_then(|v| v.get("guid"))
                .and_then(Value::as_str)
                == Some(guid)
        });
        let Some((name, _)) = matches.next() else {
            return Unavailable("Target profile is missing");
        };
        if matches.next().is_some() {
            return Unavailable("Target profile identity is ambiguous");
        }
        if name == &self.active {
            return Unavailable("Target is the current profile");
        }
        if let Some(id) = group.get("lightpacId") {
            if !id.as_str().is_some_and(|id| chroma_ids.contains(&id)) {
                return Unavailable("Linked Chroma effect is missing or unsupported");
            }
        }
        Ready {
            action,
            target: Some(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
    use serde_json::json;

    fn profiles() -> DeviceProfiles {
        let mut device = DeviceProfiles::default();
        let mut target = Profile::default();
        target.extra.insert(
            "_synapseLiteSynapse4".into(),
            json!({"decoded":{"guid":"stable-id","opaque":[1,2]}}),
        );
        device.profiles.insert("Target".into(), target);
        device
    }
    fn assignment() -> Value {
        json!({"profileNavigationAssignment":"Specific","guid":"stable-id","name":"stale label","unknown":{"keep":true}})
    }
    #[test]
    fn navigation_rename_delete_and_reopen_keep_identity_and_opaque_data() {
        let mut device = profiles();
        let group = assignment();
        let before = group.clone();
        device.rename("Target", "Renamed").unwrap();
        let device: DeviceProfiles =
            serde_json::from_slice(&serde_json::to_vec(&device).unwrap()).unwrap();
        assert_eq!(
            device.navigation_status(&group, &["Specific"], &[]),
            NavigationStatus::Ready {
                action: "Specific",
                target: Some("Renamed")
            }
        );
        let mut device = device;
        device.profiles.remove("Renamed");
        device
            .profiles
            .insert("Unrelated".into(), Profile::default());
        assert_eq!(
            device.navigation_status(&group, &["Specific"], &[]),
            NavigationStatus::Unavailable("Target profile is missing")
        );
        assert_eq!(group, before);
    }
    #[test]
    fn navigation_rejects_duplicate_current_and_missing_effect_targets() {
        let mut device = profiles();
        let mut group = assignment();
        device
            .profiles
            .insert("Duplicate".into(), device.profiles["Target"].clone());
        assert_eq!(
            device.navigation_status(&group, &["Specific"], &[]),
            NavigationStatus::Unavailable("Target profile identity is ambiguous")
        );
        device.profiles.remove("Duplicate");
        device.active = "Target".into();
        assert_eq!(
            device.navigation_status(&group, &["Specific"], &[]),
            NavigationStatus::Unavailable("Target is the current profile")
        );
        device.active = "Default".into();
        group["lightpacId"] = json!("effect");
        assert_eq!(
            device.navigation_status(&group, &["Specific"], &[]),
            NavigationStatus::Unavailable("Linked Chroma effect is missing or unsupported")
        );
        assert!(matches!(
            device.navigation_status(&group, &["Specific"], &["effect"]),
            NavigationStatus::Ready { .. }
        ));
    }
    #[test]
    fn navigation_requires_explicit_capability_and_multiple_profiles() {
        let device = profiles();
        for action in ["NextProfile", "PreviousProfile", "CycleUp", "CycleDown"] {
            let group = json!({"profileNavigationAssignment":action});
            assert!(matches!(
                device.navigation_status(&group, &[action], &[]),
                NavigationStatus::Ready { target: None, .. }
            ));
            assert_eq!(
                device.navigation_status(&group, &[], &[]),
                NavigationStatus::Unavailable("Action is not verified for this device")
            );
            assert_eq!(
                DeviceProfiles::default().navigation_status(&group, &[action], &[]),
                NavigationStatus::Unavailable("Another profile is required")
            );
        }
    }
}
