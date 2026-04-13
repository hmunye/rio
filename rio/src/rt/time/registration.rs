use std::{cell::Cell, time::Instant};

use crate::rt::context;

thread_local! {
    /// Monotonic counter for constructing [`TimerHandle`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for a timer relative to all other timers.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct RawHandle(u64);

/// Handle to a timer returned by [`Driver::register_timer`].
///
/// Cancels the associated timer on `Drop`.
///
/// [`Driver::register_timer`]: crate::rt::time::Driver::register_timer
#[derive(Debug, PartialEq, Eq)]
pub struct TimerHandle(pub RawHandle);

impl TimerHandle {
    #[must_use]
    pub fn next() -> Self {
        TimerHandle(RawHandle(IDS.replace(IDS.get() + 1)))
    }

    pub fn reset(&self, deadline: Instant) -> bool {
        context::with_handle(|handle| handle.update_timer(self, deadline))
    }

    fn cancel(&self) {
        context::with_handle(|handle| handle.cancel_timer(self));
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}
