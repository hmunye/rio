use std::cell::RefCell;
use std::task::Waker;
use std::time::{Duration, Instant};

use crate::rt::time::{RawHandle, TimerHandle, TimerHeap};

/// Driver for managing asynchronous delays and time-based events within the
/// runtime.
#[derive(Debug)]
pub struct Driver {
    timers: RefCell<TimerHeap>,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Driver {
            timers: RefCell::default(),
        }
    }

    /// Registers a timer with the driver, returning its [`TimerHandle`]
    ///
    /// The timer will track `deadline`, and `waker` will be notified when the
    /// deadline has elapsed.
    pub fn register_timer(&self, deadline: Instant, waker: Waker) -> TimerHandle {
        self.timers.borrow_mut().push(deadline, waker)
    }

    /// Attempts to update the `deadline` of the timer identified by
    /// `raw_handle`, returning `true` if successful.
    pub fn update_timer(&self, raw_handle: RawHandle, deadline: Instant) -> bool {
        self.timers
            .borrow_mut()
            .update_priority(raw_handle, deadline)
    }

    /// Cancels the timer identified by `raw_handle`, ensuring it does not fire.
    pub fn cancel_timer(&self, raw_handle: RawHandle) {
        self.timers.borrow_mut().remove(raw_handle);
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
        let now = Instant::now();

        let mut iter = timers.heap_iter();

        while let Some(entry) = iter.next_entry() {
            let deadline = entry.deadline;

            if deadline <= now {
                entry.waker.wake_by_ref();
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
