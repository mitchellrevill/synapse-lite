//! Ornata host brightness policies. Decisions are pure; the bridge writes only live
//! brightness and never changes firmware effects, saved brightness or sleep timers.
use secbird_core::{Device, Profile};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Owner {
    Clear,
    Competing(Vec<String>),
    Unknown(String),
}
impl Owner {
    fn blocked_reason(&self) -> Option<String> {
        match self {
            Self::Clear => None,
            Self::Competing(names) => {
                Some(format!("competing-synapse-owner: {}", names.join(", ")))
            }
            Self::Unknown(error) => Some(format!("owner-check-unavailable: {error}")),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Environment {
    pub now_ms: u64,
    pub idle_ms: Result<u64, String>,
    pub display_off: Option<bool>,
    pub owner: Owner,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            now_ms: 0,
            idle_ms: Err("idle has not been sampled".into()),
            display_off: None,
            owner: Owner::Unknown("processes have not been sampled".into()),
        }
    }
}

/// LASTINPUTINFO contains a wrapping 32-bit tick, not a GetTickCount64 value.
/// Once the raw tick stays unchanged, accumulate elapsed time with the 64-bit clock.
/// A newly reported tick apparently in the future is treated as fresh input, avoiding
/// a non-monotonic/SendInput timestamp becoming a false 49-day idle interval.
#[derive(Default)]
pub(crate) struct IdleClock {
    previous_raw: Option<u32>,
    anchor_ms: u64,
    previous_now: u64,
}
impl IdleClock {
    pub fn sample(&mut self, now_ms: u64, raw: u32) -> Result<u64, String> {
        if self.previous_raw.is_some() && now_ms < self.previous_now {
            self.previous_raw = None;
            return Err("uptime moved backwards".into());
        }
        if self.previous_raw != Some(raw) {
            let age = (now_ms as u32).wrapping_sub(raw);
            let age = if age <= i32::MAX as u32 {
                u64::from(age)
            } else {
                0
            };
            self.anchor_ms = now_ms.saturating_sub(age);
            self.previous_raw = Some(raw);
        }
        self.previous_now = now_ms;
        Ok(now_ms.saturating_sub(self.anchor_ms))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Config {
    pub profile: String,
    pub owned: bool,
    pub source_known: bool,
    pub enabled: bool,
    pub value: Option<u8>,
    pub idle_enabled: bool,
    pub idle_minutes: u16,
    pub display_enabled: bool,
    pub away_percent: Option<u8>,
    pub error: Option<String>,
}
impl Config {
    pub fn from_profile(
        profile_name: &str,
        profile: &Profile,
        owned: bool,
        away: Option<u8>,
    ) -> Self {
        let source = profile.lighting_settings.as_ref();
        let brightness = source.and_then(|s| s.brightness.as_ref());
        let policy = source.and_then(|s| s.switch_off.as_ref());
        let legacy = profile.lighting.get("backlight").and_then(|z| z.brightness);
        Self {
            profile: profile_name.into(),
            owned,
            source_known: brightness.is_some(),
            enabled: brightness.map_or(legacy.is_some_and(|v| v > 0), |v| v.enabled),
            value: brightness.map(|v| v.value).or(legacy),
            idle_enabled: policy.is_some_and(|p| p.idle_enabled),
            idle_minutes: policy.map_or(1, |p| p.idle_minutes),
            display_enabled: policy.is_some_and(|p| p.display_off),
            away_percent: away.map(|p| p.min(100)),
            error: source
                .and_then(|s| s.validate().err())
                .map(|e| e.to_string()),
        }
    }
    pub fn baseline(&self) -> Option<u8> {
        self.value.map(|value| if self.enabled { value } else { 0 })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub available: bool,
    pub can_execute: bool,
    pub suppressed: bool,
    pub target: Option<u8>,
    pub blocked: Option<String>,
}
pub(crate) fn plan(config: &Config, env: &Environment, running: bool, stopping: bool) -> Plan {
    let blocked = if !running {
        Some("host-runtime-not-running".into())
    } else if !config.owned {
        Some("lighting-ownership-disabled".into())
    } else if let Some(reason) = env.owner.blocked_reason() {
        Some(reason)
    } else if let Some(error) = &config.error {
        Some(format!("invalid-lighting-intent: {error}"))
    } else if !stopping
        && config.source_known
        && config.enabled
        && config.display_enabled
        && env.display_off.is_none()
    {
        Some("display-state-not-yet-known".into())
    } else if !stopping
        && config.source_known
        && config.enabled
        && config.idle_enabled
        && env.idle_ms.is_err()
    {
        Some(format!(
            "idle-input-unavailable: {}",
            env.idle_ms.as_ref().unwrap_err()
        ))
    } else {
        None
    };
    if blocked.is_some() {
        return Plan {
            available: false,
            can_execute: false,
            suppressed: false,
            target: None,
            blocked,
        };
    }
    if !config.source_known || stopping {
        let scale = if stopping {
            100
        } else {
            config.away_percent.unwrap_or(100)
        };
        return Plan {
            available: config.source_known,
            can_execute: false,
            suppressed: !stopping && scale < 100 && config.baseline() != Some(0),
            target: config
                .baseline()
                .map(|value| (u16::from(value) * u16::from(scale) / 100) as u8),
            blocked: None,
        };
    }
    let display = config.enabled && config.display_enabled && env.display_off == Some(true);
    let idle = config.enabled
        && config.idle_enabled
        && env
            .idle_ms
            .as_ref()
            .is_ok_and(|ms| *ms >= u64::from(config.idle_minutes) * 60_000);
    let baseline = config.baseline();
    let target = baseline.map(|value| {
        if display || idle {
            0
        } else {
            (u16::from(value) * u16::from(config.away_percent.unwrap_or(100)) / 100) as u8
        }
    });
    let suppressed = display || idle || matches!((target, baseline), (Some(a),Some(b)) if a < b);
    Plan {
        available: true,
        can_execute: true,
        suppressed,
        target,
        blocked: None,
    }
}

pub(crate) trait BrightnessIo {
    fn read_saved(&mut self) -> Result<u8, String>;
    fn write_live(&mut self, percent: u8) -> Result<(), String>;
}
pub(crate) struct DeviceIo<'a>(pub &'a Device);
impl BrightnessIo for DeviceIo<'_> {
    fn read_saved(&mut self) -> Result<u8, String> {
        if self.0.pid != 605 || self.0.interface != 2 {
            return Err("runtime bridge is verified only for Ornata V2/interface2".into());
        }
        self.0.brightness(5).map_err(|e| e.to_string())
    }
    fn write_live(&mut self, percent: u8) -> Result<(), String> {
        if self.0.pid != 605 || self.0.interface != 2 {
            return Err("runtime bridge is verified only for Ornata V2/interface2".into());
        }
        self.0
            .set_brightness_live(5, percent)
            .map_err(|e| e.to_string())
    }
}

#[derive(Default)]
struct Session {
    config: Option<Config>,
    applied: Option<u8>,
    modified: bool,
    read_ready: bool,
    saved_baseline: Option<u8>,
    dirty: bool,
    error: Option<String>,
    retry_at: u64,
    failures: u32,
    status: Option<Value>,
}
impl Session {
    fn failure(&mut self, now: u64, error: String) {
        self.error = Some(error);
        let waits = [1_000, 3_000, 10_000, 30_000];
        self.retry_at = now.saturating_add(waits[(self.failures as usize).min(waits.len() - 1)]);
        self.failures = self.failures.saturating_add(1);
    }
    fn reconcile(
        &mut self,
        config: &Config,
        env: &Environment,
        running: bool,
        stopping: bool,
        io: &mut impl BrightnessIo,
    ) {
        let desired = plan(config, env, running, stopping);
        if self.config.as_ref() != Some(config) {
            self.dirty = true;
            self.read_ready = false;
            self.retry_at = 0;
        }
        self.config = Some(config.clone());
        if desired.blocked.is_some() {
            self.dirty = true;
            self.read_ready = false;
        } else if (!config.source_known || !desired.suppressed) && !self.modified {
            // Merely observing a normal profile never imposes a live value.
            self.error = None;
            self.failures = 0;
        } else if stopping || env.now_ms >= self.retry_at {
            let scale = if stopping {
                100
            } else {
                config.away_percent.unwrap_or(100)
            };
            let mut target = desired.target.or_else(|| {
                self.saved_baseline
                    .map(|saved| (u16::from(saved) * u16::from(scale) / 100) as u8)
            });
            let need_write =
                self.dirty || self.applied != target || (!desired.suppressed && self.modified);
            if need_write {
                if !self.read_ready || target.is_none() {
                    match io.read_saved() {
                        Ok(saved) => {
                            self.read_ready = true;
                            self.saved_baseline = Some(saved);
                            if desired.target.is_none() {
                                target = Some((u16::from(saved) * u16::from(scale) / 100) as u8);
                            }
                        }
                        Err(error) => {
                            self.failure(env.now_ms, error);
                            self.publish(config, env, &desired);
                            return;
                        }
                    }
                }
                if let Some(target) = target {
                    match io.write_live(target) {
                        Ok(()) => {
                            self.applied = Some(target);
                            self.modified = desired.suppressed;
                            self.dirty = false;
                            self.error = None;
                            self.failures = 0;
                            self.retry_at = 0;
                        }
                        Err(error) => self.failure(env.now_ms, error),
                    }
                }
            }
        }
        self.publish(config, env, &desired);
    }
    fn publish(&mut self, config: &Config, env: &Environment, desired: &Plan) {
        self.status = Some(json!({
            "available": desired.available && self.error.is_none(),
            "can_execute_policies": desired.can_execute && self.error.is_none(),
            "blocked_reason": desired.blocked.clone().or_else(|| (!config.source_known).then(|| "no-retained-source-brightness".into())),
            "suppressed": desired.suppressed,
            "error":self.error,
            "profile":config.profile,
            "configured_percent":config.baseline(),
            "desired_live_percent":desired.target,
            "acknowledged_live_percent":self.applied,
            "restoration_pending":self.modified,
            "retry_at_ms":(self.error.is_some()).then_some(self.retry_at),
            "idle_ms":env.idle_ms.as_ref().ok(),
            "display_off":env.display_off,
            "global_away_percent":config.away_percent,
            "limitations":["Known Synapse process names are a conservative gate, not proof of exclusive ownership.","WDL, macro-recorder, OTFM and third-party animation ownership are not established."]
        }));
    }
}

pub(crate) struct Runtime {
    sessions: BTreeMap<String, Session>,
    pub running: bool,
    pub stopping: bool,
    pub environment: Environment,
    idle_clock: IdleClock,
    #[cfg(test)]
    pub test_environment: Option<Environment>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            sessions: BTreeMap::new(),
            running: false,
            stopping: false,
            environment: Environment::default(),
            idle_clock: IdleClock::default(),
            #[cfg(test)]
            test_environment: None,
        }
    }
}
impl Runtime {
    pub fn start(&mut self) {
        self.running = true;
        self.stopping = false;
    }
    pub fn finish_stop(&mut self) {
        self.running = false;
        for session in self.sessions.values_mut() {
            if let Some(status) = &mut session.status {
                status["available"] = json!(false);
                status["can_execute_policies"] = json!(false);
                status["blocked_reason"] = json!("host-runtime-stopped");
            }
        }
    }
    pub fn sample_host(&mut self) {
        #[cfg(test)]
        if let Some(environment) = &self.test_environment {
            self.environment = environment.clone();
            return;
        }
        if !self.running {
            return;
        }
        let (now, raw) = native_idle();
        self.environment.now_ms = now;
        self.environment.idle_ms = raw.and_then(|raw| self.idle_clock.sample(now, raw));
        self.environment.owner = native_owners().map_or_else(Owner::Unknown, |names| {
            if names.is_empty() {
                Owner::Clear
            } else {
                Owner::Competing(names)
            }
        });
    }
    pub fn invalidate(&mut self, keys: &[String]) {
        for (key, session) in &mut self.sessions {
            if keys.contains(key) {
                session.dirty = true;
                session.read_ready = false;
            }
        }
    }
    pub fn invalidate_all(&mut self) {
        for session in self.sessions.values_mut() {
            session.dirty = true;
            session.read_ready = false;
        }
    }
    pub fn reconcile(&mut self, key: &str, config: &Config, io: &mut impl BrightnessIo) {
        self.sessions.entry(key.into()).or_default().reconcile(
            config,
            &self.environment,
            self.running,
            self.stopping,
            io,
        );
    }
    pub fn retain(&mut self, keys: &HashSet<String>) {
        self.sessions.retain(|key, session| {
            if keys.contains(key) {
                return true;
            }
            if !session.modified {
                return false;
            }
            // A missing HID handle is not proof that the physical device reset.
            // Keep a pending lease so reconnect can restore the latest own value.
            session.applied = None;
            session.dirty = true;
            session.read_ready = false;
            if let Some(status) = &mut session.status {
                status["available"] = json!(false);
                status["can_execute_policies"] = json!(false);
                status["blocked_reason"] = json!("device-disconnected");
            }
            true
        });
    }
    pub fn manages(&self, key: &str, config: &Config) -> bool {
        self.running && (config.source_known || self.sessions.get(key).is_some_and(|s| s.modified))
    }
    pub fn blocks_frames(&self, key: &str) -> bool {
        self.sessions.get(key).is_some_and(|s| {
            (s.modified || s.config.as_ref().is_some_and(|c| c.source_known))
                && s.status.as_ref().is_some_and(|v| {
                    v["suppressed"] == true
                        || v["blocked_reason"].is_string()
                        || v["error"].is_string()
                })
        })
    }
    pub fn owner_clear(&self) -> bool {
        self.environment.owner == Owner::Clear
    }
    pub fn can_execute(&self, config: &Config) -> bool {
        plan(config, &self.environment, self.running, self.stopping).can_execute
    }
    pub fn should_suppress(&self, config: &Config) -> bool {
        let decision = plan(config, &self.environment, self.running, self.stopping);
        decision.suppressed || decision.blocked.is_some()
    }
    pub fn status(&self) -> Value {
        Value::Object(
            self.sessions
                .iter()
                .map(|(key, s)| (key.clone(), s.status.clone().unwrap_or(Value::Null)))
                .collect(),
        )
    }
    pub fn store_error(&mut self, error: &str) {
        for session in self.sessions.values_mut() {
            session.error = Some(error.into());
            session.dirty = true;
            if let Some(status) = &mut session.status {
                status["error"] = json!(error);
                status["available"] = json!(false);
                status["can_execute_policies"] = json!(false);
                status["blocked_reason"] = json!("profile-store-unavailable");
            }
        }
    }
    pub fn has_pending_restore(&self) -> bool {
        self.sessions.values().any(|s| s.modified)
    }
}

