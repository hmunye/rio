use std::cell::Cell;
use std::fmt;

use crate::runtime::context;

thread_local! {
    /// Monotonic counter for creating [`Id`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque ID for uniquely identifying a task relative to all other currently
/// running tasks.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct Id(pub(crate) u64);

impl Id {
    #[inline]
    pub(crate) fn next() -> Self {
        Id(IDS.with(|i| i.replace(i.get() + 1)))
    }

    /// Returns the raw numeric value of the task ID.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Returns the [`Id`] of the "active task" on the current thread.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn foo() {
///     // ID of the currently "active" task.
///     println!("task ID: {}", rio::task::id());
/// }
///
/// fn main() {
///     let rt = rio::runtime::Runtime::new();
///
///     rt.block_on(async {
///         foo().await;
///     });
/// }
/// ```
#[inline]
#[must_use]
pub fn id() -> Id {
    context::current_task_id().expect("no runtime context associated with the current thread")
}
