//! Bounded Reactive diagnostic replay. No default/guessed vendor animation.
//!
//! Callers must supply a pulse recording and explicitly choose retrigger policy.
//! This is deliberately separate from Preview::from_snapshot until native fade,
//! random-colour and retrigger semantics have been established. No device I/O,
//! keyboard registration, generated input, wall clock or event log lives here.
use super::{
    invalid,
    reactive_input::{KeyboardKey, LedAddress, ReactiveInputMap},
};
use crate::{
    lighting_pixels::{BlendMode, Layer, Pixel},
    Result, Rgb,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const MAX_PIXELS: usize = 512;
const MAX_REGIONS: usize = 128;
const MAX_STORAGE: usize = 4 * 1024 * 1024;

/// No implicit restart/additive/queue behavior is asserted for vendor Reactive.
#[derive(Clone, Copy)]
pub enum Retrigger {
    Restart,
    IgnoreWhileActive,
}

/// Identity of one explicitly started replay. Tokens from another instance or
/// from before stop/unplug/restart cannot feed the current session.
#[derive(Clone)]
pub struct ReplaySession(Arc<()>);

/// Caller-owned samples at a declared cadence; never inferred from duration.
pub struct PulseRecording {
    interval_ms: u32,
    samples: Box<[Pixel]>,
}

impl PulseRecording {
    pub fn new(interval_ms: u32, samples: &[Pixel]) -> Result<Self> {
        if interval_ms == 0 || interval_ms > 1000 || samples.is_empty() || samples.len() > 16384 {
            return Err(invalid(
                "Reactive recording needs 1..16384 samples at 1..1000ms",
            ));
        }
        Ok(Self {
            interval_ms,
            samples: samples.into(),
        })
    }
}

pub struct ReplayRegion {
    pub leds: Vec<LedAddress>,
    pub pulse: PulseRecording,
}
struct Region {
    pulse: PulseRecording,
    // Exactly one state slot per admitted LED. No trigger queue can grow.
    active: Vec<(LedAddress, Option<u64>)>,
}
pub struct ReactiveReplay {
    input: ReactiveInputMap,
    indices: BTreeMap<LedAddress, usize>,
    regions: Vec<Region>,
    held: BTreeSet<KeyboardKey>,
    session: Option<ReplaySession>,
    last_ms: u64,
    retrigger: Retrigger,
    scratch: Box<[Pixel]>,
}
impl ReactiveReplay {
    /// `inventory` is the render grid, `regions` are in compositing order. Both
    /// must come from the selected composition; this does not authenticate assets.
    pub fn new(
        input: ReactiveInputMap,
        inventory: &[LedAddress],
        regions: Vec<ReplayRegion>,
        retrigger: Retrigger,
    ) -> Result<Self> {
        if inventory.is_empty() || inventory.len() > MAX_PIXELS || regions.len() > MAX_REGIONS {
            return Err(invalid("Reactive replay exceeds LED/region budget"));
        }
        let mut indices = BTreeMap::new();
        for (index, &led) in inventory.iter().enumerate() {
            if indices.insert(led, index).is_some() {
                return Err(invalid("Reactive render grid has duplicate LEDs"));
            }
        }
        let mut bytes = inventory.len() * std::mem::size_of::<Pixel>();
        let mut compiled = Vec::with_capacity(regions.len());
        for region in regions {
            let mut seen = BTreeSet::new();
            for &led in &region.leds {
                if !indices.contains_key(&led) || !seen.insert(led) {
                    return Err(invalid("Reactive region LED is foreign or duplicated"));
                }
            }
            bytes += region.pulse.samples.len() * std::mem::size_of::<Pixel>()
                + region.leds.len() * std::mem::size_of::<(LedAddress, Option<u64>)>();
            if bytes > MAX_STORAGE {
                return Err(invalid("Reactive replay exceeds 4MiB pulse budget"));
            }
            compiled.push(Region {
                pulse: region.pulse,
                active: region.leds.into_iter().map(|led| (led, None)).collect(),
            });
        }
        Ok(Self {
            input,
            indices,
            regions: compiled,
            held: BTreeSet::new(),
            session: None,
            last_ms: 0,
            retrigger,
            scratch: vec![Pixel::EMPTY; inventory.len()].into_boxed_slice(),
        })
    }
    pub fn start_local_preview(&mut self) -> ReplaySession {
        self.stop();
        let session = ReplaySession(Arc::new(()));
        self.session = Some(session.clone());
        session
    }
    /// Also used on unplug, source error, identity change and ownership loss.
    /// A later start is explicit; no reconnect replay or old held state survives.
    pub fn stop(&mut self) {
        self.session = None;
        self.last_ms = 0;
        self.held.clear();
        self.scratch.fill(Pixel::EMPTY);
        for region in &mut self.regions {
            for (_, active) in &mut region.active {
                *active = None;
            }
        }
    }
    /// A late source-loss notification cannot cancel a newer session.
    pub fn stop_session(&mut self, session: &ReplaySession) -> bool {
        if !self.accepts(session) {
            return false;
        }
        self.stop();
        true
    }
    fn accepts(&self, session: &ReplaySession) -> bool {
        self.session
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(&current.0, &session.0))
    }
    fn clock(&mut self, now_ms: u64) -> Result<()> {
        if now_ms < self.last_ms {
            self.stop();
            return Err(invalid(
                "Reactive replay clock moved backwards; session stopped",
            ));
        }
        self.last_ms = now_ms;
        Ok(())
    }
    /// Synthetic or separately authorized exact-device transitions. Repeats are
    /// ignored while held; source/vendor repeat semantics remain unverified.
    /// No event payloads are retained. Returns the count of addressed LED slots.
    pub fn key(
        &mut self,
        session: &ReplaySession,
        placement: &str,
        key: KeyboardKey,
        down: bool,
        now_ms: u64,
    ) -> Result<usize> {
        if !self.accepts(session) || placement != self.input.placement() {
            return Ok(0);
        }
        self.clock(now_ms)?;
        let leds = self.input.leds(placement, key);
        if leds.is_empty() {
            return Ok(0);
        }
        if !down {
            self.held.remove(&key);
            return Ok(0);
        }
        if !self.held.insert(key) {
            return Ok(0);
        }
        let mut count = 0;
        for region in &mut self.regions {
            let duration = region.pulse.interval_ms as u64 * region.pulse.samples.len() as u64;
            for (led, active) in &mut region.active {
                if leds.contains(led)
                    && (matches!(self.retrigger, Retrigger::Restart)
                        || active.is_none_or(|at| now_ms - at >= duration))
                {
                    *active = Some(now_ms);
                    count += 1;
                }
            }
        }
        Ok(count)
    }
    /// Blend into the caller's existing lower-layer frame. None samples reveal
    /// that frame; black samples cover it. Brightness is applied once, after
    /// composition. A stopped/stale session never modifies the caller's frame.
    pub fn blend(
        &mut self,
        session: &ReplaySession,
        now_ms: u64,
        brightness: u8,
        output: &mut [Rgb],
    ) -> Result<bool> {
        if !self.accepts(session) {
            return Ok(false);
        }
        if brightness > 100 || output.len() != self.scratch.len() {
            return Err(invalid("Reactive frame size/brightness is invalid"));
        }
        self.clock(now_ms)?;
        for region in &mut self.regions {
            self.scratch.fill(Pixel::EMPTY);
            for (led, active) in &mut region.active {
                let Some(at) = *active else {
                    continue;
                };
                let frame = (now_ms - at) / u64::from(region.pulse.interval_ms);
                if frame >= region.pulse.samples.len() as u64 {
                    *active = None;
                    continue;
                }
                self.scratch[self.indices[led]] = region.pulse.samples[frame as usize];
            }
            Layer {
                pixels: &self.scratch,
                opacity: 1.0,
                mask: None,
                mode: BlendMode::Transparency,
            }
            .blend(output)?;
        }
        for pixel in output {
            *pixel = Rgb(
                (u32::from(pixel.0) * u32::from(brightness) / 100) as u8,
                (u32::from(pixel.1) * u32::from(brightness) / 100) as u8,
                (u32::from(pixel.2) * u32::from(brightness) / 100) as u8,
            );
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "reactive_replay_tests.rs"]
mod tests;
