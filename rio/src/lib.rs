//! Minimal Asynchronous Runtime for Rust.
//!
//! `rio` is a lightweight, single-threaded asynchronous runtime designed for
//! high-concurrency, memory-efficient applications.
//!
//! ### Cooperative vs. Preemptive
//!
//! Unlike the __preemptive__ multitasking found in OS kernels, where the kernel
//! interrupts threads at arbitrary points, `rio` uses a __cooperative__ model.
//!
//! In `rio`, tasks are responsible for yielding control back to the scheduler.
//! By leveraging Rust's `async/await` model, `rio` can manage thousands of
//! tasks concurrently with minimal memory overhead, as tasks do not require
//! their own independent call-stacks.
//!
//! <div class="warning">
//!     <h5>Avoid Blocking</h5>
//! </div>
//!
//! Because `rio` is single-threaded and cooperative, __you must never perform
//! blocking operations__ (like `std::thread::sleep` or synchronous I/O) within
//! a task. Blocking a task stops the entire runtime. Instead, use the utilities
//! provided for working with [asynchronous tasks][task], including [yielding],
//! [timeouts, sleeps, and intervals][time].
//!
//! [yielding]: crate::task::yield_now

// TODO: Add unit tests.
// TODO: Add runtime shutdown.

#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(rust_2018_idioms)]
#![warn(missing_debug_implementations)]
#![allow(clippy::use_self)]
#![allow(clippy::redundant_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::option_if_let_else)]

// NOTE: Must be defined first!
#[macro_use]
pub(crate) mod macros;

cfg_macros! {
    pub use rio_macros::main;
    pub use rio_macros::test;
}

pub mod rt;
pub mod task;

pub use task::spawn;

cfg_time! {
    pub mod time;
}
