//! One explicitly applied Studio composition, separate from draft preview.
//!
//! The owner calls this synchronously under its existing device/worker lock.
//! Stop admission uses atomics and needs neither that lock nor a device query.
//! An in-progress syscall is not interruptible; its completion must still drain.
//! This component does not acquire device mode or interprocess ownership.
use super::{preparation, PreparedStudio};
use crate::host_uniform::{io::StreamState, Availability, Completion};
use secbird_core::{
    studio::{document::Draft, Archive},
    Device, Error, Result,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};

#[derive(Debug, Default)]
struct Commands {
    latest: AtomicU64,
    stop_epoch: AtomicU64,
    exhausted: AtomicBool,
}

/// Clone this before taking the engine lock. Requesting stop never waits on it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Control(Arc<Commands>);
impl PartialEq for Control {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Control {}
pub(crate) struct ApplyIntent {
    owner: Control,
    sequence: u64,
    stop_epoch: u64,
}
impl Control {
    fn increment(&self, counter: &AtomicU64) -> Result<u64> {
        if self.0.exhausted.load(Ordering::Acquire) {
            return Err(invalid("Studio command counters exhausted"));
        }
        match counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1)
        }) {
            Ok(previous) => Ok(previous + 1),
            Err(_) => {
                self.0.exhausted.store(true, Ordering::Release);
                Err(invalid("Studio command counters exhausted"))
            }
        }
    }
    pub(crate) fn apply_intent(&self) -> Result<ApplyIntent> {
        let sequence = self.increment(&self.0.latest)?;
        Ok(ApplyIntent {
            owner: self.clone(),
            sequence,
            stop_epoch: self.0.stop_epoch.load(Ordering::Acquire),
        })
    }
    pub(crate) fn request_stop(&self) -> Result<()> {
        self.increment(&self.0.stop_epoch).map(|_| ())
    }
    fn current(&self, intent: &ApplyIntent) -> bool {
        Arc::ptr_eq(&self.0, &intent.owner.0)
            && !self.cancelled(intent.stop_epoch)
            && self.0.latest.load(Ordering::Acquire) == intent.sequence
    }
    fn cancelled(&self, epoch: u64) -> bool {
        self.0.exhausted.load(Ordering::Acquire)
            || self.0.stop_epoch.load(Ordering::Acquire) != epoch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ticket {
    owner: Control,
    generation: u64,
    sequence: u64,
    frame_index: u64,
    started: Duration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Poll {
    Frame(Ticket),
    Wait(Duration),
    Suppressed,
    Blocked,
    InFlight,
    Draining,
    Faulted,
    Stopped,
}
struct Active {
    key: String,
    path: Vec<u8>,
    connection_epoch: u64,
    composition_id: String,
    revision: String,
    intent: ApplyIntent,
    prepared: PreparedStudio,
    controls: StreamState,
    pending: Option<Ticket>,
    transfer_started: bool,
    next_sequence: u64,
    next_due: Option<Duration>,
    accepted_frames: u64,
    failure: Option<Completion>,
}

#[derive(Default)]
pub(crate) struct OutputSession {
    control: Control,
    last_applied: u64,
    active: Option<Active>,
}
impl OutputSession {
    /// Numeric simulator fixture only; production apply must authenticate assets.
    #[cfg(test)]
    pub(crate) fn synthetic_for_test(device: &Device, connection_epoch: u64) -> Self {
        let draft = Draft::open(include_str!(
            "../../../secbird-core/tests/fixtures/studio-preview-profile.json"
        ))
        .unwrap();
        let prepared =
            PreparedStudio::build(draft.read_snapshot(), crate::host_studio::tests::map(), 1)
                .unwrap();
        let mut session = Self::default();
        session
            .activate(
                session.control().apply_intent().unwrap(),
                device,
                connection_epoch,
                prepared,
            )
            .unwrap();
        session
    }
    pub(crate) fn control(&self) -> Control {
        self.control.clone()
    }

    /// Resolve the saved archive entry, never an editor's mutable Draft. Failed
    /// compilation or a superseded intent leaves the old applied scene intact.
    /// Caller supplies the current handle-generation and has checked ownership.
    // Keep saved-entry identity, authenticated assets, ownership epoch and source RNG explicit.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_saved(
        &mut self,
        intent: ApplyIntent,
        archive: &Archive,
        index: usize,
        device: &Device,
        manifest: &Path,
        connection_epoch: u64,
        random_state: u32,
    ) -> Result<()> {
        self.check_apply(&intent)?;
        let draft = Draft::open(
            archive
                .profile_json(index)
                .ok_or_else(|| invalid("Studio saved profile index is absent"))?,
        )?;
        let ready = preparation::prepare(device, draft.read_snapshot(), manifest, random_state)?;
        self.activate(intent, device, connection_epoch, ready.into_prepared())
    }
    fn check_apply(&self, intent: &ApplyIntent) -> Result<()> {
        if !self.control.current(intent) || intent.sequence <= self.last_applied {
            return Err(invalid(
                "Studio apply was stopped, superseded or already consumed",
            ));
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.pending.is_some())
        {
            return Err(invalid(
                "Studio apply must wait for the previous batch to drain",
            ));
        }
        Ok(())
    }
    fn activate(
        &mut self,
        intent: ApplyIntent,
        device: &Device,
        connection_epoch: u64,
        prepared: PreparedStudio,
    ) -> Result<()> {
        self.check_apply(&intent)?;
        let composition: secbird_core::studio::ProfileHeader =
            serde_json::from_str(prepared.snapshot.json())
                .map_err(|error| invalid(&error.to_string()))?;
        let id = composition
            .id
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("Studio saved composition has no id"))?;
        let revision = digest(prepared.snapshot.json().as_bytes())?;
        // Compilation and hashing can take time. Stop/supersession must be
        // rechecked immediately before publishing the replacement generation.
        self.check_apply(&intent)?;
        self.last_applied = intent.sequence;
        self.active = Some(Active {
            key: device.key(),
            path: device.path.as_bytes().to_vec(),
            connection_epoch,
            composition_id: id,
            revision,
            intent,
            prepared,
            controls: StreamState::default(),
            pending: None,
            transfer_started: false,
            next_sequence: 0,
            next_due: None,
            accepted_frames: 0,
            failure: None,
        });
        Ok(())
    }
    /// Holds only the selected target, including faults and stop/drain. Caller
    /// must inhibit competing Quick writers until poll acknowledges Stopped.
    pub(crate) fn holds_target(&self, key: &str) -> bool {
        self.active.as_ref().is_some_and(|active| active.key == key)
    }
    pub(crate) fn target_key(&self) -> Option<&str> {
        self.active.as_ref().map(|active| active.key.as_str())
    }
    pub(crate) fn stop_requested(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| self.control.cancelled(active.intent.stop_epoch))
    }
    pub(crate) fn is_faulted(&self) -> bool {
        self.active.as_ref().is_some_and(|active|active.failure.is_some())
    }

    pub(crate) fn poll(&mut self, now: Duration, availability: Availability) -> Poll {
        let Some(active) = &mut self.active else {
            return Poll::Stopped;
        };
        if self.control.cancelled(active.intent.stop_epoch) {
            if active.pending.is_some() {
                return Poll::Draining;
            }
            self.active = None;
            return Poll::Stopped;
        }
        if active.pending.is_some() {
            return Poll::InFlight;
        }
        if active.failure.is_some() {
            return Poll::Faulted;
        }
        if availability != Availability::Ready {
            active.next_due = None;
            return if availability == Availability::Suppressed {
                Poll::Suppressed
            } else {
                Poll::Blocked
            };
        }
        if let Some(due) = active.next_due {
            if now < due {
                return Poll::Wait(due - now);
            }
        }
        let Some(next) = active.next_sequence.checked_add(1) else {
            active.failure = Some(refused("Studio frame sequence exhausted"));
            return Poll::Faulted;
        };
        let ticket = Ticket {
            owner: self.control.clone(),
            generation: active.intent.sequence,
            sequence: active.next_sequence,
            frame_index: active.prepared.frame_index(),
            started: now,
        };
        active.next_sequence = next;
        active.pending = Some(ticket.clone());
        active.transfer_started = false;
        Poll::Frame(ticket)
    }

    /// Existing worker's batch admission must enclose this entire call. Fresh
    /// owner/policy sampling belongs to the caller, not to a cached identity GET.
    /// No method here spawns work or retries a partial transfer.
    pub(crate) fn transfer(
        &mut self,
        ticket: &Ticket,
        device: &mut Device,
        connection_epoch: u64,
        deadline: Instant,
        mut worker_cancelled: impl FnMut() -> bool,
    ) -> Result<Completion> {
        let active = self
            .active
            .as_mut()
            .filter(|active| active.pending.as_ref() == Some(ticket) && !active.transfer_started)
            .ok_or_else(|| invalid("Studio transfer ticket is stale or already consumed"))?;
        active.transfer_started = true;
        let control = self.control.clone();
        if worker_cancelled() || control.cancelled(active.intent.stop_epoch) {
            return Ok(refused("Studio transfer cancelled before admission"));
        }
        if active.connection_epoch != connection_epoch
            || active.key != device.key()
            || active.path != device.path.as_bytes()
            || device.pid != 605
            || device.interface != 2
        {
            return Ok(refused(
                "Studio connection changed; explicit preparation is required",
            ));
        }
        let frame = match active.prepared.current_frame() {
            Ok(frame) => frame,
            Err(error) => return Ok(refused(&error.to_string())),
        };
        Ok(active
            .controls
            .submit_frame(device, &frame, deadline, || {
                worker_cancelled() || control.cancelled(active.intent.stop_epoch)
            })
            .completion)
    }
    /// Even cancellation after accepted OUTs consumes the pending ticket. It
    /// does not claim that those writes were undone or that old RGB was restored.
    pub(crate) fn complete(
        &mut self,
        ticket: &Ticket,
        now: Duration,
        mut completion: Completion,
    ) -> Result<()> {
        let active = self
            .active
            .as_mut()
            .filter(|active| active.pending.as_ref() == Some(ticket))
            .ok_or_else(|| invalid("Studio completion is stale or already consumed"))?;
        active.pending = None;
        if now < ticket.started {
            completion.error =
                Some("Studio monotonic clock moved backwards after possible output".into());
        }
        if completion.accepted_packets != 7
            || completion.attempted_packets != 7
            || completion.error.is_some()
        {
            active.failure = Some(completion);
            return Ok(());
        }
        active.accepted_frames = active.accepted_frames.saturating_add(1);
        if !self.control.cancelled(active.intent.stop_epoch) {
            if let Err(error) = active.prepared.advance() {
                completion.error = Some(error.to_string());
                active.failure = Some(completion);
            }
        }
        active.next_due = Some(
            ticket
                .started
                .saturating_add(active.prepared.frame_interval()),
        );
        Ok(())
    }
    pub(crate) fn status(&self) -> Value {
        match &self.active {
            None => json!({"state":"stopped","target":null,"drained":true}),
            Some(active) => {
                let stopped = self.control.cancelled(active.intent.stop_epoch);
                json!({"state": if stopped { if active.pending.is_some() {"draining"} else {"stop-awaiting-owner"} }
                    else if active.failure.is_some() {"faulted"} else if active.pending.is_some() {"in-flight"} else {"applied"},
                    "target":active.key,"composition_id":active.composition_id,"composition_sha256":active.revision,
                    "generation":active.intent.sequence,"connection_epoch":active.connection_epoch,
                    "frame_index":active.prepared.frame_index(),"accepted_frames":active.accepted_frames,
                    "pending_sequence":active.pending.as_ref().map(|ticket|ticket.sequence),"drained":active.pending.is_none(),
                    "failure":active.failure.as_ref().map(|failure|json!({"accepted_packets":failure.accepted_packets,
                        "attempted_packets":failure.attempted_packets,"error":failure.error})),
                    "quick_target_held":true,"mode_acquired":false,"prior_rgb_restored":false,
                    "interprocess_ownership_lease":false,"control_observations":active.controls.status()})
            }
        }
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidArg(message.into())
}
fn refused(message: &str) -> Completion {
    Completion {
        accepted_packets: 0,
        attempted_packets: 0,
        error: Some(message.into()),
    }
}
fn digest(bytes: &[u8]) -> Result<String> {
    let mut hash = [0u8; 32];
    let len = u32::try_from(bytes.len())
        .map_err(|_| invalid("Studio revision exceeds hash input bound"))?;
    let result = unsafe {
        BCryptHash(
            BCRYPT_SHA256_ALG_HANDLE,
            std::ptr::null(),
            0,
            bytes.as_ptr(),
            len,
            hash.as_mut_ptr(),
            32,
        )
    };
    if result < 0 {
        return Err(invalid("Studio revision hash failed"));
    }
    Ok(hash.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests;
