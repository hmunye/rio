use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Reads bytes from a source.
///
/// This trait is analogous to the [`std::io::Read`] trait, but integrates with
/// the asynchronous task system.
pub trait AsyncRead {
    /// Attempts to read bytes into `buf`, returning the number of bytes read.
    ///
    /// Returns [`Poll::Pending`] if the read operation would block.
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>>;
}

/// Writes bytes asynchronously.
///
/// The trait inherits from [`std::io::Write`] and indicates that an I/O object
/// is `nonblocking`.
pub trait AsyncWrite {
    /// Attempts to write bytes from `buf`, returning the number of bytes
    /// written.
    ///
    /// Returns [`Poll::Pending`] if the write operation would block.
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>;

    /// Flushes any buffered data.
    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>>;

    /// Shuts down the write half of this object.
    fn poll_shutdown(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>>;
}
