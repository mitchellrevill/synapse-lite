//! Local editor: one embedded page plus a small JSON API on 127.0.0.1.
//!
//! Any website can send requests to localhost, so every request must:
//! - carry a Host header naming this server (blocks DNS rebinding),
//! - have no Origin, or our own origin,
//! - include the per-session token (page: `?t=` query, API: `X-Secbird-Token` header;
//!   a custom header also forces a CORS preflight, which we never grant).

use crate::engine::Engine;
use secbird_core::{actions, config_dir, Device, Error, Kind, Rgb, Store};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
mod lighting;
#[cfg(test)]
mod application_paths_tests;
#[cfg(test)]
mod physical_static_probe;
#[cfg(test)]
mod physical_uniform_probe;
#[cfg(test)]
mod physical_wave_probe;

const PAGE: &str = include_str!("editor.html");
const LOGO_PNG: &[u8] = include_bytes!("../../../assets/logo-64.png");
const MAX_HEADER: usize = 16 * 1024;
/// Profile imports with painted keys can be a few hundred KB.
const MAX_BODY: usize = 2 * 1024 * 1024;

pub struct Editor {
    pub url: String,
}

/// Bind a random local port and serve on a background thread.
pub fn start(engine: Arc<Mutex<Engine>>) -> std::io::Result<Editor> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let token = new_token();
    let url = format!("http://127.0.0.1:{port}/?t={token}");
    let server = Server { port, token, engine };
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            server.handle(stream);
        }
    });
    Ok(Editor { url })
}

/// 128 bits from std's OS-seeded hasher keys; no extra crate needed.
fn new_token() -> String {
    let nanos = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut token = String::with_capacity(32);
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(nanos);
        token.push_str(&format!("{:016x}", h.finish()));
    }
    token
}

