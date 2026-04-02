//! Time-based Scheduling Utilities.
//!
//! Provides utilities for managing asynchronous delays and time-based events,
//! including:
//!
//! - [`Sleep`]: Future that completes after a specified [`Duration`] or at a
//!   specific [`Instant`].
//!
//! - [`Interval`]: Yields at a fixed period. Initialized with a [`Duration`],
//!   it repeatedly yields each time the duration elapses.
//!
//! [`Instant`]: std::time::Instant
//! [`Duration`]: std::time::Duration

mod sleep;
pub use sleep::{Sleep, sleep, sleep_until};

mod interval;
pub use interval::{Interval, interval, interval_at};
