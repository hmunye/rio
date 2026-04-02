//! Time-based Scheduling Utilities.
//!
//! Provides utilities for managing asynchronous delays and time-based events,
//! including:
//!
//! - [`Sleep`]: `Future` that completes after a specified [`Duration`] or
//!   [`Instant`].
//!
//! - [`Interval`]: Yields at a fixed [`Duration`] period, each time the period
//!   elapses.
//!
//! - [`Timeout`]: Wraps a `Future`, applying a time limit on its completion.
//!
//! [`Instant`]: std::time::Instant
//! [`Duration`]: std::time::Duration

mod sleep;
pub use sleep::{Sleep, sleep, sleep_until};

mod interval;
pub use interval::{Interval, interval, interval_at};

mod timeout;
pub use timeout::{Elapsed, Timeout, timeout, timeout_at};
