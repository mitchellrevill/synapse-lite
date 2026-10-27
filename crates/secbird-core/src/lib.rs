//! Lightweight control of Razer peripherals over USB HID.
//!
//! ```no_run
//! let api = hidapi::HidApi::new()?;
//! let db = secbird_core::Db::builtin()?;
//! for dev in secbird_core::discover(&api, &db) {
//!     println!("{} firmware {:?}", dev.name(), dev.firmware()?);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod actions;
pub mod buttons;
pub mod commands;
#[cfg(windows)]
pub mod debloat;
pub mod db;
pub mod device;
#[cfg(windows)]
pub mod driver_control;
pub mod effects;
pub mod error;
pub mod firmware;
pub mod features;
pub mod log;
pub mod macros;
pub mod net;
pub mod performance;
pub mod power;
pub mod profile;
pub mod profile_compat;
pub mod proto;
pub mod report;
pub mod sim;
pub mod transport;
pub mod update;

pub use db::{Db, DeviceSpec, Kind, Lighting, Matrix};
pub use device::{discover, Device};
pub use error::{Error, Result};
pub use features::{Dimming, DpiStages, GameMode, Rgb, SleepTimer};
pub use profile::{DevicePower, Profile, Store, Zone};
pub use transport::Transport;

/// Use an isolated fork store; upstream exports remain compatible through import.
#[cfg(windows)]
pub fn config_dir() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("SYNAPSE_LITE_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return path.into();
    }
    std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("SynapseLite")
}

/// `$XDG_CONFIG_HOME/synapse-lite`, usually `~/.config/synapse-lite`.
#[cfg(not(windows))]
pub fn config_dir() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("SYNAPSE_LITE_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return path.into();
    }
    let home = || std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"));
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(home)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("synapse-lite")
}
pub use hidapi;
