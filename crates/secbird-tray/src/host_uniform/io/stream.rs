//! Expiring control observations for an already admitted uniform stream.
//! This is a local sampling policy, not an ownership lease. Every batch still
//! uses PreparedOrnataFrame's fresh mode GET and per-packet cancellation.
use super::*;
use serde_json::{json, Value};

const REFRESH_AFTER: Duration = Duration::from_millis(250);
const MAX_ADMISSION_AGE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
struct Observation {
    start: Instant,
    end: Instant,
}

#[derive(Default)]
pub(crate) struct StreamState {
    observation: Option<Observation>,
    checks: u64,
}
impl StreamState {
    pub(crate) fn invalidate(&mut self) {
        self.observation = None;
    }
    fn needs_check(&self, now: Instant) -> bool {
        self.observation
            .is_none_or(|check| now.saturating_duration_since(check.start) >= REFRESH_AFTER)
    }
    pub(crate) fn status(&self) -> Value {
        let now = Instant::now();
        json!({
            "refresh_target_ms": REFRESH_AFTER.as_millis(),
            "maximum_first_packet_admission_age_ms": MAX_ADMISSION_AGE.as_millis(),
            "last_control_check_age_us": self.observation.map(|check| now.saturating_duration_since(check.start).as_micros()),
            "last_control_check_span_us": self.observation.map(|check| check.end.saturating_duration_since(check.start).as_micros()),
            "completed_control_checkpoints": self.checks,
            "fresh_mode_get_each_batch": true,
            "observation_is_ownership_lease": false,
            "freshness_scope": "before-first-OUT-only; ordinary batch deadline/cancellation continues per packet"
        })
    }
    pub(crate) fn submit(
        &mut self,
        device: &mut Device,
        ticket: FrameTicket,
        deadline: Instant,
        cancelled: impl FnMut() -> bool,
    ) -> Submission {
        let frame = match uniform_frame(ticket) {
            Ok(frame) => frame,
            Err(error) => return Submission::refused(error),
        };
        self.submit_frame(device, &frame, deadline, cancelled)
    }
    #[cfg(test)]
    fn submit_with_clock(
        &mut self,
        device: &mut Device,
        ticket: FrameTicket,
        deadline: Instant,
        cancelled: impl FnMut() -> bool,
        now: impl FnMut() -> Instant,
    ) -> Submission {
        let frame = match uniform_frame(ticket) {
            Ok(frame) => frame,
            Err(error) => return Submission::refused(error),
        };
        self.submit_frame_with_clock(device, &frame, deadline, cancelled, now)
    }
    /// The caller supplies physical RGB ordering; this boundary does not choose
    /// a layout or change the existing control sampling and cancellation policy.
    pub(crate) fn submit_frame(
        &mut self,
        device: &mut Device,
        frame: &PreparedOrnataFrame,
        deadline: Instant,
        cancelled: impl FnMut() -> bool,
    ) -> Submission {
        self.submit_frame_with_clock(device, frame, deadline, cancelled, Instant::now)
    }
    fn submit_frame_with_clock(
        &mut self,
        device: &mut Device,
        frame: &PreparedOrnataFrame,
        deadline: Instant,
        mut cancelled: impl FnMut() -> bool,
        mut now: impl FnMut() -> Instant,
    ) -> Submission {
        let started = now();
        let checkpoint = self.needs_check(started);
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
            mode_query_elapsed: Duration::ZERO,
            usb_out_elapsed: Duration::ZERO,
        };
        let run = (|| -> Result<()> {
            if device.pid != 605
                || device.interface != 2
                || !device
                    .spec
                    .as_ref()
                    .and_then(|spec| spec.matrix)
                    .is_some_and(|matrix| matrix.rows == 6 && matrix.cols == 22)
            {
                return Err(Error::Unsupported(
                    "Uniform stream requires Ornata025D/interface2 physical6x22".into(),
                ));
            }
            if cancelled() || now() >= deadline {
                return Err(Error::InvalidArg(
                    "Uniform stream cancelled or deadline expired before control admission".into(),
                ));
            }
            let observed_at = if checkpoint {
                // Conservative timestamp: query start, never response completion.
                let check_started = now();
                custom_frame(device)?;
                check_started
            } else {
                self.observation
                    .expect("non-checkpoint has an observation")
                    .start
            };
            let mut cancellation_checks = 0usize;
            let mut stale_admission = false;
            let outcome = frame.transfer_out(device, deadline, || {
                let user_cancelled = cancelled();
                // Core calls once before mode GET and then before each OUT. Only
                // the first two calls enforce observation freshness. Crossing
                // this age mid-batch must not manufacture a partial frame.
                let stale = cancellation_checks < 2
                    && now().saturating_duration_since(observed_at) > MAX_ADMISSION_AGE;
                cancellation_checks += 1;
                stale_admission |= stale;
                user_cancelled || stale
            })?;
            result.mode_query_elapsed = outcome.mode_query_elapsed;
            result.usb_out_elapsed = outcome.usb_out_elapsed;
            result.completion.accepted_packets = outcome.accepted_packets;
            result.completion.attempted_packets = outcome.attempted_packets;
            if let Some(mode) = &outcome.observed_mode {
                result.observed_mode = Some(mode.mode());
                result.observed_parameter = mode.param();
            }
            if stale_admission {
                return Err(Error::InvalidArg(
                    "Control observation expired before first frame OUT; no write retry".into(),
                ));
            }
            if !outcome.usb_out_complete() {
                return Err(Error::InvalidArg(format!(
                    "Uniform stream batch stopped: {:?}; accepted{} attempted{} OUTs; no retry",
                    outcome.stop, outcome.accepted_packets, outcome.attempted_packets
                )));
            }
            if checkpoint {
                if cancelled() || now() >= deadline {
                    return Err(Error::InvalidArg(
                        "Uniform control checkpoint cancelled or expired after accepted OUTs"
                            .into(),
                    ));
                }
                let mode = device.ornata_mode_readback()?;
                result.mode_after = Some(mode.mode());
                result.parameter_after = mode.param();
                if mode.mode() != 3 {
                    return Err(Error::Unsupported(format!(
                        "Uniform checkpoint mode changed to{}",
                        mode.mode()
                    )));
                }
                if cancelled() || now() >= deadline {
                    return Err(Error::InvalidArg(
                        "Uniform control checkpoint expired before postflight CustomFrame GET"
                            .into(),
                    ));
                }
                custom_frame(device)?;
                let end = now();
                if end.saturating_duration_since(observed_at) > MAX_ADMISSION_AGE {
                    return Err(Error::InvalidArg("Uniform control checkpoint query span exceeded maximum observation age after possible OUTs".into()));
                }
                self.observation = Some(Observation {
                    start: observed_at,
                    end,
                });
                self.checks = self.checks.saturating_add(1);
                result.control_verified = true;
            }
            Ok(())
        })();
        if let Err(error) = run {
            result.completion.error = Some(error.to_string());
            self.invalidate();
        }
        result.elapsed = now().saturating_duration_since(started);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host_static::test_support,
        host_uniform::{Availability, Playback, Poll, Program},
    };
    use secbird_core::proto::Command;
    fn ticket() -> FrameTicket {
        match Playback::new(Program::Spectrum, 1, Duration::ZERO)
            .poll(Duration::ZERO, Availability::Ready)
        {
            Poll::Frame(ticket) => ticket,
            _ => unreachable!(),
        }
    }
    fn scripted_clock(base: Instant, times: &[u64]) -> impl FnMut() -> Instant {
        let mut times = times
            .iter()
            .copied()
            .collect::<std::collections::VecDeque<_>>();
        let mut last = 0;
        move || {
            last = times.pop_front().unwrap_or(last);
            base + Duration::from_millis(last)
        }
    }
    #[test]
    fn checkpoints_are_initial_periodic_and_invalidated_without_removing_fresh_mode() {
        let (mut device, spy) = test_support::fixture();
        let mut state = StreamState::default();
        let base = Instant::now();
        for (time, checkpoint, controls, modes) in
            [(0, true, 2, 2), (100, false, 2, 3), (250, true, 4, 5)]
        {
            let result = state.submit_with_clock(
                &mut device,
                ticket(),
                Instant::now() + Duration::from_secs(2),
                || false,
                || base + Duration::from_millis(time),
            );
            assert!(result.completion.error.is_none());
            assert_eq!(result.control_verified, checkpoint);
            assert_eq!(result.observed_mode, Some(3));
            assert_eq!(result.mode_after, checkpoint.then_some(3));
            let observed = spy.state.lock().unwrap();
            assert_eq!(observed.effects_read, controls);
            assert_eq!(
                observed
                    .commands
                    .iter()
                    .filter(|command| **command == Command::new(0, 0x84))
                    .count(),
                modes
            );
        }
        assert_eq!(state.checks, 2);
        state.invalidate();
        let result = state.submit_with_clock(
            &mut device,
            ticket(),
            Instant::now() + Duration::from_secs(2),
            || false,
            || base + Duration::from_millis(260),
        );
        assert!(result.control_verified);
        assert_eq!(spy.state.lock().unwrap().effects_read, 6);
    }

    #[test]
    fn prepared_spatial_frames_keep_pixels_and_fresh_mode_between_cached_controls() {
        let (mut device, spy) = test_support::fixture();
        let mut state = StreamState::default();
        let base = Instant::now();
        for (time, bias) in [(0, 0), (100, 77)] {
            let pixels: Vec<_> = (0..132)
                .map(|i| secbird_core::Rgb((i + bias) as u8, (i / 22) as u8, (i % 22) as u8))
                .collect();
            let frame = PreparedOrnataFrame::new(&pixels).unwrap();
            let result = state.submit_frame_with_clock(
                &mut device,
                &frame,
                Instant::now() + Duration::from_secs(2),
                || false,
                || base + Duration::from_millis(time),
            );
            assert!(result.completion.error.is_none());
            assert_eq!(
                (
                    result.completion.accepted_packets,
                    result.completion.attempted_packets
                ),
                (7, 7)
            );
            assert_eq!(result.control_verified, time == 0);
            assert_eq!(result.observed_mode, Some(3));
            assert_eq!(
                spy.sim.state().matrix.as_ref().unwrap().2,
                pixels.iter().map(|c| [c.0, c.1, c.2]).collect::<Vec<_>>()
            );
        }
        let observed = spy.state.lock().unwrap();
        assert_eq!(observed.attempts, 14);
        assert_eq!(observed.effects_read, 2);
        assert_eq!(
            observed
                .commands
                .iter()
                .filter(|command| **command == Command::new(0, 0x84))
                .count(),
            3
        );
    }
    #[test]
    fn first_out_admission_boundary_is_exact_and_never_a_mid_batch_ttl() {
        for (age, accepted) in [(500, 7), (501, 0)] {
            let (mut device, spy) = test_support::fixture();
            let base = Instant::now();
            let mut state = StreamState {
                observation: Some(Observation {
                    start: base,
                    end: base,
                }),
                checks: 1,
            };
            // Start below the250ms refresh target. Mode GET can consume the
            // safety margin; only first-OUT admission checks its resulting age.
            let result = state.submit_with_clock(
                &mut device,
                ticket(),
                Instant::now() + Duration::from_secs(2),
                || false,
                scripted_clock(base, &[240, 240, 240, age, 800]),
            );
            assert_eq!(result.completion.accepted_packets, accepted);
            assert_eq!(result.completion.attempted_packets, accepted);
            assert_eq!(spy.state.lock().unwrap().attempts, accepted);
            assert_eq!(result.observed_mode, Some(3));
            assert_eq!(result.completion.error.is_some(), age > 500);
            if age > 500 {
                assert!(state.observation.is_none());
            }
        }
    }
    #[test]
    fn slow_initial_mode_query_refuses_before_out_and_slow_postcheck_keeps_real_counts() {
        for (times, accepted) in [
            (&[0, 0, 0, 0, 501, 501][..], 0),
            (&[0, 0, 0, 0, 0, 0, 0, 501, 501][..], 7),
        ] {
            let (mut device, spy) = test_support::fixture();
            let mut state = StreamState::default();
            let result = state.submit_with_clock(
                &mut device,
                ticket(),
                Instant::now() + Duration::from_secs(2),
                || false,
                scripted_clock(Instant::now(), times),
            );
            assert!(result.completion.error.is_some());
            assert_eq!(result.completion.accepted_packets, accepted);
            assert_eq!(spy.state.lock().unwrap().attempts, accepted);
            assert!(!result.control_verified);
            assert!(state.observation.is_none());
        }
    }
    #[test]
    fn wrong_scope_cancel_control_change_and_partial_output_never_reuse_an_observation() {
        for failure in ["scope", "cancel", "control", "partial"] {
            let (mut device, spy) = test_support::fixture();
            let mut state = StreamState::default();
            if failure == "scope" {
                device.interface = 0;
            }
            if failure == "control" {
                spy.sim.state().effects.insert(0, vec![3, 0, 0, 0]);
            }
            if failure == "partial" {
                spy.state.lock().unwrap().fail_at = Some(2);
            }
            let result = state.submit(
                &mut device,
                ticket(),
                Instant::now() + Duration::from_secs(2),
                || failure == "cancel",
            );
            assert!(result.completion.error.is_some());
            assert_eq!(
                result.completion.accepted_packets,
                if failure == "partial" { 2 } else { 0 }
            );
            assert_eq!(
                result.completion.attempted_packets,
                if failure == "partial" { 3 } else { 0 }
            );
            assert!(state.observation.is_none());
            if failure == "scope" || failure == "cancel" {
                assert!(spy.state.lock().unwrap().commands.is_empty());
            }
        }
    }
}
