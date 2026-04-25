use std::cell::Cell;
use std::time::Instant;

use crate::rt::context;

thread_local! {
    /// Monotonic counter for constructing [`TimerHandle`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for a timer relative to all other timers.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct RawTimerHandle(u64);

/// Handle to a timer entry returned by [`Driver::register_timer`].
///
/// Cancels the associated timer on `Drop`.
///
/// [`Driver::register_timer`]: crate::rt::time::Driver::register_timer
#[derive(Debug, PartialEq, Eq)]
pub struct TimerHandle(pub RawTimerHandle);

impl TimerHandle {
    #[must_use]
    pub fn next() -> Self {
        TimerHandle(RawTimerHandle(IDS.replace(IDS.get() + 1)))
    }

    pub fn update_deadline(&self, deadline: Instant) -> bool {
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
