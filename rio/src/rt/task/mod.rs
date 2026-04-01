#[allow(clippy::module_inception)]
mod task;
pub use task::{Task, TaskStage, TaskState};

mod waker;
pub use waker::LocalWaker;
