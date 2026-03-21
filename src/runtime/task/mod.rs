//! Tasks
//!
//! A task is a lightweight, non-blocking unit of execution ("green thread"),
//! managed by the `rio` runtime. Tasks run cooperatively, executing until they
//! yield, at which point the runtime may poll other ready tasks. Tasks should
//! avoid blocking the current thread and instead use asynchronous primitives
//! to make progress efficiently.

mod join;
pub use join::{JoinError, JoinHandle};

mod spawn;
pub use spawn::spawn;

mod yield_now;
pub use yield_now::yield_now;

mod id;
pub use id::{Id, id};

#[allow(clippy::module_inception)]
mod task;
pub(crate) use task::{Stage, Task, TaskState};

mod waker;
pub(crate) use waker::LocalWaker;
