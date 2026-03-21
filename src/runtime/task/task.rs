use std::cell::RefCell;
use std::fmt;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll};

use crate::runtime::task::{Id, LocalWaker};

/// Lifecycle stages of a task.
#[derive(Debug, Default)]
pub enum Stage {
    #[default]
    Running,
    Finished(Box<dyn std::any::Any>),
    Consumed,
}

/// Shared state for accessing a task's resolved output.
#[derive(Debug, Default)]
pub struct TaskState {
    pub(crate) stage: RefCell<Stage>,
    pub(crate) waker: RefCell<Option<LocalWaker>>,
}

/// Lightweight, non-blocking unit of execution ("green thread"), managed by the
/// `rio` runtime.
///
/// A [`JoinHandle<T>`] can be used to await the output of the spawned task.
///
/// [`JoinHandle<T>`]: crate::runtime::task::JoinHandle
pub struct Task {
    pub(crate) id: Id,
    pub(crate) state: Rc<TaskState>,
    fut: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    #[inline]
    pub fn new_with<F, Fut>(f: F) -> Self
    where
        F: FnOnce(Weak<TaskState>) -> Fut,
        Fut: Future<Output = ()> + 'static,
    {
        let state = Rc::new(TaskState::default());

        Task {
            id: Id::next(),
            fut: Box::pin(f(Rc::downgrade(&state))),
            state,
        }
    }

    #[inline]
    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.fut.as_mut().poll(cx)
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        matches!(
            *self.state.stage.borrow(),
            Stage::Finished(_) | Stage::Consumed
        )
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
