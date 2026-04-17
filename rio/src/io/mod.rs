//! Non-blocking I/O Utilities.
//!
//! Provides [`AsyncRead`] and [`AsyncWrite`] traits, and their extensions
//! [`AsyncReadExt`] and [`AsyncWriteExt`], which enable non-blocking I/O
//! operations in asynchronous contexts, analogous to the `std::io` traits
//! [`Read`] and [`Write`].
//!
//! [`Read`]: std::io::Read
//! [`Write`]: std::io::Write

mod async_read;
pub use async_read::{AsyncRead, AsyncReadExt};

mod async_write;
pub use async_write::{AsyncWrite, AsyncWriteExt};

mod read;
mod write;
mod write_all;
