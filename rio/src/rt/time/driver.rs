use std::cell::RefCell;
use std::collections::BinaryHeap;
use std::task::Waker;
use std::time::Instant;

use crate::rt::time::TimerEntry;

/// Driver for managing asynchronous delays and time-based events within the
/// runtime.
#[derive(Debug)]
pub struct Driver {
    timers: RefCell<BinaryHeap<TimerEntry>>,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Driver {
            timers: RefCell::default(),
        }
    }

    /// Registers a timer with the driver.
    ///
    /// The timer will track `deadline`, and `waker` will be notified when the
    /// deadline has elapsed.
    pub fn register_timer(&self, deadline: Instant, waker: Waker) {
        self.timers
            .borrow_mut()
            .push(TimerEntry { deadline, waker });
    }

    /// Drives the timers registered with the driver.
    ///
    /// Notifies all `Waker`s whose time-based events (e.g., timers) have
    /// elapsed, ensuring the associated tasks are ready to be polled by the
    /// scheduler.
    pub fn drive(&self) {
        self.drive_timers();
    }

    /// Processes timers whose deadlines have elapsed.
    ///
    /// For each timer that has reached its deadline, its registered `Waker` is
    /// notified. Timers with deadlines not yet elapsed remain registered.
    fn drive_timers(&self) {
        let mut timers = self.timers.borrow_mut();

        if timers.is_empty() {
            return;
        }

        let now = Instant::now();

        while let Some(entry) = timers.peek() {
            if entry.deadline <= now
                && let Some(entry) = timers.pop()
            {
                entry.waker.wake();
            } else {
                // Since the earliest deadline in the heap hasn't elapsed, all
                // other deadlines are guaranteed not to have elapsed either.
                break;
            }
        }
    }
}
