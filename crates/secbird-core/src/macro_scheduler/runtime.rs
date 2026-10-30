//! One serialized source timeline, with distinct keyboard and mouse ownership.
use super::{Error, NativeContext};
use crate::{
    keyboard_output_ledger::{self as keyboard, Completion, Owner},
    keyboard_symbolic_bridge::{plan_keyboard_packets, ExplicitNativeKeyboard, RoutingIdentity},
    mouse_output_ledger as mouse,
    mouse_symbolic_bridge::plan_mouse_packet,
    rzcontrol_packet::{InputPacket, KeyboardInput, MouseInput},
    source_macro::{Event, SymbolicPlan, Transition},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

const MAX_STEPS: usize = 512;

/// A caller must supply each used input class explicitly. Neither route is a
/// fallback for the other; a token does not establish physical ownership.
#[derive(Clone, Copy)]
pub struct Contexts {
    pub keyboard: Option<NativeContext>,
    pub mouse: Option<NativeContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
    Impulse,
}

/// Unique within one run, across both underlying ledgers. Callers must use a
/// fresh Owner for each run and serialize all access to that run's scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket {
    owner: Owner,
    serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    ticket: Ticket,
    identity: RoutingIdentity,
    packet: InputPacket,
    direction: Direction,
}
impl Command {
    pub fn ticket(self) -> Ticket {
        self.ticket
    }
    pub fn identity(self) -> RoutingIdentity {
        self.identity
    }
    pub fn packet(self) -> InputPacket {
        self.packet
    }
    pub fn direction(self) -> Direction {
        self.direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    Dispatch(Command),
    Pending,
    WaitUntil(Duration),
    Finished,
    Stopped {
        recovery_confirmed: bool,
        uncertain_delivery: bool,
    },
}

#[derive(Clone, Copy)]
enum Step {
    Delay(u32),
    Key {
        template: KeyboardInput,
        down: bool,
        gap: bool,
    },
    Button {
        template: MouseInput,
        down: bool,
    },
    Wheel(MouseInput),
}

#[derive(Clone, Copy)]
enum NativeCommand {
    Keyboard(keyboard::Command),
    Mouse(mouse::Command),
}
impl NativeCommand {
    fn packet(self) -> InputPacket {
        match self {
            Self::Keyboard(command) => command.packet(),
            Self::Mouse(command) => command.packet(),
        }
    }
    fn direction(self) -> Direction {
        match self {
            Self::Keyboard(command) => match command.direction() {
                keyboard::Direction::Down => Direction::Down,
                keyboard::Direction::Up => Direction::Up,
            },
            Self::Mouse(command) => match command.direction() {
                mouse::Direction::Down => Direction::Down,
                mouse::Direction::Up => Direction::Up,
                mouse::Direction::Impulse => Direction::Impulse,
            },
        }
    }
    fn wrong_phase(self) -> Error {
        match self {
            Self::Keyboard(_) => Error::Ledger(keyboard::Failure::WrongPhase),
            Self::Mouse(_) => Error::MouseLedger(mouse::Failure::WrongPhase),
        }
    }
}
struct Pending {
    command: Command,
    native: NativeCommand,
    dispatched: bool,
    source: bool,
}

/// One balanced pass, one command outstanding across both input classes.
/// The private ledgers do not arbitrate concurrent macros or physical input.
pub struct Scheduler {
    steps: Vec<Step>,
    owner: Owner,
    contexts: Contexts,
    keyboard: keyboard::Ledger,
    mouse: mouse::Ledger,
    cursor: usize,
    pending: Option<Pending>,
    next_ticket: u64,
    last_clock: Option<Duration>,
    anchor: Duration,
    deadline: Option<Duration>,
    gap_paid: bool,
    stopping: bool,
    recovery_blocked: [bool; 2],
    recovery_error: Option<Error>,
}
impl Scheduler {
    /// Existing keyboard-only admission. Mouse events require with_contexts.
    pub fn new(plan: &SymbolicPlan, context: NativeContext, owner: Owner) -> Result<Self, Error> {
        Self::with_contexts(
            plan,
            Contexts {
                keyboard: Some(context),
                mouse: None,
            },
            owner,
        )
    }

