use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::Instant,
};
static EVENTS: OnceLock<Mutex<VecDeque<&'static str>>> = OnceLock::new();
static DURATIONS: OnceLock<Mutex<VecDeque<u64>>> = OnceLock::new();
pub struct ApplyTimer(Instant);
pub fn measure_apply() -> ApplyTimer {
    ApplyTimer(Instant::now())
}
impl Drop for ApplyTimer {
    fn drop(&mut self) {
        let mut v = DURATIONS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if v.len() == 32 {
            v.pop_front();
        }
        v.push_back(self.0.elapsed().as_millis().min(u64::MAX as u128) as u64);
    }
}
static START: OnceLock<Instant> = OnceLock::new();
pub fn start() {
    START.get_or_init(Instant::now);
}
pub fn observe_log(message: &str) {
    let bytes = &message.as_bytes()[..message.len().min(4096)];
    let has = |word: &[u8]| {
        bytes
            .windows(word.len())
            .any(|s| s.eq_ignore_ascii_case(word))
    };
    let category = if has(b"error") || has(b"fail") {
        "runtime_error"
    } else if has(b"reject") {
        "operation_rejected"
    } else {
        return;
    };
    let mut events = EVENTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if events.len() == 16 {
        events.pop_front();
    }
    events.push_back(category);
}
pub fn preview(include_errors: bool) -> Value {
    let events = EVENTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    json!({"schema":1,"application":"Synapse Lite","version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
 "coverage":{"native_editor":"implemented","profile_storage":"implemented","hardware_verification":"not established by this report","catalogue_presence_is_hardware_support":false},
 "privacy":{"profile_bodies":false,"serials":false,"user_paths":false,"raw_logs":false,"network_upload":false},
 "performance":{"uptime_seconds":START.get().map(|t|t.elapsed().as_secs()),"memory_bytes":null,"recent_profile_apply_ms":DURATIONS.get_or_init(Default::default).lock().unwrap_or_else(|p|p.into_inner()).clone()},
 "recent_error_categories":if include_errors{events.iter().copied().collect::<Vec<_>>()}else{vec![]},
 "limitations":"Session-only log categories, at most 16. No memory or hardware probe performed."})
}
pub fn save_new(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_and_private() {
        for _ in 0..100 {
            observe_log("error C:/Users/secret serial123 profile-body");
        }
        let v = preview(true);
        let t = v.to_string();
        assert!(!t.contains("secret"));
        assert!(!t.contains("serial123"));
        assert!(!t.contains("profile-body"));
        assert_eq!(v["recent_error_categories"].as_array().unwrap().len(), 16);
        assert_eq!(preview(false)["recent_error_categories"], json!([]));
    }
    #[test]
    fn performance_samples_are_bounded() {
        for _ in 0..100 {
            drop(measure_apply());
        }
        let v = preview(false);
        let samples = v["performance"]["recent_profile_apply_ms"]
            .as_array()
            .unwrap();
        assert_eq!(samples.len(), 32);
        assert!(samples.iter().all(|v| v.as_u64().is_some()));
    }
    #[test]
    fn exclusive_output() {
        let p = std::env::temp_dir().join(format!(
            "support-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        save_new(&p, b"first").unwrap();
        assert!(save_new(&p, b"second").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"first");
        std::fs::remove_file(p).unwrap();
    }
}
