use std::cmp::Ordering;
use std::task::Waker;
use std::time::Instant;

/// Wrapper for a deadline and associated [`Waker`].
#[derive(Debug)]
pub struct TimerEntry {
    /// When the timer is set to expire.
    pub(crate) deadline: Instant,
    /// `Waker` to wake when the timer expires.
    pub(crate) waker: Waker,
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for TimerEntry {}