    /// Compile the entire pass before admitting any output. Repetition,
    /// conditional E1 packets, repeated makes and unbalanced holds still refuse.
    pub fn with_contexts(
        plan: &SymbolicPlan,
        contexts: Contexts,
        owner: Owner,
    ) -> Result<Self, Error> {
        for context in [contexts.keyboard, contexts.mouse].into_iter().flatten() {
            if context.source_unit.unwrap_or(0) != context.routing.unit_id() {
                return Err(Error::RoutingFields);
            }
        }
        if plan.repeats() != 1 {
            return Err(Error::UnsupportedRepetition);
        }
        if plan.events().len() > MAX_STEPS {
            return Err(Error::Budget);
        }
        let mut steps = Vec::with_capacity(plan.events().len());
        let mut keys = BTreeMap::new();
        let mut buttons = BTreeSet::new();
        for (index, event) in plan.events().iter().copied().enumerate() {
            let step = match event {
                Event::DelayMs(ms) => Step::Delay(ms),
                Event::Keyboard { transition, .. } => {
                    let context = contexts.keyboard.ok_or(Error::MissingKeyboardContext)?;
                    let native = ExplicitNativeKeyboard::from_source_event(
                        event,
                        context.modifiers,
                        context.source_unit,
                    )
                    .map_err(|_| Error::InvalidSequence)?;
                    let packet = plan_keyboard_packets(native, context.routing)
                        .map_err(|_| Error::InvalidSequence)?;
                    if packet.has_conditional_second() {
                        return Err(Error::ConditionalPacket);
                    }
                    let InputPacket::Keyboard(mut template) =
                        InputPacket::from_bytes(packet.first_packet())
                            .map_err(|_| Error::InvalidSequence)?
                    else {
                        return Err(Error::InvalidSequence);
                    };
                    let down = transition == Transition::Down;
                    template.flags &= !1;
                    if !matches!(template.flags, 0 | 2 | 4)
                        || matches!(template.make_code, 0 | 0xff)
                    {
                        return Err(Error::InvalidSequence);
                    }
                    let key = (template.make_code, template.flags);
                    if down {
                        if keys.insert(key, template).is_some() {
                            return Err(Error::InvalidSequence);
                        }
                    } else if keys.remove(&key) != Some(template) {
                        // The ledger releases the original down's exact native
                        // metadata. Refuse a different requested up atomically,
                        // before this run can produce any output.
                        return Err(Error::InvalidSequence);
                    }
                    if keys.len() > keyboard::MAX_KEYS {
                        return Err(Error::Budget);
                    }
                    Step::Key {
                        template,
                        down,
                        gap: plan.source_keyboard_gap_before_us(index).is_some(),
                    }
                }
                Event::MouseButton { .. } | Event::Wheel { .. } => {
                    let context = contexts.mouse.ok_or(Error::MissingMouseContext)?;
                    let packet = plan_mouse_packet(
                        event,
                        context.routing,
                        context.modifiers,
                        context.source_unit,
                    )
                    .map_err(|_| Error::InvalidSequence)?;
                    let InputPacket::Mouse(mut template) = InputPacket::from_bytes(packet.packet())
                        .map_err(|_| Error::InvalidSequence)?
                    else {
                        return Err(Error::InvalidSequence);
                    };
                    if let Event::MouseButton { transition, .. } = event {
                        let down = transition == Transition::Down;
                        if !down {
                            template.button_flags >>= 1;
                        }
                        if (down && !buttons.insert(template.button_flags))
                            || (!down && !buttons.remove(&template.button_flags))
                        {
                            return Err(Error::InvalidSequence);
                        }
                        Step::Button { template, down }
                    } else {
                        Step::Wheel(template)
                    }
                }
            };
            steps.push(step);
        }
        if !keys.is_empty() || !buttons.is_empty() {
            return Err(Error::InvalidSequence);
        }
        Ok(Self {
            steps,
            owner,
            contexts,
            keyboard: keyboard::Ledger::default(),
            mouse: mouse::Ledger::default(),
            cursor: 0,
            pending: None,
            next_ticket: 0,
            last_clock: None,
            anchor: Duration::ZERO,
            deadline: None,
            gap_paid: false,
            stopping: false,
            recovery_blocked: [false; 2],
            recovery_error: None,
        })
    }
    pub fn contexts(&self) -> Contexts {
        self.contexts
    }
    /// Prepared program position; an armed delay can still be waiting.
    pub fn source_cursor(&self) -> usize {
        self.cursor
    }
    pub fn possibly_held_count(&self) -> usize {
        self.keyboard.possibly_held_count() + self.mouse.possibly_held_count()
    }
    pub fn retained_step_bytes(&self) -> usize {
        self.steps.capacity() * std::mem::size_of::<Step>()
    }
    fn drained(&self) -> bool {
        self.keyboard.is_drained() && self.mouse.is_drained()
    }
    fn uncertain(&self) -> bool {
        self.keyboard.has_uncertain_delivery() || self.mouse.has_uncertain_delivery()
    }
    fn clock(&mut self, now: Duration) -> Result<(), Error> {
        if self.last_clock.is_some_and(|old| now < old) {
            self.stop();
            return Err(Error::ClockWentBack);
        }
        if self.last_clock.is_none() {
            self.anchor = now;
        }
        self.last_clock = Some(now);
        Ok(())
    }
    fn due(&mut self, delay: Duration, now: Duration) -> Result<bool, Error> {
        let Some(deadline) = self.anchor.checked_add(delay) else {
            self.stop();
            return Err(Error::ClockOverflow);
        };
        if now < deadline {
            self.deadline = Some(deadline);
            Ok(false)
        } else {
            self.anchor = now;
            Ok(true)
        }
    }
    fn arm(&mut self, native: NativeCommand, source: bool) -> Result<Poll, Error> {
        let Some(serial) = self.next_ticket.checked_add(1) else {
            self.stop();
            self.recovery_blocked = [true; 2];
            return Err(Error::TicketExhausted);
        };
        self.next_ticket = serial;
        let context = match native {
            NativeCommand::Keyboard(_) => {
                self.contexts.keyboard.expect("admitted keyboard context")
            }
            NativeCommand::Mouse(_) => self.contexts.mouse.expect("admitted mouse context"),
        };
        let command = Command {
            ticket: Ticket {
                owner: self.owner,
                serial,
            },
            identity: context.routing,
            packet: native.packet(),
            direction: native.direction(),
        };
        self.pending = Some(Pending {
            command,
            native,
            dispatched: false,
            source,
        });
        Ok(Poll::Dispatch(command))
    }
    fn key_command(&mut self, source: bool) -> Result<Poll, Error> {
        let command = self
            .keyboard
            .next_command()
            .map_err(Error::Ledger)?
            .ok_or(Error::InvalidSequence)?;
        self.arm(NativeCommand::Keyboard(command), source)
    }
    fn mouse_command(&mut self, source: bool) -> Result<Poll, Error> {
        let command = self
            .mouse
            .next_command()
            .map_err(Error::MouseLedger)?
            .ok_or(Error::InvalidSequence)?;
        self.arm(NativeCommand::Mouse(command), source)
    }
    fn recovery(&mut self) -> Result<Poll, Error> {
        // One exhausted/refused input class must not strand the other class.
        if !self.recovery_blocked[0] {
            match self.keyboard.next_command() {
                Ok(Some(command)) => return self.arm(NativeCommand::Keyboard(command), false),
                Ok(None) => {}
                Err(error) => {
                    self.recovery_blocked[0] = true;
                    self.recovery_error.get_or_insert(Error::Ledger(error));
                }
            }
        }
        if !self.recovery_blocked[1] {
            match self.mouse.next_command() {
                Ok(Some(command)) => return self.arm(NativeCommand::Mouse(command), false),
                Ok(None) => {}
                Err(error) => {
                    self.recovery_blocked[1] = true;
                    self.recovery_error.get_or_insert(Error::MouseLedger(error));
                }
            }
        }
        if let Some(error) = self.recovery_error.take() {
            return Err(error);
        }
        Ok(Poll::Stopped {
            recovery_confirmed: self.drained(),
            uncertain_delivery: self.uncertain(),
        })
    }
    /// Return each command once; only terminal completion advances an output.
    pub fn poll(&mut self, now: Duration) -> Result<Poll, Error> {
        self.clock(now)?;
        if self.pending.is_some() {
            return Ok(Poll::Pending);
        }
        if self.stopping {
            return self.recovery();
        }
        if let Some(deadline) = self.deadline {
            if now < deadline {
                return Ok(Poll::WaitUntil(deadline));
            }
            self.deadline = None;
            self.anchor = now;
        }
        while let Some(step) = self.steps.get(self.cursor).copied() {
            match step {
                Step::Delay(ms) => {
                    self.cursor += 1;
                    self.gap_paid = false;
                    if !self.due(Duration::from_millis(ms.into()), now)? {
                        return Ok(Poll::WaitUntil(self.deadline.unwrap()));
                    }
                }
                Step::Key {
                    template,
                    down,
                    gap,
                } => {
                    if gap && !self.gap_paid {
                        self.gap_paid = true;
                        if !self.due(Duration::from_millis(10), now)? {
                            return Ok(Poll::WaitUntil(self.deadline.unwrap()));
                        }
                    }
                    let result = if down {
                        self.keyboard.press(self.owner, template)
                    } else {
                        self.keyboard.release(self.owner, template)
                    };
                    if let Err(error) = result {
                        self.stop();
                        return Err(Error::Ledger(error));
                    }
                    return self.key_command(true);
                }
                Step::Button { template, down } => {
                    let result = if down {
                        self.mouse.press(self.owner, template)
                    } else {
                        self.mouse.release(self.owner, template)
                    };
                    if let Err(error) = result {
                        self.stop();
                        return Err(Error::MouseLedger(error));
                    }
                    return self.mouse_command(true);
                }
                Step::Wheel(input) => {
                    if let Err(error) = self.mouse.impulse(input) {
                        self.stop();
                        return Err(Error::MouseLedger(error));
                    }
                    return self.mouse_command(true);
                }
            }
        }
        if !self.drained() {
            self.stop();
            return self.recovery();
        }
        Ok(Poll::Finished)
    }
    pub fn dispatch_started(&mut self, ticket: Ticket) -> Result<(), Error> {
        let pending = self
            .pending
            .as_mut()
            .filter(|p| p.command.ticket == ticket)
            .ok_or(Error::WrongTicket)?;
        match pending.native {
            NativeCommand::Keyboard(command) => self
                .keyboard
                .dispatch_started(command.ticket())
                .map_err(Error::Ledger)?,
            NativeCommand::Mouse(command) => self
                .mouse
                .dispatch_started(command.ticket())
                .map_err(Error::MouseLedger)?,
        }
        pending.dispatched = true;
        Ok(())
    }
    pub fn complete(
        &mut self,
        ticket: Ticket,
        outcome: Completion,
        now: Duration,
    ) -> Result<(), Error> {
        let pending = self
            .pending
            .as_ref()
            .filter(|p| p.command.ticket == ticket)
            .ok_or(Error::WrongTicket)?;
        if !pending.dispatched {
            return Err(pending.native.wrong_phase());
        }
        let clock_result = self.clock(now);
        let native = self.pending.as_ref().unwrap().native;
        match native {
            NativeCommand::Keyboard(command) => self
                .keyboard
                .complete(command.ticket(), outcome)
                .map_err(Error::Ledger)?,
            NativeCommand::Mouse(command) => self
                .mouse
                .complete(command.ticket(), outcome)
                .map_err(Error::MouseLedger)?,
        }
        let pending = self.pending.take().unwrap();
        if outcome == Completion::Uncertain {
            self.stop();
        } else if pending.source && !self.stopping {
            self.cursor += 1;
            self.gap_paid = false;
            self.anchor = now;
        }
        clock_result
    }
    pub fn refused_before_dispatch(&mut self, ticket: Ticket) -> Result<(), Error> {
        let pending = self
            .pending
            .as_ref()
            .filter(|p| p.command.ticket == ticket)
            .ok_or(Error::WrongTicket)?;
        if pending.dispatched {
            return Err(pending.native.wrong_phase());
        }
        match pending.native {
            NativeCommand::Keyboard(command) => self
                .keyboard
                .refused_before_dispatch(command.ticket())
                .map_err(Error::Ledger)?,
            NativeCommand::Mouse(command) => self
                .mouse
                .refused_before_dispatch(command.ticket())
                .map_err(Error::MouseLedger)?,
        }
        self.pending = None;
        self.stop();
        Ok(())
    }
    /// Dispatched downs/impulses retain their pending completion. No new wheel,
    /// down or source wait is admitted after stop. This function sends nothing.
    pub fn stop(&mut self) {
        self.stopping = true;
        self.deadline = None;
        self.keyboard.stop();
        self.mouse.stop();
        if self
            .pending
            .as_ref()
            .is_some_and(|p| !p.dispatched && p.command.direction != Direction::Up)
        {
            self.pending = None;
        }
    }
}
