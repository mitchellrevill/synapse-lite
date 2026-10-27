//! Exact durable subset of the shipped UI-to-firmware conversion (module6394)
//! and generateSetChromaEffectCommonData. No host animation or mode acquisition.
use super::*;
use secbird_core::{effects::Effect, features::Rgb, profile::Zone};

struct Plan {
    args: Vec<u8>,
    color: Option<Rgb>,
    effect: Option<Effect>,
}
impl Plan {
    fn parse(id: u8, setting: &Value) -> Result<Self, Error> {
        if !setting.is_object() {
            return Err(Error::InvalidArg("Effect setting must be an object".into()));
        }
        let mut args = vec![1, 0, id, 0, 0, 0];
        let mut color = None;
        let effect = match id {
            1 => { let value = rgb(setting, "color1")?; color = Some(value); None }
            3 => Some(Effect::Spectrum),
            2 | 7 => {
                if setting.get("isRandom").and_then(Value::as_bool) != Some(false) {
                    return Err(Error::Unsupported("Random Breathing/Starlight requires source flags3; native profile reapplication does not preserve that flag yet".into()));
                }
                let mut colors = vec![rgb(setting, "color1")?];
                match setting.get("color2") {
                    None => {}
                    Some(Value::String(text)) if text == "no-color" => {}
                    Some(Value::String(_)) => {
                        let second = rgb(setting, "color2")?;
                        // The source helper tests numeric color2>0. Keep latent black
                        // explicit until the native codec can preserve that distinction.
                        if second == Rgb(0, 0, 0) {
                            return Err(Error::Unsupported("A retained black second color is not faithfully represented by native effect profiles yet".into()));
                        }
                        colors.push(second);
                    }
                    Some(_) => return Err(Error::InvalidArg("color2 must be a #RRGGBB color or no-color".into())),
                }
                args[3] = colors.len() as u8;
                if id == 7 {
                    let duration = duration(setting)?; args[4] = duration;
                    Some(Effect::Starlight { colors, duration })
                } else { Some(Effect::Breathing(colors)) }
            }
            5 => {
                let color = rgb(setting, "color1")?;
                let duration = duration(setting)?; args[4] = duration;
                Some(Effect::Reactive { color, duration })
            }
            4 => return Err(Error::Unsupported("Source Wave direction/rate is not preserved by the current native profile codec; effect unchanged".into())),
            11 | 12 => return Err(Error::Unsupported(format!("Source effect {id} requires a screen/audio host pipeline that this firmware-only endpoint does not implement"))),
            6 | 8 | 13 | 19 => return Err(Error::Unsupported(format!("Source effect {id} has no implemented durable native projection; its host animation is also unavailable"))),
            _ => return Err(Error::Unsupported(format!("Source effect {id} has no verified durable firmware projection"))),
        };
        let colors = match &effect {
            Some(Effect::Breathing(values)) | Some(Effect::Starlight { colors: values, .. }) => {
                values.clone()
            }
            Some(Effect::Reactive { color, .. }) => vec![*color],
            _ => color.into_iter().collect(),
        };
        args[5] = colors.len() as u8;
        for c in colors {
            args.extend([c.0, c.1, c.2]);
        }
        Ok(Self {
            args,
            color,
            effect,
        })
    }
    fn project(&self, profile: &mut Profile) {
        profile.keys.clear();
        let zone = profile
            .lighting
            .entry("backlight".into())
            .or_insert_with(Zone::default);
        zone.color = self.color;
        zone.effect = self.effect.clone();
        zone.off = false;
        // Retained brightness and switch-off policies are independent of effects.
    }
    fn write_verified(&self, dev: &Device) -> Result<(), Error> {
        // Ordinary transport validation stays strict. An unexpectedly short SET
        // acknowledgement is an error, not permission to retry a physical write.
        // Use exactly the same native writer as Profile::apply. A matching
        // source-region0 GET is required per action, rather than assuming that
        // LED5 aliases region0 because brightness uses such an alias.
        let led = dev.led_id("backlight")?;
        if led != 5
            || !dev
                .spec
                .as_ref()
                .is_some_and(|spec| spec.lighting == secbird_core::db::Lighting::Extended)
        {
            return Err(Error::Unsupported(
                "Source firmware effects require the native extended backlight LED5 projection"
                    .into(),
            ));
        }
        if let Some(effect) = &self.effect {
            dev.set_effect(led, effect)?;
        } else if let Some(color) = self.color {
            dev.set_static_color(led, color)?;
        } else {
            return Err(Error::InvalidArg("Missing native effect projection".into()));
        }
        let read = dev.ornata_effect_readback(1, 0)?;
        if usize::from(read.declared_size()) < self.args.len()
            || !read.selectors_match()
            || read.raw_args()[..self.args.len()] != self.args
        {
            return Err(Error::InvalidArg(
                "Firmware effect readback differs from requested selectors or parameters".into(),
            ));
        }
        Ok(())
    }
}
fn rgb(setting: &Value, key: &str) -> Result<Rgb, Error> {
    let text = setting.get(key).and_then(Value::as_str).unwrap_or("");
    let hex = text
        .strip_prefix('#')
        .filter(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| Error::InvalidArg(format!("{key} must be a #RRGGBB color")))?;
    let value = u32::from_str_radix(hex, 16).unwrap();
    Ok(Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8))
}
fn duration(setting: &Value) -> Result<u8, Error> {
    setting
        .get("duration")
        .and_then(Value::as_u64)
        .filter(|n| (1..=3).contains(n))
        .map(|n| n as u8)
        .ok_or_else(|| Error::InvalidArg("Source duration must be integer1..3".into()))
}

