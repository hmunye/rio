//! Utilities for tracking time.
//!
//! Provides a number of types for executing code after a set period of time.
//!
//! - [`Sleep`] is a future that does no work and completes at a specific
//!   [`Instant`] in time.
//!
//! - [`Interval`] is a stream yielding a value at a fixed period. It is
//!   initialized with a [`Duration`] and repeatedly yields each time the
//!   duration elapses.
//!
//! - [`Timeout`]: Wraps a future or stream, setting an upper bound to the
//!   amount of time it is allowed to execute. If the future or stream does not
//!   complete in time, then it is canceled and an error is returned.
//!
//! [`Sleep`]: crate::time::Sleep
//! [`Instant`]: std::time::Instant
//! [`Duration`]: std::time::Duration

mod sleep;
pub use sleep::{Sleep, sleep, sleep_until};