fn base64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn query_param(&self, name: &str) -> Option<&str> {
        self.query.split('&').find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let header_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEADER {
            return None;
        }
    };
    let head = std::str::from_utf8(&buf[..header_end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();

    let len: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    if len > MAX_BODY {
        return None;
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(len);
    Some(Request { method, path: path.to_string(), query: query.to_string(), headers, body })
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn json(v: Value) -> Self {
        Response { status: "200 OK", content_type: "application/json", body: v.to_string().into_bytes() }
    }

    fn error(status: &'static str, msg: impl Into<String>) -> Self {
        let body = json!({ "error": msg.into() }).to_string().into_bytes();
        Response { status, content_type: "application/json", body }
    }

    fn text(status: &'static str, msg: &str) -> Self {
        Response { status, content_type: "text/plain; charset=utf-8", body: msg.as_bytes().to_vec() }
    }
}

struct Server {
    port: u16,
    token: String,
    engine: Arc<Mutex<Engine>>,
}

impl Server {
    fn handle(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let Some(req) = read_request(&mut stream) else { return };
        let resp = self.route(&req);
        let head = format!(
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
             X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n\
             Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src data:\r\n\
             Connection: close\r\n\r\n",
            resp.status,
            resp.content_type,
            resp.body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&resp.body);
    }

    fn origin_ok(&self, req: &Request) -> bool {
        let hosts = [format!("127.0.0.1:{}", self.port), format!("localhost:{}", self.port)];
        let host_ok = req.header("host").is_some_and(|h| hosts.iter().any(|x| x == h));
        let origin_ok = req
            .header("origin")
            .is_none_or(|o| hosts.iter().any(|x| o == format!("http://{x}")));
        host_ok && origin_ok
    }

    fn route(&self, req: &Request) -> Response {
        self.route_with_audio(req, crate::audio_endpoints::discover)
    }

    fn route_with_audio(&self, req: &Request, audio: impl FnOnce()->Result<crate::audio_endpoints::Snapshot,String>) -> Response {
        self.route_with_metadata(req,audio,crate::apps::running_paths)
    }
    fn route_with_metadata(&self, req:&Request, audio:impl FnOnce()->Result<crate::audio_endpoints::Snapshot,String>, applications:impl FnOnce()->Vec<String>)->Response {
        if !self.origin_ok(req) {
            return Response::text("403 Forbidden", "Forbidden");
        }
        if req.method == "GET" && req.path == "/" {
            return if req.query_param("t") == Some(self.token.as_str()) {
                let logo = format!("data:image/png;base64,{}", base64(LOGO_PNG));
                let page = PAGE.replace("{{LOGO}}", &logo);
                Response { status: "200 OK", content_type: "text/html; charset=utf-8", body: page.into_bytes() }
            } else {
                Response::text("403 Forbidden", "Open the editor from the Secretarybird tray menu.")
            };
        }
        if !req.path.starts_with("/api/") {
            return Response::text("404 Not Found", "Not found");
        }
        if req.header("x-secbird-token") != Some(self.token.as_str()) {
            return Response::error("403 Forbidden", "bad token");
        }
        // Endpoint metadata is independent of HID and must not hold its engine
        // mutex while calling Windows audio services.
        if req.path=="/api/audio/endpoints" {
            if req.method!="GET" {return Response::error("405 Method Not Allowed","Audio discovery accepts GET only");}
            if !req.body.is_empty() {return Response::error("400 Bad Request","Audio discovery takes no request body");}
            return match audio().and_then(|value|serde_json::to_value(value).map_err(|error|error.to_string())) {
                Ok(value)=>Response::json(value),Err(error)=>Response::error("503 Service Unavailable",error),
            };
        }
        if matches!(req.path.as_str(),"/api/apps"|"/api/application-paths") {
            if req.method!="GET" {return Response::error("405 Method Not Allowed","Application discovery accepts GET only");}
            if !req.body.is_empty() {return Response::error("400 Bad Request","Application discovery takes no request body");}
            let paths=applications();
            return Response::json(if req.path=="/api/application-paths" {json!({"paths":paths})} else {json!({"apps":paths.iter().map(|path|secbird_core::profile::normalize_app(path)).collect::<std::collections::BTreeSet<_>>()})});
        }
        let mut engine = match self.engine.lock() {
            Ok(e) => e,
            Err(poisoned) => poisoned.into_inner(),
        };
        let result = dispatch(&mut engine, &req.method, &req.path, &req.body);
        match result {
            Ok(v) => Response::json(v),
            Err(e) => Response::error("400 Bad Request", e.to_string()),
        }
    }
}

/// Shared device/profile boundary for the native editor and optional legacy
/// browser editor. Calling this function opens no listener or browser process.
pub(crate) fn dispatch(engine: &mut Engine, method: &str, path: &str, body: &[u8]) -> Result<Value, Error> {
    if engine.is_measurement_read_only() && method != "GET" {
        return Err(Error::Unsupported("Read-only measurement refuses mutation requests".into()));
    }
    match (method, path) {
        ("GET", "/api/state") => Ok(state(engine)),
        ("GET", "/api/apps") => Ok(json!({ "apps": crate::apps::running() })),
        ("GET", "/api/application-paths") => Ok(json!({ "paths": crate::apps::running_paths() })),
        ("GET", "/api/live") => {
            let (seq, dpi) = engine.live_dpi();
            Ok(json!({ "seq": seq, "dpi": dpi.iter().map(|(k, (x, y))| (k.clone(), json!([x, y]))).collect::<serde_json::Map<_, _>>() }))
        }
        ("POST", "/api/rescan") => {
            engine.rescan();
            Ok(state(engine))
        }
        ("GET", "/api/settings") => Ok(settings_json(engine)),
        ("GET", "/api/export") => Store::load(&config_dir().join("profiles.json"))
            .and_then(|s| serde_json::to_value(&s).map_err(|e| Error::Db(e.to_string()))),
        ("POST", path) => {
            let result=post(engine,path,body);
            if result.is_ok() && (path.starts_with("/api/lighting") || path.starts_with("/api/source-") || path.starts_with("/api/profile") || matches!(path,"/api/color"|"/api/effect"|"/api/off"|"/api/keys"|"/api/import")) {
                engine.request_lighting_refresh();
            }
            result
        },
        _ => Err(Error::NotFound(path.to_string())),
    }
}

fn null_as_empty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

#[derive(Deserialize)]
struct Body {
    /// Device key (`PID:SERIAL`); older clients send only `pid`. The editor sends null when
    /// no device can be controlled (#51), which the app-wide requests don't need.
    #[serde(default, deserialize_with = "null_as_empty")]
    id: String,
    #[serde(default)]
    pid: String,
    #[serde(default)]
    x: u16,
    #[serde(default)]
    y: u16,
    #[serde(default)]
    hz: u32,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    value: u8,
    #[serde(default)]
    action: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    previous_name: String,
    #[serde(default)]
    keys: Vec<String>,
    /// Finding a keyboard's layout: the `[row, col]` light to show; none puts the profile back.
    #[serde(default)]
    light: Option<(u8, u8)>,
    #[serde(default)]
    apps: Vec<String>,
    #[serde(default)]
    paths: Option<Vec<String>>,
    /// A Settings page switch (`key`) and whether it is on; `data` for an imported file.
    #[serde(default)]
    key: String,
    #[serde(default)]
    on: bool,
    /// Device-wide lighting ownership, independent of the active profile.
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    data: Value,
    /// Button name, layer ("normal" / "hypershift") and action text ("" = reset).
    #[serde(default)]
    button: String,
    #[serde(default)]
    layer: String,
    #[serde(default)]
    assign: String,
    /// DPI stages: the 1-based stage in use and `[x, y]` per stage.
    #[serde(default)]
    active: u8,
    #[serde(default)]
    stages: Vec<(u16, u16)>,
    #[serde(default)]
    performance: Option<secbird_core::performance::Performance>,
    #[serde(default)]
    expected_profile: Option<String>,
    /// Source Power actions require an exact physical-device identity.
    #[serde(default)]
    expected_device: Option<String>,
    #[serde(default)]
    critical_threshold_percent: Option<u8>,
    #[serde(default)]
    tracking_distance_mm: Option<u8>,
    #[serde(default)]
    dynamic_sensitivity: Option<secbird_core::dynamic_sensitivity::DynamicSensitivity>,
    #[serde(default)]
    rotation: Option<secbird_core::dynamic_sensitivity::Rotation>,
    #[serde(default)]
    lighting_settings: Option<secbird_core::lighting::LightingSettings>,
    #[serde(default)]
    source_effect_id: Option<u8>,
    #[serde(default)]
    source_effect_setting: Option<Value>,
    #[serde(default)]
    custom_colors: Option<Vec<Value>>,
    /// Sleep timer in minutes (0 = leave it), and the auto-sleep switch.
    #[serde(default)]
    minutes: u16,
    #[serde(default)]
    auto: Option<bool>,
    /// Laptop keyboard when idle: stays-lit, half, quarter or dark.
    #[serde(default)]
    dimming: String,
    /// Keyboards: the keys gaming mode blocks, `off` or like `win,alt-tab`.
    #[serde(default)]
    game_mode: String,
    /// Macro steps as text (`tap:ctrl+c wait:50`), and a macro id.
    #[serde(default)]
    script: String,
    #[serde(default)]
    macro_id: u16,
    /// Lighting effect text (`spectrum`, `wave:left@3`).
    #[serde(default)]
    effect: String,
}

/// Every button of a mouse with what it does on each layer, and which ones the active
/// profile remaps.
fn buttons_json(dev: &Device, store: &mut Store) -> Value {
    use secbird_core::buttons::{setting_key, Input, Layer};
    // Keyboards, keypads and laptops list their keys; everything else its mouse buttons.
    let keyboard = dev.spec.as_ref().is_some_and(|s| matches!(s.kind, Kind::Keyboard | Kind::Keypad | Kind::Laptop));
    let inputs: Vec<(Input, _)> = match keyboard {
        true => dev.keys().into_iter().map(|(id, a)| (Input::Key(id), a)).collect(),
        false => dev.buttons().into_iter().map(|(id, a)| (Input::Button(id), a)).collect(),
    };
    let profiles = store.for_device(dev);
    let remapped = &profiles.active().buttons;
    let onboard = profiles.onboard.as_ref();
    // Before a "Save to mouse" the slot holds our remap; the original is in its backup.
    let original = |key: &str| onboard.and_then(|o| o.before.get(key)).cloned();
    let list: Vec<Value> = inputs
        .into_iter()
        .map(|(input, normal)| {
            let name = input.name();
            let hyper = dev.input(input, Layer::HyperShift).map(|a| a.to_string()).ok();
            let is_remapped = |layer| remapped.contains_key(&setting_key(&name, layer));
            // What the device itself holds: the live value unless the active profile remapped it.
            let saved = |layer, live: Option<String>| match is_remapped(layer) {
                true => original(&setting_key(&name, layer)).or_else(|| dev.saved_input(input, layer).map(|a| a.to_string()).ok()),
                false => live,
            };
            json!({
                "name": name,
                "label": input.label(),
                "normal": normal.to_string(),
                "hypershift": hyper,
                "remapped": { "normal": is_remapped(Layer::Normal), "hypershift": is_remapped(Layer::HyperShift) },
                "saved": {
                    "normal": saved(Layer::Normal, Some(normal.to_string())),
                    "hypershift": saved(Layer::HyperShift, hyper.clone()),
                },
            })
        })
        .collect();
    // What "Save to mouse" put in the mouse's slot, and whether that profile changed since.
    let onboard = onboard.map(|o| {
        let changed = profiles.profiles.get(&o.profile).map(|p| &p.buttons) != Some(&o.buttons);
        json!({ "slot": o.slot, "profile": o.profile, "count": o.buttons.len(), "changed": changed })
    });
    json!({ "buttons": list, "onboard": onboard })
}

fn post(engine: &mut Engine, path: &str, body: &[u8]) -> Result<Value, Error> {
    let b: Body = serde_json::from_slice(body).map_err(|e| Error::InvalidArg(e.to_string()))?;
    if path=="/api/profile" && b.action=="paths" && (b.id.is_empty() || b.expected_device.as_deref()!=Some(&b.id) || !engine.devices().iter().any(|dev|dev.key()==b.id)) {return Err(Error::InvalidArg("Executable links require the selected device identity".into()));}
    if path == "/api/customize-multimedia" {
        let dev = engine.devices().iter().find(|d| d.key() == b.id).ok_or_else(|| Error::InvalidArg("Selected device disconnected".into()))?;
        let profile = b.expected_profile.as_deref().filter(|p| !p.is_empty()).ok_or_else(|| Error::InvalidArg("Selected profile identity required".into()))?;
        let edit = serde_json::from_value(b.data).map_err(|e| Error::InvalidArg(format!("Invalid multimedia assignment: {e}")))?;
        let mut store = Store::load(&config_dir().join("profiles.json"))?;
        secbird_core::profile_compat::multimedia::update(&mut store,&b.id,profile,dev.pid,&edit)?;
        store.save()?;
        return Ok(json!({"ok":true,"status":"Saved locally. Multimedia device/host output is unavailable.","state":state(engine)}));
    }
    if path=="/api/sensor-rotation" {
        if b.id.is_empty() || b.expected_device.as_deref()!=Some(&b.id){return Err(Error::InvalidArg("Rotation requires the selected device identity".into()));}
        let profile=b.expected_profile.as_deref().filter(|p|!p.is_empty()).ok_or_else(||Error::InvalidArg("Rotation requires the selected profile identity".into()))?;
        engine.set_rotation(&b.id,profile,b.rotation.ok_or_else(||Error::InvalidArg("Rotation settings missing".into()))?)?;
        return Ok(json!({"ok":true,"state":state(engine)}));
    }
    if path=="/api/dynamic-sensitivity" {
        if b.id.is_empty() || b.expected_device.as_deref()!=Some(&b.id){return Err(Error::InvalidArg("Advanced requires the selected mouse identity".into()));}
        let dev=engine.devices().iter().find(|d|d.key()==b.id).ok_or_else(||Error::InvalidArg("Selected mouse disconnected".into()))?;
        let profile=b.expected_profile.as_deref().filter(|p|!p.is_empty()).ok_or_else(||Error::InvalidArg("Advanced requires the selected profile identity".into()))?;
        let intent=b.dynamic_sensitivity.ok_or_else(||Error::InvalidArg("Sensitivity settings missing".into()))?;
        let mut store=Store::load(&config_dir().join("profiles.json"))?;
        secbird_core::dynamic_sensitivity::set_dynamic(dev,&mut store,&b.id,profile,intent)?;
        return Ok(json!({"ok":true,"state":state(engine)}));
    }
    if path == "/api/calibration-tracking" {
        let expected=b.expected_device.as_deref().filter(|s|!s.is_empty()).ok_or_else(||Error::InvalidArg("Calibration requires a device identity".into()))?;
        if b.id!=expected { return Err(Error::InvalidArg("Calibration device identity changed".into())); }
        let dev=engine.devices().iter().find(|dev|dev.key()==expected).ok_or_else(||Error::InvalidArg("Selected mouse is not connected".into()))?;
        let mm=b.tracking_distance_mm.ok_or_else(||Error::InvalidArg("Tracking distance is missing".into()))?;
        let mut store=Store::load(&config_dir().join("profiles.json"))?;
        secbird_core::mouse_sensor::set_tracking_distance(dev,&mut store,expected,mm)?;
        return Ok(json!({"ok":true,"state":state(engine)}));
    }
    if path == "/api/sync" {
        engine.guard_studio_quick_action("", path)?;
        return sync(engine, &b.color, &b.effect);
    }
    if let Some(answer) = app_post(engine, path, &b) {
        return answer;
    }
    if matches!(path, "/api/critical-threshold" | "/api/device-sleep") {
        validate_power_identity(&b)?;
    }
    let target = if b.id.is_empty() { b.pid.to_ascii_uppercase() } else { b.id.clone() };
    engine.guard_studio_quick_action(&target, path)?;

    if path=="/api/lighting-settings" && engine.studio_holds(&target) {
        if b.expected_device.as_deref()!=Some(target.as_str()) || b.id!=target {
            return Err(Error::InvalidArg("Studio brightness requires the exact selected device identity".into()));
        }
        let profile=b.expected_profile.as_deref().filter(|profile|!profile.is_empty())
            .ok_or_else(||Error::InvalidArg("Studio brightness profile identity is missing".into()))?;
        let want=b.lighting_settings.as_ref().ok_or_else(||Error::InvalidArg("lighting_settings is required".into()))?;
        return engine.set_studio_brightness(&target,profile,want);
    }

    if path == "/api/lighting-settings" {
        let mut store = Store::load(&config_dir().join("profiles.json"))?;
        let dev = engine
            .devices()
            .iter()
            .find(|d| d.key() == target)
            .ok_or_else(|| Error::NotFound(format!("device {target}")))?;
        // Source host programs retain brightness intent for LIVE reconciliation.
        // Normal_Mode0 preserves the established brightness action path.
        if dev.pid == 605 {
            // Keep Normal0 brightness semantics. Driver3 uses a saved intent and
            // a frame-before-LIVE gate, never a firmware brightness SET here.
            if let Some((candidate, name)) = lighting::uniform_settings_candidate(dev, &store, &b)?
            {
                engine.validate_static_context(&target, &candidate, &name)?;
                let gate = engine.host_static_gate(&target, &candidate, &name);
                let worker_ready = engine.uniform_is_enabled();
                let mode = dev.ornata_mode_readback()?;
                if mode.mode() == 3 {
                    let result = lighting::host_uniform_settings(
                        dev,
                        &mut store,
                        candidate,
                        b.lighting_settings.as_ref().unwrap(),
                        gate,
                        worker_ready,
                    );
                    if result.is_ok() {
                        engine.uniform_settings_changed(&target);
                    }
                    return result;
                } else if mode.mode() != 0 {
                    return Err(Error::Unsupported(format!(
                        "Uniform brightness mode{} is unsupported; intent was not saved",
                        mode.mode()
                    )));
                }
            }
            if let Some((candidate, name, color)) =
                lighting::static_settings_candidate(dev, &store, &b)?
            {
                engine.validate_static_context(&target, &candidate, &name)?;
                let gate = engine.host_static_gate(&target, &candidate, &name);
                let already_rendered = engine.static_is_prepared(&target, &name, color);
                let dev = engine.device_mut(&target).unwrap();
                let mode = dev.ornata_mode_readback()?;
                if mode.mode() == 3 {
                    let result = lighting::host_static_settings(
                        dev,
                        &mut store,
                        candidate,
                        b.lighting_settings.as_ref().unwrap(),
                        color,
                        gate,
                        already_rendered,
                    );
                    engine.record_host_static(&target, &name, color, &result);
                    return result;
                } else if mode.mode() != 0 {
                    return Err(Error::Unsupported(format!(
                        "Static brightness mode{} is unsupported; intent was not saved",
                        mode.mode()
                    )));
                }
            }
        }
    }
    // Frame OUT errors are never passed through the generic rescan/retry route.
    if path == "/api/source-effect" {
        let mut store = Store::load(&config_dir().join("profiles.json"))?;
        let gate =
            engine.host_static_gate(&target, &store, b.expected_profile.as_deref().unwrap_or(""));
        let worker_ready = engine.uniform_is_enabled();
        let mut trace = crate::host_uniform::ActionTrace::default();
        let dev = engine
            .device_mut(&target)
            .ok_or_else(|| Error::NotFound(format!("device {target}")))?;
        let response = lighting::effect(dev, &mut store, &b, gate, worker_ready, &mut trace);
        match &response {
            Ok(_) if matches!(b.source_effect_id, Some(2 | 3 | 4)) => {
                let profiles = store
                    .devices
                    .get(&target)
                    .or_else(|| store.devices.get("025D"))
                    .unwrap();
                if let Some(program) =
                    crate::host_uniform::Program::from_profile(profiles.active(), 605)?
                {
                    engine.uniform_action_record(
                        &target,
                        &profiles.active,
                        program,
                        &response,
                        Some(&trace),
                    );
                }
            }
            Ok(_) => engine.clear_uniform_action(&target),
            Err(error) => engine.latch_uniform_action_failure(&target, error, &trace),
        }
        if b.source_effect_id == Some(1) {
            if let Some(profiles) = store.devices.get(&target) {
                if let Some(color) = profiles.active().source_static_color(605) {
                    engine.record_host_static(&target, &profiles.active, color, &response);
                }
            }
        }
        return response;
    }
    // A device can disappear between requests; rescan once and retry before giving up.
    let attempt = |engine: &mut Engine| -> Result<Value, Error> {
        let dev = engine
            .devices()
            .iter()
            .find(|d| d.key() == target || format!("{:04X}", d.pid) == target)
            .ok_or_else(|| Error::NotFound(format!("device {target}")))?;
        let mut store = Store::load(&config_dir().join("profiles.json"))?;
        match path {
            "/api/lighting-control" => {
                let enabled = b.enabled.ok_or_else(|| Error::InvalidArg("enabled must be a boolean".into()))?;
                actions::set_lighting_control(dev, &mut store, enabled)?;
                Ok(json!({ "lighting_control": enabled }))
            }
            "/api/dpi" => {
                let (x, y) = actions::set_dpi(dev, &mut store, b.x, b.y)?;
                Ok(json!({ "dpi": [x, y] }))
            }
            "/api/polling" => actions::set_polling(dev, &mut store, b.hz).map(|_| json!({})),
            "/api/stages" => {
                let want = secbird_core::DpiStages { active: b.active, stages: b.stages.clone(), ..Default::default() };
                let sent = actions::set_dpi_stages(dev, &mut store, &want)?;
                let (x, y) = sent.stages[sent.active as usize - 1];
                Ok(json!({ "stages": { "active": sent.active, "list": sent.stages }, "dpi": [x, y] }))
            }
            "/api/performance" => {
                if b.expected_profile.as_deref() != Some(store.for_device(dev).active.as_str()) {
                    return Err(Error::InvalidArg("The active profile changed; refresh before changing performance".into()));
                }
                let want = b.performance.as_ref().ok_or_else(|| Error::InvalidArg("performance state is required".into()))?;
                let sent = actions::set_performance(dev, &mut store, want)?;
                Ok(json!({ "performance": sent }))
            }
            "/api/dimming" => {
                actions::set_dimming(dev, &mut store, b.dimming.parse()?)?;
                Ok(json!({ "dimming": dev.dimming().ok().flatten().map(|d| d.to_string()) }))
            }
            "/api/gaming" => {
                actions::set_game_mode(dev, &mut store, b.game_mode.parse()?)?;
                Ok(json!({ "game_mode": dev.game_mode().ok().flatten().map(|g| g.to_string()) }))
            }
            "/api/critical-threshold" => critical_threshold_post(dev, &mut store, &b),
            "/api/device-sleep" => device_sleep_post(dev, &b),
            "/api/sleep" => {
                if let Some(on) = b.auto {
                    actions::set_auto_sleep(dev, &mut store, on)?;
                }
                if b.minutes > 0 {
                    actions::set_sleep_minutes(dev, &mut store, b.minutes)?;
                }
                Ok(json!({ "sleep": sleep_json(dev) }))
            }
            "/api/color" => {
                actions::set_color(dev, &mut store, &b.zone, b.color.parse::<Rgb>()?).map(|_| json!({}))
            }
            "/api/brightness" => actions::set_brightness(dev, &mut store, &b.zone, b.value).map(|_| json!({})),
            "/api/lighting-settings" => lighting::intent(dev,&mut store,&b,false),
            "/api/lighting-policy" => lighting::intent(dev,&mut store,&b,true),
            "/api/source-custom-colors" => lighting::custom_colors(dev,&mut store,&b),
            "/api/off" => actions::lights_off(dev, &mut store, &b.zone).map(|_| json!({})),
            "/api/effect" => actions::set_effect(dev, &mut store, &b.zone, b.effect.parse()?).map(|_| json!({})),
            "/api/keys" => {
                let keys = b.keys.iter().map(|k| k.parse::<Rgb>()).collect::<Result<Vec<_>, _>>()?;
                actions::set_keys(dev, &mut store, keys).map(|_| json!({}))
            }
            "/api/find-light" => match b.light {
                Some((row, col)) => actions::show_light(dev, &mut store, row, col).map(|_| json!({})),
                None => {
                    let key=dev.key();
                    let failures: Vec<String> =
                        engine.apply(&[key]).into_iter().flat_map(|r| r.failures).collect();
                    match failures.is_empty() {
                        true => Ok(json!({})),
                        false => Err(Error::InvalidArg(failures.join("; "))),
                    }
                }
            },
            "/api/buttons" => Ok(buttons_json(dev, &mut store)),
            "/api/buttons-restore" => {
                actions::restore_all_buttons(dev, &mut store)?;
                Ok(buttons_json(dev, &mut store))
            }
            "/api/save-to-mouse" => {
                actions::save_buttons_to_mouse(dev, &mut store)?;
                Ok(buttons_json(dev, &mut store))
            }
            "/api/put-back" => {
                actions::put_back_onboard(dev, &mut store)?;
                Ok(buttons_json(dev, &mut store))
            }
            "/api/macros" => macros_json(dev),
            "/api/firmware" => {
                use secbird_core::firmware::{check, Status};
                let connected: Vec<u16> = engine.devices().iter().map(|d| d.pid).collect();
                Ok(match check(dev, &connected)? {
                    Status::UpToDate { current } => json!({ "current": current, "state": "current" }),
                    Status::NoneListed { current } => json!({ "current": current, "state": "none" }),
                    Status::Update { current, latest } => json!({
                        "current": current, "state": "update", "version": latest.version,
                        "url": latest.url, "guide": latest.guide,
                    }),
                })
            }
            "/api/macro" => {
                match b.action.as_str() {
                    "new" => {
                        let script: secbird_core::macros::Script = b.script.parse()?;
                        dev.create_macro(&b.name, &script.0)?;
                    }
                    "delete" => {
                        // A button still playing this macro would silently do nothing.
                        let used: Vec<String> = store
                            .for_device(dev)
                            .profiles
                            .iter()
                            .filter(|(_, p)| p.buttons.values().any(|a| macro_of(a) == Some(b.macro_id)))
                            .map(|(name, _)| name.clone())
                            .collect();
                        if !used.is_empty() {
                            return Err(Error::InvalidArg(format!(
                                "a button in {} plays this macro; change that button first",
                                used.join(", ")
                            )));
                        }
                        dev.delete_macro(b.macro_id)?;
                    }
                    other => return Err(Error::InvalidArg(format!("unknown action '{other}'"))),
                }
                macros_json(dev)
            }
            "/api/button" => {
                let layer = match b.layer.as_str() {
                    "hypershift" => secbird_core::buttons::Layer::HyperShift,
                    _ => secbird_core::buttons::Layer::Normal,
                };
                let action = match b.assign.trim() {
                    "" => None,
                    text => Some(text.parse()?),
                };
                actions::set_button(dev, &mut store, &b.button, layer, action)?;
                Ok(buttons_json(dev, &mut store))
            }
            "/api/profile" => {
                let profiles = store.for_device(dev);
                match b.action.as_str() {
                    "new" => profiles.create(b.name.trim())?,
                    "add" => {
                        let name = b.name.trim();
                        if profiles.profiles.contains_key(name) {
                            return Err(Error::InvalidArg(format!("profile '{name}' already exists")));
                        }
                        let profile = secbird_core::profile_compat::new_native_profile(dev.pid, name)?;
                        profiles.profiles.insert(name.into(), profile);
                        profiles.switch(name)?;
                    }
                    "duplicate" => {
                        profiles.create(b.name.trim())?;
                        profiles.switch(b.name.trim())?;
                    }
                    "rename" => profiles.rename(&b.previous_name, b.name.trim())?,
                    "delete" => profiles.delete(&b.name)?,
                    "use" => profiles.switch(&b.name)?,
                    "apps" => profiles.set_apps(&b.name, &b.apps)?,
                    "paths" => profiles.set_app_paths(&b.name, b.paths.as_deref().ok_or_else(||Error::InvalidArg("paths must be an explicit list; an empty list clears links".into()))?)?,
                    other => return Err(Error::InvalidArg(format!("unknown action '{other}'"))),
                }
                let switched = matches!(b.action.as_str(), "use" | "add" | "duplicate");
                secbird_core::profile_compat::validate_store_budget(&store)?;
                store.save()?;
                let key=dev.key();
                let device=device_json(dev);
                let failures:Vec<String>=if switched {engine.apply(&[key]).into_iter().flat_map(|r|r.failures).collect()} else {Vec::new()};
                Ok(json!({ "device": device, "failures": failures }))
            }
            _ => Err(Error::NotFound(path.to_string())),
        }
    };

    if path == "/api/find-light" {
        engine.pause_frames(b.light.is_some());
    }
    match attempt(engine) {
        Err(Error::Hid(_)) | Err(Error::NotFound(_)) | Err(Error::NoReply(_)) => {
            engine.rescan();
            attempt(engine)
        }
        other => other,
    }
}

/// One color, or one effect started together, on every zone of every connected device.
fn sync(engine: &mut Engine, color: &str, effect: &str) -> Result<Value, Error> {
    let mut store = Store::load(&config_dir().join("profiles.json"))?;
    let results = match effect.is_empty() {
        true => actions::sync_color(engine.devices(), &mut store, color.parse()?),
        false => actions::sync_effect(engine.devices(), &mut store, &effect.parse()?),
    };
    let zones = results.iter().filter(|r| r.result.is_ok()).count();
    let mut pids: Vec<&str> = results.iter().filter(|r| r.result.is_ok()).map(|r| r.key.as_str()).collect();
    pids.dedup();
    // Devices that can't show it at all (older lighting, or an effect the firmware doesn't
    // run) are named once; anything else is a real failure, per zone.
    let mut skipped: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for r in &results {
        match &r.result {
            Err(Error::Unsupported(_)) => {
                if !skipped.contains(&r.device) {
                    skipped.push(r.device.clone());
                }
            }
            Err(e) => failed.push(format!("{} {}: {e}", r.device, r.zone)),
            Ok(()) => {}
        }
    }
    Ok(json!({ "zones": zones, "devices": pids.len(), "skipped": skipped, "failed": failed, "state": state(engine) }))
}

/// The app's own settings, for the editor's Settings page.
fn settings_json(engine: &Engine) -> Value {
    let s = crate::settings::TraySettings::load();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "measurement_read_only":engine.is_measurement_read_only(),
        "query_only_device_handles":engine.is_measurement_read_only() && engine.devices().iter().all(Device::is_query_only),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "folder": config_dir().display().to_string(),
        "autostart": crate::settings::autostart_enabled(),
        "updates": !s.updates_off,
        "firmware_weekly": s.firmware_weekly,
        "popups": !s.popups_off,
        "battery_warning": !s.battery_warning_off,
        "battery_warning_at": s.low_battery(),
        "auto_switch": !s.auto_switch_off,
        "away_lights": s.away_lights.as_deref().unwrap_or(""),
        "synapse": engine.synapse(),
    })
}

