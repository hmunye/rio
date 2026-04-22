use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, ops};

use crate::io::read::{Read, read};
use crate::io::read_exact::{ReadExact, read_exact};

/// Reads bytes asynchronously from a source, analogous to [`std::io::Read`].
pub trait AsyncRead {
    /// Reads available bytes from this reader into `buf`.
    ///
    /// Returns `Poll::Ready(Ok(n))` where `n` is the number of bytes actually
    /// read.
    ///
    /// *   `n == 0`:        Indicates end-of-file (EOF).
    /// *   `n < buf.len()`: Partial read (caller must retry).
    /// *   `Poll::Pending`: No data is available yet.
    ///
    /// # Errors
    ///
    /// Returns `Poll::Ready(Err(e))` if an I/O error is encountered.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>>;
}

impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for &mut T {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self).poll_read(cx, buf)
    }
}

impl<P> AsyncRead for Pin<P>
where
    P: ops::DerefMut,
    P::Target: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.as_deref_mut().poll_read(cx, buf)
    }
}

/// Extension trait to [`AsyncRead`] which adds utility methods.
pub trait AsyncReadExt: AsyncRead {
    /// Reads available bytes from this reader into `buf`.
    ///
    /// Returns a future that resolves to `Ok(n)` where `n` is the number of
    /// bytes read:
    ///
    /// *   `n == buf.len()`: Entire buffer was filled.
    /// *   `n < buf.len()`:  Partial read (caller must retry).
    /// *   `n == 0`:         The reader is closed or `buf.len()` is 0.
    ///
    /// # Errors
    ///
    /// Returns `Err` if an I/O error is encountered. On error, no bytes are
    /// read. Partial reads are __not__ considered an error.
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self>
    where
        Self: Unpin,
    {
        read(self, buf)
    }

    /// Reads the exact number of bytes required to fill `buf`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if an I/O error or [`ErrorKind::UnexpectedEof`] is
    /// encountered. On error, the contents of `buf` are unspecified.
    ///
    /// [`ErrorKind::UnexpectedEof`]: std::io::ErrorKind::UnexpectedEof
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExact<'a, Self>
    where
        Self: Unpin,
    {
        read_exact(self, buf)
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}
