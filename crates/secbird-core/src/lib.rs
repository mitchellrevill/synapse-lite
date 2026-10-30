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
pub mod breathing_lighting;
pub mod commands;
#[cfg(windows)]
pub mod debloat;
pub mod db;
pub mod device;
pub mod device_catalog;
#[cfg(windows)]
pub mod driver_control;
pub mod effects;
pub mod error;
pub mod firmware;
pub mod fire_lighting;
pub mod features;
pub mod keyboard_symbolic_bridge;
pub mod keyboard_output_ledger;
#[cfg(windows)]
pub mod keyboard_notifications;
#[cfg(windows)]
pub mod keyboard_input_identity;
pub mod lighting;
pub mod lighting_pixels;
pub mod lighting_readback;
pub mod log;
pub mod macros;
pub mod mouse_symbolic_bridge;
pub mod mouse_output_ledger;
pub mod mouse_sensor;
pub mod dynamic_sensitivity;
pub mod sensor_curve;
#[cfg(windows)]
pub mod sensor_rotation;
#[cfg(windows)]
pub mod sensor_rotation_control;
#[cfg(windows)]
pub mod mouse_input_identity;
pub mod net;
pub mod ornata_frame;
pub mod ornata_driver_mode;
pub mod ornata_identity;
pub mod ornata_gaming_indicator;
#[cfg(windows)]
pub mod ornata_manifest;
pub mod ornata_spatial;
pub mod performance;
pub mod onboard;
pub mod power;
pub mod wheel_controls;
pub mod profile;
pub mod profile_compat;
pub mod proto;
pub mod report;
pub mod rzcontrol_packet;
pub mod sim;
pub mod source_macro;
pub mod native_host_launch;
pub mod native_paste_plan;
pub mod macro_scheduler;
pub mod snap_tap;
pub mod studio;
pub mod spectrum_lighting;
pub mod transport;
pub mod uniform_lighting;
pub mod update;
pub mod wave_lighting;
pub mod studio_wave_lighting;
pub mod audio_analysis;

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

pub mod linked_games;

pub mod ambient_sampling;
pub mod global_shortcuts;

pub mod device_information;
pub mod support_bundle;
pub mod lighting_identify;