/// Settings page actions that are about the app, not one device. `None` for other paths.
fn app_post(engine: &mut Engine, path: &str, b: &Body) -> Option<Result<Value, Error>> {
    use crate::settings::{set_autostart, TraySettings};
    Some(match path {
        "/api/settings" => {
            let mut s = TraySettings::load();
            match b.key.as_str() {
                "autostart" => set_autostart(b.on),
                "updates" => s.updates_off = !b.on,
                "firmware_weekly" => s.firmware_weekly = b.on,
                "popups" => s.popups_off = !b.on,
                "battery_warning" => s.battery_warning_off = !b.on,
                "battery_warning_at" => {
                    let range = crate::settings::BATTERY_RANGE;
                    if !range.contains(&b.value) {
                        return Some(Err(Error::InvalidArg(format!(
                            "pick a battery warning level from {}% to {}%",
                            range.start(),
                            range.end()
                        ))));
                    }
                    s.battery_warning_at = Some(b.value);
                }
                "auto_switch" => s.auto_switch_off = !b.on,
                // `data`: "dim", "off", or "" to leave the lights as they are.
                "away_lights" => {
                    let want = b.data.as_str().unwrap_or_default();
                    if !want.is_empty() && !crate::settings::AWAY_LIGHTS.iter().any(|(name, _)| *name == want) {
                        return Some(Err(Error::InvalidArg(format!("unknown choice '{want}' for the lights when away"))));
                    }
                    s.away_lights = (!want.is_empty()).then(|| want.to_string());
                }
                other => return Some(Err(Error::InvalidArg(format!("unknown setting '{other}'")))),
            }
            s.save();
            Ok(settings_json(engine))
        }
        "/api/update-check" => secbird_core::update::check().map(|r| match r {
            Some(r) => json!({ "latest": r.version, "url": r.url, "installed": secbird_core::update::installed() }),
            None => json!({ "latest": null }),
        }),
        // The tray downloads, checks and installs it, and shows how it went; this tray and
        // this editor page close when the installer starts.
        "/api/update-install" => (|| {
            if !secbird_core::update::installed() {
                return Err(Error::Unsupported("this copy was unzipped, not installed, so it can't update itself; download the new version from the release page".into()));
            }
            let r = secbird_core::update::check()?.ok_or_else(|| Error::InvalidArg("you already have the latest version".into()))?;
            if !crate::win::request_update(r.clone()) {
                return Err(Error::Unsupported("the tray isn't running, so it can't update; start Secretarybird and try again".into()));
            }
            Ok(json!({ "version": r.version }))
        })(),
        "/api/import" => (|| {
            let bytes = serde_json::to_vec(&b.data).map_err(|e| Error::InvalidArg(e.to_string()))?;
            import_profile_document(engine, &bytes, (!b.id.is_empty()).then_some(b.id.as_str()))
        })(),
        "/api/export-synapse4" => (|| {
            let store = Store::load(&config_dir().join("profiles.json"))?;
            let exported = secbird_core::profile_compat::export_synapse4(&store, &b.id)?;
            let document: Value = serde_json::from_slice(&exported.bytes).map_err(|e| Error::Db(e.to_string()))?;
            Ok(json!({ "document": document, "warnings": exported.warnings }))
        })(),
        "/api/open-folder" => {
            crate::settings::open_config_folder();
            Ok(json!({}))
        }
        "/api/report" => Ok(json!({ "text": engine.report_text() })),
        "/api/restore-all-buttons" => Ok(json!({ "results": engine.restore_buttons().into_iter().map(|(name, r)| match r {
            Ok(n) => json!({ "device": name, "restored": n }),
            Err(e) => json!({ "device": name, "error": e.to_string() }),
        }).collect::<Vec<_>>() })),
        _ => return None,
    })
}

