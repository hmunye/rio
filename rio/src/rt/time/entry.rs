use std::cmp::Ordering;
use std::task::Waker;
use std::time::Instant;

#[derive(Debug)]
pub struct TimerEntry {
    pub deadline: Instant,
    pub waker: Waker,
}

impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // NOTE: Inverted so it is in min-heap order within `BinaryHeap`.
        other.deadline.cmp(&self.deadline)
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