pub(super) fn apply(dev: &Device, store: &mut Store, body: &Body) -> Result<Value, Error> {
    apply_with_owner(dev, store, body, process_owner_gate)
}
fn apply_with_owner(
    dev: &Device,
    store: &mut Store,
    body: &Body,
    gate: impl FnOnce() -> Result<(), Error>,
) -> Result<Value, Error> {
    let id = body
        .source_effect_id
        .ok_or_else(|| Error::InvalidArg("source_effect_id is required".into()))?;
    let setting = body
        .source_effect_setting
        .as_ref()
        .ok_or_else(|| Error::InvalidArg("source_effect_setting is required".into()))?;
    let plan = Plan::parse(id, setting)?;
    let before = store.devices.clone();
    let mut attempted = false;
    let result = (|| {
        let profiles = owner(dev, store, body)?;
        let name = profiles.active.clone();
        let profile = profiles.active_mut();
        secbird_core::profile_compat::set_source_quick_effect(profile, 605, &name, id, setting)?;
        plan.project(profile);
        secbird_core::profile_compat::validate_store_budget(store)?;
        gate()?;
        // Mode GET is read-only; failures before it must never mutate resolution keys.
        let mode = dev.ornata_mode_readback()?;
        if mode.mode() != 0 {
            return Err(Error::Unsupported(format!("Firmware effects require existing Normal_Mode0; current mode is {}. Mode acquisition is not implemented", mode.mode())));
        }
        attempted = true;
        plan.write_verified(dev)?;
        store.save()?;
        Ok(
            json!({"saved":true,"source_effect_id":id,"execution":"firmware","verified":true,"host_animation_parity":false}),
        )
    })();
    if let Err(error) = result {
        store.devices = before;
        if attempted {
            return Err(Error::InvalidArg(format!("Effect command may have changed the hardware; profile was not saved. No automatic write retry was made: {error}")));
        }
        return Err(error);
    }
    result
}

