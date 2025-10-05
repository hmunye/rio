use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::task::Context;

use crate::rt::task::{TaskHandle, TaskId};
use crate::rt::waker::TaskWaker;

type TaskEntry = (TaskHandle, TaskWaker);

/// Single-threaded `Task` scheduler.
#[derive(Debug)]
pub(crate) struct Scheduler {
    /// Stores all live tasks keyed by their ID, each paired with a `TaskWaker`.
    /// Enables efficient `O(1)` lookup.
    tasks: RefCell<HashMap<TaskId, TaskEntry>>,
    /// Queue of task IDs ready to be polled. Storing only IDs keeps the queue
    /// lightweight and avoids cloning or holding multiple `Task` handles.
    /// `RefCell` allows `TaskWaker`s to have shared mutable access.
    ready: RefCell<VecDeque<TaskId>>,
    /// Holds tasks that are spawned while the scheduler is actively running,
    /// preventing reentrant mutable borrows. Each `tick` borrows the scheduler
    /// fields mutably, but spawning also requires a mutable borrow. To avoid
    /// double-borrowing during active polling, newly spawned tasks are
    /// temporarily stored here and later transferred on each tick.
    pending: RefCell<Vec<TaskEntry>>,
}

impl Scheduler {
    /// Creates a new `Scheduler`.
    #[inline]
    pub(crate) fn new() -> Self {
        Scheduler {
            tasks: Default::default(),
            ready: RefCell::new(Default::default()),
            pending: Default::default(),
        }
    }

    /// Schedules the given `TaskHandle` and associated `TaskWaker`, blocking
    /// the current thread until the underlying `Task` resolves.
    pub(crate) fn block_on_task(&self, task: TaskHandle, waker: TaskWaker) {
        let id = task.borrow().id;

        self.schedule(id);
        self.tasks.borrow_mut().insert(id, (task, waker));

        // Temporarily spinning.
        while !self.tasks.borrow().is_empty() || !self.pending.borrow().is_empty() {
            self.tick();
        }
    }

    /// Schedules the given `TaskHandle` and associated `TaskWaker`, executing
    /// it concurrently with other tasks.
    pub(crate) fn spawn_task(&self, task: TaskHandle, waker: TaskWaker) {
        self.pending.borrow_mut().push((task, waker));
    }

    /// Marks the `Task` associated with the provided ID as ready to be polled.
    #[inline]
    pub(crate) fn schedule(&self, id: TaskId) {
        self.ready.borrow_mut().push_back(id);
    }

    /// Polls all currently ready tasks on the `ready` queue, handling any
    /// pending spawned tasks as well.
    fn tick(&self) {
        // Process all pending spawned tasks.
        {
            let mut pending = self.pending.borrow_mut();
            for (task, waker) in pending.drain(..) {
                let id = task.borrow().id;
                self.schedule(id);
                self.tasks.borrow_mut().insert(id, (task, waker));
            }
        }

        while let Some(id) = self.ready.borrow_mut().pop_front() {
            // Temporarily remove the task entry from the map.
            let entry = self.tasks.borrow_mut().remove(&id);
            let Some((task, waker)) = entry else {
                continue;
            };

            // Mark as not currently scheduled.
            task.borrow().scheduled.set(false);

            let mut ctx = Context::from_waker(&waker);
            let poll = {
                let mut task_ref = task.borrow_mut();
                task_ref.poll(&mut ctx)
            };

            if poll.is_pending() {
                // Re-insert the (task, waker) for future polling.
                self.tasks.borrow_mut().insert(id, (task, waker));
            }

            // Drop the `TaskHandle` and `TaskWaker` if `Poll::Ready`...
        }
    }
}
