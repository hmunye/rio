use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::task::Context;

use crate::rt::task::LocalWaker;
use crate::rt::{Handle, Task, context};
use crate::task;
use crate::task::coop::{self, Budget};

/// `rio` Scheduler.
///
/// Single-threaded scheduler responsible for scheduling and polling tasks.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores all registered tasks, mapping each task [`Id`] to its [`Task`]
    /// and [`LocalWaker`].
    ///
    /// [`Id`]: task::Id
    tasks: RefCell<HashMap<task::Id, (Task, LocalWaker)>>,
    /// Queue of task [`Id`]s ready to be polled on the _current_ "tick".
    ///
    /// [`Id`]: task::Id
    ready: RefCell<VecDeque<task::Id>>,
    /// Task [`Id`]s to be polled on the _next_ "tick". Each slot corresponds to
    /// the amount of execution budget used by that task during the last "tick".
    ///
    /// [`Id`]: task::Id
    deferred: RefCell<[Vec<task::Id>; (Budget::INITIAL + 1) as usize]>,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Scheduler {
            tasks: RefCell::default(),
            ready: RefCell::default(),
            deferred: RefCell::new(std::array::from_fn(|_| Vec::new())),
        }
    }

    /// Spawns an asynchronous task, blocking the current thread until the
    /// provided future completes and the scheduler is fully idle (i.e., no
    /// registered tasks remaining).
    pub fn spawn_blocking<F: Future + 'static>(&self, handle: Handle, fut: F) -> F::Output {
        let mut output = std::mem::MaybeUninit::uninit();
        let output_ptr = &raw mut output;

        let task = Task::new_with(fut, move |out| {
            // SAFETY: `output_ptr` aliases a stack-allocated `MaybeUninit` that
            // remains valid for the duration of this function, since it does
            // not return until all tasks are resolved.
            unsafe {
                (*output_ptr).write(out);
            }
        });

        self.spawn(task, handle);

        while !self.is_idle() {
            self.tick();
        }

        // SAFETY: This function does not return until the scheduler is idle,
        // which guarantees that all spawned tasks, including the one polling
        // `fut`, have resolved, meaning `output` was initialized.
        unsafe { output.assume_init() }
    }

    /// Registers the provided asynchronous task for execution by the scheduler.
    pub fn spawn(&self, task: Task, handle: Handle) {
        self.register_task(task, handle);
    }

    /// Schedules the task with the specified `Id` as ready for polling on the
    /// _current_ "tick".
    pub fn schedule_task(&self, id: task::Id) {
        self.ready.borrow_mut().push_back(id);
    }

    /// Schedules the task with the specified `Id` to be polled on the _next_
    /// "tick", weighted by how much of the execution budget it used during the
    /// previous "tick".
    pub fn defer_task(&self, id: task::Id, used_budget: u8) {
        self.deferred.borrow_mut()[used_budget as usize].push(id);
    }

    fn register_task(&self, task: Task, handle: Handle) {
        let id = task.id;
        let waker = LocalWaker::new(id, handle);

        self.register_task_with_waker(task, waker);
        self.schedule_task(id);
    }

    fn register_task_with_waker(&self, task: Task, waker: LocalWaker) {
        self.tasks.borrow_mut().insert(task.id, (task, waker));
    }

    fn is_idle(&self) -> bool {
        self.tasks.borrow().is_empty()
    }

    fn run_task(&self, mut task: Task, waker: LocalWaker) {
        let mut cx = Context::from_waker(&waker);

        let prev_id = context::set_task_id(Some(task.id));
        context::update_snapshot();

        if task.poll(&mut cx).is_pending() {
            self.register_task_with_waker(task, waker);
        }

        context::set_task_id(prev_id);
    }

    fn tick(&self) {
        context::with_handle(Handle::drive_timers);

        // Queue deferred tasks in the order of execution budget used during
        // the previous "tick" (ascending order).
        self.ready.borrow_mut().extend(
            self.deferred
                .borrow_mut()
                .iter_mut()
                .flat_map(|q| q.drain(..)),
        );

        // Start each "tick" with an initial budget, shared between all ready
        // tasks.
        coop::with_initial(|| {
            // Limit the scope of any borrows of `self` within a "tick" loop to
            // avoid mutable aliasing, as polling a task may trigger child tasks
            // to interact with the scheduler (e.g, schedule themselves).
            loop {
                let Some(id) = self.ready.borrow_mut().pop_front() else {
                    break;
                };

                let Some((task, waker)) = self.tasks.borrow_mut().remove(&id) else {
                    panic!("task entry should not be missing for task #{id}");
                };

                self.run_task(task, waker);

                if !coop::has_budget_remaining() {
                    break;
                }
            }
        });
    }
}
