#[allow(clippy::module_inception)]
mod task;
pub use task::Task;

mod waker;
pub use waker::LocalWaker;
