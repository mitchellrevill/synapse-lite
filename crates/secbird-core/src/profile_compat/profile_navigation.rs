//! Source-shaped Profile Switch authoring. No profile activation or device I/O.
use crate::{
    profile::{DeviceProfiles, NavigationStatus, Profile},
    Error, Result,
};
use serde_json::{json, Value};
#[cfg(test)]
mod tests;

pub const CHOICES: &[(&str, &str)] = &[
    ("NextProfile", "Next Profile"),
    ("PreviousProfile", "Previous Profile"),
    ("CycleUp", "Cycle Up Profile"),
    ("CycleDown", "Cycle Down Profile"),
    ("Specific", "Specific Profile"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub input_id: String,
    pub input_type: String,
    pub hyper: bool,
    pub action: String,
    pub guid: Option<String>,
    pub lightpac_id: Option<String>,
}
impl Edit {
    pub fn group(&self) -> Value {
        let mut group = json!({"profileNavigationAssignment": self.action});
        if self.action == "Specific" {
            if let Some(guid) = &self.guid {
                group["guid"] = json!(guid);
            }
            if let Some(id) = &self.lightpac_id {
                group["lightpacId"] = json!(id);
            }
        }
        group
    }
}

/// Same-device GUID resolution happens again at the save boundary. A stale name
/// never retargets a reference, and unknown source data survives explicit edits.
pub fn candidate(
    profile: &Profile,
    pid: u16,
    profiles: &DeviceProfiles,
    chroma_ids: &[&str],
    edit: &Edit,
) -> Result<Profile> {
    super::windows_shortcuts::validate_input(pid, &edit.input_id, &edit.input_type).map_err(
        |_| Error::Unsupported("Profile switching is unavailable for this input".into()),
    )?;
    if edit.action != "Specific" && (edit.guid.is_some() || edit.lightpac_id.is_some()) {
        return Err(Error::InvalidArg(
            "Only Specific Profile accepts a target or lighting link".into(),
        ));
    }
    let group = edit.group();
    let supported: Vec<_> = CHOICES.iter().map(|choice| choice.0).collect();
    if let NavigationStatus::Unavailable(reason) =
        profiles.navigation_status(&group, &supported, chroma_ids)
    {
        return Err(Error::InvalidArg(reason.into()));
    }
    let mut next = profile.clone();
    let rows = next
        .extra
        .get_mut("_synapseLiteSynapse4")
        .and_then(|source| source.get_mut("decoded"))
        .and_then(|decoded| decoded.get_mut("mappings"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| Error::InvalidArg("Source mappings unavailable".into()))?;
    if rows.iter().any(|row| {
        row["inputID"] == edit.input_id && row.get("isHyperShift").is_some_and(|v| !v.is_boolean())
    }) {
        return Err(Error::InvalidArg(
            "Ambiguous source assignment layer".into(),
        ));
    }
    let indices: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row["inputID"] == edit.input_id
                && row["isHyperShift"].as_bool().unwrap_or(false) == edit.hyper
        })
        .map(|(index, _)| index)
        .collect();
    if indices.len() > 1 {
        return Err(Error::InvalidArg(
            "Duplicate source assignments; original records retained".into(),
        ));
    }
    let mut row = indices.first().map(|index| rows[*index].clone())
        .unwrap_or_else(|| json!({"inputID":edit.input_id,"inputType":edit.input_type,"isHyperShift":edit.hyper}));
    if !row.is_object() || row["inputType"] != edit.input_type || row.get("mapping").is_some() {
        return Err(Error::Unsupported(
            "Incompatible or analog source assignment".into(),
        ));
    }
    let native_key = super::mappings::setting_key(&row, pid);
    row["isDefault"] = json!(false);
    row["outputType"] = json!("profileNavigationGroup");
    if row.get("profileNavigationGroup").is_none() {
        row["profileNavigationGroup"] = json!({});
    }
    let target = row["profileNavigationGroup"]
        .as_object_mut()
        .ok_or_else(|| Error::InvalidArg("Malformed profile navigation group; preserved".into()))?;
    for name in [
        "profileNavigationAssignment",
        "guid",
        "lightpacId",
        "Name",
        "ProfileId",
    ] {
        target.remove(name);
    }
    target.extend(group.as_object().expect("group is an object").clone());
    if let Some(index) = indices.first() {
        rows[*index] = row;
    } else {
        rows.push(row);
    }
    if let Some(key) = native_key {
        next.buttons.remove(&key);
    }
    Ok(next)
}
