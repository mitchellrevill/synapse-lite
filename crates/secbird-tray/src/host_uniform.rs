//! Bounded uniform/spatial-index programs and a local submission scheduler.
//!
//! RGB tables are derived from the shipped native renderer, not observed optical
//! output. Source Spectrum is UI3/native4; Breathing is UI2/native5 (native3 is
//! Reactive). Explicit Wave is UI4/native2 and emits an index, never a dummy RGB
//! color. The native outer scheduler is unknown. Our policy emits at most one
//! frame per poll, never catches up in a burst, and freezes phase while suppressed.
//! Program/scheduler logic is pure; the separate io child is a bounded adapter.
use secbird_core::{
    effects::Effect,
    uniform_lighting::UniformCycle,
    wave_lighting::{CardinalDirection, WaveCycle, WaveRegion},
    Error, Profile, Result, Rgb,
};
use serde_json::{json, Value};
use std::time::Duration;

pub(crate) mod io;

const SOURCE: &str = "_synapseLiteSynapse4";
const OPT_IN: &str = "uniformHostIntent";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Program {
    Spectrum,
    Breathing {
        first: Rgb,
        second: Option<Rgb>,
    },
    /// Explicit frame-index intent only. Discovery remains uniform-only until
    /// the Engine can authenticate and map a selected physical layout.
    Wave(crate::host_wave::Program),
}
impl Program {
    pub(crate) fn source_id(self) -> u8 {
        match self {
            Self::Spectrum => 3,
            Self::Breathing { .. } => 2,
            Self::Wave(_) => 4,
        }
    }
    pub(crate) fn native_id(self) -> u8 {
        match self {
            Self::Spectrum => 4,
            Self::Breathing { .. } => 5,
            Self::Wave(_) => 2,
        }
    }
    pub(crate) fn from_profile(profile: &Profile, pid: u16) -> Result<Option<Self>> {
        if pid != 605
            || secbird_core::profile_compat::source_quick_effects(profile).is_none()
            || !profile.keys.is_empty()
        {
            return Ok(None);
        }
        let Some(zone) = profile.lighting.get("backlight") else {
            return Ok(None);
        };
        if zone.off {
            return Ok(None);
        }
        match zone.effect.as_ref() {
            Some(Effect::Spectrum)=>Ok(Some(Self::Spectrum)),
            Some(Effect::Breathing(colors))=>match colors.as_slice() {
                [first]=>Ok(Some(Self::Breathing{first:*first,second:None})),
                [_,second] if *second==Rgb(0,0,0)=>Err(Error::Unsupported("Black second-color count semantics are not verified".into())),
                [first,second]=>Ok(Some(Self::Breathing{first:*first,second:Some(*second)})),
                _=>Err(Error::Unsupported("Host Breathing supports exactly one or two fixed colors; random/other modes are not implemented".into())),
            },
            _=>Ok(None),
        }
    }
    fn cycle(self) -> Cycle {
        if let Self::Wave(program) = self {
            let direction = match program {
                crate::host_wave::Program::Left => CardinalDirection::Left,
                crate::host_wave::Program::Right => CardinalDirection::Right,
            };
            let region =
                WaveRegion::new(7, 23, 0, 0, 23, 7).expect("verified fixed logical geometry");
            return Cycle::Wave(
                WaveCycle::source_quick(region, direction).expect("bounded source Quick Wave"),
            );
        }
        Cycle::Uniform(match self {
            Self::Spectrum => UniformCycle::spectrum(),
            Self::Breathing {
                first,
                second: None,
            } => UniformCycle::breathing(first),
            Self::Breathing {
                first,
                second: Some(second),
            } => UniformCycle::breathing_two_colors(first, second),
            Self::Wave(_) => unreachable!("spatial variant handled before uniform construction"),
        })
    }
    fn marker(self) -> Option<Value> {
        match self {
            Self::Spectrum => Some(json!({"version":1,"source_id":3})),
            Self::Breathing { first, second } => Some(
                json!({"version":1,"source_id":2,"first":first.to_string(),"second":second.map(|c|c.to_string())}),
            ),
            Self::Wave(_) => None,
        }
    }
    /// Native opt-in binds the current projection, not stale retained vendor colors.
    /// The marker is outside decoded: exporting cannot alter the vendor payload.
    pub(crate) fn opted_in(self, profile: &Profile) -> bool {
        if let Self::Wave(program) = self {
            return program.opted_in(profile);
        }
        profile.extra.get(SOURCE).and_then(|v| v.get(OPT_IN)) == self.marker().as_ref()
    }
    pub(crate) fn opt_in(self, profile: &mut Profile) -> Result<()> {
        if let Self::Wave(program) = self {
            return program.opt_in(profile);
        }
        let metadata = profile
            .extra
            .get_mut(SOURCE)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                Error::InvalidArg("Source metadata is required before uniform host opt-in".into())
            })?;
        metadata.insert(
            OPT_IN.into(),
            self.marker().expect("uniform variant has marker"),
        );
        Ok(())
    }
    pub(crate) fn clear_opt_in(profile: &mut Profile) {
        if let Some(metadata) = profile.extra.get_mut(SOURCE).and_then(Value::as_object_mut) {
            metadata.remove(OPT_IN);
        }
        crate::host_wave::Program::clear_opt_in(profile);
    }
}

