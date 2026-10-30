//! Uniform Static in an already-active Ornata Driver_Mode3/CustomFrame8 path.
//! No mode, brightness, profile1 effect, input or macro writes. Process ownership
//! and suppression belong to the Engine/action gate; this helper adds fresh device
//! preflight and reports USB OUT/control registers, never optical verification.
use secbird_core::{ornata_frame::PreparedOrnataFrame, Device, Error, Result, Rgb};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum Gate {
    Ready,
    Deferred(String),
    NeedsBaseline {
        environment: crate::lighting_runtime::Environment,
        away_percent: Option<u8>,
    },
}
impl Gate {
    pub(crate) fn with_brightness(
        self,
        name: &str,
        profile: &secbird_core::Profile,
    ) -> Result<Self> {
        let Self::NeedsBaseline {
            environment,
            away_percent,
        } = self
        else {
            return Ok(self);
        };
        let config =
            crate::lighting_runtime::Config::from_profile(name, profile, true, away_percent);
        let decision = crate::lighting_runtime::plan(&config, &environment, true, false);
        if let Some(reason) = decision.blocked {
            return Err(Error::Unsupported(format!(
                "Host Static baseline policy blocked: {reason}"
            )));
        }
        if !config.source_known {
            return Err(Error::InvalidArg(
                "Host Static brightness baseline is missing".into(),
            ));
        }
        if !config.enabled
            || decision.suppressed
            || decision.target == Some(0)
            || away_percent.is_some()
        {
            Ok(Self::Deferred(
                "hardware baseline/policy/global-away suppression".into(),
            ))
        } else {
            Ok(Self::Ready)
        }
    }
}

