use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::runtime::task::Id;

/// Lightweight, non-blocking unit of execution (a "green thread"), managed by
/// the `rio` runtime.
///
/// The task wraps a [`Future`] where [`Future::Output`] is type-erased. The
/// yielded result is captured externally (e.g., by a [`JoinHandle<T>`]).
///
/// [`JoinHandle<T>`]:
pub struct Task {
    pub(crate) id: Id,
    fut: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    #[inline]
    pub fn new<F: Future<Output = ()> + 'static>(fut: F) -> Self {
        Task {
            id: Id::next(),
            fut: Box::pin(fut),
        }
    }

    #[inline]
    pub fn poll(&mut self, ctx: &mut Context<'_>) -> Poll<()> {
        self.fut.as_mut().poll(ctx)
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
