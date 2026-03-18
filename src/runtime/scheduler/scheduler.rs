use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;
use std::task::Context;

use crate::runtime::scheduler;
use crate::runtime::task::{self, Task};

/// Single-threaded `scheduler` for the runtime.
#[derive(Debug)]
pub struct Scheduler {
    /// Stores active tasks, mapping each task ID to its [`Task`] and
    /// [`LocalWaker`].
    ///
    /// [`LocalWaker`]: task::LocalWaker
    tasks: HashMap<task::Id, (Task, task::LocalWaker)>,
    /// Queue of task IDs ready to be polled.
    ready: VecDeque<task::Id>,
}

impl Scheduler {
    #[inline]
    pub fn new() -> Self {
        Scheduler {
            tasks: HashMap::default(),
            ready: VecDeque::default(),
        }
    }

    /// Runs the provided future to completion on the current thread, blocking
    /// until the scheduler becomes idle (i.e., no active tasks remain).
    pub fn block_on_fut<F: Future + 'static>(
        &mut self,
        handle: scheduler::Handle,
        fut: F,
    ) -> F::Output {
        let mut output = MaybeUninit::<F::Output>::uninit();
        let output_ptr = &raw mut output;

        let task = Task::new(async move {
            // SAFETY: `output_ptr` is guaranteed to be non-null and properly
            // aligned. The pointer remains valid for the duration of the task,
            // as it is allocated on the stack and is not used outside of the
            // current function. Since the runtime is single-threaded, all tasks
            // are executed on the same thread.
            unsafe {
                (*output_ptr).write(fut.await);
            }
        });

        let id = task.id;
        let waker = task::LocalWaker::new(id, handle);

        self.schedule_task(id);
        self.tasks.insert(id, (task, waker));

        while !self.is_idle() {
            self.tick();
        }

        // SAFETY: All tasks are guaranteed to be polled to completion before
        // this function returns, which ensures that the task responsible for
        // initializing `output` has finished.
        unsafe { output.assume_init() }
    }

    /// Schedules the task with the specified `id` to be polled.
    #[inline]
    pub fn schedule_task(&mut self, id: task::Id) {
        self.ready.push_back(id);
    }

    /// Returns `true` if the scheduler has no active tasks.
    #[inline]
    fn is_idle(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Polls all tasks currently in the ready queue.
    fn tick(&mut self) {
        while let Some(id) = self.ready.pop_front()
            && let Some((task, waker)) = self.tasks.get_mut(&id)
        {
            let mut ctx = Context::from_waker(waker);

            if task.poll(&mut ctx).is_ready() {
                self.tasks.remove(&id);
            }
        }
    }
}
