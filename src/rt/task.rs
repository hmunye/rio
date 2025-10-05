use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

thread_local! {
    /// Guarantees that each `Task` is assigned a unique ID.
    static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}

/// Handle to a `Task`, using `Rc` and `RefCell` for shared ownership and
/// interior mutability in single-threaded contexts.
pub(crate) type TaskHandle = Rc<RefCell<Task>>;

/// Uniquely identifier for a single `Task`.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TaskId(u64);

impl TaskId {
    #[inline]
    fn new() -> Self {
        TaskId(NEXT_ID.with(|c| {
            let id = c.get();
            c.set(id + 1);
            id
        }))
    }
}

/// Lightweight, non-blocking units of execution, similar to OS threads, but
/// rather than being managed by the OS scheduler, they are managed by the
/// [runtime].
///
/// [runtime]: crate::rt
pub(crate) struct Task {
    /// Unique identifier for a task.
    pub(crate) id: TaskId,
    /// Pinned, heap-allocated, type-erased future.
    future: Pin<Box<dyn Future<Output = ()>>>,
    /// Indicates whether the task has already been scheduled for polling. This
    /// avoids re-queuing already scheduled tasks.
    pub(crate) scheduled: Cell<bool>,
}

impl Task {
    /// Create a new `Task` using the provided future.
    #[inline]
    pub(crate) fn new<F: Future<Output = ()> + 'static>(future: F) -> Self {
        Task {
            id: TaskId::new(),
            future: Box::pin(future),
            scheduled: Cell::new(false),
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
