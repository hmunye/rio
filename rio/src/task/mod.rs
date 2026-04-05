//! Asynchronous Tasks.
//!
//! Tasks are the fundamental units of non-blocking execution within the `rio`
//! runtime. Often referred to as __green threads__, they operate cooperatively,
//! able to yield control back to the runtime to allow other ready tasks to
//! progress.
//!
//! Tasks __must never block__ the current thread. Always utilize the provided
//! non-blocking asynchronous primitives or opt-in to [`cooperative scheduling`]
//! to ensure the runtime remains responsive.
//!
//! [`cooperative scheduling`]: coop

pub mod coop;

mod id;
pub use id::{Id, id};

mod spawn;
pub use spawn::spawn;

mod yield_now;
pub use yield_now::yield_now;

mod join;
pub use join::{JoinError, JoinHandle};
