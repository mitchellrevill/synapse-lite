//! The tray's own settings (`tray.json`), starting with Windows, and the settings folder:
//! read by the tray and changed from its menu or the editor's Settings page.

use secbird_core::config_dir;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const APP_NAME: &str = "Synapse Lite";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Tray settings in `tray.json`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TraySettings {
    #[serde(default = "minimized_default")]
    pub start_minimized: bool,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(skip)]
    pub read_error: Option<String>,
    /// Local per-device battery policy; opaque leaves remain intact.
    #[serde(default = "empty_battery_devices")]
    pub battery_devices: serde_json::Value,
    /// Check Razer's product pages for newer firmware once a week. Off until turned on.
    #[serde(default)]
    pub firmware_weekly: bool,
    /// Unix time of the last weekly check.
    #[serde(default)]
    pub firmware_checked: u64,
    /// Device key -> firmware version already announced, so each is announced once.
    #[serde(default)]
    pub firmware_notified: std::collections::BTreeMap<String, String>,
    /// Preserved for upstream compatibility. The fork does not schedule network checks.
    #[serde(default = "updates_disabled")]
    pub updates_off: bool,
    /// Unix time of the last daily new-version check.
    #[serde(default)]
    pub update_checked: u64,
    /// The newest version already announced, so each is announced once.
    #[serde(default)]
    pub update_notified: String,
    /// The version that ran last, to say "Updated to ..." once after an update.
    #[serde(default)]
    pub last_version: String,
    /// No small popups above the clock for DPI, on-board profile and game profile changes.
    #[serde(default)]
    pub popups_off: bool,
    /// No notification when a wireless device runs low.
    #[serde(default)]
    pub battery_warning_off: bool,
    /// Battery percent at or below which that notification shows; `None` is the default.
    #[serde(default)]
    pub battery_warning_at: Option<u8>,
    /// Don't switch profiles when a program tied to one comes to the front.
    #[serde(default)]
    pub auto_switch_off: bool,
    /// What the lights do while the PC is locked or its screen is off (#48): "dim", "off",
    /// or left as they are when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub away_lights: Option<String>,
}

fn empty_battery_devices() -> serde_json::Value { serde_json::json!({}) }

fn updates_disabled() -> bool {
    true
}
fn minimized_default() -> bool {
    true
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            start_minimized: true,
            extra: Default::default(),
            read_error: None,
            battery_devices: empty_battery_devices(),
            firmware_weekly: false,
            firmware_checked: 0,
            firmware_notified: Default::default(),
            updates_off: true,
            update_checked: 0,
            update_notified: String::new(),
            last_version: String::new(),
            popups_off: false,
            battery_warning_off: false,
            battery_warning_at: None,
            auto_switch_off: false,
            away_lights: None,
        }
    }
}

/// Choices for [`TraySettings::away_lights`], with the percent of the saved brightness.
pub const AWAY_LIGHTS: [(&str, u8); 2] = [("dim", 25), ("off", 0)];

/// Low battery levels a person can pick, and the default.
pub const BATTERY_RANGE: std::ops::RangeInclusive<u8> = 1..=95;
pub const BATTERY_DEFAULT: u8 = 15;

impl TraySettings {
    /// The low battery level, falling back to the default for a missing or hand-edited value.
    pub fn low_battery(&self) -> u8 {
        self.battery_warning_at
            .filter(|v| BATTERY_RANGE.contains(v))
            .unwrap_or(BATTERY_DEFAULT)
    }

    /// Brightness, in percent of the saved level, for the lights while away; `None` leaves them.
    pub fn away_percent(&self) -> Option<u8> {
        let want = self.away_lights.as_deref()?;
        AWAY_LIGHTS
            .iter()
            .find(|(name, _)| *name == want)
            .map(|(_, pct)| *pct)
    }

    pub fn path() -> std::path::PathBuf {
        config_dir().join("tray.json")
    }

    pub fn load() -> Self {
        Self::read(&Self::path()).unwrap_or_else(|error| Self {
            read_error: Some(error),
            ..Self::default()
        })
    }

    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        use std::io::Read;
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Cannot read tray settings: {e}")),
        };
        let mut bytes = Vec::new();
        file.take(MAX_SETTINGS_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SETTINGS_BYTES {
            return Err("Tray settings exceed 128 KiB; original retained".into());
        }
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
        serde_json::from_slice(bytes)
            .map_err(|e| format!("Cannot parse tray settings; original retained: {e}"))
    }

    pub fn write(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(error) = &self.read_error {
            return Err(error.clone());
        }
        // Refuse to overwrite a file which became unreadable since loading it.
        let current = Self::read(path)?;
        let mut saved = self.clone();
        saved.extra.extend(current.extra);
        let bytes = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SETTINGS_BYTES {
            return Err("Tray settings exceed 128 KiB; original retained".into());
        }
        std::fs::create_dir_all(path.parent().ok_or("Settings path has no folder")?)
            .map_err(|e| e.to_string())?;
        crate::native_editor::profile_files::write(path, &bytes)
    }

    pub fn save(&self) -> Result<(), String> {
        let result = self.write(&Self::path());
        if let Err(error) = &result {
            secbird_core::log::append(error);
        }
        result
    }
}

