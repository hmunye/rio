//! Time Driver
//!
//! Responsible for managing time-based asynchronous operations within the
//! runtime.
//!
//! This driver provides a central mechanism for executing code after a
//! specified delay, at a fixed interval, or when a timeout occurs. It maintains
//! a priority queue of pending timers and wakes or notifies tasks when their
//! scheduled deadlines are reached.
//!
//! High-level utilities such as [`Sleep`], [`Interval`], and [`Timeout`] are
//! built on top of this driver, each using it to schedule wake-ups of their
//! associated tasks.

mod handle;
pub use handle::Handle;

mod driver;
pub use driver::Driver;

mod entry;
pub use entry::TimerEntry;

mod heap;
pub use heap::MinHeap;
