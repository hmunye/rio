//! Minimal asynchronous runtime for exploring `async` Rust.

#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unused_must_use)]

#[cfg(not(target_os = "linux"))]
compile_error!("This crate is only compatible with Linux systems that support epoll(7).");

pub mod rt;
pub use rt::spawn;

pub mod time;

pub(crate) mod util;