pub(crate) fn competing_process_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "razerappengine.exe" || name == "rzsynapse.exe" || name.starts_with("razer synapse")
}
fn native_owners() -> Result<Vec<String>, String> {
    use windows_sys::Win32::{Foundation::*, System::Diagnostics::ToolHelp::*};
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(format!("process snapshot failed: {}", GetLastError()));
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut names = Vec::new();
        let mut ok = Process32FirstW(snapshot, &mut entry);
        if ok == 0 {
            let code = GetLastError();
            CloseHandle(snapshot);
            return Err(format!("process enumeration failed: {code}"));
        }
        while ok != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if competing_process_name(&name) {
                names.push(name);
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        let code = GetLastError();
        CloseHandle(snapshot);
        if code != ERROR_NO_MORE_FILES {
            return Err(format!("process enumeration incomplete: {code}"));
        }
        names.sort();
        names.dedup();
        Ok(names)
    }
}
fn native_idle() -> (u64, Result<u32, String>) {
    use windows_sys::Win32::{
        System::SystemInformation::GetTickCount64,
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    };
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let ok = GetLastInputInfo(&mut info);
        let error = if ok == 0 {
            Some(windows_sys::Win32::Foundation::GetLastError())
        } else {
            None
        };
        let now = GetTickCount64();
        (
            now,
            match error {
                None => Ok(info.dwTime),
                Some(code) => Err(format!("GetLastInputInfo failed: {code}")),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct FakeIo {
        saved: u8,
        live: u8,
        reads: usize,
        writes: Vec<u8>,
        fail_read: usize,
        fail_write: usize,
    }
    impl BrightnessIo for FakeIo {
        fn read_saved(&mut self) -> Result<u8, String> {
            self.reads += 1;
            if self.fail_read > 0 {
                self.fail_read -= 1;
                Err("transient saved read".into())
            } else {
                Ok(self.saved)
            }
        }
        fn write_live(&mut self, value: u8) -> Result<(), String> {
            if self.fail_write > 0 {
                self.fail_write -= 1;
                return Err("transient live write".into());
            }
            self.live = value;
            self.writes.push(value);
            Ok(())
        }
    }
    fn config() -> Config {
        Config {
            profile: "Default".into(),
            owned: true,
            source_known: true,
            enabled: true,
            value: Some(73),
            idle_enabled: true,
            idle_minutes: 1,
            display_enabled: true,
            away_percent: None,
            error: None,
        }
    }
    fn environment() -> Environment {
        Environment {
            now_ms: 60_000,
            idle_ms: Ok(0),
            display_off: Some(false),
            owner: Owner::Clear,
        }
    }
    fn runtime() -> Runtime {
        let mut r = Runtime::default();
        r.start();
        r.environment = environment();
        r
    }

    #[test]
    fn pure_policy_composes_display_idle_global_away_and_user_disabled_state() {
        let mut cfg = config();
        let mut env = environment();
        assert_eq!(plan(&cfg, &env, true, false).target, Some(73));
        cfg.away_percent = Some(50);
        assert_eq!(plan(&cfg, &env, true, false).target, Some(36));
        env.idle_ms = Ok(60_000);
        assert_eq!(plan(&cfg, &env, true, false).target, Some(0));
        cfg.idle_enabled = false;
        assert_eq!(plan(&cfg, &env, true, false).target, Some(36));
        env.display_off = Some(true);
        assert_eq!(plan(&cfg, &env, true, false).target, Some(0));
        cfg.enabled = false;
        assert_eq!(plan(&cfg, &env, true, false).target, Some(0));
        assert!(!plan(&cfg, &env, true, false).suppressed);
        cfg.enabled = true;
        env.display_off = None;
        assert!(plan(&cfg, &env, true, false).blocked.is_some());
        cfg.display_enabled = false;
        assert!(plan(&cfg, &env, true, false).blocked.is_none());
        cfg.idle_enabled = true;
        env.idle_ms = Err("input unavailable".into());
        assert!(plan(&cfg, &env, true, false).blocked.is_some());
        assert_eq!(
            plan(&cfg, &env, true, true).target,
            Some(73),
            "shutdown releases policies and global dimming"
        );
    }
    #[test]
    fn repeated_ticks_are_idempotent_and_wake_uses_latest_profile_brightness() {
        let mut r = runtime();
        let mut cfg = config();
        let mut io = FakeIo {
            saved: 73,
            live: 73,
            ..Default::default()
        };
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.reads, 0);
        assert!(io.writes.is_empty());
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.writes, vec![0]);
        for i in 1..10 {
            r.environment.now_ms += 3_000;
            r.environment.idle_ms = Ok(60_000 + i * 3_000);
            r.reconcile("keyboard", &cfg, &mut io);
        }
        assert_eq!(io.reads, 1);
        assert_eq!(io.writes, vec![0]);
        cfg.value = Some(41);
        cfg.profile = "Other".into();
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 0);
        r.environment.idle_ms = Ok(0);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 41);
        assert_eq!(io.saved, 73);
        let count = io.writes.len();
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.writes.len(), count);
    }
    #[test]
    fn source_wake_never_brightens_above_global_dim_and_shutdown_restores_baseline() {
        let mut r = runtime();
        let mut cfg = config();
        cfg.away_percent = Some(50);
        let mut io = FakeIo {
            saved: 73,
            ..Default::default()
        };
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 36);
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 0);
        r.environment.idle_ms = Ok(0);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 36);
        r.stopping = true;
        r.environment.display_off = Some(true);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 73);
        assert!(!r.has_pending_restore());
    }
    #[test]
    fn losing_source_settings_releases_our_lease_without_overriding_global_dim() {
        let mut r = runtime();
        let mut cfg = config();
        let mut io = FakeIo {
            saved: 73,
            ..Default::default()
        };
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        cfg.source_known = false;
        cfg.value = Some(35);
        cfg.idle_enabled = false;
        cfg.display_enabled = false;
        cfg.away_percent = Some(50);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 17);
        cfg.value = None;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(
            io.live, 36,
            "missing configured value falls back to saved hardware under the same global cap"
        );
        let count = io.writes.len();
        let reads = io.reads;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.writes.len(), count);
        assert_eq!(io.reads, reads);
        cfg.value = Some(35);
        cfg.away_percent = None;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 35);
        assert!(!r.has_pending_restore());
        let writes = io.writes.len();
        cfg.away_percent = Some(50);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(
            io.writes.len(),
            writes,
            "legacy profile without an existing lease stays with its legacy global-away path"
        );
    }

    #[test]
    fn owner_unknown_competing_or_revoked_causes_no_write_and_preserves_pending_restore() {
        let mut r = runtime();
        let mut cfg = config();
        let mut io = FakeIo {
            saved: 73,
            ..Default::default()
        };
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        let count = io.writes.len();
        for owner in [
            Owner::Competing(vec!["RazerAppEngine.exe".into()]),
            Owner::Unknown("denied".into()),
        ] {
            r.environment.owner = owner;
            r.reconcile("keyboard", &cfg, &mut io);
            assert_eq!(io.writes.len(), count);
            assert_eq!(r.status()["keyboard"]["can_execute_policies"], false);
            assert!(r.has_pending_restore());
        }
        r.environment.owner = Owner::Clear;
        cfg.owned = false;
        r.stopping = true;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.writes.len(), count);
        assert!(r.has_pending_restore());
        cfg.owned = true;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 73);
        assert!(!r.has_pending_restore());
    }
    #[test]
    fn read_and_write_failures_retry_with_backoff_without_losing_restore_state() {
        let mut r = runtime();
        let cfg = config();
        let mut io = FakeIo {
            saved: 73,
            fail_read: 1,
            ..Default::default()
        };
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.reads, 1);
        assert!(io.writes.is_empty());
        r.environment.now_ms = 60_500;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.reads, 1);
        r.environment.now_ms = 61_000;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.reads, 2);
        assert_eq!(io.live, 0);
        io.fail_write = 1;
        r.environment.idle_ms = Ok(0);
        r.environment.now_ms = 62_000;
        r.reconcile("keyboard", &cfg, &mut io);
        assert!(r.has_pending_restore());
        assert!(r.status()["keyboard"]["error"].is_string());
        r.environment.now_ms = 62_500;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 0);
        r.environment.now_ms = 63_000;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 73);
        assert!(!r.has_pending_restore());
    }
    #[test]
    fn disconnect_and_reconnect_do_not_reuse_stale_live_acknowledgements() {
        let mut r = runtime();
        let cfg = config();
        let mut io = FakeIo {
            saved: 73,
            ..Default::default()
        };
        r.environment.idle_ms = Ok(60_000);
        r.reconcile("keyboard", &cfg, &mut io);
        r.retain(&HashSet::new());
        assert_eq!(
            r.status()["keyboard"]["blocked_reason"],
            "device-disconnected"
        );
        io.live = 73;
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(io.live, 0);
        assert_eq!(io.reads, 2);
        r.retain(&HashSet::new());
        r.environment.idle_ms = Ok(0);
        r.reconcile("keyboard", &cfg, &mut io);
        assert_eq!(
            io.live, 73,
            "wake while disconnected must not leave the old zero override behind"
        );
        r.retain(&HashSet::new());
        assert_eq!(r.status(), json!({}));
    }
    #[test]
    fn idle_clock_handles_32_bit_wrap_long_unchanged_input_and_future_timestamps() {
        let mut clock = IdleClock::default();
        let now = (1u64 << 32) + 2000;
        let raw = u32::MAX - 3000;
        assert_eq!(clock.sample(now, raw).unwrap(), 5001);
        assert_eq!(clock.sample(now + 180_000, raw).unwrap(), 185001);
        let recent = (now + 180_000) as u32;
        assert_eq!(clock.sample(now + 180_000, recent).unwrap(), 0);
        let future = recent.wrapping_add(100);
        assert_eq!(clock.sample(now + 180_000, future).unwrap(), 0);
        assert_eq!(clock.sample(now + 183_000, future).unwrap(), 3000);
        assert!(clock.sample(now, future).is_err());
    }
    #[test]
    fn process_owner_gate_matches_known_synapse_names_without_blocking_unrelated_helpers() {
        assert!(competing_process_name("RazerAppEngine.exe"));
        assert!(competing_process_name("RZSYNAPSE.EXE"));
        assert!(competing_process_name("Razer Synapse 3.exe"));
        assert!(!competing_process_name("RazerCentralService.exe"));
        assert!(!competing_process_name("secbird-tray.exe"));
    }

    #[test]
    fn simulated_bridge_changes_only_live_register_and_never_effect_or_sleep() {
        let sim = secbird_core::sim::Sim::keyboard();
        let mut dev = sim
            .open(
                605,
                secbird_core::sim::Sim::spec_for(secbird_core::sim::KEYBOARD_PID),
            )
            .unwrap();
        dev.interface = 2;
        dev.set_brightness(5, 73).unwrap();
        sim.state().log.clear();
        let mut r = runtime();
        let mut cfg = config();
        let mut bridge = DeviceIo(&dev);
        r.environment.idle_ms = Ok(60_000);
        r.reconcile(&dev.key(), &cfg, &mut bridge);
        assert_eq!(sim.state().live_brightness[&5], 0);
        assert_eq!(dev.brightness(5).unwrap(), 73);
        cfg.value = Some(41);
        r.reconcile(&dev.key(), &cfg, &mut bridge);
        r.environment.idle_ms = Ok(0);
        r.reconcile(&dev.key(), &cfg, &mut bridge);
        assert_eq!(sim.state().live_brightness[&5], 104);
        assert_eq!(dev.brightness(5).unwrap(), 73);
        assert!(sim
            .state()
            .log
            .iter()
            .all(|c| *c == secbird_core::commands::MATRIX_BRIGHTNESS
                || *c == secbird_core::commands::MATRIX_BRIGHTNESS.get()
                || *c == secbird_core::commands::ACTIVE_SLOT.get()));
    }
}
