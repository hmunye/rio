use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::task::Context;

use crate::runtime::task::{self, Task};
use crate::runtime::{context, scheduler};
use crate::task::JoinHandle;

/// Single-threaded `scheduler` for the `rio` runtime.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores "active" tasks, mapping each task [`Id`] to its [`Task`] and an
    /// associated [`LocalWaker`].
    ///
    /// [`Id`]: task::Id
    /// [`LocalWaker`]: task::LocalWaker
    tasks: UnsafeCell<HashMap<task::Id, (Task, task::LocalWaker)>>,
    /// Queue of task [`Id`]s ready to be polled.
    ///
    /// [`Id`]: task::Id
    pending: UnsafeCell<VecDeque<task::Id>>,
}

impl Scheduler {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Scheduler {
            tasks: UnsafeCell::default(),
            pending: UnsafeCell::default(),
        }
    }

    /// Runs the provided future to completion on the current thread, blocking
    /// until the scheduler becomes idle (i.e., no active tasks remaining).
    pub fn block_on_fut<F: Future + 'static>(
        &self,
        handle: scheduler::Handle,
        fut: F,
    ) -> F::Output {
        // NOTE: Still ends up cloning the waker when polling `JoinHandle`.
        let task = Task::new_with(|weak| async move {
            let res = fut.await;

            if let Some(state) = weak.upgrade() {
                state.stage.replace(task::Stage::Finished(Box::new(res)));
            }
        });

        let join = JoinHandle::new(task.id, Rc::clone(&task.state));

        self.register_task(handle, task);

        while !self.is_idle() {
            self.tick();
        }

        join.take_output().expect("failed to join on blocked task")
    }

    /// Registers the provided task with the scheduler and schedules it for
    /// polling.
    #[inline]
    pub fn register_task(&self, handle: scheduler::Handle, task: task::Task) {
        let id = task.id;
        let waker = task::LocalWaker::new(task.id, handle);

        self.register_task_with_waker(task, waker);
        self.schedule_task(id);
    }

    /// Schedules the task with the specified `Id` to be polled.
    #[inline]
    pub fn schedule_task(&self, id: task::Id) {
        // SAFETY: `self.pending` is not mutably aliased when calling this
        // method.
        unsafe {
            (*self.pending.get()).push_back(id);
        }
    }

    #[inline]
    fn register_task_with_waker(&self, task: task::Task, waker: task::LocalWaker) {
        // SAFETY: `self.tasks` is not mutably aliased when calling this method.
        unsafe {
            (*self.tasks.get()).insert(task.id, (task, waker));
        }
    }

    /// Returns `true` if there are no "active" tasks remaining within the
    /// scheduler.
    #[inline]
    fn is_idle(&self) -> bool {
        // SAFETY: `self.tasks` is not mutably aliased when calling this method.
        // `is_empty` method also does not modify the state of `self.tasks`.
        unsafe { (*self.tasks.get()).is_empty() }
    }

    /// Polls all tasks currently in the pending queue.
    fn tick(&self) {
        context::with_current(|handle| handle.time.process_timers());

        // We need to Limit the scope of any mutable borrows of `self.pending`
        // and `self.tasks` to avoid mutable aliasing, as polling a task may
        // trigger child tasks to interact with the scheduler (e.g., schedule
        // themselves) during each event loop "tick".
        loop {
            unsafe {
                // SAFETY: `self.pending` is not mutably aliased when calling
                // this method.
                let Some(id) = (*self.pending.get()).pop_front() else {
                    break;
                };

                // SAFETY: `self.tasks` is not mutably aliased when calling this
                // method.
                let (mut task, waker) = (*self.tasks.get())
                    .remove(&id)
                    .expect("all pending task IDs should map to and active task entry");

                // Ensure we don't try to poll a completed or canceled task.
                if task.is_complete() || task.is_canceled() {
                    continue;
                }

                let mut cx = Context::from_waker(&waker);

                let prev_id = context::set_current_task_id(Some(id));

                if task.poll(&mut cx).is_pending() {
                    self.register_task_with_waker(task, waker);
                }

                context::set_current_task_id(prev_id);
            }
        }
    }
}
