//! Local alert policy. Missing readback breaks transition continuity, not low-alert suppression.
use std::collections::BTreeMap;
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub threshold: u8,
    pub charging: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Preferences {
    fn default() -> Self { Self { enabled: true, threshold: 15, charging: false, extra: BTreeMap::new() } }
}
pub fn preferences(settings: &crate::settings::TraySettings, key: &str) -> Preferences {
    let mut p = settings.extra.get("battery_devices").and_then(|v| v.get(key)).and_then(|v| serde_json::from_value::<Preferences>(v.clone()).ok()).unwrap_or_else(|| Preferences { threshold: settings.low_battery(), ..Default::default() });
    p.enabled &= !settings.battery_warning_off;
    if !(1..=95).contains(&p.threshold) { p.threshold = settings.low_battery(); }
    p
}
#[derive(Default)]
struct DeviceState { low: bool, charging: Option<bool>, seen: bool }
#[derive(Default)]
pub struct Monitor { devices: BTreeMap<String, DeviceState> }
impl Monitor {
    pub fn begin(&mut self) { for state in self.devices.values_mut() { state.seen = false; } }
    pub fn observe(&mut self, key: &str, percent: u8, charging: Option<bool>, prefs: &Preferences) -> Vec<&'static str> {
        let state = self.devices.entry(key.into()).or_default();
        state.seen = true;
        let mut alerts = Vec::new();
        if percent > 100 { state.charging = None; return alerts; }
        if prefs.enabled && prefs.charging && state.charging.is_some() && charging.is_some() && state.charging != charging {
            alerts.push(if charging == Some(true) { "Charging started" } else { "Charging stopped" });
        }
        // Hysteresis avoids repeated alerts as a rounded reading jitters at the threshold.
        if charging == Some(true) || percent >= prefs.threshold.saturating_add(10).min(100) { state.low = false; }
        if charging == Some(false) && percent <= prefs.threshold && !state.low {
            if prefs.enabled { alerts.push("Battery low"); }
            state.low = true;
        }
        state.charging = charging;
        alerts
    }
    pub fn end(&mut self) { for state in self.devices.values_mut().filter(|s| !s.seen) { state.charging = None; } }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn episodes_survive_unknown_and_disconnect() {
        let mut m=Monitor::default(); let p=Preferences::default();
        assert_eq!(m.observe("a",15,Some(false),&p),vec!["Battery low"]);
        assert!(m.observe("a",14,None,&p).is_empty());
        m.begin(); m.end();
        assert!(m.observe("a",13,Some(false),&p).is_empty());
        m.observe("a",25,Some(false),&p);
        assert_eq!(m.observe("a",15,Some(false),&p),vec!["Battery low"]);
        assert_eq!(m.observe("b",10,Some(false),&p),vec!["Battery low"]);
    }
    #[test] fn transitions_require_continuity_and_opt_in() {
        let mut m=Monitor::default(); let p=Preferences { charging:true,..Default::default() };
        assert!(m.observe("a",50,Some(true),&p).is_empty());
        assert_eq!(m.observe("a",50,Some(false),&p),vec!["Charging stopped"]);
        assert!(m.observe("a",50,Some(false),&p).is_empty());
        m.begin();m.end();
        assert!(m.observe("a",50,Some(true),&p).is_empty());
        let p=Preferences { enabled:false,..p };
        assert!(m.observe("a",5,Some(false),&p).is_empty());
    }
}
