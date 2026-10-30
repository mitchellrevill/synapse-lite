//! Driver3 Spectrum/fixed-Breathing/Wave source actions. The parent owns staging,
//! scope/owner/profile/budget validation and the final save/rollback boundary.
use super::*;
use crate::host_uniform::{ActionTrace, Availability, FrameContent, Playback, Poll, Program};
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) fn apply(
    dev: &mut Device,
    store: &mut Store,
    body: &Body,
    gate: crate::host_static::Gate,
    worker_ready: bool,
    trace: &mut ActionTrace,
) -> Result<Value, Error> {
    let path = crate::native_editor::assets_dir().join("DeviceManifest_605_0.json");
    apply_at_manifest(dev, store, body, gate, worker_ready, trace, &path)
}

fn apply_at_manifest(
    dev: &mut Device,
    store: &mut Store,
    body: &Body,
    gate: crate::host_static::Gate,
    worker_ready: bool,
    trace: &mut ActionTrace,
    manifest_path: &Path,
) -> Result<Value, Error> {
    trace.prepared_wave = None;
    if !worker_ready {
        return Err(Error::Unsupported(
            "Source host effects require the running off-UI worker; no frame was submitted".into(),
        ));
    }
    if matches!(&gate, crate::host_static::Gate::NeedsBaseline { .. }) {
        return Err(Error::InvalidArg(
            "Uniform brightness baseline must be initialized before dispatch".into(),
        ));
    }
    let name = owner(dev, store, body)?.active.clone();
    let program =
        Program::from_profile(owner(dev, store, body)?.active(), dev.pid)?.ok_or_else(|| {
            Error::Unsupported("Selected effect has no supported host projection".into())
        })?;
    trace.requested = Some(program);
    program.opt_in(owner(dev, store, body)?.active_mut())?;
    secbird_core::profile_compat::validate_store_budget(store)?;
    if let Program::Wave(choice) = program {
        trace.prepared_wave = Some(crate::host_wave::preparation::prepare(
            dev,
            choice,
            manifest_path,
        )?);
    }
    let execution = if matches!(program, Program::Wave(_)) {
        "host-wave"
    } else {
        "host-uniform"
    };
    match gate {
        crate::host_static::Gate::Deferred(reason) => {
            crate::host_uniform::io::preflight(dev)?;
            let mut response = json!({"execution":execution,"deferred":true,"rendered":false,"reason":reason,"source_effect_id":program.source_id(),"native_effect_id":program.native_id(),"accepted_packets":0,"attempted_packets":0,"optically_verified":false,"cadence_certified":false});
            if let Some(ready) = trace.prepared_wave.as_ref() {
                response["wave_preparation"] = ready.metadata_at(Instant::now());
            }
            Ok(response)
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
            let wave = matches!(program, Program::Wave(_));
            if !wave {
                trace.output_attempted = true;
            }
            let submission = match ticket.content {
                FrameContent::Uniform(_) => crate::host_uniform::io::submit(
                    dev,
                    ticket,
                    Instant::now() + Duration::from_millis(500),
                    || false,
                ),
                FrameContent::SpatialIndex => {
                    let ready = trace.prepared_wave.as_ref().ok_or_else(|| {
                        Error::InvalidArg(
                            "Spatial Wave has no authenticated prepared layout".into(),
                        )
                    })?;
                    let frame = ready.prepared().render_frame(ticket.palette_index)?;
                    crate::host_uniform::io::submit_prepared(
                        dev,
                        &frame,
                        Instant::now() + Duration::from_millis(500),
                        || false,
                    )
                }
            };
            if wave {
                trace.output_attempted = submission.completion.attempted_packets != 0;
            }
            trace.completion = Some(submission.completion.clone());
            if let Some(error) = &submission.completion.error {
                if wave && !trace.output_attempted {
                    return Err(Error::InvalidArg(format!("Wave preflight refused before any frame OUT; accepted0 attempted0; no retry: {error}")));
                }
                return Err(Error::InvalidArg(format!("Uniform source action accepted{} attempted{} packets; hardware may have changed; no retry: {error}",submission.completion.accepted_packets,submission.completion.attempted_packets)));
            }
            let mut response = json!({"execution":execution,"deferred":false,"frame_submitted":true,"source_effect_id":program.source_id(),"native_effect_id":program.native_id(),"palette_index":ticket.palette_index,"frame_count":playback.frame_count(),"accepted_packets":submission.completion.accepted_packets,"attempted_packets":submission.completion.attempted_packets,"control_state_verified":submission.control_verified,"mode_before":submission.observed_mode,"parameter_before":submission.observed_parameter,"mode_after":submission.mode_after,"parameter_after":submission.parameter_after,"parameter_unchanged":submission.parameter_unchanged(),"optically_verified":false,"cadence_certified":false,"profile_name":name,"nominal_interval_ms":40,"last_submit_us":submission.elapsed.as_micros() as u64,"phase_policy":"local-start-zero-freeze-no-catchup"});
            if let Some(ready) = trace.prepared_wave.as_ref() {
                response["wave_preparation"] = ready.metadata_at(Instant::now());
            }
            Ok(response)
        }
        crate::host_static::Gate::NeedsBaseline { .. } => Err(Error::InvalidArg(
            "Uniform brightness baseline must be initialized before dispatch".into(),
        )),
    }
}