/// Import a selected local file without a JSON transport round trip. The merged
/// candidate remains local until its size/complexity has passed validation.
pub(crate) fn import_profile_document(engine: &mut Engine, bytes: &[u8], target: Option<&str>) -> Result<Value, Error> {
    if engine.is_measurement_read_only() { return Err(Error::Unsupported("Read-only measurement refuses imports".into())); }
    let incoming = secbird_core::profile_compat::import_document(bytes)?;
    let connected: Vec<_> = engine.all_keys().into_iter().filter(|key| target.is_none_or(|target| key == target)).collect();
    incoming.store.validate_import_targets(&connected)?;
    let mut store = Store::load(&config_dir().join("profiles.json"))?;
    let added = store.import_for(incoming.store, &connected);
    secbird_core::profile_compat::validate_store_budget(&store)?;
    store.save()?;
    Ok(json!({ "added": added, "warnings": incoming.warnings, "state": state(engine) }))
}

/// Resolve storage only in the local candidate. Store::for_device recognizes an
/// exact identity, the same nonempty serial across196/197, and its own legacy PID.
/// Refused candidates are dropped, so no migration is saved before validation.
fn bound_import_candidate(
    mut store: Store,
    dev: &Device,
    expected_active: &str,
) -> Result<Store, Error> {
    if store.for_device(dev).active != expected_active {
        return Err(Error::InvalidArg(
            "The active profile changed; reopen profile import".into(),
        ));
    }
    Ok(store)
}