#[derive(Debug, Clone)]
enum Cycle {
    Uniform(UniformCycle),
    Wave(WaveCycle),
}
impl Cycle {
    fn frame_count(&self) -> usize {
        match self {
            Self::Uniform(cycle) => cycle.frame_count(),
            Self::Wave(cycle) => cycle.frame_count(),
        }
    }
    fn frame_interval(&self) -> Duration {
        match self {
            Self::Uniform(cycle) => cycle.frame_interval(),
            Self::Wave(cycle) => cycle.frame_interval(),
        }
    }
    fn next_content(&mut self) -> FrameContent {
        match self {
            Self::Uniform(cycle) => FrameContent::Uniform(cycle.next_color()),
            Self::Wave(_) => FrameContent::SpatialIndex,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    Ready,
    Suppressed,
    Blocked,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameContent {
    Uniform(Rgb),
    /// palette_index selects the logical spatial frame. It is not an RGB color.
    SpatialIndex,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameTicket {
    pub generation: u64,
    pub sequence: u64,
    pub palette_index: usize,
    pub content: FrameContent,
    started_at: Duration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Poll {
    Frame(FrameTicket),
    Wait(Duration),
    Suppressed,
    Blocked,
    InFlight,
    Faulted,
}
#[derive(Debug, Clone)]
pub(crate) struct Completion {
    pub accepted_packets: usize,
    pub attempted_packets: usize,
    pub error: Option<String>,
}
/// Internal action diagnostics; packet counts come from the transport outcome,
/// never from parsing a human-readable error message.
#[derive(Debug, Default, Clone)]
pub(crate) struct ActionTrace {
    pub output_attempted: bool,
    pub requested: Option<Program>,
    pub completion: Option<Completion>,
}

#[derive(Debug, Clone)]
pub(crate) struct Playback {
    program: Program,
    generation: u64,
    cycle: Cycle,
    palette_index: usize,
    next_sequence: u64,
    next_due: Option<Duration>,
    pending: Option<FrameTicket>,
    accepted_frames: u64,
    failure: Option<Completion>,
}
impl Playback {
    pub(crate) fn new(program: Program, generation: u64, now: Duration) -> Self {
        Self {
            program,
            generation,
            cycle: program.cycle(),
            palette_index: 0,
            next_sequence: 0,
            next_due: Some(now),
            pending: None,
            accepted_frames: 0,
            failure: None,
        }
    }
    pub(crate) fn program(&self) -> Program {
        self.program
    }
    pub(crate) fn frame_count(&self) -> usize {
        self.cycle.frame_count()
    }
    pub(crate) fn interval(&self) -> Duration {
        self.cycle.frame_interval()
    }
    pub(crate) fn accepted_frames(&self) -> u64 {
        self.accepted_frames
    }
    pub(crate) fn failure(&self) -> Option<&Completion> {
        self.failure.as_ref()
    }
    /// No hidden clock: caller supplies monotonic elapsed time. A late poll does
    /// one render call, without skipping LUT entries or queuing catch-up frames.
    pub(crate) fn poll(&mut self, now: Duration, availability: Availability) -> Poll {
        if self.failure.is_some() {
            return Poll::Faulted;
        }
        if self.pending.is_some() {
            return Poll::InFlight;
        }
        if availability != Availability::Ready {
            self.next_due = None;
            return if availability == Availability::Suppressed {
                Poll::Suppressed
            } else {
                Poll::Blocked
            };
        }
        if let Some(due) = self.next_due {
            if now < due {
                return Poll::Wait(due - now);
            }
        }
        let ticket = FrameTicket {
            generation: self.generation,
            sequence: self.next_sequence,
            palette_index: self.palette_index,
            content: self.cycle.next_content(),
            started_at: now,
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.palette_index = (self.palette_index + 1) % self.cycle.frame_count();
        self.pending = Some(ticket);
        Poll::Frame(ticket)
    }
    /// Accepted USB OUT is not firmware/optical verification. Any partial/error
    /// completion latches this playback; only explicit new playback restarts it.
    pub(crate) fn complete(
        &mut self,
        ticket: FrameTicket,
        now: Duration,
        completion: Completion,
    ) -> Result<()> {
        if self.pending != Some(ticket) {
            return Err(Error::InvalidArg(
                "Uniform frame completion is stale or already consumed".into(),
            ));
        }
        self.pending = None;
        if now < ticket.started_at {
            self.failure = Some(Completion {
                accepted_packets: completion.accepted_packets,
                attempted_packets: completion.attempted_packets,
                error: Some(
                    "Uniform monotonic clock moved backwards after a possible transfer".into(),
                ),
            });
            return Err(Error::InvalidArg(
                "Uniform monotonic clock moved backwards".into(),
            ));
        }
        if completion.accepted_packets != 7
            || completion.attempted_packets != 7
            || completion.error.is_some()
        {
            self.failure = Some(completion);
            return Ok(());
        }
        self.accepted_frames = self.accepted_frames.saturating_add(1);
        // Starts are never closer than the nominal40ms interval. Completion may
        // be later; the next caller still submits at most one frame, never bursts.
        self.next_due = Some(
            ticket
                .started_at
                .saturating_add(self.cycle.frame_interval()),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SPECTRUM: &[u8] =
        include_bytes!("../../secbird-core/tests/fixtures/native-spectrum-static-derived.rgb");
    const ONE: &[u8] = include_bytes!("../tests/fixtures/breathing-one-static-derived.rgb");
    const TWO: &[u8] = include_bytes!("../tests/fixtures/breathing-two-static-derived.rgb");
    fn accepted() -> Completion {
        Completion {
            accepted_packets: 7,
            attempted_packets: 7,
            error: None,
        }
    }
    fn due(playback: &mut Playback, millis: u64) -> FrameTicket {
        match playback.poll(Duration::from_millis(millis), Availability::Ready) {
            Poll::Frame(t) => t,
            other => panic!("expected frame, got {other:?}"),
        }
    }
    #[test]
    fn accepted_frame_sequence_matches_all_943_175_350_static_derived_rgb_vectors() {
        for (program, bytes, count, native) in [
            (Program::Spectrum, SPECTRUM, 943, 4),
            (
                Program::Breathing {
                    first: Rgb(255, 0, 0),
                    second: None,
                },
                ONE,
                175,
                5,
            ),
            (
                Program::Breathing {
                    first: Rgb(255, 0, 0),
                    second: Some(Rgb(0, 0, 255)),
                },
                TWO,
                350,
                5,
            ),
        ] {
            assert_eq!(program.native_id(), native);
            assert_eq!(program.source_id(), if native == 4 { 3 } else { 2 });
            assert_ne!(native, 3, "native3 is Reactive");
            let mut playback = Playback::new(program, 42, Duration::ZERO);
            assert_eq!(playback.program(), program);
            assert_eq!(playback.frame_count(), count);
            assert_eq!(playback.interval(), Duration::from_millis(40));
            let mut output = Vec::new();
            for index in 0..count {
                let now = index as u64 * 40;
                let t = due(&mut playback, now);
                assert_eq!(t.palette_index, index);
                let FrameContent::Uniform(color) = t.content else {
                    panic!("uniform fixture emitted a spatial index")
                };
                output.extend([color.0, color.1, color.2]);
                playback
                    .complete(t, Duration::from_millis(now + 1), accepted())
                    .unwrap();
            }
            assert_eq!(
                output, bytes,
                "derived ordinary-RGB fixture, not vendor execution"
            );
            assert_eq!(playback.accepted_frames(), count as u64);
            let wrap = due(&mut playback, count as u64 * 40);
            assert_eq!(wrap.palette_index, 0);
            let FrameContent::Uniform(color) = wrap.content else {
                panic!("uniform wrap emitted a spatial index")
            };
            assert_eq!([color.0, color.1, color.2], bytes[..3]);
        }
    }
    #[test]
    fn suppression_and_owner_block_freeze_phase_and_resume_without_catchup() {
        let mut playback = Playback::new(Program::Spectrum, 1, Duration::ZERO);
        let first = due(&mut playback, 0);
        playback
            .complete(first, Duration::from_millis(38), accepted())
            .unwrap();
        assert_eq!(
            playback.poll(Duration::from_millis(39), Availability::Ready),
            Poll::Wait(Duration::from_millis(1))
        );
        assert_eq!(
            playback.poll(Duration::from_secs(1), Availability::Suppressed),
            Poll::Suppressed
        );
        assert_eq!(
            playback.poll(Duration::from_secs(100), Availability::Blocked),
            Poll::Blocked
        );
        let next = due(&mut playback, 100001);
        assert_eq!(next.palette_index, 1);
        assert_eq!(
            playback.poll(Duration::from_secs(200), Availability::Ready),
            Poll::InFlight
        );
        playback
            .complete(next, Duration::from_millis(100039), accepted())
            .unwrap();
        assert_eq!(
            playback.poll(Duration::from_millis(100040), Availability::Ready),
            Poll::Wait(Duration::from_millis(1))
        );
    }
    #[test]
    fn slow_transfer_and_late_pump_do_one_next_color_not_a_burst_or_phase_skip() {
        let mut playback = Playback::new(Program::Spectrum, 1, Duration::ZERO);
        let first = due(&mut playback, 0);
        playback
            .complete(first, Duration::from_millis(55), accepted())
            .unwrap();
        let second = due(&mut playback, 55);
        assert_eq!(second.palette_index, 1);
        playback
            .complete(second, Duration::from_millis(90), accepted())
            .unwrap();
        assert_eq!(
            playback.poll(Duration::from_millis(94), Availability::Ready),
            Poll::Wait(Duration::from_millis(1))
        );
        let third = due(&mut playback, 10000);
        assert_eq!(third.palette_index, 2);
    }
    #[test]
    fn partial_and_failed_outputs_latch_counts_and_never_implicitly_retry() {
        for (accepted_packets, attempted_packets) in [(0, 0), (0, 1), (2, 3), (6, 7)] {
            let mut playback = Playback::new(Program::Spectrum, 1, Duration::ZERO);
            let first = due(&mut playback, 0);
            playback
                .complete(
                    first,
                    Duration::from_millis(40),
                    Completion {
                        accepted_packets,
                        attempted_packets,
                        error: Some("injected OUT/preflight failure".into()),
                    },
                )
                .unwrap();
            assert_eq!(
                playback.poll(Duration::from_secs(99), Availability::Ready),
                Poll::Faulted
            );
            assert_eq!(playback.accepted_frames(), 0);
            assert_eq!(
                playback.failure().unwrap().accepted_packets,
                accepted_packets
            );
            assert_eq!(
                playback.failure().unwrap().attempted_packets,
                attempted_packets
            );
        }
    }
    #[test]
    fn stale_duplicate_completion_and_new_generation_cannot_advance_current_playback() {
        let mut old = Playback::new(Program::Spectrum, 1, Duration::ZERO);
        let ticket = due(&mut old, 0);
        let mut next = Playback::new(Program::Spectrum, 2, Duration::ZERO);
        let current = due(&mut next, 0);
        assert!(next
            .complete(ticket, Duration::from_millis(1), accepted())
            .is_err());
        next.complete(current, Duration::from_millis(1), accepted())
            .unwrap();
        assert!(next
            .complete(current, Duration::from_millis(2), accepted())
            .is_err());
        assert_eq!(next.accepted_frames(), 1);
    }
    #[test]
    fn native_projection_and_native_only_opt_in_do_not_mutate_vendor_decoded_payload() {
        let mut profile =
            secbird_core::profile_compat::new_native_profile(605, "Synthetic").unwrap();
        let program = Program::from_profile(&profile, 605).unwrap().unwrap();
        assert_eq!(program, Program::Spectrum);
        assert!(!program.opted_in(&profile));
        let decoded = profile.extra[SOURCE]["decoded"].clone();
        let mut store = secbird_core::Store::default();
        let mut device = secbird_core::profile::DeviceProfiles::default();
        device.active = "Synthetic".into();
        device.profiles.clear();
        device.profiles.insert("Synthetic".into(), profile.clone());
        store.devices.insert("025D:synthetic".into(), device);
        let before = secbird_core::profile_compat::export_synapse4(&store, "025D:synthetic")
            .unwrap()
            .bytes;
        program
            .opt_in(
                store
                    .devices
                    .get_mut("025D:synthetic")
                    .unwrap()
                    .active_mut(),
            )
            .unwrap();
        let after = secbird_core::profile_compat::export_synapse4(&store, "025D:synthetic")
            .unwrap()
            .bytes;
        assert_eq!(
            before, after,
            "native opt-in must not change the foreign document/payload/hash"
        );
        program.opt_in(&mut profile).unwrap();
        assert!(program.opted_in(&profile));
        assert_eq!(profile.extra[SOURCE]["decoded"], decoded);
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(Effect::Breathing(vec![Rgb(1, 2, 3)]));
        let changed = Program::from_profile(&profile, 605).unwrap().unwrap();
        assert!(!changed.opted_in(&profile));
        changed.opt_in(&mut profile).unwrap();
        assert!(changed.opted_in(&profile));
        Program::clear_opt_in(&mut profile);
        assert!(!changed.opted_in(&profile));
        profile.lighting.get_mut("backlight").unwrap().off = true;
        assert!(Program::from_profile(&profile, 605).unwrap().is_none());
        profile.lighting.get_mut("backlight").unwrap().off = false;
        profile.keys = vec![Rgb(1, 1, 1); 132];
        assert!(Program::from_profile(&profile, 605).unwrap().is_none());
    }
    #[test]
    fn random_and_non_source_profiles_do_not_become_uniform_programs() {
        let mut profile =
            secbird_core::profile_compat::new_native_profile(605, "Synthetic").unwrap();
        profile.lighting.get_mut("backlight").unwrap().effect = Some(Effect::Breathing(vec![]));
        assert!(Program::from_profile(&profile, 605).is_err());
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(Effect::Breathing(vec![Rgb(255, 0, 0), Rgb(0, 0, 0)]));
        assert!(Program::from_profile(&profile, 605).is_err());
        profile.extra.remove(SOURCE);
        assert!(Program::from_profile(&profile, 605).unwrap().is_none());
        assert!(Program::from_profile(&profile, 196).unwrap().is_none());
    }

    #[test]
    fn explicit_wave_tickets_are_indices_for_all_25_frames_then_wrap() {
        for direction in [
            crate::host_wave::Program::Left,
            crate::host_wave::Program::Right,
        ] {
            let program = Program::Wave(direction);
            assert_eq!((program.source_id(), program.native_id()), (4, 2));
            let mut playback = Playback::new(program, 17, Duration::ZERO);
            assert_eq!(playback.frame_count(), 25);
            assert_eq!(playback.interval(), Duration::from_millis(40));
            for index in 0..25 {
                let ticket = due(&mut playback, index * 40);
                assert_eq!(ticket.palette_index, index as usize);
                assert_eq!(ticket.content, FrameContent::SpatialIndex);
                assert_eq!(ticket.generation, 17);
                playback
                    .complete(ticket, Duration::from_millis(index * 40 + 1), accepted())
                    .unwrap();
            }
            assert_eq!(playback.accepted_frames(), 25);
            let wrap = due(&mut playback, 1000);
            assert_eq!(
                (wrap.palette_index, wrap.content),
                (0, FrameContent::SpatialIndex)
            );
        }
    }

    #[test]
    fn wave_keeps_local_freeze_no_catchup_stale_completion_and_fault_contracts() {
        let program = Program::Wave(crate::host_wave::Program::Left);
        let mut playback = Playback::new(program, 1, Duration::ZERO);
        let first = due(&mut playback, 0);
        let mut fake_color = first;
        fake_color.content = FrameContent::Uniform(Rgb(1, 2, 3));
        assert!(playback
            .complete(fake_color, Duration::from_millis(1), accepted())
            .is_err());
        assert_eq!(
            playback.poll(Duration::from_millis(1), Availability::Ready),
            Poll::InFlight
        );
        playback
            .complete(first, Duration::from_millis(55), accepted())
            .unwrap();
        assert_eq!(
            playback.poll(Duration::from_secs(1), Availability::Suppressed),
            Poll::Suppressed
        );
        assert_eq!(
            playback.poll(Duration::from_secs(100), Availability::Blocked),
            Poll::Blocked
        );
        let second = due(&mut playback, 100_001);
        assert_eq!(second.palette_index, 1, "suppression must not skip phase");
        playback
            .complete(second, Duration::from_millis(100_060), accepted())
            .unwrap();
        let late = due(&mut playback, 200_000);
        assert_eq!(
            late.palette_index, 2,
            "late pump emits one next index, not catchup"
        );
        playback
            .complete(
                late,
                Duration::from_millis(200_001),
                Completion {
                    accepted_packets: 2,
                    attempted_packets: 3,
                    error: Some("partial spatial OUT".into()),
                },
            )
            .unwrap();
        assert_eq!(
            playback.poll(Duration::from_secs(999), Availability::Ready),
            Poll::Faulted
        );
        assert_eq!(
            (
                playback.failure().unwrap().accepted_packets,
                playback.failure().unwrap().attempted_packets
            ),
            (2, 3)
        );
        let mut replacement = Playback::new(program, 2, Duration::ZERO);
        let current = due(&mut replacement, 0);
        assert_eq!(current.palette_index, 0);
        assert!(replacement
            .complete(first, Duration::from_millis(1), accepted())
            .is_err());
        replacement
            .complete(current, Duration::from_millis(1), accepted())
            .unwrap();
        assert!(replacement
            .complete(current, Duration::from_millis(2), accepted())
            .is_err());
    }

    #[test]
    fn explicit_wave_opt_in_delegates_but_uniform_discovery_never_autoactivates_it() {
        let mut profile =
            secbird_core::profile_compat::new_native_profile(605, "Synthetic").unwrap();
        Program::Spectrum.opt_in(&mut profile).unwrap();
        secbird_core::profile_compat::set_source_quick_effect(
            &mut profile,
            605,
            "Synthetic",
            4,
            &json!({"direction":1}),
        )
        .unwrap();
        profile.lighting.get_mut("backlight").unwrap().effect =
            Some(crate::host_wave::Program::Left.native_projection());
        let decoded = profile.extra[SOURCE]["decoded"].clone();
        let program = Program::Wave(crate::host_wave::Program::Left);
        assert!(!program.opted_in(&profile));
        assert_eq!(Program::from_profile(&profile, 605).unwrap(), None);
        program.opt_in(&mut profile).unwrap();
        assert!(program.opted_in(&profile));
        assert_eq!(profile.extra[SOURCE]["decoded"], decoded);
        assert_eq!(Program::from_profile(&profile, 605).unwrap(), None);
        Program::clear_opt_in(&mut profile);
        assert!(!program.opted_in(&profile));
        assert!(profile.extra[SOURCE].get(OPT_IN).is_none());
        assert!(profile.extra[SOURCE].get("waveHostIntent").is_none());
    }
}
