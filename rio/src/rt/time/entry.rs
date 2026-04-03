use std::cmp::Ordering;
use std::task::Waker;
use std::time::Instant;

use crate::rt::time::RawHandle;

#[derive(Debug)]
pub struct TimerEntry {
    pub deadline: Instant,
    pub waker: Waker,
    pub raw_handle: RawHandle,
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
