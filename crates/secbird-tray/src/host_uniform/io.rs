//! One bounded physical uniform frame. No ownership lease, profile persistence,
//! brightness, mode SET, input or macro operation. Caller checks owner/profile and
//! policy immediately before calling; each batch still freshly checks mode3.
//! Deadline/cancellation are cooperative. Postflight GETs can extend the nominal
//! batch budget and no in-flight HID syscall can be interrupted here.
use super::{Completion, FrameTicket};
use secbird_core::{ornata_frame::PreparedOrnataFrame, Device, Error, Result};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct Submission {
    pub completion: Completion,
    pub control_verified: bool,
    pub observed_mode: Option<u8>,
    pub observed_parameter: Option<u8>,
    pub mode_after: Option<u8>,
    pub parameter_after: Option<u8>,
    pub elapsed: Duration,
}
impl Submission {
    pub(crate) fn parameter_unchanged(&self) -> Option<bool> {
        match (self.observed_parameter, self.parameter_after) {
            (Some(before), Some(after)) => Some(before == after),
            _ => None,
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
        return Err(Error::Unsupported("Uniform playback requires existing profile0/region0 CustomFrame8 flags0/rate0/count0; no acquisition was attempted".into()));
    }
    Ok(())
}
/// Read-only admission for a deferred source choice; no frame is submitted.
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
            "Uniform playback requires Ornata025D/interface2 physical6x22".into(),
        ));
    }
    let mode = device.ornata_mode_readback()?;
    if mode.mode() != 3 {
        return Err(Error::Unsupported(format!(
            "Uniform playback requires existing Driver_Mode3, observed{}",
            mode.mode()
        )));
    }
    custom_frame(device)
}

