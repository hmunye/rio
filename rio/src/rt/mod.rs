//! `rio` Runtime.
//!
//! Provides a single-threaded task scheduler and time driver, necessary for
//! running asynchronous [`tasks`].
//!
//! [`tasks`]: crate::task

mod runtime;
pub use runtime::Runtime;

mod handle;
pub(crate) use handle::Handle;

pub(crate) mod context;

mod scheduler;
pub(crate) use scheduler::Scheduler;

mod task;
pub(crate) use task::Task;

mod time;
