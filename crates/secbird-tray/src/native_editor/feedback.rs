//! Acknowledged native failure-dialog queue, an explicit port adaptation.
use std::collections::{HashSet, VecDeque};
#[derive(Default)]
pub(super) struct Failures {
    pending: VecDeque<String>,
    active: Option<String>,
    acknowledged: HashSet<String>,
}
impl Failures {
    pub fn record(&mut self, message: &str) -> bool {
        if message.is_empty()
            || self.active.as_deref() == Some(message)
            || self.pending.iter().any(|m| m == message)
            || self.acknowledged.contains(message)
        {
            return false;
        }
        self.pending.push_back(message.to_string());
        true
    }
    pub fn begin(&mut self) -> Option<String> {
        if self.active.is_some() {
            return None;
        }
        self.active = self.pending.pop_front();
        self.active.clone()
    }
    pub fn acknowledge(&mut self) {
        if let Some(message) = self.active.take() {
            self.acknowledged.insert(message);
        }
    }
    pub fn outstanding(&self) -> bool {
        self.active.is_some() || !self.pending.is_empty()
    }
    pub fn new_operation(&mut self) {
        self.acknowledged.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_pending_failures_survive_acknowledgement_without_duplicate_prompts() {
        let mut f = Failures::default();
        assert!(f.record("first"));
        assert!(!f.record("first"));
        assert!(f.record("second"));
        assert_eq!(f.begin().as_deref(), Some("first"));
        assert!(f.begin().is_none());
        assert!(!f.record("first"));
        f.acknowledge();
        assert!(!f.record("first"));
        assert_eq!(f.begin().as_deref(), Some("second"));
        f.acknowledge();
        assert!(!f.outstanding());
        f.new_operation();
        assert!(f.record("first"));
    }
    #[test]
    fn reset_does_not_discard_active_or_pending_errors() {
        let mut f = Failures::default();
        f.record("active");
        f.begin();
        f.record("pending");
        f.new_operation();
        assert!(!f.record("active"));
        assert!(!f.record("pending"));
        f.acknowledge();
        assert_eq!(f.begin().as_deref(), Some("pending"));
    }
}
