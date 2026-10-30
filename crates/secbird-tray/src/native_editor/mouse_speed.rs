//! Source196 count/ms feedback: absolute axis sums,60Hz leading/trailing publish,
//! reset accumulator at publication, and150ms display expiry. No OS access.
const FRAME_TICKS: u64 = 166_667; //100ns ticks; monotonic clock is the native port's choice.
const DISPLAY_EXPIRY_TICKS: u64 = 1_500_000;
#[derive(Default)]
pub(super) struct Speed {
    previous: Option<u64>,
    sum_x: f64,
    sum_y: f64,
    elapsed: u64,
    pending: Option<f64>,
    published_at: Option<u64>,
    displayed: f64,
}
impl Speed {
    pub(super) fn value(&self) -> f64 {
        self.displayed
    }
    pub(super) fn needs_tick(&self) -> bool {
        self.pending.is_some() || self.displayed != 0.
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(super) fn push(&mut self, x: i32, y: i32, tick: u64) -> bool {
        if x == 0 && y == 0 {
            return false;
        }
        if let Some(previous) = self.previous {
            if tick > previous {
                self.sum_x += f64::from(x.unsigned_abs());
                self.sum_y += f64::from(y.unsigned_abs());
                self.elapsed = self.elapsed.saturating_add(tick - previous);
            }
        }
        self.previous = Some(tick);
        if self.elapsed > 0 {
            let speed = self.sum_x.hypot(self.sum_y) / (self.elapsed as f64 / 10_000.);
            self.pending = Some(if speed.is_finite() { speed.abs() } else { 0. });
        }
        self.tick(tick)
    }
    pub(super) fn tick(&mut self, now: u64) -> bool {
        let before = self.displayed;
        if self.pending.is_some()
            && self
                .published_at
                .is_none_or(|at| now.saturating_sub(at) >= FRAME_TICKS)
        {
            self.displayed = self.pending.take().unwrap();
            self.published_at = Some(now);
            self.previous = None;
            self.sum_x = 0.;
            self.sum_y = 0.;
            self.elapsed = 0;
        }
        if self
            .published_at
            .is_some_and(|at| now.saturating_sub(at) >= DISPLAY_EXPIRY_TICKS)
        {
            self.displayed = 0.;
        }
        self.displayed != before
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_movement_establishes_time_then_leading_publish_resets_counts() {
        let mut speed = Speed::default();
        assert!(!speed.push(100, 100, 10));
        assert!(speed.push(3, 4, 10_010));
        assert_eq!(speed.value(), 5.);
        assert!(!speed.push(100, 100, 20_010));
        assert!(!speed.push(6, 8, 30_010));
        assert!(speed.tick(176_677));
        assert_eq!(speed.value(), 10.);
    }
    #[test]
    fn reversed_axes_accumulate_absolute_counts_and_trailing_uses_latest_ratio() {
        let mut speed = Speed::default();
        speed.push(1, 1, 10);
        speed.push(3, 4, 10_010);
        speed.push(99, 99, 20_010);
        speed.push(3, 4, 30_010);
        speed.push(-3, -4, 80_010);
        assert_eq!(speed.value(), 5.);
        assert!(speed.tick(176_677));
        assert!((speed.value() - 10. / 6.).abs() < 1e-12);
    }
    #[test]
    fn display_expires_without_rewriting_accumulator_and_clear_drops_pending() {
        let mut speed = Speed::default();
        speed.push(1, 1, 10);
        speed.push(3, 4, 10_010);
        assert!(!speed.tick(1_510_009));
        assert!(speed.tick(1_510_010));
        assert_eq!(speed.value(), 0.);
        speed.push(1, 1, 1_520_000);
        speed.push(i32::MIN, 0, 1_530_000);
        assert!(speed.value().is_finite());
        speed.clear();
        assert_eq!(speed.value(), 0.);
        assert!(!speed.tick(u64::MAX));
    }
    #[test]
    fn repeated_or_backwards_clock_and_button_only_records_do_not_create_speed() {
        let mut speed = Speed::default();
        speed.push(1, 1, 100);
        speed.push(2, 2, 100);
        speed.push(2, 2, 99);
        speed.push(0, 0, 200);
        assert_eq!(speed.value(), 0.);
    }
}
