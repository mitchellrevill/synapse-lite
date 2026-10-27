//! Driver3 Spectrum/fixed-Breathing source actions. The parent owns staging,
//! scope/owner/profile/budget validation and the final save/rollback boundary.
use super::*;
use crate::host_uniform::{ActionTrace, Availability, Playback, Poll, Program};
use std::time::{Duration, Instant};

pub(super) fn apply(
    dev: &mut Device,
    store: &mut Store,
    body: &Body,
    gate: crate::host_static::Gate,
    worker_ready: bool,
    trace: &mut ActionTrace,
) -> Result<Value, Error> {
    if !worker_ready {
        return Err(Error::Unsupported("Spectrum/fixed Breathing requires the running off-UI uniform worker; no frame was submitted".into()));
    }
    let name = owner(dev, store, body)?.active.clone();
    let program =
        Program::from_profile(owner(dev, store, body)?.active(), dev.pid)?.ok_or_else(|| {
            Error::Unsupported("Selected effect has no fixed uniform host projection".into())
        })?;
    trace.requested = Some(program);
    program.opt_in(owner(dev, store, body)?.active_mut())?;
    secbird_core::profile_compat::validate_store_budget(store)?;
    match gate {
        crate::host_static::Gate::Deferred(reason) => {
            crate::host_uniform::io::preflight(dev)?;
            Ok(
                json!({"execution":"host-uniform","deferred":true,"rendered":false,"reason":reason,"source_effect_id":program.source_id(),"native_effect_id":program.native_id(),"accepted_packets":0,"attempted_packets":0,"optically_verified":false,"cadence_certified":false}),
            )
        }
        crate::host_static::Gate::Ready => {
            let mut playback = Playback::new(program, 0, Duration::ZERO);
            let ticket = match playback.poll(Duration::ZERO, Availability::Ready) {
                Poll::Frame(ticket) => ticket,
                _ => {
                    return Err(Error::InvalidArg(
                        "Uniform initial frame was not ready".into(),
                    ))
                }
            };
            trace.output_attempted = true;
            let submission = crate::host_uniform::io::submit(
                dev,
                ticket,
                Instant::now() + Duration::from_millis(500),
                || false,
            );
            trace.completion = Some(submission.completion.clone());
            if let Some(error) = &submission.completion.error {
                return Err(Error::InvalidArg(format!("Uniform source action accepted{} attempted{} packets; hardware may have changed; no retry: {error}",submission.completion.accepted_packets,submission.completion.attempted_packets)));
            }
            Ok(
                json!({"execution":"host-uniform","deferred":false,"frame_submitted":true,"source_effect_id":program.source_id(),"native_effect_id":program.native_id(),"palette_index":0,"frame_count":playback.frame_count(),"accepted_packets":submission.completion.accepted_packets,"attempted_packets":submission.completion.attempted_packets,"control_state_verified":submission.control_verified,"mode_before":submission.observed_mode,"parameter_before":submission.observed_parameter,"mode_after":submission.mode_after,"parameter_after":submission.parameter_after,"parameter_unchanged":submission.parameter_unchanged(),"optically_verified":false,"cadence_certified":false,"profile_name":name,"nominal_interval_ms":40,"last_submit_us":submission.elapsed.as_micros() as u64,"phase_policy":"local-start-zero-freeze-no-catchup"}),
            )
        }
        crate::host_static::Gate::NeedsBaseline { .. } => Err(Error::InvalidArg(
            "Uniform brightness baseline must be initialized before dispatch".into(),
        )),
    }
}
