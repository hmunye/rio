//! Minimal asynchronous runtime for exploring `async` Rust.

#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unused_must_use)]

pub mod rt;
pub use rt::spawn;
