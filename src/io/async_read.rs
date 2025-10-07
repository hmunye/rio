use std::future::{self, Future};
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

/// Implemented as an extension trait, adding utility methods to `AsyncRead`
/// types.
pub trait AsyncReadExt: AsyncRead {
    /// Attempts to read bytes into `buf`, returning the number of bytes read.
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> impl Future<Output = io::Result<usize>> + 'a
    where
        Self: std::marker::Unpin,
    {
        future::poll_fn(move |ctx| Pin::new(&mut *self).poll_read(ctx, buf))
    }
}

impl<T: AsyncRead + ?Sized> AsyncReadExt for T {}