/// GUI import binding is independent of lighting/layout ownership. Only an
/// exact connected physical device can receive the validated local candidate.
pub(crate) fn import_profile_document_bound(
    engine: &mut Engine,
    bytes: &[u8],
    expected_device: &str,
    expected_active: &str,
) -> Result<Value, Error> {
    if engine.is_measurement_read_only() { return Err(Error::Unsupported("Read-only measurement refuses imports".into())); }
    let dev = engine
        .devices()
        .iter()
        .find(|dev| dev.key() == expected_device)
        .ok_or_else(|| Error::NotFound(format!("device {expected_device}; refresh profiles")))?;
    let store = Store::load(&config_dir().join("profiles.json"))?;
    let mut store=bound_import_candidate(store,dev,expected_active)?;
    let incoming = secbird_core::profile_compat::import_document(bytes)?;
    let connected = vec![expected_device.to_string()];
    incoming.store.validate_import_targets(&connected)?;
    let added = store.import_for(incoming.store, &connected);
    secbird_core::profile_compat::validate_store_budget(&store)?;
    store.save()?;
    Ok(json!({"added":added,"warnings":incoming.warnings,"state":state(engine)}))
}

/// Snapshot an export while the caller holds the shared engine lock. File I/O
/// can then finish outside that lock, without delaying tray/device events.
pub(crate) fn export_profile_document(synapse_device: Option<&str>) -> Result<secbird_core::profile_compat::ExportReport, Error> {
    let store = Store::load(&config_dir().join("profiles.json"))?;
    if let Some(device) = synapse_device {
        return secbird_core::profile_compat::export_synapse4(&store, device);
    }
    secbird_core::profile_compat::validate_store_budget(&store)?;
    Ok(secbird_core::profile_compat::ExportReport {
        bytes: store.native_bytes()?,
        warnings: Vec::new(),
    })
}

