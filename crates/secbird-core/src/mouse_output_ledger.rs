//! Bounded ownership of mouse buttons and completion of one-shot wheel output.
//!
//! No input sender, endpoint handles, physical ownership claim or Drop-time I/O.
//! A wheel never acquires a held claim and is never inverted or replayed after
//! uncertain delivery. A live adapter must prove routing and completion itself.
pub use crate::keyboard_output_ledger::{Completion, Owner};
use crate::rzcontrol_packet::{InputPacket, MouseInput};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BUTTONS: usize = 64;
pub const MAX_CLAIMS: usize = 256;
const MAX_RELEASE_ATTEMPTS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    Impulse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    InvalidButton,
    InvalidImpulse,
    ConflictingMetadata,
    Capacity,
    Busy,
    Stopping,
    Superseded,
    WrongTicket,
    WrongPhase,
    RecoveryUnconfirmed,
    TicketExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ButtonKey {
    unit: u16,
    down_flag: u16,
}

fn stationary(input: MouseInput) -> bool {
    input.flags == 0 && input.raw_buttons == 0 && input.last_x == 0 && input.last_y == 0
}

fn button_key(input: MouseInput) -> Result<ButtonKey, Failure> {
    if !stationary(input)
        || input.button_data != 0
        || !matches!(input.button_flags, 1 | 4 | 16 | 64 | 256)
    {
        return Err(Failure::InvalidButton);
    }
    Ok(ButtonKey {
        unit: input.unit_id,
        down_flag: input.button_flags,
    })
}

fn validate_impulse(input: MouseInput) -> Result<(), Failure> {
    if !stationary(input)
        || !matches!(input.button_flags, 1024 | 2048)
        || !matches!(input.button_data, 120 | 65416)
    {
        return Err(Failure::InvalidImpulse);
    }
    Ok(())
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
    template: MouseInput,
    owners: BTreeSet<Owner>,
    possibly_held: bool,
    release_attempts: u8,
    release_refused: bool,
}

struct Pending {
    command: Command,
    button: Option<ButtonKey>,
    dispatched: bool,
}

#[derive(Default)]
pub struct Ledger {
    entries: BTreeMap<ButtonKey, Entry>,
    impulse: Option<MouseInput>,
    pending: Option<Pending>,
    next_ticket: u64,
    stopping: bool,
    uncertain: bool,
}

impl Ledger {
    /// Owner tokens identify logical sources, not physical input permission.
    /// `input` is always the down template, including when releasing ownership.
    pub fn press(&mut self, owner: Owner, input: MouseInput) -> Result<(), Failure> {
        if self.stopping {
            return Err(Failure::Stopping);
        }
        let key = button_key(input)?;
        if let Some(entry) = self.entries.get(&key) {
            if entry.template != input {
                return Err(Failure::ConflictingMetadata);
            }
            if entry.owners.contains(&owner) {
                return Ok(());
            }
        } else if self.entries.len() >= MAX_BUTTONS {
            return Err(Failure::Capacity);
        }
        let claims: usize = self.entries.values().map(|entry| entry.owners.len()).sum();
        if claims >= MAX_CLAIMS {
            return Err(Failure::Capacity);
        }
        self.entries
            .entry(key)
            .or_insert_with(|| Entry {
                template: input,
                owners: BTreeSet::new(),
                possibly_held: false,
                release_attempts: 0,
                release_refused: false,
            })
            .owners
            .insert(owner);
        Ok(())
    }

