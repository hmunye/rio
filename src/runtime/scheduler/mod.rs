//! Scheduler
//!
//! Responsible for ensuring fair execution and scheduling of tasks within the
//! runtime.

mod handle;
pub use handle::Handle;

#[allow(clippy::module_inception)]
mod scheduler;
pub use scheduler::Scheduler;
