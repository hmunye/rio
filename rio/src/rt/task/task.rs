use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::task;

/// Lightweight, non‑blocking unit of execution (**green thread**) scheduled by
/// the runtime.
pub struct Task {
    pub id: task::Id,
    fut: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// Creates a new `Task`, using the provided closure to handle the output of
    /// `fut`.
    #[must_use]
    pub fn new_with<Fut, F>(fut: Fut, f: F) -> Self
    where
        Fut: Future + 'static,
        F: FnOnce(Fut::Output) + 'static,
    {
        Task {
            id: task::Id::next(),
            fut: Box::pin(async move {
                f(fut.await);
            }),
        }
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.fut.as_mut().poll(cx)
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .field("fut", &std::any::type_name_of_val(&self.fut))
            .finish()
    }
}