    pub fn release(&mut self, owner: Owner, input: MouseInput) -> Result<(), Failure> {
        let key = button_key(input)?;
        if let Some(entry) = self.entries.get_mut(&key) {
            if entry.template != input {
                return Err(Failure::ConflictingMetadata);
            }
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

    /// Queue one wheel action without creating a held button or recovery up.
    /// Busy refuses a second action rather than replacing or merging it.
    pub fn impulse(&mut self, input: MouseInput) -> Result<(), Failure> {
        if self.stopping {
            return Err(Failure::Stopping);
        }
        validate_impulse(input)?;
        if self.impulse.is_some()
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.command.direction == Direction::Impulse)
        {
            return Err(Failure::Busy);
        }
        self.impulse = Some(input);
        Ok(())
    }

    /// Undispatched downs/impulses become stale. Dispatched output still waits
    /// for terminal completion; a late down may require its own exact release.
    pub fn stop(&mut self) {
        self.stopping = true;
        self.impulse = None;
        for entry in self.entries.values_mut() {
            entry.owners.clear();
        }
        if self.pending.as_ref().is_some_and(|pending| {
            !pending.dispatched && pending.command.direction != Direction::Up
        }) {
            self.pending = None;
        }
        self.prune();
    }

    /// Drained means no pending output or held claim. Historical uncertain wheel
    /// delivery is still exposed separately; a wheel cannot be undone here.
    pub fn is_drained(&self) -> bool {
        self.pending.is_none()
            && self.impulse.is_none()
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

    /// Releases precede new output. A returned command is not returned twice;
    /// the adapter must not dispatch a copied command more than once either.
    pub fn next_command(&mut self) -> Result<Option<Command>, Failure> {
        if self.pending.is_some() {
            return Ok(None);
        }
        self.prune();
        let release = self
            .entries
            .iter()
            .find(|(_, entry)| {
                entry.possibly_held
                    && entry.owners.is_empty()
                    && !entry.release_refused
                    && entry.release_attempts < MAX_RELEASE_ATTEMPTS
            })
            .map(|(key, _)| (*key, Direction::Up));
        if release.is_none()
            && self.entries.values().any(|entry| {
                entry.possibly_held
                    && entry.owners.is_empty()
                    && (entry.release_refused || entry.release_attempts >= MAX_RELEASE_ATTEMPTS)
            })
        {
            return Err(Failure::RecoveryUnconfirmed);
        }
        let button = release.or_else(|| {
            self.entries
                .iter()
                .find(|(_, entry)| !entry.possibly_held && !entry.owners.is_empty())
                .map(|(key, _)| (*key, Direction::Down))
        });
        let (input, direction, button) = if let Some((key, direction)) = button {
            let mut input = self.entries[&key].template;
            if direction == Direction::Up {
                input.button_flags <<= 1;
            }
            (input, direction, Some(key))
        } else if let Some(input) = self.impulse {
            (input, Direction::Impulse, None)
        } else {
            return Ok(None);
        };
        let value = self
            .next_ticket
            .checked_add(1)
            .ok_or(Failure::TicketExhausted)?;
        self.next_ticket = value;
        if direction == Direction::Impulse {
            self.impulse = None;
        }
        let command = Command {
            ticket: Ticket(value),
            direction,
            packet: InputPacket::Mouse(input),
        };
        self.pending = Some(Pending {
            command,
            button,
            dispatched: false,
        });
        Ok(Some(command))
    }

    /// Mark immediately before native API entry, not after submission returns.
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
            && self.entries[&pending.button.expect("button command")]
                .owners
                .is_empty()
        {
            self.pending = None;
            self.prune();
            return Err(Failure::Superseded);
        }
        let pending = self.pending.as_mut().expect("validated pending command");
        pending.dispatched = true;
        if let Some(key) = pending.button {
            let entry = self.entries.get_mut(&key).expect("pending button retained");
            match pending.command.direction {
                Direction::Down => entry.possibly_held = true,
                Direction::Up => entry.release_attempts += 1,
                Direction::Impulse => unreachable!("impulses have no button"),
            }
        }
        Ok(())
    }

    /// Only use for a command that never entered the native send API. An up
    /// refused here retains the possible held state and consumes no attempt.
    /// Only that button is refused; other eligible releases can still drain.
    pub fn refused_before_dispatch(&mut self, ticket: Ticket) -> Result<(), Failure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.command.ticket == ticket)
            .ok_or(Failure::WrongTicket)?;
        if pending.dispatched {
            return Err(Failure::WrongPhase);
        }
        if pending.command.direction == Direction::Up {
            // Refusal is local to this release. Other owned entries must still
            // drain, but this one must not be automatically submitted again.
            self.entries
                .get_mut(&pending.button.expect("up command has a button"))
                .expect("pending entry retained")
                .release_refused = true;
        }
        self.pending = None;
        self.stop();
        Ok(())
    }

    /// Delivered requires terminal completion under the adapter's proved native
    /// contract. Acceptance or an error with unknown side effects is Uncertain.
    pub fn complete(&mut self, ticket: Ticket, outcome: Completion) -> Result<(), Failure> {
        let pending = self
            .pending
            .as_ref()
            .filter(|pending| pending.command.ticket == ticket)
            .ok_or(Failure::WrongTicket)?;
        if !pending.dispatched {
            return Err(Failure::WrongPhase);
        }
        let pending = self.pending.take().expect("validated pending command");
        if outcome == Completion::Delivered {
            if let Some(key) = pending.button {
                let entry = self.entries.get_mut(&key).expect("pending button retained");
                entry.possibly_held = pending.command.direction == Direction::Down;
                if pending.command.direction == Direction::Up {
                    entry.release_attempts = 0;
                    entry.release_refused = false;
                }
            }
        } else {
            self.uncertain = true;
            self.stop();
        }
        self.prune();
        Ok(())
    }

    fn prune(&mut self) {
        let pending = self.pending.as_ref().and_then(|pending| pending.button);
        self.entries.retain(|key, entry| {
            Some(*key) == pending || entry.possibly_held || !entry.owners.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    fn owner(value: u64) -> Owner {
        Owner(NonZeroU64::new(value).unwrap())
    }

    fn button(flag: u16) -> MouseInput {
        MouseInput {
            unit_id: 7,
            flags: 0,
            button_flags: flag,
            button_data: 0,
            raw_buttons: 0,
            last_x: 0,
            last_y: 0,
            extra_information: 0x12345678,
        }
    }

    fn wheel(axis: u16, data: u16) -> MouseInput {
        MouseInput {
            button_flags: axis,
            button_data: data,
            ..button(1)
        }
    }

    fn deliver(ledger: &mut Ledger) -> Command {
        let command = ledger.next_command().unwrap().unwrap();
        assert!(ledger.next_command().unwrap().is_none());
        ledger.dispatch_started(command.ticket()).unwrap();
        ledger
            .complete(command.ticket(), Completion::Delivered)
            .unwrap();
        command
    }

    #[test]
    fn all_five_buttons_keep_exact_packet_metadata_on_owned_release() {
        for (down, up) in [(1, 2), (4, 8), (16, 32), (64, 128), (256, 512)] {
            let mut ledger = Ledger::default();
            let input = button(down);
            ledger.press(owner(1), input).unwrap();
            let pressed = deliver(&mut ledger);
            assert_eq!(pressed.direction(), Direction::Down);
            assert_eq!(pressed.packet(), InputPacket::Mouse(input));
            assert_eq!(ledger.possibly_held_count(), 1);
            ledger.release(owner(1), input).unwrap();
            let released = deliver(&mut ledger);
            assert_eq!(released.direction(), Direction::Up);
            let expected = InputPacket::Mouse(MouseInput {
                button_flags: up,
                ..input
            });
            assert_eq!(released.packet(), expected);
            assert_eq!(
                &released.packet().to_bytes()[28..32],
                &[0x78, 0x56, 0x34, 0x12]
            );
            assert!(ledger.is_drained());
        }
    }

    #[test]
    fn co_owned_buttons_release_only_after_last_owner_and_refuse_metadata_conflicts() {
        let mut ledger = Ledger::default();
        let input = button(1);
        ledger.press(owner(1), input).unwrap();
        ledger.press(owner(2), input).unwrap();
        assert_eq!(deliver(&mut ledger).direction(), Direction::Down);
        let changed = MouseInput {
            extra_information: 99,
            ..input
        };
        assert_eq!(
            ledger.press(owner(3), changed),
            Err(Failure::ConflictingMetadata)
        );
        assert_eq!(
            ledger.release(owner(1), changed),
            Err(Failure::ConflictingMetadata)
        );
        ledger.release(owner(2), input).unwrap();
        assert!(ledger.next_command().unwrap().is_none());
        ledger.release_owner(owner(1));
        assert_eq!(
            deliver(&mut ledger).packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 2,
                ..input
            })
        );
        assert!(ledger.is_drained());
    }

    #[test]
    fn stop_stales_undispatched_down_and_discards_queued_wheel() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(1)).unwrap();
        let down = ledger.next_command().unwrap().unwrap();
        ledger.impulse(wheel(1024, 120)).unwrap();
        ledger.stop();
        assert_eq!(
            ledger.dispatch_started(down.ticket()),
            Err(Failure::WrongTicket)
        );
        assert!(ledger.next_command().unwrap().is_none());
        assert!(ledger.is_drained());
        assert_eq!(ledger.press(owner(1), button(1)), Err(Failure::Stopping));
        assert_eq!(ledger.impulse(wheel(2048, 65416)), Err(Failure::Stopping));
    }

    #[test]
    fn stop_waits_for_late_down_completion_then_emits_its_owned_up() {
        let mut ledger = Ledger::default();
        let input = button(64);
        ledger.press(owner(1), input).unwrap();
        let down = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(down.ticket()).unwrap();
        ledger.impulse(wheel(2048, 120)).unwrap();
        ledger.stop();
        assert!(!ledger.is_drained());
        assert_eq!(ledger.possibly_held_count(), 1);
        assert!(ledger.next_command().unwrap().is_none());
        ledger
            .complete(down.ticket(), Completion::Delivered)
            .unwrap();
        let up = deliver(&mut ledger);
        assert_eq!(
            up.packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 128,
                ..input
            })
        );
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
    }

    #[test]
    fn uncertain_down_and_three_uncertain_releases_never_report_drained() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(16)).unwrap();
        let down = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(down.ticket()).unwrap();
        ledger
            .complete(down.ticket(), Completion::Uncertain)
            .unwrap();
        for _ in 0..3 {
            let up = ledger.next_command().unwrap().unwrap();
            assert_eq!(up.direction(), Direction::Up);
            assert!(ledger.next_command().unwrap().is_none());
            ledger.dispatch_started(up.ticket()).unwrap();
            assert_eq!(
                ledger.dispatch_started(up.ticket()),
                Err(Failure::WrongPhase)
            );
            ledger.complete(up.ticket(), Completion::Uncertain).unwrap();
        }
        assert_eq!(ledger.next_command(), Err(Failure::RecoveryUnconfirmed));
        assert_eq!(ledger.possibly_held_count(), 1);
        assert!(ledger.has_uncertain_delivery());
        assert!(!ledger.is_drained());
    }

    #[test]
    fn wheel_axes_and_signs_are_exactly_once_without_held_claims() {
        for axis in [1024, 2048] {
            for data in [120, 65416] {
                let mut ledger = Ledger::default();
                let input = wheel(axis, data);
                ledger.impulse(input).unwrap();
                assert_eq!(ledger.impulse(input), Err(Failure::Busy));
                assert!(!ledger.is_drained());
                let command = ledger.next_command().unwrap().unwrap();
                assert_eq!(command.direction(), Direction::Impulse);
                assert_eq!(command.packet(), InputPacket::Mouse(input));
                assert_eq!(ledger.possibly_held_count(), 0);
                assert_eq!(
                    ledger.complete(command.ticket(), Completion::Delivered),
                    Err(Failure::WrongPhase)
                );
                assert_eq!(ledger.impulse(input), Err(Failure::Busy));
                ledger.dispatch_started(command.ticket()).unwrap();
                ledger
                    .complete(command.ticket(), Completion::Delivered)
                    .unwrap();
                assert_eq!(
                    ledger.complete(command.ticket(), Completion::Delivered),
                    Err(Failure::WrongTicket)
                );
                assert!(ledger.next_command().unwrap().is_none());
                assert!(ledger.is_drained());
            }
        }
    }

    #[test]
    fn stopped_or_uncertain_wheel_is_never_replayed_or_inverted() {
        let mut ledger = Ledger::default();
        ledger.impulse(wheel(1024, 65416)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.stop();
        assert_eq!(
            ledger.dispatch_started(command.ticket()),
            Err(Failure::WrongTicket)
        );
        assert!(ledger.next_command().unwrap().is_none());
        assert!(ledger.is_drained());
        let mut ledger = Ledger::default();
        ledger.impulse(wheel(2048, 120)).unwrap();
        let command = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(command.ticket()).unwrap();
        ledger.stop();
        assert!(!ledger.is_drained());
        ledger
            .complete(command.ticket(), Completion::Uncertain)
            .unwrap();
        assert!(ledger.has_uncertain_delivery());
        assert_eq!(ledger.possibly_held_count(), 0);
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
    }

    #[test]
    fn uncertain_wheel_recovers_existing_button_without_inverse_wheel() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(4)).unwrap();
        deliver(&mut ledger);
        ledger.impulse(wheel(1024, 65416)).unwrap();
        let impulse = ledger.next_command().unwrap().unwrap();
        ledger.dispatch_started(impulse.ticket()).unwrap();
        ledger
            .complete(impulse.ticket(), Completion::Uncertain)
            .unwrap();
        let recovery = deliver(&mut ledger);
        assert_eq!(recovery.direction(), Direction::Up);
        assert_eq!(
            recovery.packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 8,
                ..button(4)
            })
        );
        assert!(ledger.is_drained());
        assert!(ledger.has_uncertain_delivery());
        assert!(ledger.next_command().unwrap().is_none());
    }

    #[test]
    fn invalid_packets_refuse_before_creating_claim_or_impulse() {
        let valid = button(1);
        for input in [
            MouseInput { flags: 1, ..valid },
            MouseInput {
                raw_buttons: 1,
                ..valid
            },
            MouseInput {
                last_x: -1,
                ..valid
            },
            MouseInput { last_y: 1, ..valid },
            MouseInput {
                button_data: 120,
                ..valid
            },
            MouseInput {
                button_flags: 2,
                ..valid
            },
            MouseInput {
                button_flags: 3,
                ..valid
            },
            wheel(1024, 120),
        ] {
            let mut ledger = Ledger::default();
            assert_eq!(ledger.press(owner(1), input), Err(Failure::InvalidButton));
            assert_eq!(ledger.release(owner(1), input), Err(Failure::InvalidButton));
            assert!(ledger.is_drained());
        }
        let valid = wheel(1024, 120);
        for input in [
            MouseInput { flags: 1, ..valid },
            MouseInput {
                raw_buttons: 1,
                ..valid
            },
            MouseInput { last_x: 1, ..valid },
            MouseInput {
                last_y: -1,
                ..valid
            },
            MouseInput {
                button_data: 1,
                ..valid
            },
            MouseInput {
                button_flags: 3072,
                ..valid
            },
            button(1),
        ] {
            let mut ledger = Ledger::default();
            assert_eq!(ledger.impulse(input), Err(Failure::InvalidImpulse));
            assert!(ledger.is_drained());
        }
    }

    #[test]
    fn unit_ids_keep_identical_buttons_and_their_releases_separate() {
        let mut ledger = Ledger::default();
        let first = button(1);
        let second = MouseInput {
            unit_id: 8,
            extra_information: 99,
            ..first
        };
        ledger.press(owner(1), first).unwrap();
        ledger.press(owner(1), second).unwrap();
        assert_eq!(deliver(&mut ledger).packet(), InputPacket::Mouse(first));
        assert_eq!(deliver(&mut ledger).packet(), InputPacket::Mouse(second));
        ledger.release(owner(1), second).unwrap();
        assert_eq!(
            deliver(&mut ledger).packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 2,
                ..second
            })
        );
        assert_eq!(ledger.possibly_held_count(), 1);
        ledger.release(owner(1), first).unwrap();
        assert_eq!(
            deliver(&mut ledger).packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 2,
                ..first
            })
        );
        assert!(ledger.is_drained());
    }

    #[test]
    fn entry_and_claim_budgets_refuse_atomically_but_existing_claims_are_idempotent() {
        let mut ledger = Ledger::default();
        for unit in 0..MAX_BUTTONS as u16 {
            ledger
                .press(
                    owner(1),
                    MouseInput {
                        unit_id: unit,
                        ..button(1)
                    },
                )
                .unwrap();
        }
        assert_eq!(
            ledger.press(
                owner(1),
                MouseInput {
                    unit_id: MAX_BUTTONS as u16,
                    ..button(1)
                }
            ),
            Err(Failure::Capacity)
        );
        assert_eq!(ledger.entries.len(), MAX_BUTTONS);
        let mut ledger = Ledger::default();
        for value in 1..=MAX_CLAIMS as u64 {
            ledger.press(owner(value), button(1)).unwrap();
        }
        ledger.press(owner(1), button(1)).unwrap();
        assert_eq!(
            ledger.press(owner(MAX_CLAIMS as u64 + 1), button(1)),
            Err(Failure::Capacity)
        );
        assert_eq!(ledger.press(owner(1), button(4)), Err(Failure::Capacity));
        assert_eq!(ledger.entries.len(), 1);
        ledger.release_owner(owner(1));
        ledger
            .press(owner(MAX_CLAIMS as u64 + 1), button(1))
            .unwrap();
        assert_eq!(
            ledger.entries.values().next().unwrap().owners.len(),
            MAX_CLAIMS
        );
    }

    #[test]
    fn releases_precede_new_down_and_refusal_does_not_invent_delivery() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(1)).unwrap();
        let old = deliver(&mut ledger);
        ledger.release(owner(1), button(1)).unwrap();
        ledger.press(owner(1), button(4)).unwrap();
        let up = ledger.next_command().unwrap().unwrap();
        assert_eq!(up.direction(), Direction::Up);
        assert_eq!(
            ledger.complete(old.ticket(), Completion::Delivered),
            Err(Failure::WrongTicket)
        );
        ledger.refused_before_dispatch(up.ticket()).unwrap();
        assert_eq!(
            ledger.dispatch_started(up.ticket()),
            Err(Failure::WrongTicket)
        );
        assert!(!ledger.is_drained());
        assert_eq!(ledger.possibly_held_count(), 1);
        assert_eq!(ledger.next_command(), Err(Failure::RecoveryUnconfirmed));
        assert!(!ledger.is_drained());
        assert_eq!(ledger.possibly_held_count(), 1);
    }

    #[test]
    fn refused_up_does_not_strand_other_held_buttons_or_retry_the_refused_button() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(1)).unwrap();
        ledger.press(owner(1), button(4)).unwrap();
        assert_eq!(deliver(&mut ledger).packet(), InputPacket::Mouse(button(1)));
        assert_eq!(deliver(&mut ledger).packet(), InputPacket::Mouse(button(4)));
        ledger.stop();
        let first = ledger.next_command().unwrap().unwrap();
        assert_eq!(
            first.packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 2,
                ..button(1)
            })
        );
        ledger.refused_before_dispatch(first.ticket()).unwrap();
        let first_key = button_key(button(1)).unwrap();
        assert_eq!(ledger.entries[&first_key].release_attempts, 0);
        assert!(ledger.entries[&first_key].release_refused);
        assert_eq!(ledger.possibly_held_count(), 2);
        let remaining = deliver(&mut ledger);
        assert_eq!(
            remaining.packet(),
            InputPacket::Mouse(MouseInput {
                button_flags: 8,
                ..button(4)
            })
        );
        assert_eq!(ledger.possibly_held_count(), 1);
        assert!(!ledger.is_drained());
        for _ in 0..5 {
            assert_eq!(ledger.next_command(), Err(Failure::RecoveryUnconfirmed));
        }
        assert_eq!(ledger.entries[&first_key].release_attempts, 0);
        assert_eq!(
            ledger.dispatch_started(first.ticket()),
            Err(Failure::WrongTicket)
        );
    }

    #[test]
    fn superseded_down_has_no_held_state_or_speculative_release() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(1)).unwrap();
        let down = ledger.next_command().unwrap().unwrap();
        ledger.release_owner(owner(1));
        assert_eq!(
            ledger.dispatch_started(down.ticket()),
            Err(Failure::Superseded)
        );
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
        assert_eq!(ledger.possibly_held_count(), 0);
    }

    #[test]
    fn stop_keeps_an_armed_up_and_dispatch_phase_is_enforced() {
        let mut ledger = Ledger::default();
        ledger.press(owner(1), button(256)).unwrap();
        deliver(&mut ledger);
        ledger.release_owner(owner(1));
        let up = ledger.next_command().unwrap().unwrap();
        ledger.stop();
        ledger.dispatch_started(up.ticket()).unwrap();
        assert_eq!(
            ledger.refused_before_dispatch(up.ticket()),
            Err(Failure::WrongPhase)
        );
        assert_eq!(
            ledger.dispatch_started(up.ticket()),
            Err(Failure::WrongPhase)
        );
        ledger.complete(up.ticket(), Completion::Delivered).unwrap();
        assert!(ledger.is_drained());
    }

    #[test]
    fn ticket_exhaustion_never_wraps_or_consumes_the_queued_impulse() {
        let mut ledger = Ledger::default();
        ledger.next_ticket = u64::MAX;
        let input = wheel(1024, 120);
        ledger.impulse(input).unwrap();
        assert_eq!(ledger.next_command(), Err(Failure::TicketExhausted));
        assert_eq!(ledger.impulse, Some(input));
        assert!(ledger.pending.is_none());
        ledger.stop();
        assert!(ledger.is_drained());
        assert!(ledger.next_command().unwrap().is_none());
    }
}
