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

mod registration;
pub(crate) use registration::PollToken;
pub use registration::{IoHandle, register_io_source};

mod interest;
pub use interest::Interest;

mod flush;
mod read;
mod read_exact;
mod shutdown;
mod write;
mod write_all;
