use std::cmp::Ordering;
use std::task::Waker;
use std::time::Instant;

use crate::rt::time::RawHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Pending,
    Fired,
}

#[derive(Debug, Clone)]
pub struct TimerEntry {
    pub deadline: Instant,
    pub waker: Waker,
    pub raw_handle: RawHandle,
    state: State,
}

impl TimerEntry {
    #[must_use]
    pub const fn new(deadline: Instant, waker: Waker, raw_handle: RawHandle) -> Self {
        Self {
            deadline,
            waker,
            raw_handle,
            state: State::Pending,
        }
    }

    pub fn mark_fired(&mut self) {
        debug_assert!(!self.is_fired(), "timer is already `Fired`");
        self.state = State::Fired;
    }

    pub fn mark_pending(&mut self) {
        debug_assert!(self.is_fired(), "timer is already `Pending`");
        self.state = State::Pending;
    }

    pub fn is_fired(&self) -> bool {
        self.state == State::Fired
    }
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