pub(crate) fn submit(
    device: &mut Device,
    ticket: FrameTicket,
    deadline: Instant,
    mut cancelled: impl FnMut() -> bool,
) -> Submission {
    let started = Instant::now();
    let mut result = Submission {
        completion: Completion {
            accepted_packets: 0,
            attempted_packets: 0,
            error: None,
        },
        control_verified: false,
        observed_mode: None,
        observed_parameter: None,
        mode_after: None,
        parameter_after: None,
        elapsed: Duration::ZERO,
    };
    let preflight = (|| {
        if device.pid != 605
            || device.interface != 2
            || !device
                .spec
                .as_ref()
                .and_then(|s| s.matrix)
                .is_some_and(|m| m.rows == 6 && m.cols == 22)
        {
            return Err(Error::Unsupported(
                "Uniform frame requires Ornata025D/interface2 physical6x22".into(),
            ));
        }
        if cancelled() {
            return Err(Error::InvalidArg(
                "Uniform frame cancelled before preflight".into(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(Error::InvalidArg(
                "Uniform frame deadline before preflight".into(),
            ));
        }
        custom_frame(device)
    })();
    if let Err(error) = preflight {
        result.completion.error = Some(error.to_string());
        result.elapsed = started.elapsed();
        return result;
    }
    let frame =
        PreparedOrnataFrame::new(&[ticket.color; 132]).expect("fixed physical frame geometry");
    match frame.transfer_out(device, deadline, &mut cancelled) {
        Ok(outcome) => {
            result.completion.accepted_packets = outcome.accepted_packets;
            result.completion.attempted_packets = outcome.attempted_packets;
            if let Some(mode) = &outcome.observed_mode {
                result.observed_mode = Some(mode.mode());
                result.observed_parameter = mode.param();
            }
            if outcome.usb_out_complete() {
                // Check only the control register, not nonexistent RGB readback.
                let postflight = (|| {
                    if cancelled() {
                        return Err(Error::InvalidArg(
                            "Uniform postflight cancelled; accepted OUTs may already be visible"
                                .into(),
                        ));
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::InvalidArg(
                            "Uniform deadline before postflight mode query".into(),
                        ));
                    }
                    let mode = device.ornata_mode_readback()?;
                    result.mode_after = Some(mode.mode());
                    result.parameter_after = mode.param();
                    if mode.mode() != 3 {
                        return Err(Error::Unsupported(format!(
                            "Uniform postflight mode changed to{}",
                            mode.mode()
                        )));
                    }
                    if cancelled() {
                        return Err(Error::InvalidArg(
                            "Uniform postflight cancelled before control query".into(),
                        ));
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::InvalidArg(
                            "Uniform deadline before postflight control query".into(),
                        ));
                    }
                    custom_frame(device)
                })();
                match postflight {
                    Ok(())=>result.control_verified=true,
                    Err(error)=>result.completion.error=Some(format!("Uniform OUT accepted7 attempted7 but postflight mode/CustomFrame control state differs: {error}")),
                }
            } else {
                result.completion.error = Some(format!(
                    "Uniform batch stopped: {:?}; a failed/partial OUT may have changed hardware",
                    outcome.stop
                ));
            }
        }
        Err(error) => result.completion.error = Some(error.to_string()),
    }
    result.elapsed = started.elapsed();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host_static::test_support,
        host_uniform::{Availability, Playback, Poll, Program},
    };
    fn ticket() -> FrameTicket {
        let mut p = Playback::new(Program::Spectrum, 1, Duration::ZERO);
        match p.poll(Duration::ZERO, Availability::Ready) {
            Poll::Frame(t) => t,
            _ => unreachable!(),
        }
    }
    #[test]
    fn accepted_uniform_frame_has_only_profile0_outs_and_control_not_optical_verification() {
        let (mut device, spy) = test_support::fixture();
        let submission = submit(
            &mut device,
            ticket(),
            Instant::now() + Duration::from_secs(1),
            || false,
        );
        assert_eq!(
            (
                submission.completion.accepted_packets,
                submission.completion.attempted_packets
            ),
            (7, 7)
        );
        assert!(submission.completion.error.is_none());
        assert!(submission.control_verified);
        assert_eq!(submission.observed_mode, Some(3));
        assert_eq!(submission.observed_parameter, None);
        assert_eq!(submission.mode_after, Some(3));
        assert_eq!(submission.parameter_after, None);
        assert_eq!(submission.parameter_unchanged(), None);
        let state = spy.sim.state();
        assert!(state
            .matrix
            .as_ref()
            .unwrap()
            .2
            .iter()
            .all(|rgb| *rgb == [255, 24, 255]));
        assert_eq!(state.effects[&0], [8, 0, 0, 0]);
        assert!(!state.effects.contains_key(&5));
        assert!(!state
            .log
            .contains(&secbird_core::commands::MATRIX_BRIGHTNESS));
    }
    #[test]
    fn partial_out_and_changed_control_keep_counts_without_retry() {
        let (mut device, spy) = test_support::fixture();
        spy.state.lock().unwrap().fail_at = Some(2);
        let submission = submit(
            &mut device,
            ticket(),
            Instant::now() + Duration::from_secs(1),
            || false,
        );
        assert_eq!(
            (
                submission.completion.accepted_packets,
                submission.completion.attempted_packets
            ),
            (2, 3)
        );
        assert!(submission.completion.error.is_some());
        assert!(!submission.control_verified);
        assert_eq!(spy.state.lock().unwrap().attempts, 3);
        let (mut device, spy) = test_support::fixture();
        spy.state.lock().unwrap().corrupt_post = true;
        let submission = submit(
            &mut device,
            ticket(),
            Instant::now() + Duration::from_secs(1),
            || false,
        );
        assert_eq!(submission.completion.accepted_packets, 7);
        assert!(submission
            .completion
            .error
            .as_ref()
            .unwrap()
            .contains("postflight"));
        assert_eq!(spy.state.lock().unwrap().attempts, 7);
    }
    #[test]
    fn scope_cancellation_deadline_and_noncustom_do_no_out() {
        for reason in ["scope", "cancel", "deadline", "control"] {
            let (mut device, spy) = test_support::fixture();
            if reason == "scope" {
                device.interface = 0;
            }
            if reason == "control" {
                spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
            }
            let deadline = if reason == "deadline" {
                Instant::now()
            } else {
                Instant::now() + Duration::from_secs(1)
            };
            let submission = submit(&mut device, ticket(), deadline, || reason == "cancel");
            assert_eq!(submission.completion.attempted_packets, 0);
            assert!(submission.completion.error.is_some());
            assert_eq!(spy.state.lock().unwrap().attempts, 0);
            if reason != "control" {
                assert!(spy.state.lock().unwrap().commands.is_empty());
            }
        }
    }
}
