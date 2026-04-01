use std::any::{self, Any};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::task::{Context, Poll, Waker};

use crate::task;

/// Lifecycle stages of a `Task`.
#[derive(Debug, Default)]
pub enum TaskStage {
    /// Task is scheduled but has not been polled yet.
    #[default]
    Scheduled,
    /// Task is currently being polled.
    Running,
    /// Task is idle but may be polled again.
    Idle,
    /// Task has completed and retains its output; cannot be polled again.
    Finished(Box<dyn Any>),
    /// Task's output has been taken by a `JoinHandle`; cannot be polled again.
    Consumed,
}

/// Internal runtime state of a `Task`.
#[derive(Debug)]
pub struct TaskState {
    pub id: task::Id,
    pub stage: RefCell<TaskStage>,
    /// `Waker` to notify the `JoinHandle` awaiting this task's output.
    pub waker: RefCell<Option<Waker>>,
    pub detached: Cell<bool>,
}

impl TaskState {
    pub fn set_stage(&self, stage: TaskStage) -> TaskStage {
        self.stage.replace(stage)
    }

    pub fn set_waker(&self, waker: Waker) {
        let mut slot = self.waker.borrow_mut();

        if !slot.as_ref().is_some_and(|w| w.will_wake(&waker)) {
            *slot = Some(waker);
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(*self.stage.borrow(), TaskStage::Running)
    }

    pub fn is_pollable(&self) -> bool {
        matches!(*self.stage.borrow(), TaskStage::Scheduled | TaskStage::Idle)
    }

    pub fn is_incomplete(&self) -> bool {
        self.is_running() || self.is_pollable()
    }

    pub const fn is_detached(&self) -> bool {
        self.detached.get()
    }
}

impl Default for TaskState {
    fn default() -> Self {
        TaskState {
            id: task::Id::next(),
            stage: RefCell::default(),
            waker: RefCell::default(),
            detached: Cell::default(),
        }
    }
}

/// Lightweight, non‑blocking unit of execution (__green thread__) scheduled by
/// the runtime.
pub struct Task {
    fut: Pin<Box<dyn Future<Output = ()>>>,
    pub state: Rc<TaskState>,
}

impl Task {
    /// Creates a new `Task`, using the provided closure to handle the output of
    /// `fut`.
    #[must_use]
    pub fn new_with<Fut, F>(fut: Fut, f: F) -> Self
    where
        Fut: Future + 'static,
        F: FnOnce(Fut::Output, Weak<TaskState>) + 'static,
    {
        let state = Rc::new(TaskState::default());
        let weak = Rc::downgrade(&state);

        Task {
            fut: Box::pin(async move {
                f(fut.await, weak);
            }),
            state,
        }
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        debug_assert!(
            !matches!(self.state.set_stage(TaskStage::Running), TaskStage::Running),
            "task #{} is already `Running` when polled",
            self.state.id
        );

        let poll = self.fut.as_mut().poll(cx);

        if self.state.is_running() {
            self.state.set_stage(TaskStage::Idle);
        }

        poll
    }

    /// Returns `true` if the task is safe to [`poll`] again.
    ///
    /// [`poll`]: Future::poll
    #[must_use]
    pub fn is_pollable(&self) -> bool {
        self.state.is_pollable()
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("fut", &any::type_name_of_val(&self.fut))
            .field("state", &self.state)
            .finish()
    }
}
