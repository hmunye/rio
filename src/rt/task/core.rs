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

/// Shared handle to a [`Task`] for single-threaded contexts.
pub(crate) type TaskHandle = Rc<RefCell<Task>>;

/// Unique identifier for a [`Task`].
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct TaskId(pub(crate) u64);

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

impl From<u64> for TaskId {
    fn from(value: u64) -> Self {
        TaskId(value)
    }
}

/// Lightweight, non-blocking unit of execution, similar to an OS thread, but
/// rather than being managed by the OS scheduler, it is managed by the
/// [runtime].
///
/// [runtime]: crate::rt
pub(crate) struct Task {
    /// Used to uniquely identify a task.
    pub(crate) id: TaskId,
    /// Pinned, heap-allocated, type-erased [`Future`].
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

    /// Polls the inner future, returning the [`Poll`] result.
    #[inline]
    pub(crate) fn poll(&mut self, ctx: &mut Context<'_>) -> Poll<()> {
        println!("poll (in Task): polling task {:?}", self.id);
        self.future.as_mut().poll(ctx)
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .field("scheduled", &self.scheduled)
            .finish()
    }
}
