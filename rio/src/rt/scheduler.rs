use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};
use std::task::Context;

use crate::rt::task::{LocalWaker, TaskStage};
use crate::rt::{Task, context};
use crate::task::JoinHandle;
use crate::task::{
    self,
    coop::{self, Budget},
};

cfg_time! {
    use std::time::Duration;

    use crate::rt::Handle;
}

cfg_io! {
    cfg_not_time! {
        use std::time::Duration;
    }
}

#[derive(Debug)]
struct Deferred {
    /// Each slot corresponds to the amount of execution budget used by that
    /// task during the previous "tick" (`0..=Budget::INITIAL`).
    buckets: [Vec<task::Id>; (Budget::INITIAL + 1) as usize],
    /// Tracks non-empty buckets for efficient lookup.
    ///
    /// NOTE: `Budget::INITIAL` must be <= 127.
    bitmap: u128,
}

impl Deferred {
    fn insert(&mut self, idx: usize, id: task::Id) {
        self.buckets[idx].push(id);
        // Mark bucket as non-empty.
        self.bitmap |= 1 << idx;
    }

    fn next_bucket(&mut self) -> Option<std::vec::Drain<'_, task::Id>> {
        if self.is_empty() {
            return None;
        }

        // Next non-empty bucket, from least to most execution budget used in
        // the previous "tick".
        let next_bucket = self.bitmap.trailing_zeros() as usize;
        let bucket = &mut self.buckets[next_bucket];

        // Mark bucket as empty since it will be fully drained.
        self.bitmap &= !(1 << next_bucket);

        Some(bucket.drain(..))
    }

    const fn is_empty(&self) -> bool {
        self.bitmap == 0
    }
}

