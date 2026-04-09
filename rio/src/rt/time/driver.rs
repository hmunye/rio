use std::cell::RefCell;
use std::task::Waker;
use std::time::{Duration, Instant};

use crate::rt::time::{Clock, TimerHandle, TimerHeap};

/// Driver for managing asynchronous delays and time-based events within the
/// runtime.
#[derive(Debug)]
pub struct Driver {
    timers: RefCell<TimerHeap>,
    clock: Clock,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Driver {
            timers: RefCell::default(),
            clock: Clock::new(),
        }
    }

    /// Registers a timer with the driver, returning its [`TimerHandle`]
    ///
    /// The timer will track `deadline`, and `waker` will be notified when the
    /// deadline has elapsed.
    pub fn register_timer(&self, deadline: Instant, waker: Waker) -> TimerHandle {
        self.timers.borrow_mut().push(deadline, waker)
    }

    /// Attempts to update the `deadline` of the timer identified by `handle`,
    /// returning `true` if successful.
    pub fn update_timer(&self, handle: &TimerHandle, deadline: Instant) -> bool {
        let mut timers = self.timers.borrow_mut();

        if let Some((entry, idx)) = timers.get_mut(handle) {
            entry.mark_pending();
            timers.update_priority_with_idx(idx, deadline)
        } else {
            false
        }
    }

    /// Cancels the timer identified by `handle`, removing it from the driver.
    pub fn cancel_timer(&self, handle: &TimerHandle) {
        self.timers.borrow_mut().remove(handle);
    }

    /// Drives the timers registered with the driver, returning a timeout
    /// duration corresponding to the earliest pending timer, if one exist.
    ///
    /// Notifies all `Waker`s whose time-based events (e.g., timers) have
    /// elapsed, ensuring the associated tasks are ready to be polled by the
    /// scheduler.
    pub fn drive(&self) -> Option<Duration> {
        self.drive_timers()
    }

    /// Processes timers whose deadlines have elapsed, returning a timeout
    /// duration corresponding to the earliest pending timer, if one exist.
    ///
    /// For each timer that has reached its deadline, its registered `Waker` is
    /// notified. Timers with deadlines not yet elapsed remain registered.
    fn drive_timers(&self) -> Option<Duration> {
        let mut timers = self.timers.borrow_mut();

        if timers.is_empty() {
            return None;
        }

        let mut timeout = None;
        let now = self.clock.now();

        let mut iter = timers.heap_iter();

        while let Some(entry) = iter.next_entry() {
            if entry.is_fired() {
                continue;
            }

            let deadline = entry.deadline;

            if deadline <= now {
                entry.waker.wake_by_ref();
                entry.mark_fired();
            } else {
                timeout = Some(deadline.duration_since(now));
                // Since the earliest deadline in the heap hasn't elapsed, all
                // other deadlines are guaranteed not to have elapsed either.
                break;
            }
        }

        timeout
    }
}

cfg_test! {
    impl Driver {
        /// Returns a reference to the time abstraction source used by the driver.
        pub const fn clock(&self) -> &Clock {
            &self.clock
        }

        /// Returns the number of timers registered with the driver.
        pub fn timers(&self) -> usize {
            self.timers.borrow().len()
        }
    }
}
