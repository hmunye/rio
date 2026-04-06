use std::cell::Cell;

use crate::rt::context;

thread_local! {
    /// Monotonic counter for constructing [`TimerHandle`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for a timer relative to all other timers.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct RawHandle(u64);

/// Opaque identifier for a timer returned by [`TimerHeap::push`].
///
/// Cancels the associated timer entry on `Drop`.
///
/// [`TimerHeap::push`]: crate::rt::time::TimerHeap::push
#[derive(Debug, PartialEq, Eq)]
pub struct TimerHandle(RawHandle);

impl TimerHandle {
    #[must_use]
    pub fn new() -> Self {
        TimerHandle(RawHandle(IDS.replace(IDS.get() + 1)))
    }

    #[must_use]
    pub const fn raw(&self) -> RawHandle {
        self.0
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