pub(crate) fn mouse_sensor_snapshot(engine: &mut Engine, id: &str, profile: &str) -> Result<Value, Error> {
    let dev=engine.devices().iter().find(|d|d.key()==id).ok_or_else(||Error::InvalidArg("Selected mouse disconnected".into()))?;
    if !matches!(dev.pid,196|197) { return Err(Error::Unsupported("Sensor controls are specific to DeathAdder V3 HyperSpeed".into())); }
    let mut store=Store::load(&config_dir().join("profiles.json"))?;
    if store.for_device(dev).active!=profile { return Err(Error::InvalidArg("Selected profile changed while reading sensor settings".into())); }
    let calibration=match dev.calibration_raw() {
        Ok(raw) => json!({"available":true,"tracking_distance_mm":raw.tracking_distance_mm(),"raw":raw}),
        Err(error) => json!({"available":false,"error":error.to_string()}),
    };
    let dynamic=match dev.dynamic_raw(false) {
        Ok(raw)=>json!({"available":true,"raw":raw,"intent":store.for_device(dev).active().dynamic_sensitivity}),
        Err(error)=>json!({"available":false,"error":error.to_string()}),
    };
    let firmware=dev.firmware_version().ok();
    let rotation=match engine.rotation_snapshot(id,profile){Ok(value)=>value,Err(error)=>json!({"available":false,"error":error.to_string()})};
    Ok(json!({"device":id,"profile":profile,"calibration":calibration,"dynamic":dynamic,"rotation":rotation,"firmware":firmware}))
}

fn state(engine: &Engine) -> Value {
    let runtime=engine.lighting_runtime_status();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "measurement_read_only": engine.is_measurement_read_only(),
        "query_only_device_handles": engine.is_measurement_read_only() && engine.devices().iter().all(Device::is_query_only),
        "devices": engine.devices().iter().map(|dev|{
            let mut device=device_json(dev);
            device["lighting_runtime"]=runtime.get(&dev.key()).cloned().unwrap_or(Value::Null);
            device
        }).collect::<Vec<_>>(),
        "unsupported": engine.unsupported().iter().map(|u| json!({
            "pid": format!("{:04X}", u.pid), "name": u.name, "reason": u.reason,
        })).collect::<Vec<_>>(),
        "synapse": engine.synapse(),
    })
}

/// The macro id an action text plays (`macro:3@hold` -> 3).
fn macro_of(action: &str) -> Option<u16> {
    action.strip_prefix("macro:")?.split('@').next()?.parse().ok()
}

/// Every macro on the device with its steps as text, and the memory left.
fn macros_json(dev: &Device) -> Result<Value, Error> {
    use secbird_core::macros::Script;
    let storage = dev.macro_storage()?;
    let list: Vec<Value> = dev
        .macros()?
        .into_iter()
        .map(|m| {
            let steps = dev.macro_steps(m.id).map(|s| Script(s).to_string());
            json!({
                "id": m.id, "name": m.name, "size": m.size,
                "steps": steps.as_ref().ok(),
                "error": steps.err().map(|e| e.to_string()),
            })
        })
        .collect();
    Ok(json!({
        "macros": list,
        "free": storage.free_bytes, "max": storage.max_bytes, "recyclable": storage.recyclable_bytes,
    }))
}

fn validate_power_identity(body: &Body) -> Result<(), Error> {
    if body.id.is_empty() || body.expected_device.as_deref() != Some(body.id.as_str()) {
        return Err(Error::InvalidArg(
            "Power actions require id and expected_device to name the same physical device".into(),
        ));
    }
    Ok(())
}

fn critical_threshold_post(dev: &Device, store: &mut Store, body: &Body) -> Result<Value, Error> {
    validate_power_identity(body)?;
    let percent = body
        .critical_threshold_percent
        .ok_or_else(|| Error::InvalidArg("critical_threshold_percent is required".into()))?;
    actions::set_critical_threshold(
        dev,
        store,
        body.expected_device.as_deref().unwrap(),
        percent,
    )?;
    Ok(json!({"critical_threshold":critical_threshold_json(dev)}))
}

fn device_sleep_post(dev: &Device, body: &Body) -> Result<Value, Error> {
    validate_power_identity(body)?;
    actions::set_device_sleep_minutes(dev, body.expected_device.as_deref().unwrap(), body.minutes)?;
    Ok(json!({"device_sleep":device_sleep_json(dev)}))
}

