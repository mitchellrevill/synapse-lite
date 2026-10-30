use super::*;
use secbird_core::sim::Sim;
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
fn exact_link_api_requires_auth_and_device_identity_and_only_changes_requested_metadata() {
    let folder = std::env::temp_dir().join(format!(
        "application-api-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let guard = Config(std::env::var_os("SYNAPSE_LITE_CONFIG_DIR"));
    std::env::set_var("SYNAPSE_LITE_CONFIG_DIR", &folder);
    let sim = Sim::mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let key = engine.devices()[0].key();
    let path = folder.join("profiles.json");
    let mut store = Store::load(&path).unwrap();
    store
        .for_device(&engine.devices()[0])
        .create("Linked")
        .unwrap();
    store.save().unwrap();
    let before = std::fs::read(&path).unwrap();
    let original: Value = serde_json::from_slice(&before).unwrap();
    sim.state().log.clear();
    let server = Server {
        port: 43210,
        token: "unit-token".into(),
        engine: Arc::new(Mutex::new(engine)),
    };
    let request = |value: Value| Request {
        method: "POST".into(),
        path: "/api/profile".into(),
        query: String::new(),
        headers: vec![
            ("host".into(), "127.0.0.1:43210".into()),
            ("x-secbird-token".into(), "unit-token".into()),
        ],
        body: serde_json::to_vec(&value).unwrap(),
    };
    for value in [
        json!({"id":key,"expected_device":key,"action":"paths","name":"Linked"}),
        json!({"action":"paths","name":"Linked","paths":["relative.exe"]}),
        json!({"id":key,"expected_device":"other","action":"paths","name":"Linked","paths":[r"C:\Games\Game.exe"]}),
        json!({"id":"missing","expected_device":"missing","action":"paths","name":"Linked","paths":[r"C:\Games\Game.exe"]}),
        json!({"id":key,"expected_device":key,"action":"paths","name":"Linked","paths":[r"C:\Games\Game.exe","relative.exe"]}),
    ] {
        assert_ne!(server.route(&request(value)).status, "200 OK");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    let mut unauthenticated = request(
        json!({"id":key,"expected_device":key,"action":"paths","name":"Linked","paths":[r"C:\Games\Game.exe"]}),
    );
    unauthenticated.headers.pop();
    assert_eq!(server.route(&unauthenticated).status, "403 Forbidden");
    let response=server.route(&request(json!({"id":key,"expected_device":key,"action":"paths","name":"Linked","paths":[r"C:\Games\Game.exe"]})));
    assert_eq!(response.status, "200 OK");
    let mut after: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        after["devices"][&key]["profiles"]["Linked"]["app_paths"],
        json!([r"c:\games\game.exe"])
    );
    after["devices"][&key]["profiles"]["Linked"]
        .as_object_mut()
        .unwrap()
        .remove("app_paths");
    assert_eq!(after, original);
    assert!(
        sim.state().log.iter().all(|c| c.id & 0x80 != 0),
        "link configuration issued a hardware SET"
    );
    let bytes = std::fs::read(&path).unwrap();
    server
        .engine
        .lock()
        .unwrap()
        .enforce_measurement_read_only()
        .unwrap();
    assert_ne!(
        server
            .route(&request(
                json!({"id":key,"expected_device":key,"action":"paths","name":"Linked","paths":[]})
            ))
            .status,
        "200 OK"
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(folder).unwrap();
    drop(guard);
}

#[test]
fn application_metadata_uses_authentication_and_never_waits_for_the_device_lock() {
    let sim = Sim::mouse();
    let engine = Engine::with_simulated_mouse(&sim).unwrap();
    let server = Server {
        port: 43210,
        token: "unit-token".into(),
        engine: Arc::new(Mutex::new(engine)),
    };
    let mut req = Request {
        method: "GET".into(),
        path: "/api/application-paths".into(),
        query: String::new(),
        headers: vec![
            ("host".into(), "127.0.0.1:43210".into()),
            ("x-secbird-token".into(), "unit-token".into()),
        ],
        body: vec![],
    };
    let held = server.engine.lock().unwrap();
    let count = std::sync::atomic::AtomicUsize::new(0);
    let provider = || {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![r"c:\games\game.exe".into()]
    };
    assert_eq!(
        server
            .route_with_metadata(&req, || unreachable!(), provider)
            .status,
        "200 OK"
    );
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    req.path = "/api/apps".into();
    let response = server.route_with_metadata(&req, || unreachable!(), provider);
    let value: Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(value["apps"], json!(["game.exe"]));
    let accepted = count.load(std::sync::atomic::Ordering::SeqCst);
    req.method = "POST".into();
    assert_eq!(
        server
            .route_with_metadata(&req, || unreachable!(), provider)
            .status,
        "405 Method Not Allowed"
    );
    req.method = "GET".into();
    req.body = vec![b'X'];
    assert_eq!(
        server
            .route_with_metadata(&req, || unreachable!(), provider)
            .status,
        "400 Bad Request"
    );
    req.body.clear();
    req.headers.pop();
    assert_eq!(
        server
            .route_with_metadata(&req, || unreachable!(), provider)
            .status,
        "403 Forbidden"
    );
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), accepted);
    drop(held);
}
