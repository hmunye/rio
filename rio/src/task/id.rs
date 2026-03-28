use std::cell::Cell;
use std::fmt;

use crate::rt::context;

thread_local! {
    /// Monotonic counter for constructing task [`Id`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for a task relative to all other currently running tasks.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Id(u64);

impl Id {
    #[must_use]
    pub(crate) fn next() -> Self {
        Id(IDS.replace(IDS.get() + 1))
    }

    /// Consumes `self`, returning its numeric value.
    #[must_use]
    pub const fn val(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Returns the [`Id`] of the currently running task on the current thread.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn foo() {
///     println!("task #{}", rio::task::id()); // task #0
/// }
///
/// fn main() {
///     let rt = rio::rt::Runtime::new();
///
///     rt.block_on(async {
///         foo().await;
///     });
/// }
/// ```
#[inline]
#[must_use]
pub fn id() -> Id {
    context::current_task().expect("no runtime context associated with the current thread")
}
