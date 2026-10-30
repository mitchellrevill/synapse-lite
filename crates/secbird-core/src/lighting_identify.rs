//! Temporary Identify lifecycle only. No transport or ownership acquisition.
//! The output worker must retain this object until restoration is acknowledged.
use std::time::{Duration, Instant};

pub const MAX_DURATION: Duration = Duration::from_secs(5);
pub const DISABLED_REASON: &str =
    "Identify unavailable: exclusive lighting ownership and exact restoration are not verified";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Device(String),
    Led { device: String, led: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Identifying,
    Restoring,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    Duration,
    Ownership,
    Restoration,
}

/// An opaque, exact prior intent (including unknown data) is moved in, never
/// normalized or persisted. A failed restore retains it for explicit retry.
/// Admission booleans describe caller obligations, not a hardware lease.
pub struct Identify<T> {
    active: Option<Active<T>>,
}
struct Active<T> {
    target: Target,
    prior: T,
    deadline: Instant,
    phase: Phase,
}
impl<T> Default for Identify<T> {
    fn default() -> Self {
        Self { active: None }
    }
}
impl<T> Identify<T> {
    pub fn start(
        &mut self,
        target: Target,
        prior: T,
        now: Instant,
        duration: Duration,
        exclusive_owner: bool,
        can_restore_exactly: bool,
    ) -> Result<(), Refusal> {
        if self.active.is_some() {
            return Err(Refusal::Busy);
        }
        if duration.is_zero() || duration > MAX_DURATION {
            return Err(Refusal::Duration);
        }
        if !exclusive_owner {
            return Err(Refusal::Ownership);
        }
        if !can_restore_exactly {
            return Err(Refusal::Restoration);
        }
        self.active = Some(Active {
            target,
            prior,
            deadline: now + duration,
            phase: Phase::Identifying,
        });
        Ok(())
    }
    /// Poll before every Identify frame. The deadline is fixed, never renewed.
    pub fn tick(&mut self, now: Instant) -> Option<Phase> {
        let active = self.active.as_mut()?;
        if now >= active.deadline {
            active.phase = Phase::Restoring;
        }
        Some(active.phase)
    }
    /// Also call on disconnect, worker shutdown, output failure or owner loss.
    /// A disconnected target must not be restored onto a replacement identity.
    pub fn cancel(&mut self) {
        if let Some(active) = &mut self.active {
            active.phase = Phase::Restoring;
        }
    }
    pub fn target(&self) -> Option<&Target> {
        self.active.as_ref().map(|a| &a.target)
    }
    pub fn restoration(&self) -> Option<&T> {
        self.active
            .as_ref()
            .filter(|a| a.phase == Phase::Restoring)
            .map(|a| &a.prior)
    }
    /// Call only after the output worker confirms the exact prior intent was
    /// accepted. Merely attempting restoration must not release this snapshot.
    pub fn acknowledge_restored(&mut self) -> bool {
        if self.restoration().is_none() {
            return false;
        }
        self.active = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> Target {
        Target::Led {
            device: "serial-A".into(),
            led: "42".into(),
        }
    }
    fn started() -> (Identify<Vec<u8>>, Instant) {
        let now = Instant::now();
        let mut session = Identify::default();
        session
            .start(
                target(),
                b"{ unknown : [1,2] }".to_vec(),
                now,
                MAX_DURATION,
                true,
                true,
            )
            .unwrap();
        (session, now)
    }
    #[test]
    fn deadline_restores_exact_bytes() {
        let (mut s, now) = started();
        assert_eq!(
            s.tick(now + MAX_DURATION - Duration::from_nanos(1)),
            Some(Phase::Identifying)
        );
        assert!(!s.acknowledge_restored());
        assert_eq!(s.tick(now + MAX_DURATION), Some(Phase::Restoring));
        assert_eq!(s.restoration().unwrap(), b"{ unknown : [1,2] }");
        assert!(s.acknowledge_restored());
        assert_eq!(s.tick(now), None);
    }
    #[test]
    fn cancellation_is_idempotent_and_retains_failed_restore() {
        let (mut s, now) = started();
        s.cancel();
        s.cancel();
        assert_eq!(s.tick(now), Some(Phase::Restoring));
        assert_eq!(s.target(), Some(&target()));
        assert_eq!(s.restoration().unwrap(), b"{ unknown : [1,2] }");
        assert_eq!(
            s.start(target(), vec![], now, MAX_DURATION, true, true),
            Err(Refusal::Busy)
        );
        assert!(s.restoration().is_some());
    }
    #[test]
    fn admission_refuses_unproved_output_and_unbounded_timing() {
        let now = Instant::now();
        let mut s = Identify::default();
        for (duration, owner, restore, error) in [
            (Duration::ZERO, true, true, Refusal::Duration),
            (
                MAX_DURATION + Duration::from_nanos(1),
                true,
                true,
                Refusal::Duration,
            ),
            (MAX_DURATION, false, true, Refusal::Ownership),
            (MAX_DURATION, true, false, Refusal::Restoration),
        ] {
            assert_eq!(
                s.start(target(), vec![9], now, duration, owner, restore),
                Err(error)
            );
            assert!(s.target().is_none());
        }
    }
    #[test]
    fn repeated_ticks_never_extend_deadline() {
        let (mut s, now) = started();
        for second in 0..5 {
            assert_eq!(
                s.tick(now + Duration::from_secs(second)),
                Some(Phase::Identifying)
            );
        }
        assert_eq!(
            s.tick(now + Duration::from_secs(60)),
            Some(Phase::Restoring)
        );
        assert_eq!(s.tick(now), Some(Phase::Restoring));
    }
    #[test]
    fn new_intent_only_after_acknowledgement() {
        let (mut s, now) = started();
        s.cancel();
        assert!(s.acknowledge_restored());
        s.start(
            Target::Device("serial-B".into()),
            vec![8],
            now,
            Duration::from_secs(1),
            true,
            true,
        )
        .unwrap();
        s.cancel();
        assert_eq!(s.restoration(), Some(&vec![8]));
    }
}
