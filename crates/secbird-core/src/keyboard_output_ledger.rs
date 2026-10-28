//! Bounded logical ownership of keyboard output through asynchronous completion.
//!
//! No driver handles, input submission, global releases or Drop-time I/O. A live
//! adapter must separately prove physical routing and exclusive input admission.
use crate::rzcontrol_packet::{InputPacket, KeyboardInput};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

pub const MAX_KEYS: usize = 256;
pub const MAX_CLAIMS: usize = 1024;
const MAX_RELEASE_ATTEMPTS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Owner(pub NonZeroU64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completion {
    Delivered,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    InvalidKey,
    ConflictingMetadata,
    Capacity,
    Stopping,
    Superseded,
    WrongTicket,
    WrongPhase,
    RecoveryUnconfirmed,
    TicketExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    unit: u16,
    scan: u16,
    prefix: u16,
}
fn key(input: KeyboardInput) -> Result<Key, Failure> {
    if !matches!(input.flags, 0 | 2 | 4)
        || input.make_code == 0
        || input.make_code == 0xff
        || input.reserved != 0
    {
        return Err(Failure::InvalidKey);
    }
    Ok(Key {
        unit: input.unit_id,
        scan: input.make_code,
        prefix: input.flags,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    ticket: Ticket,
    direction: Direction,
    packet: InputPacket,
}
impl Command {
    pub fn ticket(self) -> Ticket {
        self.ticket
    }
    pub fn direction(self) -> Direction {
        self.direction
    }
    pub fn packet(self) -> InputPacket {
        self.packet
    }
}
struct Entry {
    template: KeyboardInput,
    owners: BTreeSet<Owner>,
    possibly_held: bool,
    release_attempts: u8,
}
struct Pending {
    command: Command,
    key: Key,
    dispatched: bool,
}
#[derive(Default)]
pub struct Ledger {
    entries: BTreeMap<Key, Entry>,
    pending: Option<Pending>,
    next_ticket: u64,
    stopping: bool,
    uncertain: bool,
}
impl Ledger {
    /// Adds desired ownership. Owner tokens identify this runtime's sources, not
    /// an OS key, physical device, driver selector or permission to send input.
    pub fn press(&mut self, owner: Owner, input: KeyboardInput) -> Result<(), Failure> {
        if self.stopping {
            return Err(Failure::Stopping);
        }
        let id = key(input)?;
        if let Some(entry) = self.entries.get(&id) {
            if entry.template != input {
                return Err(Failure::ConflictingMetadata);
            }
            if entry.owners.contains(&owner) {
                return Ok(());
            }
        } else if self.entries.len() >= MAX_KEYS {
            return Err(Failure::Capacity);
        }
        if self
            .entries
            .values()
            .map(|entry| entry.owners.len())
            .sum::<usize>()
            >= MAX_CLAIMS
        {
            return Err(Failure::Capacity);
        }
        self.entries
            .entry(id)
            .or_insert_with(|| Entry {
                template: input,
                owners: BTreeSet::new(),
                possibly_held: false,
                release_attempts: 0,
            })
            .owners
            .insert(owner);
        Ok(())
    }
    pub fn release(&mut self, owner: Owner, input: KeyboardInput) -> Result<(), Failure> {
        let id = key(input)?;
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.owners.remove(&owner);
        }
        self.prune();
        Ok(())
    }
    pub fn release_owner(&mut self, owner: Owner) {
        for entry in self.entries.values_mut() {
            entry.owners.remove(&owner);
        }
        self.prune();
    }
    /// A stop request invalidates future presses immediately, but a dispatched
    /// down can still complete later. Do not confuse this with drained output.
    pub fn stop(&mut self) {
        self.stopping = true;
        for entry in self.entries.values_mut() {
            entry.owners.clear();
        }
        if self.pending.as_ref().is_some_and(|pending| {
            !pending.dispatched && pending.command.direction == Direction::Down
        }) {
            self.pending = None;
        }
        self.prune();
    }
    pub fn is_drained(&self) -> bool {
        self.pending.is_none()
            && self
                .entries
                .values()
                .all(|entry| !entry.possibly_held && entry.owners.is_empty())
    }
    pub fn has_uncertain_delivery(&self) -> bool {
        self.uncertain
    }
    pub fn possibly_held_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.possibly_held)
            .count()
    }
    /// Releases precede new downs; only one command is outstanding globally.
    /// The adapter must not dispatch a cloned command a second time.
    pub fn next_command(&mut self) -> Result<Option<Command>, Failure> {
        if self.pending.is_some() {
            return Ok(None);
        }
        self.prune();
        let up = self
            .entries
            .iter()
            .find(|(_, entry)| {
                entry.possibly_held
                    && entry.owners.is_empty()
                    && entry.release_attempts < MAX_RELEASE_ATTEMPTS
            })
            .map(|(id, _)| (*id, Direction::Up));
        if up.is_none()
            && self.entries.values().any(|entry| {
                entry.possibly_held
                    && entry.owners.is_empty()
                    && entry.release_attempts >= MAX_RELEASE_ATTEMPTS
            })
        {
            return Err(Failure::RecoveryUnconfirmed);
        }
        let next = up.or_else(|| {
            self.entries
                .iter()
                .find(|(_, entry)| !entry.possibly_held && !entry.owners.is_empty())
                .map(|(id, _)| (*id, Direction::Down))
        });
        let Some((id, direction)) = next else {
            return Ok(None);
        };
        let entry = &self.entries[&id];
        let value = self
            .next_ticket
            .checked_add(1)
            .ok_or(Failure::TicketExhausted)?;
        self.next_ticket = value;
        let mut input = entry.template;
        if direction == Direction::Up {
            input.flags |= 1;
        }
        let command = Command {
            ticket: Ticket(value),
            direction,
            packet: InputPacket::Keyboard(input),
        };
        self.pending = Some(Pending {
            command,
            key: id,
            dispatched: false,
        });
        Ok(Some(command))
    }
    /// Call immediately before entering the native send API. Even a later failed
    /// acknowledgement cannot then be assumed to mean zero side effects.
    pub fn dispatch_started(&mut self, ticket: Ticket) -> Result<(), Failure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.command.ticket == ticket)
            .ok_or(Failure::WrongTicket)?;
        if pending.dispatched {
            return Err(Failure::WrongPhase);
        }
        if pending.command.direction == Direction::Down
            && self.entries[&pending.key].owners.is_empty()
        {
            self.pending = None;
            self.prune();
            return Err(Failure::Superseded);
        }
        let pending = self.pending.as_mut().expect("validated pending command");
        pending.dispatched = true;
        let entry = self
            .entries
            .get_mut(&pending.key)
            .expect("pending entry retained");
        match pending.command.direction {
            Direction::Down => entry.possibly_held = true,
            Direction::Up => entry.release_attempts += 1,
        }
        Ok(())
    }
    /// Refusal before calling the native API needs no speculative key-up. This
    /// is not the appropriate completion for an API that was already dispatched.
    pub fn refused_before_dispatch(&mut self, ticket: Ticket) -> Result<(), Failure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.command.ticket == ticket)
            .ok_or(Failure::WrongTicket)?;
        if pending.dispatched {
            return Err(Failure::WrongPhase);
        }
        self.pending = None;
        self.stop();
        Ok(())
    }
    /// Delivered means the adapter has established completion according to its
    /// driver contract, not merely submission acceptance. Cancel/error outcomes
    /// whose side effects are unknown must be Uncertain and latch stop/recovery.
    pub fn complete(&mut self, ticket: Ticket, outcome: Completion) -> Result<(), Failure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.command.ticket == ticket)
            .ok_or(Failure::WrongTicket)?;
        if !pending.dispatched {
            return Err(Failure::WrongPhase);
        }
        let pending = self.pending.take().unwrap();
        if outcome == Completion::Delivered {
            let entry = self
                .entries
                .get_mut(&pending.key)
                .expect("pending entry retained");
            entry.possibly_held = pending.command.direction == Direction::Down;
            if pending.command.direction == Direction::Up {
                entry.release_attempts = 0;
            }
        } else {
            self.uncertain = true;
            self.stop();
        }
        self.prune();
        Ok(())
    }
    fn prune(&mut self) {
        let pending = self.pending.as_ref().map(|pending| pending.key);
        self.entries.retain(|id, entry| {
            Some(*id) == pending || entry.possibly_held || !entry.owners.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner(value: u64) -> Owner {
        Owner(NonZeroU64::new(value).unwrap())
    }
    fn input(scan: u16) -> KeyboardInput {
        KeyboardInput {
            unit_id: 7,
            make_code: scan,
            flags: 0,
            reserved: 0,
            extra_information: 0x40,
        }
    }
    fn delivered(ledger: &mut Ledger) -> Command {
        let command = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(command.ticket()).unwrap();
        ledger
            .complete(command.ticket(), Completion::Delivered)
            .unwrap();
        command
    }
    #[test]
    fn coalesces_owned_aliases_and_preserves_complete_release_template() {
        let mut ledger = Ledger::default();
        let key = input(0x1e);
        ledger.press(owner(1), key).unwrap();
        ledger.press(owner(2), key).unwrap();
        assert_eq!(delivered(&mut ledger).direction(), Direction::Down);
        ledger.release_owner(owner(1));
        assert!(ledger.next_command().unwrap().is_none());
        ledger.release_owner(owner(2));
        let command = delivered(&mut ledger);
        let InputPacket::Keyboard(released) = command.packet() else {
            panic!()
        };
        assert_eq!(released, KeyboardInput { flags: 1, ..key });
        assert!(ledger.is_drained());
    }
    #[test]
    fn stop_waits_for_late_down_completion_before_requesting_its_release() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(command.ticket()).unwrap();
        ledger.stop();
        assert!(!ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
        assert_eq!(ledger.press(owner(2), input(32)), Err(Failure::Stopping));
        ledger
            .complete(command.ticket(), Completion::Delivered)
            .unwrap();
        assert_eq!(delivered(&mut ledger).direction(), Direction::Up);
        assert!(ledger.is_drained());
    }
    #[test]
    fn released_winner_completes_before_the_next_down_and_stale_tickets_are_inert() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let old = delivered(&mut ledger);
        ledger.release(owner(1), input(30)).unwrap();
        ledger.press(owner(1), input(32)).unwrap();
        let up = ledger.next_command().unwrap().unwrap();
        assert_eq!(up.direction(), Direction::Up);
        assert_eq!(
            ledger.complete(old.ticket(), Completion::Delivered),
            Err(Failure::WrongTicket)
        );
        assert_eq!(
            ledger.complete(up.ticket(), Completion::Delivered),
            Err(Failure::WrongPhase)
        );
        ledger.dispatch_started(up.ticket()).unwrap();
        assert_eq!(
            ledger.dispatch_started(up.ticket()),
            Err(Failure::WrongPhase)
        );
        assert!(ledger.next_command().unwrap().is_none());
        ledger.complete(up.ticket(), Completion::Delivered).unwrap();
        assert_eq!(delivered(&mut ledger).direction(), Direction::Down);
        ledger.stop();
        delivered(&mut ledger);
        assert!(ledger.is_drained());
    }
    #[test]
    fn refused_before_dispatch_needs_no_phantom_release() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.refused_before_dispatch(command.ticket()).unwrap();
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
    }
    #[test]
    fn stop_invalidates_a_prepared_down_without_sending_a_release() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.stop();
        assert_eq!(
            ledger.dispatch_started(command.ticket()),
            Err(Failure::WrongTicket)
        );
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
    }
    #[test]
    fn released_ownership_supersedes_a_prepared_down_before_dispatch() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.release_owner(owner(1));
        assert_eq!(
            ledger.dispatch_started(command.ticket()),
            Err(Failure::Superseded)
        );
        assert!(ledger.is_drained());
        ledger.press(owner(2), input(32)).unwrap();
        assert_eq!(delivered(&mut ledger).direction(), Direction::Down);
        ledger.stop();
        assert_eq!(delivered(&mut ledger).direction(), Direction::Up);
        assert!(ledger.is_drained());
    }
    #[test]
    fn uncertain_completion_is_not_a_claim_of_recovery_and_retries_are_bounded() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        let down = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(down.ticket()).unwrap();
        ledger
            .complete(down.ticket(), Completion::Uncertain)
            .unwrap();
        assert!(ledger.has_uncertain_delivery());
        assert_eq!(ledger.possibly_held_count(), 1);
        for _ in 0..3 {
            let up = ledger.next_command().unwrap().unwrap();
            assert_eq!(up.direction(), Direction::Up);
            ledger.dispatch_started(up.ticket()).unwrap();
            ledger.complete(up.ticket(), Completion::Uncertain).unwrap();
        }
        assert_eq!(ledger.next_command(), Err(Failure::RecoveryUnconfirmed));
        assert!(!ledger.is_drained());
    }
    #[test]
    fn exhausted_release_does_not_starve_recovery_of_other_keys() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), input(30)).unwrap();
        ledger.press(owner(1), input(32)).unwrap();
        delivered(&mut ledger);
        delivered(&mut ledger);
        ledger.stop();
        for _ in 0..MAX_RELEASE_ATTEMPTS {
            let up = ledger.next_command().unwrap().unwrap();
            assert_eq!(
                up.packet(),
                InputPacket::Keyboard(KeyboardInput {
                    flags: 1,
                    ..input(30)
                })
            );
            ledger.dispatch_started(up.ticket()).unwrap();
            ledger.complete(up.ticket(), Completion::Uncertain).unwrap();
        }
        let other = delivered(&mut ledger);
        assert_eq!(
            other.packet(),
            InputPacket::Keyboard(KeyboardInput {
                flags: 1,
                ..input(32)
            })
        );
        assert_eq!(ledger.possibly_held_count(), 1);
        assert_eq!(ledger.next_command(), Err(Failure::RecoveryUnconfirmed));
        assert!(!ledger.is_drained());
    }
    #[test]
    fn unknown_flags_and_conflicting_metadata_are_refused_without_erasing_owners() {
        let mut ledger = Ledger::default();
        let key = input(30);
        ledger.press(owner(1), key).unwrap();
        assert_eq!(
            ledger.press(
                owner(2),
                KeyboardInput {
                    extra_information: 0,
                    ..key
                }
            ),
            Err(Failure::ConflictingMetadata)
        );
        for flags in [1, 3, 5, 6, 7, 8, u16::MAX] {
            assert_eq!(
                ledger.press(owner(2), KeyboardInput { flags, ..key }),
                Err(Failure::InvalidKey)
            );
        }
        delivered(&mut ledger);
        ledger.release_owner(owner(1));
        delivered(&mut ledger);
        assert!(ledger.is_drained());
    }
    #[test]
    fn capacity_is_bounded_and_completed_entries_are_reusable() {
        let mut ledger = Ledger::default();
        for scan in 1..=257 {
            if scan == 255 {
                continue;
            }
            ledger.press(owner(1), input(scan)).unwrap();
        }
        assert_eq!(ledger.press(owner(1), input(258)), Err(Failure::Capacity));
        ledger.stop();
        assert!(ledger.is_drained());
    }
}
