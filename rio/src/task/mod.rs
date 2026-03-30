//! Asynchronous Tasks.
//!
//! Tasks are the fundamental units of non-blocking execution within the `rio`
//! runtime. Often referred to as **green threads**, they operate cooperatively,
//! yielding control back to the scheduler to allow other ready tasks to
//! progress.
//!
//! Tasks must remain non-blocking. Avoid blocking the current thread during
//! task execution. Instead, utilize the non-blocking asynchronous primitives
//! provided by `rio` or opt-in to cooperative scheduling.

mod id;
pub use id::{Id, id};

mod spawn;
pub use spawn::spawn;

mod yield_now;
pub use yield_now::yield_now;
