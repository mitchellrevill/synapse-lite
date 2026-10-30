//! Local view lifecycle only: closing a tab never removes a device or profile.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::PathBuf};

const MAX_BYTES: u64 = 128 * 1024;
const MAX_TABS: usize = 128;

#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    version: u32,
    order: Vec<String>,
    closed: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            order: Vec::new(),
            closed: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

pub(super) struct Session {
    saved: Saved,
    connected: Vec<String>,
    recent_closed: Vec<String>,
    path: PathBuf,
    writable: bool,
    pub error: Option<String>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            saved: Saved::default(),
            connected: Vec::new(),
            recent_closed: Vec::new(),
            path: PathBuf::new(),
            writable: false,
            error: None,
        }
    }
}

impl Session {
    pub fn load(path: PathBuf) -> Self {
        let read = || -> Result<Saved, String> {
            let file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Saved::default()),
                Err(e) => return Err(format!("Cannot read tab layout: {e}")),
            };
            let mut bytes = Vec::new();
            file.take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("Tab layout exceeds128KiB".into());
            }
            let saved: Saved = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Cannot read tab layout: {e}"))?;
            if saved.version != 1 {
                return Err("Tab layout version is unsupported; original file retained".into());
            }
            for list in [&saved.order, &saved.closed] {
                if list.len() > MAX_TABS || list.iter().any(|id| !valid_id(id)) {
                    return Err(
                        "Tab layout has invalid or excessive entries; original file retained"
                            .into(),
                    );
                }
            }
            Ok(saved)
        };
        let (mut saved, error, writable) = match read() {
            Ok(saved) => (saved, None, true),
            Err(error) => (Saved::default(), Some(error), false),
        };
        deduplicate(&mut saved.closed);
        saved.order.retain(|id| !saved.closed.contains(id));
        deduplicate(&mut saved.order);
        Self {
            saved,
            connected: Vec::new(),
            recent_closed: Vec::new(),
            path,
            writable,
            error,
        }
    }

    pub fn known(&self,id:&str)->bool {self.saved.order.iter().chain(&self.saved.closed).any(|key|key==id)}

    pub fn has_device_history(&self, product: &str) -> bool {
        product.len()==4 && product.bytes().all(|byte|byte.is_ascii_hexdigit())
            && self.saved.order.iter().chain(&self.saved.closed)
                .any(|id|id.split_once(':').is_some_and(|(pid,_)|pid.eq_ignore_ascii_case(product)))
    }

    pub fn reconcile(&mut self, connected: &[String]) {
        self.connected = connected
            .iter()
            .filter(|id| valid_id(id))
            .take(MAX_TABS)
            .cloned()
            .collect();
        deduplicate(&mut self.connected);
        for id in &self.connected {
            if !self.saved.closed.contains(id)
                && !self.saved.order.contains(id)
                && self.saved.order.len() < MAX_TABS
            {
                self.saved.order.push(id.clone());
            }
        }
    }

    pub fn visible(&self) -> Vec<String> {
        self.saved
            .order
            .iter()
            .filter(|id| self.connected.contains(id) && !self.saved.closed.contains(id))
            .cloned()
            .collect()
    }

    pub fn open(&mut self, id: &str) -> bool {
        if !self.connected.iter().any(|key| key == id) {
            return false;
        }
        if !self.saved.order.iter().any(|key| key == id) {
            if self.saved.order.len() == MAX_TABS {
                self.error = Some("Tab layout is full".into());
                return false;
            }
            self.saved.order.push(id.into());
        }
        self.saved.closed.retain(|key| key != id);
        true
    }

    pub fn close(&mut self, id: &str, active: Option<&str>) -> Option<String> {
        let visible = self.visible();
        let Some(index) = visible.iter().position(|key| key == id) else {
            return active.map(str::to_owned);
        };
        let next = if active == Some(id) {
            visible
                .get(index + 1)
                .or_else(|| index.checked_sub(1).and_then(|i| visible.get(i)))
                .cloned()
        } else {
            active.map(str::to_owned)
        };
        self.saved.order.retain(|key| key != id);
        if !self.saved.closed.iter().any(|key| key == id) {
            if self.saved.closed.len() == MAX_TABS {
                self.saved.closed.remove(0);
            }
            self.saved.closed.push(id.into());
        }
        self.recent_closed.retain(|key| key != id);
        self.recent_closed.push(id.into());
        if self.recent_closed.len() > MAX_TABS {
            self.recent_closed.remove(0);
        }
        next
    }

    pub fn last_closed(&self) -> Option<String> {
        self.recent_closed
            .iter()
            .rev()
            .find(|id| self.connected.contains(id) && self.saved.closed.contains(id))
            .cloned()
    }

    pub fn move_to(&mut self, id: &str, target: usize) -> bool {
        let mut visible = self.visible();
        let Some(index) = visible.iter().position(|key| key == id) else {
            return false;
        };
        if target >= visible.len() || index == target {
            return false;
        }
        let item = visible.remove(index);
        visible.insert(target, item);
        let mut reordered = visible.iter();
        for key in &mut self.saved.order {
            if self.connected.contains(key) && !self.saved.closed.contains(key) {
                *key = reordered.next().unwrap().clone();
            }
        }
        true
    }

    pub fn save(&mut self) -> Result<(), String> {
        if !self.writable {
            return Err(self
                .error
                .clone()
                .unwrap_or_else(|| "Tab layout is read-only".into()));
        }
        let bytes = serde_json::to_vec_pretty(&self.saved).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Tab layout exceeds128KiB".into());
        }
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or("Tab layout has no folder")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        super::profile_files::write(&self.path, &bytes)
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 512 && !id.chars().any(char::is_control)
}
fn deduplicate(list: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    list.retain(|key| seen.insert(key.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn close_selects_neighbor_and_discovery_does_not_reopen_it() {
        let mut s = Session::default();
        s.reconcile(&ids(&["mouse", "keyboard", "studio"]));
        assert_eq!(s.close("keyboard", Some("keyboard")), Some("studio".into()));
        s.reconcile(&ids(&["mouse", "keyboard", "studio"]));
        assert_eq!(s.visible(), ids(&["mouse", "studio"]));
        assert_eq!(s.close("studio", Some("studio")), Some("mouse".into()));
        assert_eq!(s.close("mouse", Some("mouse")), None);
        assert_eq!(s.last_closed(), Some("mouse".into()));
        assert!(s.open("keyboard"));
        assert_eq!(s.visible(), ids(&["keyboard"]));
    }
    #[test]
    fn disconnect_is_not_user_close_and_inactive_close_keeps_selection() {
        let mut s = Session::default();
        s.reconcile(&ids(&["a", "b", "c"]));
        assert!(s.move_to("c", 0));
        assert_eq!(s.visible(), ids(&["c", "a", "b"]));
        s.reconcile(&ids(&["a", "b"]));
        s.reconcile(&ids(&["a", "b", "c"]));
        assert_eq!(s.visible(), ids(&["c", "a", "b"]));
        assert_eq!(s.close("a", Some("b")), Some("b".into()));
        assert!(!s.open("unknown"));
        assert!(!s.move_to("c", 9));
    }
    #[test]
    fn persisted_closed_order_and_unknown_fields_survive_reload() {
        let folder = std::env::temp_dir().join(format!(
            "lite-tab-layout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("tabs.json");
        std::fs::write(
            &path,
            r#"{"version":1,"order":["a","b","c"],"closed":[],"future_setting":{"keep":true}}"#,
        )
        .unwrap();
        let mut s = Session::load(path.clone());
        s.reconcile(&ids(&["a", "b", "c"]));
        s.move_to("c", 0);
        s.close("b", Some("a"));
        s.save().unwrap();
        s.save().unwrap();
        let mut reloaded = Session::load(path.clone());
        reloaded.reconcile(&ids(&["b", "a", "c"]));
        assert_eq!(reloaded.visible(), ids(&["c", "a"]));
        assert_eq!(reloaded.saved.extra["future_setting"]["keep"], true);
        std::fs::write(&path, r#"{"version":2,"order":[],"closed":[]}"#).unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut future = Session::load(path.clone());
        future.reconcile(&ids(&["a"]));
        assert!(future.save().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&folder).unwrap();
    }
}
