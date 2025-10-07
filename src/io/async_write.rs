use std::future::{self, Future};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

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

/// Implemented as an extension trait, adding utility methods to `AsyncWrite`
/// types.
pub trait AsyncWriteExt: AsyncWrite {
    /// Attempts to write bytes from `buf`, returning the number of bytes
    /// written.
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> impl Future<Output = io::Result<usize>> + 'a
    where
        Self: std::marker::Unpin,
    {
        future::poll_fn(move |ctx| Pin::new(&mut *self).poll_write(ctx, buf))
    }

    /// Attempts to write an entire buffer into this writer.
    fn write_all<'a>(&'a mut self, mut buf: &'a [u8]) -> impl Future<Output = io::Result<()>> + 'a
    where
        Self: std::marker::Unpin,
    {
        async move {
            while !buf.is_empty() {
                let n = self.write(buf).await?;

                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write all bytes",
                    ));
                }

                buf = &buf[n..];
            }

            Ok(())
        }
    }

    /// Flushes any buffered data.
    fn flush<'a>(&'a mut self) -> impl Future<Output = io::Result<()>> + 'a
    where
        Self: std::marker::Unpin,
    {
        future::poll_fn(move |ctx| Pin::new(&mut *self).poll_flush(ctx))
    }

    /// Shuts down the write half of this object.
    fn shutdown<'a>(&'a mut self) -> impl Future<Output = io::Result<()>> + 'a
    where
        Self: std::marker::Unpin,
    {
        future::poll_fn(move |ctx| Pin::new(&mut *self).poll_shutdown(ctx))
    }
}

impl<T: AsyncWrite + ?Sized> AsyncWriteExt for T {}