pub const MAX_SETTINGS_BYTES: u64 = 128 * 1024;
static SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
mod startup;
pub use startup::{autostart_enabled, set_autostart};

/// A single read/modify/write path for both editors and the tray menu.
pub fn change(
    path: &std::path::Path,
    startup: &mut dyn startup::Backend,
    key: &str,
    on: bool,
    value: u8,
    data: &str,
    read_only: bool,
) -> Result<TraySettings, String> {
    if read_only {
        return Err("Read-only mode refuses settings changes".into());
    }
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "Settings are unavailable")?;
    let mut settings = TraySettings::read(path)?;
    match key {
        "autostart" => {
            startup::set(startup, on)?;
            return Ok(settings);
        }
        "start_minimized" => {
            if !startup::enabled(startup)? {
                return Err("Enable Auto Launch before changing minimized startup".into());
            }
            // Migrate the old unconditional --tray command so the child choice is honored.
            startup::set(startup, true)?;
            settings.start_minimized = on;
        }
        "popups" => settings.popups_off = !on,
        "battery_warning" => settings.battery_warning_off = !on,
        "battery_warning_at" if BATTERY_RANGE.contains(&value) => {
            settings.battery_warning_at = Some(value)
        }
        "battery_device" if !data.is_empty() && BATTERY_RANGE.contains(&value) => {
            let inherited_threshold = settings.low_battery();
            let devices = settings.battery_devices.as_object_mut().ok_or("Invalid battery preferences object")?;
            let mut prefs = match devices.get(data) {
                Some(value) => serde_json::from_value::<crate::battery_notifications::Preferences>(value.clone())
                    .map_err(|_| "Invalid device battery preferences; existing settings were preserved")?,
                None => crate::battery_notifications::Preferences { threshold: inherited_threshold, ..Default::default() },
            };
            prefs.enabled = on;
            prefs.threshold = value;
            devices.insert(data.into(), serde_json::to_value(prefs).map_err(|e|e.to_string())?);
        }
        "battery_charging" if !data.is_empty() => {
            let inherited_threshold = settings.low_battery();
            let devices = settings.battery_devices.as_object_mut().ok_or("Invalid battery preferences object")?;
            let mut prefs = match devices.get(data) {
                Some(value) => serde_json::from_value::<crate::battery_notifications::Preferences>(value.clone())
                    .map_err(|_| "Invalid device battery preferences; existing settings were preserved")?,
                None => crate::battery_notifications::Preferences { threshold: inherited_threshold, ..Default::default() },
            };
            prefs.charging = on;
            devices.insert(data.into(), serde_json::to_value(prefs).map_err(|e|e.to_string())?);
        }
        "auto_switch" => settings.auto_switch_off = !on,
        "away_lights" if data.is_empty() || AWAY_LIGHTS.iter().any(|(key, _)| *key == data) => {
            settings.away_lights = (!data.is_empty()).then(|| data.to_owned())
        }
        _ => return Err(format!("Unsupported or invalid setting: {key}")),
    }
    settings.write(path)?;
    TraySettings::read(path)
}

pub fn change_local(
    key: &str,
    on: bool,
    value: u8,
    data: &str,
    read_only: bool,
) -> Result<TraySettings, String> {
    change(
        &TraySettings::path(),
        &mut startup::Registry,
        key,
        on,
        value,
        data,
        read_only,
    )
}

/// Explicit --tray always hides; an autostart launch honors the stored child preference.
pub fn opens_editor(args: &[String], settings: &TraySettings) -> bool {
    let hidden = args
        .iter()
        .any(|arg| arg == "--tray" || (arg == "--startup" && settings.start_minimized));
    !hidden
}

