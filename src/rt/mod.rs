//! The `rio` runtime.
//!
//! One core feature of modern operation systems is `multitasking`, which is the
//! ability to interleave the execution of multiple tasks concurrently. The two
//! main forms of multitasking are `preemptive` and `cooperative`.
//!
//! Preemptive multitasking is the approach used by operating systems to control
//! the execution of tasks (`threads`), including when they are scheduled and
//! the duration of their execution. Since threads can be interrupted at
//! arbitrary points during execution, their state must saved, including the
//! separately allocated call stack and values of CPU registers, after which the
//! state of the next scheduled thread can be restored (`context switch`). Since
//! each thread manages it's own call stack, the OS only saves the values of the
//! CPU registers, minimizing the overhead for each context switch. The main
//! advantage of this multitasking approach is the full control the OS has on
//! the execution of each thread, ensuring fairness in the scheduling of threads
//! so that each can make progress without relying on one another. One drawback
//! is that each thread requires its own call stack, increasing the memory usage
//! per-task.
//!
//! Cooperative multitasking in contrast give the responsibility of yielding CPU
//! time to the tasks. Combined with asynchronous programming, this allows tasks
//! to execute until determining they can no longer make progress. Since each
//! task controls when they yield, they can save just the minimal set of state
//! needed to resume execution, resulting in less memory usage per-task. Rust’s
//! `async/await` implementation stores local variables that are live between
//! suspension points in a compiler generated data structure, meaning tasks can
//! share the same call stack. The main drawback to this approach is that a
//! misbehaving task can potentially never yield, ensuring that no other task is
//! able to make progress, and possibly resulting in a deadlock.
//!
//! Because the OS is not involved in this cooperative multitasking, a `runtime`
//! is required to ensure each task is scheduled and polled to make progress.

mod runtime;
pub use runtime::Runtime;

pub(crate) mod io;
pub(crate) mod scheduler;
pub(crate) mod task;

thread_local! {
    /// Using thread-local storage (`TLS`) makes the implementation compatible
    /// with potential multithreading and supporting nested runtimes.
    ///
    /// It also enables explicit scoping of the current runtime context.
    pub(crate) static CURRENT_RUNTIME: std::cell::Cell<Option<*const Runtime>> = const {
        std::cell::Cell::new(None)
    };
}

/// Spawns a new asynchronous `Task` running in the background, enabling it to
/// execute concurrently with other tasks.
///
/// Returning the output of the provided `Future` is currently not supported,
/// so it will be polled solely for its side effects.
pub fn spawn<F: std::future::Future<Output = ()> + 'static>(future: F) {
    CURRENT_RUNTIME.with(|rt| {
        if let Some(ptr) = rt.get() {
            // SAFETY: The thread-local holds a raw pointer to a `Runtime`. This
            // pointer is only set via the entry point `Runtime::block_on`, and
            // cleared when the associated `EnterGuard` is dropped. Spawning is
            // only possible within the context of a runtime.
            let rt = unsafe { &*ptr };
            rt.spawn_inner(future);
        } else {
            panic!("`spawn` called outside of a rutime context");
        }
    })
}
