//! The `rio` Runtime.

#[allow(clippy::module_inception)]
mod runtime;
pub use runtime::Runtime;

mod context;
mod scheduler;

mod handle;
pub(crate) use handle::{EnterGuard, Handle};