// A native process snapshot per explicit action; never spawn PowerShell. A clear
// known-process check is not proof that WDL or third-party engines are absent.
fn process_owner_gate() -> Result<(), Error> {
    use windows_sys::Win32::{Foundation::*, System::Diagnostics::ToolHelp::*};
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(Error::Unsupported(format!(
                "Lighting owner check failed: {}",
                GetLastError()
            )));
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut names = Vec::new();
        let mut ok = Process32FirstW(snapshot, &mut entry);
        if ok == 0 {
            let code = GetLastError();
            CloseHandle(snapshot);
            return Err(Error::Unsupported(format!(
                "Lighting owner enumeration failed: {code}"
            )));
        }
        while ok != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if crate::lighting_runtime::competing_process_name(&name) {
                names.push(name);
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        let code = GetLastError();
        CloseHandle(snapshot);
        if code != ERROR_NO_MORE_FILES {
            return Err(Error::Unsupported(format!(
                "Lighting owner enumeration incomplete: {code}"
            )));
        }
        if !names.is_empty() {
            return Err(Error::Unsupported(format!(
                "Competing Synapse lighting owner: {}",
                names.join(", ")
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secbird_core::{
        proto::{Command, Report},
        sim::{Sim, KEYBOARD_PID},
        Transport,
    };
    use std::{
        ffi::CString,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::Duration,
    };
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    // Add only the two read commands absent from the shared simulator. SET still
    // passes through its real firmware-effect implementation and log.
    struct Readbacks {
        sim: Sim,
        pending: Mutex<Option<Report>>,
        mode: Arc<Mutex<u8>>,
        wrong: Arc<Mutex<bool>>,
        read_led: Arc<Mutex<u8>>,
        short: Arc<Mutex<bool>>,
    }
    impl Transport for Readbacks {
        fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
            let command = Command::new(data[7], data[8]);
            if command == Command::new(0, 0x84) || command == Command::new(15, 0x82) {
                let mut args = if command.class == 0 {
                    vec![*self.mode.lock().unwrap()]
                } else {
                    let state = self.sim.state();
                    let mut a = vec![data[9], data[10]];
                    a.extend(
                        state
                            .effects
                            .get(&*self.read_led.lock().unwrap())
                            .cloned()
                            .unwrap_or(vec![3, 0, 0, 0]),
                    );
                    a
                };
                if command.class == 15 && *self.wrong.lock().unwrap() {
                    args[2] ^= 1;
                }
                let declared = if command.class == 15 && *self.short.lock().unwrap() {
                    6
                } else {
                    args.len() as u8
                };
                // request() keeps all matching color bytes in padding while the
                // header deliberately declares only6. Those bytes must not count.
                let mut report = Report::request(data[2], command, declared, &args);
                report.0[0] = 2;
                *self.pending.lock().unwrap() = Some(report);
                Ok(())
            } else {
                self.sim.send_feature_report(data)
            }
        }
        fn get_feature_report(&self, data: &mut [u8]) -> secbird_core::Result<usize> {
            if let Some(report) = self.pending.lock().unwrap().take() {
                data[0] = 0;
                data[1..91].copy_from_slice(&report.0);
                Ok(91)
            } else {
                self.sim.get_feature_report(data)
            }
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    struct Fixture {
        sim: Sim,
        dev: Device,
        store: Store,
        folder: std::path::PathBuf,
        mode: Arc<Mutex<u8>>,
        wrong: Arc<Mutex<bool>>,
        read_led: Arc<Mutex<u8>>,
        short: Arc<Mutex<bool>>,
    }
    impl Fixture {
        fn new() -> Self {
            let sim = Sim::keyboard();
            sim.state().txid = 0x1f;
            let mode = Arc::new(Mutex::new(0));
            let wrong = Arc::new(Mutex::new(false));
            let read_led = Arc::new(Mutex::new(5));
            let short = Arc::new(Mutex::new(false));
            let transport = Readbacks {
                sim: sim.clone(),
                pending: Mutex::new(None),
                mode: mode.clone(),
                wrong: wrong.clone(),
                read_led: read_led.clone(),
                short: short.clone(),
            };
            let dev = Device::open(
                Box::new(transport),
                605,
                2,
                CString::new("sim-effects").unwrap(),
                &[0x1f],
                Sim::spec_for(KEYBOARD_PID),
            )
            .unwrap();
            let folder = std::env::temp_dir().join(format!(
                "lighting-effects-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let mut store = Store::load(&folder.join("profiles.json")).unwrap();
            store.for_device(&dev);
            sim.state().log.clear();
            Self {
                sim,
                dev,
                store,
                folder,
                mode,
                wrong,
                read_led,
                short,
            }
        }
        fn body(&self, id: u8, setting: Value) -> Body {
            serde_json::from_value(json!({"id":self.dev.key(),"expected_device":self.dev.key(),"expected_profile":"Default","source_effect_id":id,"source_effect_setting":setting})).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.folder.exists() {
                std::fs::remove_dir_all(&self.folder).unwrap();
            }
        }
    }
    fn apply_fixture(f: &mut Fixture, b: &Body) -> Result<Value, Error> {
        apply_with_owner(&f.dev, &mut f.store, b, || Ok(()))
    }
    #[test]
    fn source_packets_keep_exact_flags_duration_and_color_order() {
        let cases = [
            (
                1,
                json!({"color1":"#123456"}),
                vec![1, 0, 1, 0, 0, 1, 18, 52, 86],
            ),
            (3, json!({}), vec![1, 0, 3, 0, 0, 0]),
            (
                2,
                json!({"isRandom":false,"color1":"#123456","color2":"no-color"}),
                vec![1, 0, 2, 1, 0, 1, 18, 52, 86],
            ),
            (
                2,
                json!({"isRandom":false,"color1":"#123456","color2":"#abcdef"}),
                vec![1, 0, 2, 2, 0, 2, 18, 52, 86, 171, 205, 239],
            ),
            (
                5,
                json!({"color1":"#123456","duration":3}),
                vec![1, 0, 5, 0, 3, 1, 18, 52, 86],
            ),
            (
                7,
                json!({"isRandom":false,"color1":"#123456","color2":"#abcdef","duration":1}),
                vec![1, 0, 7, 2, 1, 2, 18, 52, 86, 171, 205, 239],
            ),
        ];
        for (id, setting, want) in cases {
            let mut f = Fixture::new();
            let b = f.body(id, setting);
            assert_eq!(
                Plan::parse(id, b.source_effect_setting.as_ref().unwrap())
                    .unwrap()
                    .args,
                want
            );
            let response = apply_fixture(&mut f, &b).unwrap();
            assert_eq!(response["verified"], true);
            assert_eq!(f.sim.state().effects[&5], want[2..]);
            assert!(!f.sim.state().effects.contains_key(&0));
            let p = f.store.for_device(&f.dev).active();
            assert!(!p.lighting["backlight"].off);
            assert!(p.keys.is_empty());
            assert_eq!(
                secbird_core::profile_compat::source_quick_effects(p).unwrap()["selectedEffectId"],
                id
            );
            assert_eq!(
                f.sim
                    .state()
                    .log
                    .iter()
                    .filter(|c| **c == secbird_core::commands::MATRIX_EFFECT)
                    .count(),
                1
            );
            // Native application emits the same effect parameters (legacy LED5).
            let p = p.clone();
            f.sim.state().log.clear();
            p.apply(&f.dev);
            assert_eq!(f.sim.state().effects[&5], want[2..]);
        }
    }
    #[test]
    fn unsupported_and_invalid_settings_never_touch_hardware_or_store() {
        let mut f = Fixture::new();
        let before = serde_json::to_value(&f.store).unwrap();
        for (id, setting) in [
            (4, json!({"direction":1})),
            (11, json!({})),
            (12, json!({})),
            (2, json!({"isRandom":true})),
            (7, json!({"isRandom":false,"color1":"#abcdef","duration":4})),
            (1, json!({"color1":"#abcdzz"})),
        ] {
            let b = f.body(id, setting);
            assert!(apply_fixture(&mut f, &b).is_err());
            assert!(f.sim.state().log.is_empty());
            assert_eq!(serde_json::to_value(&f.store).unwrap(), before);
        }
    }
    #[test]
    fn budget_stale_profile_and_ownership_refusals_undo_legacy_key_resolution() {
        for reason in ["stale", "ownership", "budget", "process"] {
            let mut f = Fixture::new();
            let key = f.dev.key();
            let mut profiles = f.store.devices.remove(&key).unwrap();
            if reason == "ownership" {
                profiles.lighting_off = true;
            }
            f.store.devices.insert("025D".into(), profiles);
            if reason == "budget" {
                f.store
                    .extra
                    .insert("large".into(), json!("x".repeat(2 * 1024 * 1024)));
            }
            let mut b = f.body(3, json!({}));
            if reason == "stale" {
                b.expected_profile = Some("old".into());
            }
            let before = serde_json::to_value(&f.store).unwrap();
            let result = apply_with_owner(&f.dev, &mut f.store, &b, || {
                if reason == "process" {
                    Err(Error::Unsupported("competing owner".into()))
                } else {
                    Ok(())
                }
            });
            assert!(result.is_err(), "{reason}");
            assert!(f.sim.state().log.is_empty());
            assert_eq!(serde_json::to_value(&f.store).unwrap(), before, "{reason}");
            assert!(!f.folder.exists());
        }
    }
    #[test]
    fn undeclared_matching_color_padding_cannot_verify_or_save() {
        let mut f = Fixture::new();
        let b = f.body(1, json!({"color1":"#123456"}));
        *f.short.lock().unwrap() = true;
        let before = serde_json::to_value(&f.store).unwrap();
        let error = apply_fixture(&mut f, &b).unwrap_err().to_string();
        assert!(error.contains("may have changed"));
        assert!(error.contains("declared"));
        assert_eq!(f.sim.state().effects[&5], [1, 0, 0, 1, 18, 52, 86]);
        assert_eq!(serde_json::to_value(&f.store).unwrap(), before);
        assert!(!f.folder.exists());
        assert_eq!(
            f.sim
                .state()
                .log
                .iter()
                .filter(|c| **c == secbird_core::commands::MATRIX_EFFECT)
                .count(),
            1
        );
    }
    #[test]
    fn native_led5_write_must_be_observable_in_source_region0_before_save() {
        let mut f = Fixture::new();
        // This adapter deliberately models NO alias: GETregion0 reads only0,
        // while the native writer changes5. Sim cannot prove a physical alias.
        *f.read_led.lock().unwrap() = 0;
        let b = f.body(1, json!({"color1":"#123456"}));
        let before = serde_json::to_value(&f.store).unwrap();
        let error = apply_fixture(&mut f, &b).unwrap_err().to_string();
        assert!(error.contains("may have changed"));
        assert!(error.contains("differs"));
        assert!(f.sim.state().effects.contains_key(&5));
        assert!(!f.sim.state().effects.contains_key(&0));
        assert_eq!(serde_json::to_value(&f.store).unwrap(), before);
        assert!(!f.folder.exists());
    }
    #[test]
    fn mode_is_not_changed_and_failed_verification_is_not_saved_or_retried() {
        let mut f = Fixture::new();
        let b = f.body(3, json!({}));
        *f.mode.lock().unwrap() = 3;
        assert!(apply_fixture(&mut f, &b)
            .unwrap_err()
            .to_string()
            .contains("Normal_Mode0"));
        assert!(f.sim.state().log.is_empty());
        *f.mode.lock().unwrap() = 0;
        *f.wrong.lock().unwrap() = true;
        let before = serde_json::to_value(&f.store).unwrap();
        let error = apply_fixture(&mut f, &b).unwrap_err().to_string();
        assert!(error.contains("may have changed"));
        assert_eq!(serde_json::to_value(&f.store).unwrap(), before);
        assert_eq!(
            f.sim
                .state()
                .log
                .iter()
                .filter(|c| **c == secbird_core::commands::MATRIX_EFFECT)
                .count(),
            1
        );
        assert!(!f.folder.exists());
    }
}

#[cfg(test)]
mod preservation_tests {
    use super::*;
    #[test]
    fn source_effect_edits_retain_brightness_policy_and_unrelated_metadata() {
        let mut profile =
            secbird_core::profile_compat::new_native_profile(605, "Synthetic").unwrap();
        let settings = LightingSettings {
            brightness: Some(Brightness {
                enabled: false,
                value: 73,
                ..Default::default()
            }),
            switch_off: Some(SwitchOffLighting {
                display_off: true,
                idle_enabled: true,
                idle_minutes: 4,
                ..Default::default()
            }),
            ..Default::default()
        };
        settings.store_in_profile(&mut profile).unwrap();
        profile
            .lighting
            .get_mut("backlight")
            .unwrap()
            .extra
            .insert("futureZone".into(), json!({"opaque":1}));
        profile
            .extra
            .insert("futureNative".into(), json!({"opaque":2}));
        let setting = json!({"isRandom":false,"color1":"#123456","color2":"no-color","futureEffect":{"opaque":3}});
        secbird_core::profile_compat::set_source_quick_effect(
            &mut profile,
            605,
            "Synthetic",
            2,
            &setting,
        )
        .unwrap();
        Plan::parse(2, &setting).unwrap().project(&mut profile);
        assert_eq!(profile.lighting_settings.as_ref(), Some(&settings));
        assert_eq!(profile.lighting["backlight"].brightness, Some(0));
        assert!(!profile.lighting["backlight"].off);
        assert_eq!(
            profile.lighting["backlight"].extra["futureZone"]["opaque"],
            1
        );
        assert_eq!(profile.extra["futureNative"]["opaque"], 2);
        assert_eq!(
            secbird_core::profile_compat::source_quick_effects(&profile).unwrap()
                ["selectedEffectSetting"]["futureEffect"]["opaque"],
            3
        );
        for duration in 1..=3 {
            let setting = json!({"duration":duration,"isRandom":false,"color1":"#123456","color2":"no-color"});
            for id in [5, 7] {
                assert_eq!(Plan::parse(id, &setting).unwrap().args[4], duration);
            }
        }
    }
}