fn custom_frame(device: &Device) -> Result<()> {
    let read = device.ornata_effect_readback(0, 0)?;
    if !read.selectors_match()
        || read.declared_size() < 6
        || read.effect_id() != 8
        || read.flags() != 0
        || read.rate() != 0
        || read.color_count() != 0
    {
        return Err(Error::Unsupported("Host Static requires existing profile0/region0 CustomFrame8 flags0/rate0/count0; no mode/effect acquisition was attempted".into()));
    }
    Ok(())
}
pub(crate) fn preflight(device: &Device) -> Result<()> {
    if device.pid != 605
        || device.interface != 2
        || !device
            .spec
            .as_ref()
            .and_then(|s| s.matrix)
            .is_some_and(|m| m.rows == 6 && m.cols == 22)
    {
        return Err(Error::Unsupported(
            "Host Static requires exact Ornata interface2 physical6x22 matrix".into(),
        ));
    }
    let mode = device.ornata_mode_readback()?;
    if mode.mode() != 3 {
        return Err(Error::Unsupported(format!(
            "Host Static requires existing Driver_Mode3; observed{}",
            mode.mode()
        )));
    }
    custom_frame(device)
}
pub(crate) fn send_uniform(device: &mut Device, color: Rgb) -> Result<Value> {
    preflight(device)?;
    let frame = PreparedOrnataFrame::new(&[color; 132])?;
    let outcome =
        frame.transfer_out(device, Instant::now() + Duration::from_millis(500), || {
            false
        })?;
    if !outcome.usb_out_complete() {
        return Err(Error::InvalidArg(format!("Host Static stopped: accepted{} attempted{} packets; hardware may have changed; no retry: {:?}",outcome.accepted_packets,outcome.attempted_packets,outcome.stop)));
    }
    let after = (|| {
        let mode = device.ornata_mode_readback()?;
        if mode.mode() != 3 {
            return Err(Error::Unsupported(format!(
                "Host Static postflight mode changed to{}",
                mode.mode()
            )));
        }
        custom_frame(device)?;
        Ok(mode)
    })();
    let mode=after.map_err(|error|Error::InvalidArg(format!("Host Static USB OUT accepted7 attempted7 but control-register verification failed; hardware may have changed; no retry: {error}")))?;
    let before = outcome
        .observed_mode
        .as_ref()
        .expect("completed batch has a mode preflight");
    let parameter_unchanged = match (before.param(), mode.param()) {
        (Some(a), Some(b)) => Some(a == b),
        _ => None,
    };
    Ok(
        json!({"execution":"host-static","deferred":false,"frame_submitted":true,"accepted_packets":outcome.accepted_packets,"attempted_packets":outcome.attempted_packets,"usb_out_complete":true,"control_state_verified":true,"optically_verified":false,"mode":mode.mode(),"mode_parameter":mode.param(),"mode_before":{"value":before.mode(),"parameter":before.param(),"declared_size":before.declared_size()},"mode_after":{"value":mode.mode(),"parameter":mode.param(),"declared_size":mode.declared_size()},"mode_parameter_unchanged":parameter_unchanged,"profile":0,"region":0,"effect":8,"host_animation_parity":false}),
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use secbird_core::{
        proto::{Command, Report},
        sim::{Sim, KEYBOARD_PID},
        Transport,
    };
    use std::{
        ffi::CString,
        sync::{Arc, Mutex},
    };
    pub(crate) struct State {
        pub mode: u8,
        pub fail_at: Option<usize>,
        pub attempts: usize,
        pub effects_read: usize,
        pub corrupt_post: bool,
        pub fail_brightness: bool,
        pub commands: Vec<Command>,
        pending: Option<Report>,
    }
    #[derive(Clone)]
    pub(crate) struct Spy {
        pub sim: Sim,
        pub state: Arc<Mutex<State>>,
    }
    impl Transport for Spy {
        fn send_feature_report(&self, data: &[u8]) -> secbird_core::Result<()> {
            let command = Command::new(data[7], data[8]);
            let mut state = self.state.lock().unwrap();
            state.commands.push(command);
            if command == Command::new(0, 0x84)
                || command == Command::new(15, 0x82)
                || command == Command::new(15, 0x84) && data[10] == 0
            {
                let mut args = if command.class == 0 {
                    vec![state.mode]
                } else if command.id == 0x84 {
                    if state.fail_brightness {
                        return Err(Error::System(
                            "Injected brightness baseline read failure".into(),
                        ));
                    }
                    let sim = self.sim.state();
                    let map = if data[9] == 0 {
                        &sim.live_brightness
                    } else {
                        &sim.brightness
                    };
                    vec![data[9], data[10], map.get(&5).copied().unwrap_or(255)]
                } else {
                    state.effects_read += 1;
                    let mut args = vec![data[9], data[10]];
                    args.extend(
                        self.sim
                            .state()
                            .effects
                            .get(&data[10])
                            .cloned()
                            .unwrap_or(vec![3, 0, 0, 0]),
                    );
                    args
                };
                if command.class == 15
                    && command.id == 0x82
                    && state.corrupt_post
                    && state.effects_read >= 2
                {
                    args[2] = 7;
                }
                let mut reply = Report::request(data[2], command, args.len() as u8, &args);
                reply.0[0] = 2;
                state.pending = Some(reply);
                Ok(())
            } else {
                if command.class == 15 && (command.id == 3 || (command.id == 2 && data[9] == 0)) {
                    let index = state.attempts;
                    state.attempts += 1;
                    if state.fail_at == Some(index) {
                        return Err(Error::System("Injected host frame OUT failure".into()));
                    }
                }
                drop(state);
                self.sim.send_feature_report(data)
            }
        }
        fn get_feature_report(&self, data: &mut [u8]) -> secbird_core::Result<usize> {
            if let Some(reply) = self.state.lock().unwrap().pending.take() {
                data[0] = 0;
                data[1..91].copy_from_slice(&reply.0);
                Ok(91)
            } else {
                self.sim.get_feature_report(data)
            }
        }
        fn read_delay(&self) -> Duration {
            Duration::ZERO
        }
    }
    pub(crate) fn fixture() -> (Device, Spy) {
        let sim = Sim::keyboard();
        sim.state().txid = 0x1f;
        sim.state().effects.insert(0, vec![8, 0, 0, 0]);
        let spy = Spy {
            sim,
            state: Arc::new(Mutex::new(State {
                mode: 3,
                fail_at: None,
                attempts: 0,
                effects_read: 0,
                corrupt_post: false,
                fail_brightness: false,
                commands: Vec::new(),
                pending: None,
            })),
        };
        let device = Device::open(
            Box::new(spy.clone()),
            605,
            2,
            CString::new("sim-host-static").unwrap(),
            &[0x1f],
            Sim::spec_for(KEYBOARD_PID),
        )
        .unwrap();
        spy.state.lock().unwrap().commands.clear();
        spy.sim.state().log.clear();
        (device, spy)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uniform_frame_uses_only_profile0_and_reports_control_state_not_optical() {
        let (mut device, spy) = test_support::fixture();
        let response = send_uniform(&mut device, Rgb(18, 52, 86)).unwrap();
        assert_eq!(response["accepted_packets"], 7);
        assert_eq!(response["control_state_verified"], true);
        assert_eq!(response["optically_verified"], false);
        assert!(response["mode_parameter"].is_null());
        assert!(response["mode_before"]["parameter"].is_null());
        assert!(response["mode_after"]["parameter"].is_null());
        assert!(response["mode_parameter_unchanged"].is_null());
        let state = spy.sim.state();
        assert!(state
            .matrix
            .as_ref()
            .unwrap()
            .2
            .iter()
            .all(|rgb| *rgb == [18, 52, 86]));
        assert_eq!(state.effects[&0], [8, 0, 0, 0]);
        assert!(!state.effects.contains_key(&5));
        assert!(!state
            .log
            .contains(&secbird_core::commands::MATRIX_BRIGHTNESS));
    }
    #[test]
    fn partial_out_and_postflight_failure_are_explicit_and_never_retried() {
        let (mut device, spy) = test_support::fixture();
        spy.state.lock().unwrap().fail_at = Some(2);
        let error = send_uniform(&mut device, Rgb(1, 2, 3))
            .unwrap_err()
            .to_string();
        assert!(error.contains("accepted2 attempted3"));
        assert_eq!(spy.state.lock().unwrap().attempts, 3);
        let (mut device, spy) = test_support::fixture();
        spy.state.lock().unwrap().corrupt_post = true;
        let error = send_uniform(&mut device, Rgb(1, 2, 3))
            .unwrap_err()
            .to_string();
        assert!(error.contains("accepted7 attempted7"));
        assert_eq!(spy.state.lock().unwrap().attempts, 7);
    }
    #[test]
    fn wrong_scope_or_noncustom_preflight_never_sends_a_frame() {
        for wrong in [0, 1, 2] {
            let (mut device, spy) = test_support::fixture();
            match wrong {
                0 => device.interface = 0,
                1 => device.spec.as_mut().unwrap().matrix.as_mut().unwrap().cols = 21,
                _ => {
                    spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
                }
            }
            assert!(send_uniform(&mut device, Rgb(1, 2, 3)).is_err());
            assert_eq!(spy.state.lock().unwrap().attempts, 0);
            if wrong < 2 {
                assert!(spy.state.lock().unwrap().commands.is_empty());
            }
        }
    }
}
