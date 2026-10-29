//! Explicit read-only Core Audio discovery. No streams, callbacks or setters.
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::OpenOptions,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

#[path = "audio_endpoints/com.rs"]
mod com;

pub(super) const MAX_ENDPOINTS: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    Console,
    Multimedia,
    Communications,
}
pub(super) const ROLES: [Role; 3] = [Role::Console, Role::Multimedia, Role::Communications];
#[derive(Debug, Serialize)]
pub(crate) struct Endpoint {
    pub id: String,
    pub name: Option<String>,
    pub state: u32,
    pub default_roles: Vec<Role>,
    pub volume_scalar: Option<f32>,
    pub muted: Option<bool>,
    pub issues: Vec<String>,
}
#[derive(Debug, Serialize)]
pub(crate) struct Snapshot {
    pub provider: &'static str,
    pub flow: &'static str,
    pub opens_audio_stream: bool,
    pub endpoints: Vec<Endpoint>,
    pub warnings: Vec<String>,
}
pub(super) fn assemble(
    mut endpoints: Vec<Endpoint>,
    defaults: [Option<String>; 3],
) -> Result<Snapshot, String> {
    if endpoints.len() > MAX_ENDPOINTS {
        return Err("Audio endpoint count exceeds64".into());
    }
    let mut ids = BTreeSet::new();
    for endpoint in &mut endpoints {
        if endpoint.id.is_empty() || !ids.insert(endpoint.id.clone()) {
            return Err("Audio endpoint identity is empty or duplicated".into());
        }
        endpoint.default_roles.clear();
        for (index, role) in ROLES.into_iter().enumerate() {
            if defaults[index].as_deref() == Some(endpoint.id.as_str()) {
                endpoint.default_roles.push(role);
            }
        }
    }
    let mut warnings = vec![];
    if defaults.iter().flatten().any(|id| !ids.contains(id)) {
        warnings.push(
            "Default endpoint changed or disappeared during discovery; refresh before selecting it"
                .into(),
        );
    }
    Ok(Snapshot {
        provider: "windows_core_audio",
        flow: "render",
        opens_audio_stream: false,
        endpoints,
        warnings,
    })
}
pub(crate) fn discover() -> Result<Snapshot, String> {
    static BUSY: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    run_discovery(
        BUSY.get_or_init(|| Arc::new(AtomicBool::new(false)))
            .clone(),
        Duration::from_millis(2500),
        com::discover,
    )
}
struct Admission(Arc<AtomicBool>);
impl Drop for Admission {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn run_discovery(
    busy: Arc<AtomicBool>,
    timeout: Duration,
    work: impl FnOnce() -> Result<Snapshot, String> + Send + 'static,
) -> Result<Snapshot, String> {
    if busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Audio discovery is still running; retry after it finishes".into());
    }
    let admission = Admission(busy);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    // Own apartment lifetime prevents changing the caller's threading model.
    // A timed-out OS call keeps its admission until it actually returns. Retry
    // cannot create another worker, and there is no native-call cancellation.
    std::thread::Builder::new()
        .name("audio-endpoints".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            let result = work();
            drop(admission);
            let _ = sender.send(result);
        })
        .map_err(|e| e.to_string())?;
    receiver
        .recv_timeout(timeout)
        .map_err(|error| match error {
            std::sync::mpsc::RecvTimeoutError::Timeout => {
                "Audio discovery timed out; the outstanding OS call is still draining".to_owned()
            }
            _ => "Audio discovery worker ended without a result".to_owned(),
        })?
}
pub(crate) fn diagnostic(args: &[String]) -> Result<(), String> {
    if args.len() != 2
        || args[0] != "--audio-endpoints"
        || args[1].is_empty()
        || args[1].starts_with("--")
    {
        return Err("Use --audio-endpoints NEW_REPORT.json alone".into());
    }
    // Reserve a new file first; an existing report must never be overwritten.
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
        .map_err(|e| e.to_string())?;
    let result = discover();
    let report = match &result {
        Ok(snapshot) => serde_json::to_value(snapshot).map_err(|e| e.to_string())?,
        Err(error) => serde_json::json!({"error":error,"opens_audio_stream":false}),
    };
    output
        .write_all(&serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn endpoint(id: &str) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: None,
            state: 1,
            default_roles: vec![],
            volume_scalar: None,
            muted: None,
            issues: vec![],
        }
    }
    #[test]
    fn roles_use_exact_identity_and_never_fall_back_to_first_endpoint() {
        let snapshot = assemble(
            vec![endpoint("a"), endpoint("b")],
            [Some("b".into()), Some("gone".into()), None],
        )
        .unwrap();
        assert!(snapshot.endpoints[0].default_roles.is_empty());
        assert_eq!(snapshot.endpoints[1].default_roles, vec![Role::Console]);
        assert_eq!(snapshot.warnings.len(), 1);
        assert!(!snapshot.opens_audio_stream);
        assert!(assemble(vec![endpoint("a"), endpoint("a")], [None, None, None]).is_err());
        assert!(assemble(vec![endpoint("")], [None, None, None]).is_err());
        assert!(assemble(
            (0..65).map(|n| endpoint(&n.to_string())).collect(),
            [None, None, None]
        )
        .is_err());
        assert!(assemble(vec![], [None, None, None])
            .unwrap()
            .endpoints
            .is_empty());
    }
    #[test]
    fn malformed_diagnostic_does_not_create_a_report() {
        for args in [
            vec![],
            vec!["--audio-endpoints".into()],
            vec!["--audio-endpoints".into(), "x".into(), "extra".into()],
        ] {
            assert!(diagnostic(&args).is_err());
        }
    }
    #[test]
    fn timeout_retains_single_worker_admission_until_actual_completion() {
        let busy = Arc::new(AtomicBool::new(false));
        let (release, wait) = std::sync::mpsc::channel();
        assert!(
            run_discovery(busy.clone(), Duration::from_millis(1), move || {
                wait.recv().unwrap();
                assemble(vec![], [None, None, None])
            })
            .unwrap_err()
            .contains("timed out")
        );
        assert!(busy.load(Ordering::Acquire));
        assert!(
            run_discovery(busy.clone(), Duration::from_secs(1), || panic!(
                "overlapping worker started"
            ))
            .unwrap_err()
            .contains("still running")
        );
        release.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while busy.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!busy.load(Ordering::Acquire));
        assert!(
            run_discovery(busy.clone(), Duration::from_secs(1), || assemble(
                vec![],
                [None, None, None]
            ))
            .is_ok()
        );
    }
    #[test]
    #[ignore = "reads real Windows playback endpoint metadata; no capture or writes"]
    fn live_discovery_repeats_on_owned_apartments() {
        for _ in 0..20 {
            let snapshot = discover().unwrap();
            assert_eq!(snapshot.flow, "render");
            assert!(!snapshot.opens_audio_stream);
            for endpoint in &snapshot.endpoints {
                assert!(!endpoint.id.is_empty());
                assert!(endpoint.state > 0);
                assert!(endpoint
                    .volume_scalar
                    .is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)));
            }
        }
    }
}
