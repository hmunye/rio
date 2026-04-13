//! Non-blocking I/O Utilities.
//!
//! Provides [`AsyncRead`] and [`AsyncWrite`] traits, which enable non-blocking
//! I/O operations in asynchronous contexts, analogous to the `std::io` traits
//! [`Read`] and [`Write`].
//!
//! [`Read`]: std::io::Read
//! [`Write`]: std::io::Write

mod async_read;
pub use async_read::AsyncRead;

mod async_write;
pub use async_write::AsyncWrite;
