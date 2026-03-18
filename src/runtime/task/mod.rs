//! Tasks
//!
//! A task is a lightweight, non-blocking unit of execution (a "green thread"),
//! managed by the `rio` runtime. Tasks run cooperatively, executing until they
//! yield, at which point the runtime may poll other ready tasks. Tasks should
//! avoid blocking the underlying thread and instead use asynchronous primitives
//! to make progress efficiently.

#[allow(clippy::module_inception)]
mod task;
pub(crate) use task::Task;

mod id;
pub(crate) use id::Id;

mod waker;
pub(crate) use waker::LocalWaker;
