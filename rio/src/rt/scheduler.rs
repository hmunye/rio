use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::task::Context;

use crate::rt::{Handle, Task, context, task::LocalWaker};
use crate::task;

/// `rio` Scheduler.
///
/// Single-threaded scheduler responsible for scheduling and polling tasks using \
/// **cooperative multitasking**.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores all registered tasks, mapping each task [`Id`] to its [`Task`]
    /// and associated [`LocalWaker`].
    ///
    /// [`Id`]: task::Id
    tasks: RefCell<HashMap<task::Id, (Task, LocalWaker)>>,
    /// Queue of task [`Id`]s ready to be polled.
    ///
    /// [`Id`]: task::Id
    ready: RefCell<VecDeque<task::Id>>,
}

impl Scheduler {
    #[must_use]
    pub fn new() -> Self {
        Scheduler {
            tasks: RefCell::default(),
            ready: RefCell::default(),
        }
    }

    pub fn spawn_blocking<F: Future + 'static>(&self, handle: Handle, fut: F) -> F::Output {
        let mut output = std::mem::MaybeUninit::uninit();
        let output_ptr = &raw mut output;

        let task = Task::new_with(fut, move |out| {
            // SAFETY: `output_ptr` aliases a stack-allocated `MaybeUninit` that
            // remains valid for the duration of this function, since it blocks
            // until all tasks are resolved.
            unsafe {
                (*output_ptr).write(out);
            }
        });

        self.register_task(task, handle);

        while !self.is_idle() {
            self.tick();
        }

        // SAFETY: This function blocks until the scheduler is idle, which
        // guarantees that all spawned tasks, including the one polling `fut`,
        // have resolved, ensuring `output` is initialized.
        unsafe { output.assume_init() }
    }

    pub fn spawn(&self, task: Task, handle: Handle) {
        self.register_task(task, handle);
    }

    pub fn schedule_task(&self, id: task::Id) {
        self.ready.borrow_mut().push_back(id);
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

    fn tick(&self) {
        // Limit the scope of any borrows of `self.ready` and `self.tasks`
        // within a "tick" to avoid mutable aliasing, as polling a task may
        // trigger child tasks to interact with the scheduler using their handle
        // (e.g, schedule themselves).
        loop {
            let Some(id) = self.ready.borrow_mut().pop_front() else {
                break;
            };

            let Some((mut task, waker)) = self.tasks.borrow_mut().remove(&id) else {
                panic!("task entry should not be missing for task #{id}");
            };

            let mut cx = Context::from_waker(&waker);

            let prev_id = context::set_current_task(Some(id));

            if task.poll(&mut cx).is_pending() {
                self.register_task_with_waker(task, waker);
            }

            context::set_current_task(prev_id);
        }
    }
}