fn critical_threshold_json(dev: &Device) -> Value {
    if !matches!(dev.pid, 196 | 197) {
        return json!({"supported":false,"available":false,"enabled":false,"raw":null,"percent":null,"polling_hz":null,"error":"Low-power threshold is not verified for this model"});
    }
    let raw = dev.critical_threshold_raw();
    let polling = dev.polling_rate();
    let error = match (&raw, &polling) {
        (Err(e), _) => Some(e.to_string()),
        (_, Err(e)) => Some(format!("Cannot determine polling rate: {e}")),
        (_, Ok(hz)) if *hz > 1000 => {
            Some("Low Power Mode is not supported at polling rates of 2000 Hz and above".into())
        }
        _ => None,
    };
    json!({"supported":true,"available":raw.is_ok(),"enabled":raw.is_ok() && polling.as_ref().is_ok_and(|hz|*hz<=1000),
        "raw":raw.as_ref().ok(),"percent":raw.ok().map(secbird_core::power::critical_percent),"polling_hz":polling.ok(),
        "min":5,"max":100,"step":5,"error":error})
}

fn device_sleep_json(dev: &Device) -> Value {
    if !matches!(dev.pid, 196 | 197) {
        return json!({"supported":false,"available":false,"seconds":null,"minutes":null,"minutes_exact":null,"min":1,"max":15,"error":"Device sleep is not verified for this model"});
    }
    match dev.device_sleep_seconds() {
        Ok(seconds) => {
            json!({"supported":true,"available":true,"seconds":seconds,"minutes":seconds.div_ceil(60),"minutes_exact":f64::from(seconds)/60.0,"min":1,"max":15,"error":null})
        }
        Err(e) => {
            json!({"supported":true,"available":false,"seconds":null,"minutes":null,"minutes_exact":null,"min":1,"max":15,"error":e.to_string()})
        }
    }
}

fn sleep_json(dev: &Device) -> Value {
    match dev.sleep_timer() {
        Ok(Some(t)) => json!({ "minutes": t.minutes, "max": t.max_minutes, "auto": t.auto_sleep }),
        _ => Value::Null,
    }
}

fn device_json(dev: &Device) -> Value {
    let spec = dev.spec.as_ref();
    let (profile_names, active) = Engine::profiles(dev);
    let all = Store::load(&config_dir().join("profiles.json")).ok().map(|mut s| s.for_device(dev).clone());
    let profile = all.as_ref().and_then(|p| serde_json::to_value(p.active()).ok()).unwrap_or(Value::Null);
    let lighting_control = all.as_ref().is_some_and(|p| !p.lighting_off);
    let power_settings = all.as_ref().and_then(|p| p.power.as_ref()).and_then(|p| serde_json::to_value(p).ok());
    let (lighting_settings,lighting_readback,lighting_origin)=lighting::snapshot(dev,all.as_ref().map(|profiles|profiles.active()));
    let source_quick_effects=all.as_ref().and_then(|profiles|secbird_core::profile_compat::source_quick_effects(profiles.active())).cloned();
    // Every profile's settings, so the editor can show them side by side.
    let profiles_data = all.and_then(|p| serde_json::to_value(p.profiles).ok()).unwrap_or(Value::Null);
    let dpi_range = spec.and_then(|s| s.dpi.as_ref());
    let polling_options: &[u32] = match spec.and_then(|s| s.polling) {
        Some(secbird_core::db::Polling::High) => &[125, 250, 500, 1000, 2000, 4000, 8000],
        _ => &[125, 500, 1000],
    };
    let kind = match spec.map(|s| s.kind) {
        Some(Kind::Mouse) => "mouse",
        Some(Kind::Keyboard) => "keyboard",
        Some(Kind::Headset) => "headset",
        Some(Kind::Mousepad) => "mousepad",
        Some(Kind::Keypad) => "keypad",
        Some(Kind::Laptop) => "laptop",
        Some(Kind::Accessory) => "accessory",
        None => "unknown",
    };
    json!({
        "id": dev.key(),
        "pid": format!("{:04X}", dev.pid),
        "name": dev.name(),
        "kind": kind,
        "known": spec.is_some(),
        "lighting_control": lighting_control,
        "firmware": dev.firmware_version().ok(),
        // Read once at discovery; asking the device again costs a USB round trip per page load.
        "serial": (!dev.serial.is_empty()).then(|| dev.serial.clone()),
        // Some keyboards acknowledge DPI reads but return zero; only mice have DPI.
        // Keep probing unknown devices, as with polling and DPI stages.
        "dpi": spec
            .is_none_or(|s| s.kind == Kind::Mouse)
            .then(|| dev.dpi().ok())
            .flatten()
            .map(|(x, y)| json!({
                "x": x, "y": y,
                "min": dpi_range.map_or(100, |r| r.min),
                "max": dpi_range.map_or(20000, |r| r.max),
                "step": dpi_range.map_or(50, |r| r.step),
            })),
        // Mousepads and docks answer the polling question too, but have no polling rate to
        // set: only show it where the device list says there is one (or the model is unknown).
        "polling": spec
            .is_none_or(|s| s.polling.is_some())
            .then(|| dev.polling_rate().ok())
            .flatten()
            .map(|hz| json!({ "hz": hz, "options": polling_options })),
        "battery": dev.battery().ok().flatten(),
        "stages": spec
            .is_none_or(|s| s.dpi.is_some())
            .then(|| dev.dpi_stages().ok())
            .flatten()
            .map(|s| json!({ "active": s.active, "list": s.stages, "max": secbird_core::commands::MAX_DPI_STAGES })),
        "sleep": sleep_json(dev),
        "device_sleep": device_sleep_json(dev),
        "critical_threshold": critical_threshold_json(dev),
        "power_settings": power_settings,
        "lighting_settings":lighting_settings,
        "lighting_readback":lighting_readback,
        "lighting_settings_origin":lighting_origin,
        "source_quick_effects":source_quick_effects,
        "dimming": dev.dimming().ok().flatten().map(|d| d.to_string()),
        // Only keyboards that answer the read get the Gaming section.
        "game_mode": dev.game_mode().ok().flatten().map(|g| g.to_string()),
        "macros": dev.macro_storage().is_ok(),
        "matrix": spec.and_then(|s| s.matrix).map(|m| json!({ "rows": m.rows, "cols": m.cols, "first": m.first })),
        "leds": spec.map(|s| s.leds.iter().map(|l| json!({ "name": l.name, "id": l.id })).collect::<Vec<_>>()).unwrap_or_default(),
        "profiles": profile_names,
        "active": active,
        "profile": profile,
        "profilesData": profiles_data,
    })
}

#[cfg(test)]
mod tests {
    use super::base64;

    use super::*;
    use secbird_core::sim::Sim;
    use std::sync::atomic::{AtomicUsize,Ordering};
    static SEQUENCE:AtomicUsize=AtomicUsize::new(0);

