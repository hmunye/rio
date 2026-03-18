//! `rio` Runtime
//!
//! The `rio` runtime provides a single-threaded task scheduler for executing
//! asynchronous [`tasks`]. It manages task lifecycle, scheduling, and polling
//! of futures, enabling cooperative multitasking without blocking the
//! underlying thread. Tasks are executed until they yield, allowing the
//! runtime to efficiently run multiple tasks concurrently on the same thread.
//!
//! [`tasks`]: crate::runtime::task

pub mod task;

#[allow(clippy::module_inception)]
mod runtime;
pub use runtime::Runtime;

mod handle;
pub use handle::EnterGuard;
pub(crate) use handle::Handle;

mod context;

mod scheduler;
pub(crate) use scheduler::Scheduler;
