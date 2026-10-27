//! Source-profile brightness and switch-off intent for the Ornata V2.
//!
//! Brightness enable is independent of the selected effect. Idle/display policies
//! describe host-event behavior; storing them does not install a firmware timer.
use crate::{Device, Error, Profile, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Brightness {
    #[serde(rename = "isEnabled")]
    pub enabled: bool,
    /// Retained slider value, including while the brightness switch is off.
    pub value: u8,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Brightness {
    pub fn validate(&self) -> Result<()> {
        if self.value > 100 {
            return Err(Error::InvalidArg("brightness must be within 0-100".into()));
        }
        Ok(())
    }
    pub fn projected(&self) -> u8 {
        if self.enabled {
            self.value
        } else {
            0
        }
    }
    /// Source toggle preserves the value, even when it is zero.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Source slider edits enable brightness except at zero.
    pub fn set_value(&mut self, value: u8) -> Result<()> {
        if value > 100 {
            return Err(Error::InvalidArg("brightness must be within 0-100".into()));
        }
        self.value = value;
        self.enabled = value != 0;
        Ok(())
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Self {
            enabled: true,
            value: 100,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchOffLighting {
    /// Source `isDisplayOn` means "when display is turned Off", not current display state.
    #[serde(rename = "isDisplayOn")]
    pub display_off: bool,
    #[serde(rename = "isIdleEnabled")]
    pub idle_enabled: bool,
    #[serde(rename = "idleMinutes")]
    pub idle_minutes: u16,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl SwitchOffLighting {
    pub fn validate(&self) -> Result<()> {
        if !(1..=15).contains(&self.idle_minutes) {
            return Err(Error::InvalidArg(
                "lighting idle time must be within 1-15 minutes".into(),
            ));
        }
        Ok(())
    }
    pub fn needs_runtime(&self) -> bool {
        self.display_off || self.idle_enabled
    }
}
impl Default for SwitchOffLighting {
    fn default() -> Self {
        Self {
            display_off: false,
            idle_enabled: false,
            idle_minutes: 1,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LightingSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<Brightness>,
    #[serde(
        default,
        rename = "switchOffLighting",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_off: Option<SwitchOffLighting>,
    /// Native extensions have no assumed foreign meaning; retain them in native backups.
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl LightingSettings {
    pub fn validate(&self) -> Result<()> {
        if let Some(value) = &self.brightness {
            value.validate()?;
        }
        if let Some(value) = &self.switch_off {
            value.validate()?;
        }
        Ok(())
    }
    pub fn needs_runtime(&self) -> bool {
        self.switch_off
            .as_ref()
            .is_some_and(SwitchOffLighting::needs_runtime)
    }
    pub fn policy_controls_enabled(&self) -> bool {
        self.brightness.as_ref().is_some_and(|v| v.enabled)
    }
    pub fn idle_slider_enabled(&self) -> bool {
        self.policy_controls_enabled() && self.switch_off.as_ref().is_some_and(|v| v.idle_enabled)
    }

    /// Existing native profiles keep their old application behavior. This read-only
    /// fallback can present a recorded byte, but cannot recover a lost retained value.
    pub fn from_profile(profile: &Profile) -> Result<Option<Self>> {
        if let Some(value) = &profile.lighting_settings {
            value.validate()?;
            return Ok(Some(value.clone()));
        }
        let Some(value) = profile.lighting.get("backlight").and_then(|z| z.brightness) else {
            return Ok(None);
        };
        let settings = Self {
            brightness: Some(Brightness {
                enabled: value > 0,
                value,
                extra: BTreeMap::new(),
            }),
            ..Default::default()
        };
        settings.validate()?;
        Ok(Some(settings))
    }

    /// Keep a physical brightness projection for older readers. Zone.off continues
    /// to mean effect-off; brightness switch changes must never replace the effect.
    pub fn store_in_profile(&self, profile: &mut Profile) -> Result<()> {
        self.validate()?;
        if let Some(brightness) = &self.brightness {
            profile
                .lighting
                .entry("backlight".into())
                .or_default()
                .brightness = Some(brightness.projected());
        } else if let Some(zone) = profile.lighting.get_mut("backlight") {
            zone.brightness = None;
        }
        profile.lighting_settings = Some(self.clone());
        Ok(())
    }

    /// Apply only the verified brightness projection. No effect or host policy is changed.
    pub fn apply_brightness(&self, device: &Device) -> Result<Option<u8>> {
        self.validate()?;
        if device.pid != 605 {
            return Err(Error::Unsupported(
                "source brightness intent is verified only for Ornata V2".into(),
            ));
        }
        let Some(brightness) = &self.brightness else {
            return Ok(None);
        };
        let led = device.led_id("backlight")?;
        let value = brightness.projected();
        device.set_brightness(led, value)?;
        if device.brightness(led)? != value {
            return Err(Error::Unsupported(
                "the device did not retain the requested brightness".into(),
            ));
        }
        Ok(Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_retains_value_and_slider_zero_has_source_semantics() {
        let mut brightness = Brightness {
            value: 73,
            ..Default::default()
        };
        brightness.set_enabled(false);
        assert_eq!(brightness.value, 73);
        assert_eq!(brightness.projected(), 0);
        brightness.set_enabled(true);
        assert_eq!(brightness.projected(), 73);
        brightness.set_value(0).unwrap();
        assert!(!brightness.enabled);
        assert_eq!(brightness.value, 0);
        brightness.set_enabled(true);
        assert!(brightness.enabled);
        assert_eq!(brightness.projected(), 0);
        brightness.set_value(41).unwrap();
        assert!(brightness.enabled);
        assert_eq!(brightness.value, 41);
        assert!(brightness.set_value(101).is_err());
        assert_eq!(brightness.value, 41);
    }
    #[test]
    fn policies_retain_disabled_fields_and_follow_brightness_enable_for_controls() {
        let mut settings = LightingSettings {
            brightness: Some(Brightness::default()),
            switch_off: Some(SwitchOffLighting {
                display_off: true,
                idle_enabled: true,
                idle_minutes: 7,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(settings.needs_runtime());
        assert!(settings.idle_slider_enabled());
        let saved = settings.switch_off.clone();
        settings.brightness.as_mut().unwrap().set_enabled(false);
        assert!(!settings.policy_controls_enabled());
        assert!(!settings.idle_slider_enabled());
        assert_eq!(settings.switch_off, saved);
        settings.switch_off.as_mut().unwrap().idle_minutes = 0;
        assert!(settings.validate().is_err());
    }
    #[test]
    fn every_slider_value_uses_verified_floor_encoding_and_exact_percent_readback() {
        let sim = crate::sim::Sim::keyboard();
        let dev = sim
            .open(605, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        let mut settings = LightingSettings {
            brightness: Some(Brightness::default()),
            ..Default::default()
        };
        for value in 0..=100 {
            settings
                .brightness
                .as_mut()
                .unwrap()
                .set_value(value)
                .unwrap();
            assert_eq!(settings.apply_brightness(&dev).unwrap(), Some(value));
            assert_eq!(
                sim.state().brightness[&5],
                (u16::from(value) * 255 / 100) as u8
            );
        }
        assert!(!sim.state().log.contains(&crate::commands::MATRIX_EFFECT));
        let wrong = sim
            .open(196, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        let before = sim.state().log.len();
        assert!(settings.apply_brightness(&wrong).is_err());
        assert_eq!(sim.state().log.len(), before);
    }

    #[test]
    fn brightness_projection_preserves_effect_and_opaque_fields() {
        let mut settings = LightingSettings {
            brightness: Some(Brightness {
                enabled: false,
                value: 73,
                extra: BTreeMap::from([("future".into(), serde_json::json!(42))]),
            }),
            ..Default::default()
        };
        let mut profile = Profile::default();
        profile
            .lighting
            .entry("backlight".into())
            .or_default()
            .effect = Some(crate::effects::Effect::Spectrum);
        settings.store_in_profile(&mut profile).unwrap();
        assert!(!profile.lighting["backlight"].off);
        assert_eq!(profile.lighting["backlight"].brightness, Some(0));
        let sim = crate::sim::Sim::keyboard();
        let dev = sim
            .open(605, crate::sim::Sim::spec_for(crate::sim::KEYBOARD_PID))
            .unwrap();
        assert_eq!(settings.apply_brightness(&dev).unwrap(), Some(0));
        settings.brightness.as_mut().unwrap().set_enabled(true);
        assert_eq!(settings.apply_brightness(&dev).unwrap(), Some(73));
        assert!(!sim.state().log.contains(&crate::commands::MATRIX_EFFECT));
        let saved: Profile =
            serde_json::from_slice(&serde_json::to_vec(&profile).unwrap()).unwrap();
        assert_eq!(
            saved
                .lighting_settings
                .as_ref()
                .unwrap()
                .brightness
                .as_ref()
                .unwrap()
                .extra["future"],
            42
        );
    }
}