pub fn open_config_folder() {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    unsafe {
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(&dir.display().to_string()).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use startup::tests::Fake;
    struct Folder(std::path::PathBuf);
    impl Folder {
        fn new() -> Self {
            static SEQUENCE: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "lite-settings-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> std::path::PathBuf {
            self.0.join("tray.json")
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn shared_settings_preserve_unknowns_and_real_consumer_values() {
        let folder = Folder::new();
        let path = folder.file();
        std::fs::write(
            &path,
            br#"{"future":{"nested":[true,"opaque",null]},"firmware_weekly":true}"#,
        )
        .unwrap();
        let mut startup = Fake::default();
        let s = change(&path, &mut startup, "popups", false, 0, "", false).unwrap();
        assert!(s.popups_off);
        let s = change(
            &path,
            &mut startup,
            "battery_warning_at",
            false,
            27,
            "",
            false,
        )
        .unwrap();
        assert_eq!(s.low_battery(), 27);
        let s = change(&path, &mut startup, "auto_switch", false, 0, "", false).unwrap();
        assert!(s.auto_switch_off);
        let s = change(&path, &mut startup, "away_lights", false, 0, "dim", false).unwrap();
        assert_eq!(s.away_percent(), Some(25));
        assert_eq!(
            s.extra["future"],
            serde_json::json!({"nested":[true,"opaque",null]})
        );
        assert!(s.firmware_weekly);
        assert_eq!(startup.writes, 0);
        let before = std::fs::read(&path).unwrap();
        for (key, value, data) in [
            ("battery_warning_at", 0, ""),
            ("battery_warning_at", 96, ""),
            ("away_lights", 0, "invented"),
            ("updates", 0, ""),
        ] {
            assert!(change(&path, &mut startup, key, true, value, data, false).is_err());
        }
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn battery_device_edits_preserve_inheritance_unknowns_and_refuse_corruption() {
        let folder = Folder::new();
        let path = folder.file();
        let mut backend = Fake::default();
        let initial = serde_json::json!({"battery_warning_at":27,"future":[1,null],
            "battery_devices":{"other":{"threshold":20,"future":{"opaque":true}}}});
        std::fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
        let saved = change(&path, &mut backend, "battery_charging", true, 0, "mouse", false).unwrap();
        assert_eq!(saved.battery_devices["mouse"]["threshold"], 27);
        assert_eq!(saved.battery_devices["mouse"]["charging"], true);
        let saved = change(&path, &mut backend, "battery_device", false, 19, "other", false).unwrap();
        assert_eq!(saved.battery_devices["other"]["future"], initial["battery_devices"]["other"]["future"]);
        assert_eq!(saved.battery_devices["mouse"]["threshold"], 27);
        assert_eq!(saved.extra["future"], initial["future"]);
        let before = std::fs::read(&path).unwrap();
        for (key, on, threshold, device, read_only) in [
            ("battery_device", true, 0, "mouse", false),
            ("battery_device", true, 96, "mouse", false),
            ("battery_device", true, 20, "", false),
            ("battery_charging", false, 0, "mouse", true),
        ] {
            assert!(change(&path, &mut backend, key, on, threshold, device, read_only).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        for malformed in [serde_json::json!(null), serde_json::json!({"threshold":"future-format","opaque":42})] {
            let damaged = serde_json::json!({"battery_devices":{"mouse":malformed}});
            let bytes = serde_json::to_vec(&damaged).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            for key in ["battery_device", "battery_charging"] {
                assert!(change(&path, &mut backend, key, true, 20, "mouse", false).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
            }
        }
        assert_eq!(backend.writes, 0);
    }

    #[test]
    fn damaged_oversize_and_failed_writes_preserve_original() {
        let folder = Folder::new();
        let path = folder.file();
        let mut backend = Fake::default();
        for bytes in [
            b"{broken".to_vec(),
            vec![b' '; MAX_SETTINGS_BYTES as usize + 1],
            br#"{"popups_off":"bad type"}"#.to_vec(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(change(&path, &mut backend, "autostart", true, 0, "", false).is_err());
            assert!(TraySettings::default().write(&path).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        assert_eq!(backend.writes, 0);
        std::fs::write(&path, b"{}").unwrap();
        let mut s = TraySettings::read(&path).unwrap();
        s.extra.insert(
            "huge".into(),
            serde_json::json!("x".repeat(MAX_SETTINGS_BYTES as usize)),
        );
        assert!(s.write(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
        std::fs::create_dir(folder.0.join("directory.json")).unwrap();
        assert!(s.write(&folder.0.join("directory.json")).is_err());
        let original_permissions = std::fs::metadata(&path).unwrap().permissions();
        let mut read_only = original_permissions.clone();
        read_only.set_readonly(true);
        std::fs::set_permissions(&path, read_only).unwrap();
        assert!(TraySettings::default().write(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{}");
        std::fs::set_permissions(&path, original_permissions).unwrap();
    }
    #[test]
    fn startup_dependency_and_window_semantics_are_not_decorative() {
        let folder = Folder::new();
        let path = folder.file();
        let mut backend = Fake::default();
        assert!(change(&path, &mut backend, "start_minimized", false, 0, "", false).is_err());
        assert!(!path.exists());
        change(&path, &mut backend, "autostart", true, 0, "", false).unwrap();
        let s = change(&path, &mut backend, "start_minimized", false, 0, "", false).unwrap();
        assert!(opens_editor(&["--startup".into()], &s));
        assert!(opens_editor(&[], &s));
        assert!(!opens_editor(&["--tray".into()], &s));
        let s = change(&path, &mut backend, "start_minimized", true, 0, "", false).unwrap();
        assert!(!opens_editor(&["--startup".into()], &s));
        assert!(opens_editor(&[], &s));
        let before = std::fs::read(&path).unwrap();
        let writes = backend.writes;
        for key in ["autostart", "popups", "away_lights", "start_minimized"] {
            assert!(change(&path, &mut backend, key, false, 0, "", true).is_err());
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(backend.writes, writes);
        backend.fail = true;
        assert!(change(&path, &mut backend, "start_minimized", false, 0, "", false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
