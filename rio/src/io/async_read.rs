use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, ops};

/// Reads bytes asynchronously from a source, analogous to [`std::io::Read`].
pub trait AsyncRead {
    /// Reads available bytes from this source into `buf`.
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
    /// TODO:
    fn read<'a>(&'a mut self, _buf: &'a mut [u8]) -> impl Future<Output = io::Result<usize>>
    where
        Self: Unpin,
    {
        std::future::ready(Ok(0))
    }

    /// TODO:
    fn read_exact<'a>(&'a mut self, _buf: &'a mut [u8]) -> ()
    where
        Self: Unpin,
    {
        todo!()
    }

    /// TODO:
    fn read_to_end<'a>(&'a mut self, _buf: &'a mut Vec<u8>) -> ()
    where
        Self: Unpin,
    {
        todo!()
    }

    /// TODO:
    fn read_to_string<'a>(&'a mut self, _dst: &'a mut String) -> ()
    where
        Self: Unpin,
    {
        todo!()
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}
