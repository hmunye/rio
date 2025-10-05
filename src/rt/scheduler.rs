use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::task::Context;
use std::time::Instant;

use crate::rt::task::{TaskHandle, TaskId};
use crate::rt::waker::TaskWaker;
use crate::util::MinHeap;

thread_local! {
    /// Ensures timers can be associated with the `Task` that was most recently
    /// polled (i.e., the currently task being polled).
    ///
    /// Initially set to the `TaskId` of the first task (`Runtime::block_on`).
    static CURRENT_TASK: Cell<TaskId> = Cell::new(TaskId::default());
}

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
    /// A priority queue of timers associated with tasks, keyed by their
    /// scheduled wake-up time. Lexicographical ordering is used, meaning
    /// `wake_at` times are compared first.
    timers: RefCell<MinHeap<(Instant, TaskId)>>,
}

impl Scheduler {
    /// Creates a new `Scheduler`.
    #[inline]
    pub(crate) fn new() -> Self {
        Scheduler {
            tasks: Default::default(),
            ready: RefCell::new(Default::default()),
            pending: Default::default(),
            timers: Default::default(),
        }
    }

    /// Schedules the given `TaskHandle` and associated `TaskWaker`, blocking
    /// the current thread until the underlying `Task` resolves.
    pub(crate) fn block_on_task(&self, task: TaskHandle, waker: TaskWaker) {
        let id = task.borrow().id;

        self.schedule(id);
        self.tasks.borrow_mut().insert(id, (task, waker));

        // Spin for now until there are no more tasks (pending or active).
        while !self.tasks.borrow().is_empty() || !self.pending.borrow().is_empty() {
            self.tick();
        }
    }

    /// Schedules the given `TaskHandle` and associated `TaskWaker`, executing
    /// it concurrently with other tasks.
    pub(crate) fn spawn_task(&self, task: TaskHandle, waker: TaskWaker) {
        self.pending.borrow_mut().push((task, waker));
    }

    /// Adds a timer to the scheduler, associating it with the currently active
    /// `Task`.
    pub(crate) fn add_timer(&self, duration: Instant) {
        let task_id = CURRENT_TASK.with(|c| c.get());
        self.timers.borrow_mut().push((duration, task_id));
    }

    /// Marks the `Task` associated with the provided ID as ready to be polled.
    #[inline]
    pub(crate) fn schedule(&self, id: TaskId) {
        self.ready.borrow_mut().push_back(id);
    }

    /// Polls all currently ready tasks on the `ready` queue, handling any
    /// pending spawned tasks as well.
    fn tick(&self) {
        self.process_pending();
        self.process_timers();

        while let Some(id) = self.ready.borrow_mut().pop_front() {
            // Temporarily remove the task entry from the map.
            let entry = self.tasks.borrow_mut().remove(&id);
            let Some((task, waker)) = entry else {
                continue;
            };

            // Mark as not currently scheduled.
            task.borrow().scheduled.set(false);

            // Set the thread-local task ID to the current task's ID.
            CURRENT_TASK.with(|c| c.set(task.borrow().id));

            let mut ctx = Context::from_waker(&waker);
            let poll = {
                let mut task_ref = task.borrow_mut();
                task_ref.poll(&mut ctx)
            };

            // Reset the current task ID after polling, ensuring that
            // `CURRENT_TASK` reflects either the root task or the most recently
            // polled task.
            CURRENT_TASK.with(|c| c.set(TaskId::default()));

            if poll.is_pending() {
                // Re-insert the (task, waker) for future polling.
                self.tasks.borrow_mut().insert(id, (task, waker));
            }

            // Drop the `TaskHandle` and `TaskWaker` if `Poll::Ready`...
        }
    }

    /// Handle all pending spawned tasks, queuing them to be polled on the next
    /// `tick`.
    fn process_pending(&self) {
        let mut pending = self.pending.borrow_mut();

        for (task, waker) in pending.drain(..) {
            let id = task.borrow().id;
            self.schedule(id);
            self.tasks.borrow_mut().insert(id, (task, waker));
        }
    }

    /// Processes all timers that have expired, scheduling the corresponding
    /// `TaskId`.
    ///
    /// The timers are processed in order of their scheduled wake-up time.
    fn process_timers(&self) {
        let time_now = Instant::now();

        loop {
            let entry = self.timers.borrow_mut().pop();
            let Some((wake_at, id)) = entry else {
                break;
            };

            if wake_at <= time_now {
                self.schedule(id);
            } else {
                self.timers.borrow_mut().push((wake_at, id));
                // Since the earliest timeout in the heap hasn't expired, all
                // other timers are guaranteed not to have expired either, so
                // early return.
                break;
            }
        }
    }
}
