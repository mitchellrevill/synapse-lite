//! Worker-owned local profile save. Deliberately has no HTTP route or device SET.
use super::*;
use secbird_core::profile_compat::mouse_assignment::{self, Assignment};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    expected_profile: String,
    expected_payload: secbird_core::Profile,
    assignment: Assignment,
}

pub(crate) fn save_mouse_assignment(engine: &mut Engine, body: &Value) -> Result<Value, Error> {
    if engine.is_measurement_read_only() {
        return Err(Error::Unsupported(
            "Read-only mode refuses assignment saves".into(),
        ));
    }
    let request: Request =
        serde_json::from_value(body.clone()).map_err(|e| Error::InvalidArg(e.to_string()))?;
    let dev = engine
        .devices()
        .iter()
        .find(|dev| dev.key() == request.id)
        .ok_or_else(|| Error::NotFound("Selected device disconnected; reopen assignment".into()))?;
    let mut store = Store::load(&config_dir().join("profiles.json"))?;
    let profiles = store.for_device(dev);
    if profiles.active != request.expected_profile || profiles.active() != &request.expected_payload
    {
        return Err(Error::InvalidArg(
            "Saved profile changed; reopen assignment before saving".into(),
        ));
    }
    let input=request.assignment.input_id.clone();let hyper=request.assignment.hyper;
    *profiles.active_mut()=secbird_core::profile_compat::assignment_state::prepare_layer(profiles.active(),dev.pid,&request.expected_profile,&input,hyper)?;
    mouse_assignment::set_pending(profiles.active_mut(),dev.pid,&request.expected_profile,request.assignment)?;
    secbird_core::profile_compat::assignment_state::mark_local(profiles.active_mut(),dev.pid,&input,hyper)?;
    store.save()?;
    // Ordinary cached presentation only. Never activates/replays the profile.
    Ok(state(engine))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Config(Option<std::ffi::OsString>);
    impl Drop for Config {
        fn drop(&mut self) {
            match &self.0 {
                Some(value) => std::env::set_var("SYNAPSE_LITE_CONFIG_DIR", value),
                None => std::env::remove_var("SYNAPSE_LITE_CONFIG_DIR"),
            }
        }
    }
    #[test]
    fn assignment_save_checks_owner_payload_read_only_and_never_sets_device() {
        let folder = std::env::temp_dir().join(format!(
            "mouse-assignment-server-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let _config = Config(std::env::var_os("SYNAPSE_LITE_CONFIG_DIR"));
        std::env::set_var("SYNAPSE_LITE_CONFIG_DIR", &folder);
        let sim = secbird_core::sim::Sim::wireless_mouse();
        let mut engine = Engine::with_simulated_mouse(&sim).unwrap();
        let dev = &engine.devices()[0];
        let key = dev.key();
        let path = folder.join("profiles.json");
        let mut store = Store::load(&path).unwrap();
        *store.for_device(dev).active_mut() =
            secbird_core::profile_compat::new_native_profile(dev.pid, "Default").unwrap();
        store.save().unwrap();
        let original = store.devices[&key].active().clone();
        let body = json!({"id":key,"expected_profile":"Default","expected_payload":original,"assignment":{"input_id":"RightClick","input_type":"MouseInput","hyper":false,"choice":"DoubleClick","turbo":null}});
        let before = std::fs::read(&path).unwrap();
        for (field, value) in [
            ("id", json!("other")),
            ("expected_profile", json!("other")),
            ("expected_payload", json!({"future":"changed"})),
        ] {
            let mut wrong = body.clone();
            wrong[field] = value;
            assert!(save_mouse_assignment(&mut engine, &wrong).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        save_mouse_assignment(&mut engine, &body).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert_ne!(saved, before);
        let reopened = Store::load(&path).unwrap();
        assert_eq!(reopened.devices[&key].active().buttons, original.buttons);
        assert!(save_mouse_assignment(&mut engine, &body).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        engine.enforce_measurement_read_only().unwrap();
        let mut latest = body;
        latest["expected_payload"] = serde_json::to_value(reopened.devices[&key].active()).unwrap();
        assert!(save_mouse_assignment(&mut engine, &latest).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert!(sim.state().log.iter().all(|command| command.id & 0x80 != 0));
    }
}
