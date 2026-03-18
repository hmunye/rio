use std::cell::Cell;
use std::fmt;

use crate::runtime::context;

thread_local! {
    /// Monotonic counter for assigning IDs.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque ID for uniquely identifying a task relative to all other currently
/// running tasks.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Id(pub(crate) u64);

impl Id {
    pub fn next() -> Self {
        Id(IDS.with(|i| i.replace(i.get() + 1)))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Returns the [`Id`] of the currently active task on the current thread.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
#[allow(unused)]
pub fn id() -> Id {
    context::current_task_id().expect("function called outside of a runtime-context")
}
