use std::cell::UnsafeCell;
use std::task::Waker;
use std::time::Instant;

use crate::runtime::time::{MinHeap, TimerEntry};

/// Driver for scheduling tasks after a set period of time.
#[derive(Debug)]
pub struct Driver {
    /// Priority queue of timers associated with tasks, keyed by their scheduled
    /// wake-up time.     
    timers: UnsafeCell<MinHeap<TimerEntry>>,
}

impl Driver {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Driver {
            timers: UnsafeCell::default(),
        }
    }

    #[inline]
    pub fn register_timer(&self, deadline: Instant, waker: Waker) {
        // SAFETY: `self.timers` is not mutably aliased when calling this
        // method.
        unsafe {
            (*self.timers.get()).push(TimerEntry { deadline, waker });
        }
    }

    pub fn process_timers(&self) {
        // SAFETY: `self.timers` is not mutably aliased when calling
        // this method. `is_empty` method also does not modify the state of
        // `self.timers`.
        unsafe {
            if (*self.timers.get()).is_empty() {
                return;
            }
        }

        let time_now = Instant::now();

        loop {
            unsafe {
                // SAFETY: `self.timers` is not mutably aliased when calling
                // this method.
                let Some(entry) = (*self.timers.get()).pop() else {
                    // No timers have been registered.
                    break;
                };

                if entry.deadline <= time_now {
                    entry.waker.wake();
                } else {
                    // SAFETY: `self.timers` is not mutably aliased when
                    // calling this method.
                    (*self.timers.get()).push(entry);

                    // Since the earliest deadline in the heap hasn't been
                    // reached, all other deadline are guaranteed not to have
                    // been reached either, so stop processing.
                    break;
                }
            }
        }
    }
}