/// `rio` Scheduler.
///
/// Single-threaded scheduler responsible for scheduling and polling tasks.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores all registered tasks, mapping each [`Id`] to its [`Task`] and
    /// [`LocalWaker`].
    ///
    /// [`Id`]: task::Id
    tasks: RefCell<HashMap<task::Id, (Task, LocalWaker)>>,
    /// Queue of task [`Id`]s ready to be polled, potentially on the _current_
    /// "tick".
    ///
    /// [`Id`]: task::Id
    ready: RefCell<VecDeque<task::Id>>,
    /// Task [`Id`]s to be polled on the _next_ "tick".
    ///
    /// [`Id`]: task::Id
    deferred: RefCell<Deferred>,
    shutdown: Cell<bool>,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Scheduler {
            tasks: RefCell::default(),
            ready: RefCell::default(),
            deferred: RefCell::new(Deferred {
                buckets: std::array::from_fn(|_| Vec::new()),
                bitmap: 0,
            }),
            shutdown: Cell::new(false),
        }
    }

    /// Spawns an asynchronous task, blocking the current thread until the
    /// provided future completes and the scheduler is fully idle (i.e., no
    /// registered tasks remaining).
    pub fn spawn_blocking<F: Future + 'static>(
        &self,
        fut: F,
        handle: Weak<Scheduler>,
    ) -> F::Output {
        let task = Task::new_with(fut, |out, weak| {
            if let Some(state) = weak.upgrade() {
                // We know the handle is not dropped; retain the output.
                state.set_stage(TaskStage::Finished(Box::new(out)));
            }
        });

        let join = JoinHandle {
            // NOTE: Uses an `Rc` (not `Weak`) so the task’s output remains
            // accessible even when the task is dropped.
            state: Rc::clone(&task.state),
            _marker: std::marker::PhantomData,
        };

        self.spawn(task, handle);

        while self.work_remaining() && !self.shutdown_ready(join.is_finished()) {
            self.tick();
        }

        join.take_output()
            .expect("`block_on` task missing output: all tasks should have completed")
    }

    /// Registers the provided asynchronous task for polling, potentially on the
    /// current "tick".
    pub fn spawn(&self, task: Task, handle: Weak<Scheduler>) {
        self.register_task(task, handle);
    }

    /// Schedules the task with the specified `Id` as ready for polling,
    /// potentially on the _current_ "tick".
    pub fn schedule_task(&self, id: task::Id) {
        self.ready.borrow_mut().push_back(id);
    }

    /// Schedules the task with the specified `Id` to be polled on the _next_
    /// "tick", weighted by how much of the execution budget it used during the
    /// current "tick".
    pub fn defer_task(&self, id: task::Id, used_budget: u8) {
        self.deferred.borrow_mut().insert(used_budget as usize, id);
    }

    /// Signals the scheduler to begin shutting down.
    pub fn shutdown_background(&self) {
        self.shutdown.set(true);
    }

    fn register_task(&self, task: Task, handle: Weak<Scheduler>) {
        let id = task.state.id;
        let waker = LocalWaker::new(id, handle);

        self.register_task_with_waker(id, task, waker);
        self.schedule_task(id);
    }

    fn register_task_with_waker(&self, id: task::Id, task: Task, waker: LocalWaker) {
        self.tasks.borrow_mut().insert(id, (task, waker));
    }

    fn work_remaining(&self) -> bool {
        !self.tasks.borrow().is_empty()
    }

    const fn shutdown_ready(&self, block_on_complete: bool) -> bool {
        self.shutdown.get() && block_on_complete
    }

    fn run_task(&self, id: task::Id, mut task: Task, waker: LocalWaker) {
        let mut cx = Context::from_waker(&waker);

        context::update_snapshot();

        if task.is_pollable() && task.poll(&mut cx).is_pending() {
            self.register_task_with_waker(id, task, waker);
        }
    }

    fn tick(&self) {
        #[cfg(all(feature = "time", feature = "io"))]
        {
            let timeout = self.compute_io_timeout(context::with_handle(Handle::drive_timers));
            context::with_handle(|handle| handle.drive_io(timeout));
        }

        #[cfg(all(feature = "time", not(feature = "io")))]
        {
            if let Some(timeout) = context::with_handle(Handle::drive_timers) {
                self.try_park(timeout);
            }
        }

        #[cfg(all(feature = "io", not(feature = "time")))]
        {
            let timeout = self.compute_io_timeout(None);
            context::with_handle(|handle| handle.drive_io(timeout));
        }

        // Queue deferred tasks in ascending order of execution budget used
        // during the previous "tick".
        while let Some(ids) = self.deferred.borrow_mut().next_bucket() {
            self.ready.borrow_mut().extend(ids);
        }

        // Start each "tick" with an initial budget, shared between all tasks.
        coop::with_initial(|| {
            // Limit the scope of any borrows of `self` within the loop to avoid
            // mutable aliasing, as polling a task may trigger child tasks to
            // interact with the scheduler (e.g, schedule themselves).
            loop {
                let Some(id) = self.ready.borrow_mut().pop_front() else {
                    break;
                };

                let Some((task, waker)) = self.tasks.borrow_mut().remove(&id) else {
                    continue;
                };

                self.run_task(id, task, waker);

                if !coop::has_budget_remaining() {
                    break;
                }
            }
        });
    }
}

#[cfg(any(feature = "time", feature = "io"))]
impl Scheduler {
    fn is_idle(&self) -> bool {
        self.ready.borrow().is_empty() && self.deferred.borrow().is_empty()
    }
}

cfg_time! {
    cfg_not_io! {
        impl Scheduler {
            fn try_park(&self, timeout: Duration) {
                if self.is_idle() {
                    // eprintln!("[[parking thread until next timer deadline]]");
                    std::thread::park_timeout(timeout);
                }
            }
        }
    }
}

cfg_io! {
    impl Scheduler {
        fn compute_io_timeout(&self, timeout: Option<Duration>) -> i32 {
            if self.is_idle() {
                match timeout {
                    Some(t) => t.as_millis().min(i32::MAX as u128) as i32,
                    None => -1,
                }
            } else {
                0
            }
        }
    }
}
