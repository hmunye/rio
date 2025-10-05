use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use crate::rt::CURRENT_RUNTIME;
use crate::rt::scheduler::Scheduler;
use crate::rt::task::Task;
use crate::rt::waker::TaskWaker;

/// The `rio` runtime.
#[derive(Debug, Clone)]
pub struct Runtime {
    /// The executor responsible for scheduling and polling tasks. Wrapped in an
    /// `Rc` to allow cloning for each `TaskWaker`, enabling them to reschedule
    /// their associated `Task`.
    scheduler: Rc<Scheduler>,
}

/// Guard used to set the thread-local `Runtime` context during initialization.
///
/// When dropped, the `Runtime` is cleared automatically.
struct EnterGuard;

impl EnterGuard {
    /// Initializes the thread-local `Runtime`, returning an `EnterGuard`.
    fn new(rt: &Runtime) -> Self {
        CURRENT_RUNTIME.with(|c| c.set(Some(rt)));
        EnterGuard
    }
}

impl Drop for EnterGuard {
    fn drop(&mut self) {
        CURRENT_RUNTIME.with(|c| c.set(None));
    }
}

impl Runtime {
    /// Creates a new `Runtime` instance.
    #[inline]
    pub fn new() -> Self {
        Runtime {
            scheduler: Rc::new(Scheduler::new()),
        }
    }

    /// Runs the provided `Future` to completion, serving as the runtime’s entry
    /// point.
    ///
    /// This function blocks on the current thread until `future` resolves,
    /// returning its result.
    pub fn block_on<F: Future + 'static>(&self, future: F) -> F::Output {
        let _enter = EnterGuard::new(self);

        // Used to capture the result of `future`.
        let output = Rc::new(RefCell::new(None));
        let out_clone = Rc::clone(&output);

        let task = Rc::new(RefCell::new(Task::new(async move {
            *out_clone.borrow_mut() = Some(future.await);
        })));

        let waker = TaskWaker::new(Rc::clone(&task), Rc::clone(&self.scheduler));

        self.scheduler.block_on_task(task, waker);

        output
            .borrow_mut()
            .take()
            .expect("`block_on` must produce the provided future's output")
    }

    /// Spawns a new asynchronous `Task` on the current `Runtime`.
    pub(crate) fn spawn_inner<F: Future<Output = ()> + 'static>(&self, future: F) {
        let task = Rc::new(RefCell::new(Task::new(future)));
        let waker = TaskWaker::new(Rc::clone(&task), Rc::clone(&self.scheduler));

        self.scheduler.spawn_task(task, waker);
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