    #[test]
    fn audio_discovery_requires_auth_read_only_requests_and_no_engine_lock() {
        let sim=Sim::wireless_mouse();let engine=Engine::with_simulated_mouse(&sim).unwrap();
        let server=Server{port:43210,token:"unit-token".into(),engine:Arc::new(Mutex::new(engine))};
        let mut req=Request{method:"GET".into(),path:"/api/audio/endpoints".into(),query:String::new(),headers:vec![("host".into(),"127.0.0.1:43210".into()),("x-secbird-token".into(),"unit-token".into())],body:vec![]};
        let calls=AtomicUsize::new(0);
        let provider=||{calls.fetch_add(1,Ordering::SeqCst);assert!(server.engine.try_lock().is_ok(),"Audio metadata held the HID engine lock");crate::audio_endpoints::assemble(vec![],[None,None,None])};
        let response=server.route_with_audio(&req,provider);assert_eq!(response.status,"200 OK");assert_eq!(calls.load(Ordering::SeqCst),1);
        let value:Value=serde_json::from_slice(&response.body).unwrap();assert_eq!(value["opens_audio_stream"],false);
        for invalid in 0..4 {
            let mut request=Request{method:req.method.clone(),path:req.path.clone(),query:String::new(),headers:req.headers.clone(),body:vec![]};
            match invalid {0=>request.headers[0].1="evil.example".into(),1=>request.headers[1].1="wrong".into(),2=>request.method="POST".into(),_=>request.body=b"{}".to_vec()}
            let response=server.route_with_audio(&request,provider);assert_ne!(response.status,"200 OK");assert_eq!(calls.load(Ordering::SeqCst),1);
        }
        req.headers.push(("origin".into(),"https://outside.example".into()));
        assert_eq!(server.route_with_audio(&req,provider).status,"403 Forbidden");assert_eq!(calls.load(Ordering::SeqCst),1);
        req.headers.pop();let response=server.route_with_audio(&req,||Err("unavailable".into()));assert_eq!(response.status,"503 Service Unavailable");
    }

    fn power_device(high:bool)->(Sim,Device) {
        let sim=Sim::wireless_mouse();
        sim.state().critical_threshold=Some(78);
        let mut spec=Sim::spec_for(secbird_core::sim::WIRELESS_MOUSE_PID).unwrap();
        if high {spec.polling=Some(secbird_core::db::Polling::High);sim.state().polling_hs=Some(secbird_core::commands::polling_hs_code(2000).unwrap());}
        let dev=sim.open(196,Some(spec)).unwrap();
        (sim,dev)
    }
    #[test]
    fn bound_import_resolves_same_serial_connection_alias_and_legacy_without_hid() {
        for (stored, pid) in [
            ("00C4:unit", 0x00c5),
            ("00C5:unit", 0x00c4),
            ("00C5", 0x00c5),
            ("025D", 0x025d),
            ("00C5:unit", 0x00c5),
        ] {
            let sim = Sim::mouse();
            let mut dev = sim.open(pid, None).unwrap();
            dev.serial = "unit".into();
            sim.state().log.clear();
            let mut store = Store::default();
            let mut profiles = secbird_core::profile::DeviceProfiles::default();
            profiles.create("Work").unwrap();
            profiles.switch("Work").unwrap();
            store.devices.insert(stored.into(), profiles);
            let candidate = bound_import_candidate(store, &dev, "Work").unwrap();
            assert_eq!(candidate.devices[&dev.key()].active, "Work");
            assert!(candidate.devices[&dev.key()].profiles.contains_key("Work"));
            if stored != dev.key() {
                assert!(!candidate.devices.contains_key(stored));
            }
            assert!(sim.state().log.is_empty());
        }
    }
    #[test]
    fn bound_import_refuses_different_serial_and_stale_active_without_disk_migration() {
        let sim = Sim::mouse();
        let mut dev = sim.open(0x00c5, None).unwrap();
        dev.serial = "unit".into();
        sim.state().log.clear();
        for (stored, expected) in [
            ("00C4:other", "Work"),
            ("00C4:unit", "Stale"),
            ("00C5", "Stale"),
        ] {
            let folder = std::env::temp_dir().join(format!(
                "bound-import-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let path = folder.join("profiles.json");
            let mut store = Store::load(&path).unwrap();
            let mut profiles = secbird_core::profile::DeviceProfiles::default();
            profiles.create("Work").unwrap();
            profiles.switch("Work").unwrap();
            store.devices.insert(stored.into(), profiles);
            store.save().unwrap();
            let before = std::fs::read(&path).unwrap();
            assert!(bound_import_candidate(Store::load(&path).unwrap(), &dev, expected).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert!(sim.state().log.is_empty());
            std::fs::remove_file(path).unwrap();
            std::fs::remove_dir(folder).unwrap();
        }
    }
    #[test]
    fn power_request_types_identity_and_snapshot_errors_are_explicit() {
        assert!(serde_json::from_value::<Body>(json!({"critical_threshold_percent":"30"})).is_err());
        assert!(serde_json::from_value::<Body>(json!({"critical_threshold_percent":30.5})).is_err());
        let (sim,dev)=power_device(false);
        let before=sim.state().log.len();
        for value in [json!({}),json!({"id":dev.key()}),json!({"id":dev.key(),"expected_device":"other"})] {
            let b:Body=serde_json::from_value(value).unwrap();
            assert!(validate_power_identity(&b).is_err());
        }
        assert_eq!(sim.state().log.len(),before);
        let snapshot=critical_threshold_json(&dev);
        assert_eq!(snapshot["available"],true);
        assert_eq!(snapshot["enabled"],true);
        assert_eq!(snapshot["raw"],78);
        assert_eq!(snapshot["percent"],30);
        assert_eq!(snapshot["error"],Value::Null);
        let (_,dev)=power_device(true);
        let snapshot=critical_threshold_json(&dev);
        assert_eq!(snapshot["available"],true);
        assert_eq!(snapshot["enabled"],false);
        assert!(snapshot["error"].as_str().unwrap().contains("2000 Hz"));
        let sim=Sim::mouse();
        let unsupported=sim.open(605,None).unwrap();
        let before=sim.state().log.len();
        assert_eq!(critical_threshold_json(&unsupported)["supported"],false);
        assert_eq!(device_sleep_json(&unsupported)["supported"],false);
        assert_eq!(sim.state().log.len(),before);
        let missing=sim.open(196,None).unwrap();
        assert_eq!(critical_threshold_json(&missing)["available"],false);
        assert!(critical_threshold_json(&missing)["error"].is_string());
    }
    #[test]
    fn source_power_handlers_persist_device_threshold_and_keep_sleep_out_of_profiles() {
        let (sim,dev)=power_device(false);
        let folder=std::env::temp_dir().join(format!("synapse-power-server-{}-{}",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed)));
        let mut store=Store::load(&folder.join("profiles.json")).unwrap();
        let key=dev.key();
        let b:Body=serde_json::from_value(json!({"id":key,"expected_device":key})).unwrap();
        let before=sim.state().log.len();
        assert!(critical_threshold_post(&dev,&mut store,&b).is_err());
        assert_eq!(sim.state().log.len(),before);
        let b:Body=serde_json::from_value(json!({"id":key,"expected_device":key,"critical_threshold_percent":45})).unwrap();
        let response=critical_threshold_post(&dev,&mut store,&b).unwrap();
        assert_eq!(response["critical_threshold"]["percent"],45);
        assert_eq!(store.for_device(&dev).power.as_ref().unwrap().critical_threshold_percent,Some(45));
        assert!(store.for_device(&dev).active().extra.is_empty());
        let disk_before=std::fs::read(folder.join("profiles.json")).unwrap();
        let b:Body=serde_json::from_value(json!({"id":key,"expected_device":key,"minutes":4})).unwrap();
        let response=device_sleep_post(&dev,&b).unwrap();
        assert_eq!(response["device_sleep"]["seconds"],240);
        assert_eq!(response["device_sleep"]["minutes"],4);
        assert!(store.for_device(&dev).active().sleep_minutes.is_none());
        assert_eq!(std::fs::read(folder.join("profiles.json")).unwrap(),disk_before);
        sim.state().idle_seconds=Some(301);
        let snapshot=device_sleep_json(&dev);
        assert_eq!(snapshot["seconds"],301);
        assert_eq!(snapshot["minutes"],6);
        assert!(snapshot["minutes_exact"].as_f64().unwrap()>5.0);
        std::fs::remove_file(folder.join("profiles.json")).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
