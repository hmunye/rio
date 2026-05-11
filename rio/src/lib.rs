//! Minimal Asynchronous Runtime for Rust.
//!
//! `rio` is a lightweight, single-threaded asynchronous runtime designed for
//! simplicity and efficiency.
//!
//! ### Cooperative vs. Preemptive
//!
//! Unlike the __preemptive__ multitasking found in operating systems, where the
//! kernel can interrupt threads at arbitrary points, `rio` relies on a
//! __cooperative__ model.
//!
//! In `rio`, tasks are responsible for yielding control back to the scheduler.
//! By leveraging Rust's `async/await` model, `rio` can manage thousands of
//! tasks concurrently with minimal memory overhead, as they do not require
//! independent call-stacks and context switches.
//!
//! <div class="warning">
//!     <h5>CPU-bound Tasks and Blocking Code</h5>
//! </div>
//!
//! Because `rio` is single-threaded and cooperative, __you must never perform
//! blocking operations__ (like `std::thread::sleep` or synchronous I/O) within
//! a task. Blocking a task stalls the entire runtime. Instead, use the
//! utilities provided for working with [asynchronous tasks][task], including
//! [timeouts, sleeps, intervals][time], [non-blocking I/O][io],
//! [asynchronous networking][net], and [cooperative scheduling][task::coop].
//!
//! ### Feature Flags
//!
//! `rio` uses a set of feature flags to reduce the amount of compiled code,
//! including:
//!
//! - `default`: Enables none of the features listed below.
//! - `full`: Enables all features listed below.
//! - `macros`: Enables `#[rio::main]` and `#[rio::test]` macros.
//! - `time`: Enables `rio::time` types and allows the scheduler to enable the
//!   timer driver.
//! - `io`: Enables `rio::io` types and allows the scheduler to enable the I/O
//!   driver.
//! - `net`: Enables `rio::net` types such as [`TcpStream`][net::TcpStream] and
//!   [`TcpListener`][net::TcpListener].

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
#![allow(clippy::unused_self)]
#![allow(clippy::borrow_as_ptr)]
#![allow(clippy::single_match_else)]

#[cfg(all(
    feature = "io",
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd",
    ))
))]
compile_error!(
    "`io` feature requires a platform with `epoll(7)` (Linux) \
or `kqueue(2)` (macOS/BSD) support"
);

// TODO: For `io` feature, code is compiled which is only used by the `net`
// feature. Check output in CI from `cargo hack` for other unused code.

// Must be defined first!
#[macro_use]
pub(crate) mod macros;

cfg_macros! {
    pub use rio_macros::main;
    pub use rio_macros::test;
}

cfg_time! {
    pub mod time;
}

cfg_io! {
    pub mod io;
}

cfg_net! {
    pub mod net;
}

pub mod rt;
pub mod task;

pub use task::spawn;
