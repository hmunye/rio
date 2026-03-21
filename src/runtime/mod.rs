//! `rio` Runtime
//!
//! The `rio` runtime provides a single-threaded task scheduler for executing
//! asynchronous [`tasks`]. It manages the task lifecycle, including scheduling
//! and polling, enabling cooperative multitasking without blocking the
//! current thread.
//!
//! [`tasks`]: crate::runtime::task

pub mod task;

#[allow(clippy::module_inception)]
mod runtime;
pub use runtime::Runtime;

mod time;

mod handle;
pub(crate) use handle::Handle;

pub(crate) mod context;

mod scheduler;
pub(crate) use scheduler::Scheduler;
