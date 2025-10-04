use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// Handle to a `Task`, using `Rc` and `RefCell` for shared ownership and
/// interior mutability in single-threaded contexts.
pub(crate) type TaskHandle = Rc<RefCell<Task>>;

/// Uniquely identifier for a single `Task`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Id(u64);

impl Id {
    #[inline]
    const fn new() -> Self {
        // Ensures each `Task` can be assigned a unique ID.
        static NEXT_ID: u64 = 0;

        Id(NEXT_ID + 1)
    }
}

/// Lightweight, non-blocking units of execution, similar to OS threads, but
/// rather than being managed by the OS scheduler, they are managed by the
/// [runtime].
///
/// [runtime]: crate::rt
pub(crate) struct Task {
    /// Unique identifier for a task.
    #[allow(dead_code)]
    pub(crate) id: Id,
    /// Pinned, heap-allocated, type-erased future.
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// Create a new `Task` using the provided future.
    #[inline]
    pub(crate) fn new<F: Future<Output = ()> + 'static>(future: F) -> Self {
        Task {
            id: Id::new(),
            future: Box::pin(future),
        }
    }

    /// Polls the inner future, returning a `Poll<()>`.
    #[inline]
    pub(crate) fn poll(&mut self, ctx: &mut Context<'_>) -> Poll<()> {
        self.future.as_mut().poll(ctx)
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").finish()
    }
}
