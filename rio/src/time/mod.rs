//! Time-based Scheduling Utilities.
//!
//! Provides utilities for managing asynchronous delays and time-based events,
//! including:
//!
//! - [`Sleep`]: Future that completes after a specified [`Duration`] or at a
//!   specific [`Instant`].
//!
//! [`Sleep`]: crate::time::Sleep
//! [`Instant`]: std::time::Instant
//! [`Duration`]: std::time::Duration

mod sleep;
pub use sleep::{Sleep, sleep, sleep_until};
