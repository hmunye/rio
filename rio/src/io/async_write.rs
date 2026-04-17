use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, ops};

use crate::io::write::{Write, write};
use crate::io::write_all::{WriteAll, write_all};

/// Writes bytes asynchronously, analogous to [`std::io::Write`].
pub trait AsyncWrite {
    /// Writes bytes from `buf` into this writer.
    ///
    /// Returns `Poll::Ready(Ok(n))` where `n` is the number of bytes written.
    ///
    /// * `n == 0`:        The writer is closed or the buffer is empty.
    /// * `n < buf.len()`: Partial write (caller must retry).
    /// * `Poll::Pending`: Writer is not ready for writing.
    ///
    /// # Errors
    ///
    /// Returns `Poll::Ready(Err(e))` if an I/O error is encountered.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>>;

    /// Flushes any buffered data to reach their destination.
    ///
    /// Returns `Poll::Ready(Ok(()))` when all buffered data has been flushed,
    /// or `Poll::Pending` if flushing cannot be done immediately.
    ///
    /// # Errors
    ///
    /// Returns `Poll::Ready(Err(e))` if an I/O error is encountered.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;

    /// Initiates a graceful shutdown of this writer.
    ///
    /// Returns `Poll::Ready(Ok(()))` when the writer is fully shutdown, or
    /// `Poll::Pending` if the shutdown was initiated but not complete.
    ///
    /// # Errors
    ///
    /// Returns `Poll::Ready(Err(e))` if an I/O error is encountered.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
}

impl<T: AsyncWrite + Unpin + ?Sized> AsyncWrite for &mut T {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut **self).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut **self).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut **self).poll_shutdown(cx)
    }
}

impl<P> AsyncWrite for Pin<P>
where
    P: ops::DerefMut,
    P::Target: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.as_deref_mut().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_deref_mut().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.as_deref_mut().poll_shutdown(cx)
    }
}

/// Extension trait to [`AsyncWrite`] which adds utility methods.
pub trait AsyncWriteExt: AsyncWrite {
    /// Attempts to write bytes from a buffer into this writer.
    ///
    /// Returns `Ok(n)` if no error occurred, where `n` is the number of bytes
    /// written:
    ///
    /// * `n == src.len()`: Entire buffer was written or buffer was empty.
    /// * `n < src.len()`:  Partial write.
    /// * `n == 0`:         The writer is closed or `src` is empty.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if encountered. Partial writes are **not**
    /// considered an error.
    fn write<'a>(&'a mut self, src: &'a [u8]) -> Write<'a, Self>
    where
        Self: Unpin,
    {
        write(self, src)
    }

    /// Attempts to write an entire buffer into this writer.
    ///
    /// Will continuously call [`write`] until there is no more data to be
    /// written. This method will not return until the entire buffer has been
    /// successfully written or an error occurs.
    ///
    /// # Errors
    ///
    /// Returns the first [`write`] I/O error that occurs.
    fn write_all<'a>(&'a mut self, src: &'a [u8]) -> WriteAll<'a, Self>
    where
        Self: Unpin,
    {
        write_all(self, src)
    }

    /// TODO:
    fn flush(&mut self) -> ()
    where
        Self: Unpin,
    {
        todo!()
    }

    /// TODO:
    fn shutdown(&mut self) -> ()
    where
        Self: Unpin,
    {
        todo!()
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}
