//! Ownership-bound source Lighting actions and explicitly labeled readbacks.
use super::*;
use secbird_core::{
    lighting::{Brightness, LightingSettings, SwitchOffLighting},
    Profile,
};

fn owner<'a>(
    dev: &Device,
    store: &'a mut Store,
    body: &Body,
) -> Result<&'a mut secbird_core::profile::DeviceProfiles, Error> {
    if dev.pid != 605
        || body.id != dev.key()
        || body.expected_device.as_deref() != Some(dev.key().as_str())
    {
        return Err(Error::InvalidArg(
            "Lighting requires the same exact Ornata device identity".into(),
        ));
    }
    let profiles = store.for_device(dev);
    if body.expected_profile.as_deref() != Some(profiles.active.as_str()) {
        return Err(Error::InvalidArg(
            "The active profile changed; refresh Lighting".into(),
        ));
    }
    if profiles.lighting_off {
        return Err(Error::InvalidArg(
            "Lighting control is disabled for this device".into(),
        ));
    }
    Ok(profiles)
}

pub(super) fn intent(
    dev: &Device,
    store: &mut Store,
    body: &Body,
    policy: bool,
) -> Result<Value, Error> {
    let want = body
        .lighting_settings
        .as_ref()
        .ok_or_else(|| Error::InvalidArg("lighting_settings is required".into()))?;
    let before = store.devices.clone();
    let result = (|| {
        let profiles = owner(dev, store, body)?;
        if policy
            && LightingSettings::from_profile(profiles.active())?
                .and_then(|s| s.brightness)
                .is_none()
        {
            // First policy edit in a fresh native profile: record a baseline from
            // saved hardware without issuing a brightness SET or inventing100.
            let raw = dev.ornata_brightness_readback(1, 0)?;
            if !raw.selectors_match() {
                return Err(Error::InvalidArg(
                    "Brightness readback selectors differ".into(),
                ));
            }
            let baseline = Brightness {
                enabled: raw.raw_brightness() != 0,
                value: source_percent(raw.raw_brightness()),
                ..Default::default()
            };
            if want.brightness.as_ref() != Some(&baseline) {
                return Err(Error::InvalidArg(
                    "Brightness changed; refresh before editing policies".into(),
                ));
            }
            let mut initial =
                LightingSettings::from_profile(profiles.active())?.unwrap_or_default();
            initial.brightness = Some(baseline);
            initial.store_in_profile(profiles.active_mut())?;
        }
        let profile = body.expected_profile.as_deref().unwrap();
        let value = if policy {
            actions::set_lighting_policy(dev, store, &body.id, profile, want)?
        } else {
            actions::set_lighting_settings(dev, store, &body.id, profile, want)?
        };
        Ok(json!({"lighting_settings":value}))
    })();
    if result.is_err() {
        store.devices = before;
    }
    result
}

pub(super) fn custom_colors(dev: &Device, store: &mut Store, body: &Body) -> Result<Value, Error> {
    let slots = body
        .custom_colors
        .as_ref()
        .ok_or_else(|| Error::InvalidArg("custom_colors is required".into()))?;
    let before = store.devices.clone();
    let result = (|| {
        let profiles = owner(dev, store, body)?;
        let name = profiles.active.clone();
        secbird_core::profile_compat::set_source_custom_colors(
            profiles.active_mut(),
            605,
            &name,
            slots,
        )?;
        secbird_core::profile_compat::validate_store_budget(store)?;
        store.save()?;
        Ok(json!({"saved":true}))
    })();
    if result.is_err() {
        store.devices = before;
    }
    result
}

mod effects;

pub(super) fn effect(dev: &Device, store: &mut Store, body: &Body) -> Result<Value, Error> {
    effects::apply(dev, store, body)
}

fn source_percent(raw: u8) -> u8 {
    (u16::from(raw) * 100).div_ceil(255) as u8
}

pub(super) fn snapshot(dev: &Device, profile: Option<&Profile>) -> (Value, Value, Value) {
    if dev.pid != 605 {
        return (Value::Null, Value::Null, Value::Null);
    }
    let read = dev.ornata_brightness_readback(1, 0);
    let readback = match &read {
        Ok(value) => {
            json!({"available":true,"raw":value.raw_brightness(),"percent":source_percent(value.raw_brightness()),"selectors_match":value.selectors_match(),"error":null})
        }
        Err(error) => {
            json!({"available":false,"raw":null,"percent":null,"selectors_match":false,"error":error.to_string()})
        }
    };
    let parsed = profile.map(LightingSettings::from_profile).transpose();
    let (mut settings, mut origin) = match parsed {
        Ok(value) => (
            value.flatten().unwrap_or_default(),
            if profile.is_some_and(|p| p.lighting_settings.is_some()) {
                "profile-intent"
            } else {
                "legacy-or-hardware"
            },
        ),
        Err(error) => return (Value::Null, readback, json!({"error":error.to_string()})),
    };
    if settings.brightness.is_none() {
        if let Ok(value) = &read {
            if value.selectors_match() {
                settings.brightness = Some(Brightness {
                    enabled: value.raw_brightness() != 0,
                    value: source_percent(value.raw_brightness()),
                    ..Default::default()
                });
                origin = "hardware-brightness-with-default-policy";
            }
        }
    }
    if settings.switch_off.is_none() {
        settings.switch_off = Some(SwitchOffLighting::default());
    }
    (
        serde_json::to_value(&settings).unwrap_or(Value::Null),
        readback,
        json!({"source":origin,"retained_value_known":profile.is_some_and(|p|p.lighting_settings.is_some()),"synapse_profile_state_read":false}),
    )
}
