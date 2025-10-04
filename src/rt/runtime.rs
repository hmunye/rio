use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use crate::rt::scheduler::Scheduler;
use crate::rt::task::Task;
use crate::rt::waker::TaskWaker;

/// The `rio` runtime.
#[derive(Debug)]
pub struct Runtime {
    scheduler: Rc<Scheduler>,
}

impl Runtime {
    /// Creates a new `Runtime` instance.
    #[inline]
    pub fn new() -> Self {
        Runtime {
            scheduler: Rc::new(Scheduler::new()),
        }
    }

    /// Runs a future to completion, serving as the runtime’s entry point.
    ///
    /// This runs the given future on the current thread, blocking until it is
    /// complete, and yielding its resolved result.
    pub fn block_on<F: Future + 'static>(&self, future: F) -> F::Output {
        let output = Rc::new(RefCell::new(None));
        let out_clone = Rc::clone(&output);

        let task = Rc::new(RefCell::new(Task::new(async move {
            // Ensure we can read out a possible output. `Task` requires a
            // `Future<Output = ()>`.
            *out_clone.borrow_mut() = Some(future.await);
        })));

        let waker = TaskWaker::new(&task, Rc::clone(&self.scheduler));

        // Blocks until the provided task resolves.
        self.scheduler.block_on(task, waker);

        // Value stored in `output` will always be `Some(F::Output)`.
        output.borrow_mut().take().unwrap()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
