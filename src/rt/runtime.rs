use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use crate::rt::scheduler::Scheduler;
use crate::rt::task::{Task, TaskWaker};

thread_local! {
    /// Using thread-local storage (`TLS`) makes the implementation compatible
    /// with potential multithreading and supporting nested runtimes.
    ///
    /// It also enables explicit scoping of the current runtime context.
    static CURRENT_RUNTIME: std::cell::Cell<Option<*const Runtime>> =
        const { std::cell::Cell::new(None) };
}

/// The `rio` runtime.
#[derive(Debug)]
pub struct Runtime {
    /// Responsible for scheduling and polling tasks.
    pub(crate) scheduler: Rc<Scheduler>,
}

/// Guard used to set the thread-local current runtime context when calling
/// [`Runtime::block_on`].
///
/// The runtime context is cleared on [`Drop`].
struct EnterGuard;

impl EnterGuard {
    /// Initializes the thread-local current runtime, creating an `EnterGuard`.
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
    #[must_use]
    pub fn new() -> Self {
        Runtime {
            scheduler: Rc::new(Scheduler::new()),
        }
    }

    /// Runs the provided future to completion, serving as the runtime’s entry
    /// point.
    ///
    /// This function blocks the current thread until `future` resolves,
    /// returning its output.
    ///
    /// # Panics
    ///
    /// Panics if the future's output could not be retrieved.
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

    /// Returns a reference to the current threads [`Runtime`].
    ///
    /// # Panics
    ///
    /// This function panics if not called within the context of a runtime.
    pub(crate) fn current() -> &'static Runtime {
        CURRENT_RUNTIME.with(|rt| {
            rt.get().map_or_else(
                || panic!("runtime function called outside of a runtime context"),
                |ptr| {
                    // SAFETY: The thread-local holds a raw pointer to a runtime
                    // instance. This pointer is only set via the entry point
                    // `Runtime::block_on` and cleared when the created
                    // `EnterGuard` is dropped.
                    unsafe { &*ptr }
                },
            )
        })
    }

    /// Spawns a new asynchronous task on the current threads [`Runtime`].
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
