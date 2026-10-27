//! Immutable worker previews and source profile/macro checkbox state.
use secbird_core::profile_compat::gui::{Choice, Kind, Preview};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static TOKENS: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Owner {
    pub device: String,
    pub profile: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Import,
    Export,
}
#[derive(Clone, Debug)]
pub(super) enum Control {
    Close,
    Browse,
    Cloud(bool),
    All,
    Profile(usize),
    Macro(usize, usize),
    Commit,
}
pub(super) struct View {
    pub token: u64,
    pub owner: Owner,
    pub mode: Mode,
    pub filename: String,
    pub data: Option<Arc<Preview>>,
    pub choices: Vec<Choice>,
    // Keep the source checkbox presentation, but synchronize available macro
    // action data when a parent transition checks or clears its visible marks.
    pub macro_marks: Vec<BTreeSet<String>>,
    pub scroll_grab: Option<f64>,
    pub cloud: bool,
    pub deselect_all: bool,
    pub loading: bool,
    pub scroll: f64,
    pub error: String,
    pub warning: bool,
    pub device_name: String,
}
impl View {
    pub fn new(owner: Owner, mode: Mode, device_name: String) -> Self {
        Self {
            token: TOKENS.fetch_add(1, Ordering::Relaxed),
            owner,
            mode,
            filename: String::new(),
            data: None,
            choices: Vec::new(),
            macro_marks: Vec::new(),
            scroll_grab: None,
            cloud: false,
            deselect_all: false,
            loading: mode == Mode::Export,
            scroll: 0.0,
            error: String::new(),
            warning: false,
            device_name,
        }
    }
    pub fn native(&self) -> bool {
        self.data
            .as_ref()
            .is_some_and(|p| p.kind == Kind::NativeBackup)
            && self.mode == Mode::Import
    }
    pub fn enabled(&self) -> bool {
        !self.cloud
            && !self.loading
            && self.error.is_empty()
            && self.choices.iter().any(|c| c.selected)
            && self
                .data
                .as_ref()
                .is_some_and(|p| p.compatible(&self.owner.device))
    }
    pub fn expanded(&self) -> Vec<(bool, usize)> {
        self.data
            .as_ref()
            .map(|d| {
                d.rows
                    .iter()
                    .zip(&self.choices)
                    .map(|(r, c)| (c.selected, r.macros.len()))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn apply(&mut self, message: Message) -> bool {
        match message {
            Message::Preview { token, owner, data }
                if token == self.token && owner == self.owner =>
            {
                self.loading = false;
                self.warning = (self.mode == Mode::Import && data.has_source_warning)
                    || data.kind == Kind::NativeBackup;
                self.choices = data.initial_choices(
                    (self.mode == Mode::Export).then_some(self.owner.profile.as_str()),
                );
                self.macro_marks = data
                    .rows
                    .iter()
                    .zip(&self.choices)
                    .map(|(row, c)| {
                        if c.selected {
                            row.macros.iter().map(|m| m.guid.clone()).collect()
                        } else {
                            BTreeSet::new()
                        }
                    })
                    .collect();
                self.error = if data.compatible(&self.owner.device) {
                    String::new()
                } else {
                    "This source document belongs to another device family. Select that device before importing.".into()
                };
                self.data = Some(data);
                self.scroll = 0.0;
                true
            }
            Message::Error {
                token,
                owner,
                error,
            } if token == self.token && owner == self.owner => {
                self.loading = false;
                self.error = error;
                self.data = None;
                self.choices.clear();
                self.macro_marks.clear();
                self.scroll_grab = None;
                true
            }
            _ => false,
        }
    }
    pub fn toggle(&mut self, c: &Control) {
        if self.loading {
            return;
        }
        match *c {
            Control::Cloud(cloud) => {
                self.cloud = cloud;
                self.choices.clear();
                self.macro_marks.clear();
                self.scroll_grab = None;
                self.data = None;
                self.scroll = 0.0;
                self.error.clear();
            }
            Control::All if !self.native() => {
                let select = self.deselect_all;
                for (i, c) in self.choices.iter_mut().enumerate() {
                    if c.selected != select {
                        self.macro_marks[i] = if select {
                            self.data.as_ref().unwrap().rows[i]
                                .macros
                                .iter()
                                .map(|m| m.guid.clone())
                                .collect()
                        } else {
                            BTreeSet::new()
                        };
                        c.macros = if select {
                            self.data.as_ref().unwrap().rows[i]
                                .macros
                                .iter()
                                .filter(|m| m.available)
                                .map(|m| m.guid.clone())
                                .collect()
                        } else {
                            BTreeSet::new()
                        };
                    }
                    c.selected = select
                }
                self.deselect_all = !select;
            }
            Control::Profile(i) if !self.native() => {
                if let (Some(c), Some(row)) = (
                    self.choices.get_mut(i),
                    self.data.as_ref().and_then(|d| d.rows.get(i)),
                ) {
                    c.selected = !c.selected;
                    self.macro_marks[i] = if c.selected {
                        row.macros.iter().map(|m| m.guid.clone()).collect()
                    } else {
                        BTreeSet::new()
                    };
                    if c.selected {
                        c.macros = row
                            .macros
                            .iter()
                            .filter(|m| m.available)
                            .map(|m| m.guid.clone())
                            .collect();
                    }
                }
            }
            Control::Macro(i, m) if !self.native() => {
                if let (Some(c), Some(row)) = (
                    self.choices.get_mut(i),
                    self.data.as_ref().and_then(|d| d.rows.get(i)),
                ) {
                    if let Some(m) = row.macros.get(m).filter(|m| m.available) {
                        if !self.macro_marks[i].remove(&m.guid) {
                            self.macro_marks[i].insert(m.guid.clone());
                        }
                        if !c.macros.remove(&m.guid) {
                            c.macros.insert(m.guid.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
#[derive(Debug)]
pub(super) enum Message {
    Preview {
        token: u64,
        owner: Owner,
        data: Arc<Preview>,
    },
    Error {
        token: u64,
        owner: Owner,
        error: String,
    },
}
#[derive(Clone)]
pub(super) struct ExportPlan {
    pub owner: Owner,
    pub data: Arc<Preview>,
    pub choices: Vec<Choice>,
}
pub(super) enum Request {
    PreviewImport {
        token: u64,
        owner: Owner,
        path: PathBuf,
    },
    PreviewExport {
        token: u64,
        owner: Owner,
    },
    Import {
        owner: Owner,
        data: Arc<Preview>,
        choices: Vec<Choice>,
    },
    Export {
        plan: ExportPlan,
        path: PathBuf,
        format: super::profile_files::Format,
    },
}
impl Request {
    pub fn preview_owner(&self) -> Option<(u64, Owner)> {
        match self {
            Self::PreviewImport { token, owner, .. } | Self::PreviewExport { token, owner } => {
                Some((*token, owner.clone()))
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_preview_cannot_replace_new_dialog() {
        let owner = Owner {
            device: "025D:a".into(),
            profile: "One".into(),
        };
        let mut v = View::new(owner.clone(), Mode::Import, "Ornata".into());
        let token = v.token;
        assert!(!v.apply(Message::Error {
            token: token + 1,
            owner: owner.clone(),
            error: "late".into()
        }));
        assert!(v.error.is_empty());
        assert!(v.apply(Message::Error {
            token,
            owner,
            error: "invalid".into()
        }));
        assert_eq!(v.error, "invalid");
        assert!(!v.enabled());
    }
}
