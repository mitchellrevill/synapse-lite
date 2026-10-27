//! Profile preview/commit jobs. Parsing and bounded file work stay off the UI thread.
use super::*;
use profile_modal_model::{Message, Owner, Request};
use secbird_core::profile_compat::{self, gui};

fn owner(engine: &Engine, store: &secbird_core::Store, want: &Owner) -> Result<(), String> {
    if !engine.devices().iter().any(|d| d.key() == want.device) {
        return Err("The selected device disconnected; reopen the profile dialog".into());
    }
    let current = gui::stored_device_key(store, &want.device)
        .and_then(|key| store.devices.get(&key))
        .map(|d| d.active.as_str())
        .unwrap_or("Default");
    if current != want.profile {
        return Err("The active profile changed; reopen the profile dialog".into());
    }
    Ok(())
}
fn execute(request: Request, engine: &Arc<Mutex<Engine>>) -> Result<Reply, String> {
    match request {
        Request::PreviewImport { token, owner, path } => {
            let bytes = profile_files::read(&path)?;
            let data = Arc::new(gui::preview_import(&bytes).map_err(|e| e.to_string())?);
            let notices = data.warnings.clone();
            Ok(Reply {
                state: None,
                buttons: None,
                status: "Profile preview ready; nothing was applied".into(),
                failed: false,
                notices,
                profile_modal: Some(Message::Preview { token, owner, data }),
            })
        }
        Request::PreviewExport { token, owner: want } => {
            let e = engine.lock().map_err(|_| "Device worker is unavailable")?;
            let backup = server::export_profile_document(None).map_err(|e| e.to_string())?;
            let mut store = profile_compat::import_document(&backup.bytes)
                .map_err(|e| e.to_string())?
                .store;
            owner(&e, &store, &want)?;
            let actual = e
                .devices()
                .iter()
                .find(|d| d.key() == want.device)
                .ok_or("Selected device disconnected")?;
            store.for_device(actual);
            let candidate = serde_json::to_vec(&store).map_err(|e| e.to_string())?;
            let data = Arc::new(
                gui::preview_export(&candidate, &want.device, &want.profile)
                    .and_then(|preview| preview.preserve_native_backup(&backup.bytes))
                    .map_err(|e| e.to_string())?,
            );
            Ok(Reply {
                state: None,
                buttons: None,
                status: "Choose profiles and macros to export".into(),
                failed: false,
                notices: Vec::new(),
                profile_modal: Some(Message::Preview {
                    token,
                    owner: want,
                    data,
                }),
            })
        }
        Request::Import {
            owner: want,
            data,
            choices,
        } => {
            let mut e = engine.lock().map_err(|_| "Device worker is unavailable")?;
            let backup = server::export_profile_document(None).map_err(|e| e.to_string())?;
            let current = profile_compat::import_document(&backup.bytes)
                .map_err(|e| e.to_string())?
                .store;
            owner(&e, &current, &want)?;
            let selected = gui::selected_import(&data, &choices, &current, &want.device)
                .map_err(|e| e.to_string())?;
            let answer = server::import_profile_document_bound(
                &mut e,
                &selected.bytes,
                &want.device,
                &want.profile,
            )
            .map_err(|e| e.to_string())?;
            let mut notices = selected.warnings;
            for n in strings(&answer["warnings"]) {
                if !notices.contains(&n) {
                    notices.push(n)
                }
            }
            Ok(Reply {
                state: answer.get("state").cloned(),
                buttons: None,
                status: format!(
                    "Imported {} profile(s); no hardware profile was activated",
                    answer["added"].as_u64().unwrap_or(0)
                ),
                failed: false,
                notices,
                profile_modal: None,
            })
        }
        Request::Export { plan, path, format } => {
            let e = engine.lock().map_err(|_| "Device worker is unavailable")?;
            let latest = server::export_profile_document(None).map_err(|e| e.to_string())?;
            let current = profile_compat::import_document(&latest.bytes)
                .map_err(|e| e.to_string())?
                .store;
            owner(&e, &current, &plan.owner)?;
            if latest.bytes != plan.data.full_native_backup() {
                return Err(
                    "Saved profiles changed after export preview; reopen Export Profiles".into(),
                );
            }
            let report=match format{profile_files::Format::Synapse4=>gui::selected_export(&plan.data,&plan.choices).map_err(|e|e.to_string())?,profile_files::Format::Native=>profile_compat::ExportReport{bytes:plan.data.full_native_backup().to_vec(),warnings:vec!["Full native backup includes all saved devices, profiles and opaque settings; source profile/macro selection applies only to Synapse export".into()]}};
            drop(e);
            profile_files::write(&path, &report.bytes)?;
            Ok(Reply {
                state: None,
                buttons: None,
                status: "Profile export saved".into(),
                failed: false,
                notices: report.warnings,
                profile_modal: None,
            })
        }
    }
}
pub(super) fn run(request: Request, engine: &Arc<Mutex<Engine>>) -> Reply {
    let preview = request.preview_owner();
    match execute(request, engine) {
        Ok(r) => r,
        Err(error) => match preview {
            Some((token, owner)) => Reply {
                state: None,
                buttons: None,
                status: "Profile preview could not be loaded".into(),
                failed: false,
                notices: Vec::new(),
                profile_modal: Some(Message::Error {
                    token,
                    owner,
                    error,
                }),
            },
            None => Reply {
                state: None,
                buttons: None,
                status: error,
                failed: true,
                notices: Vec::new(),
                profile_modal: None,
            },
        },
    }
}
