use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::io::AsyncWrite;

pub const fn write<'a, W>(writer: &'a mut W, buf: &'a [u8]) -> Write<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    Write {
        writer,
        buf,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncWriteExt::write`].
///
/// [`AsyncWriteExt::write`]: crate::io::AsyncWriteExt::write
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Write<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
    // <https://docs.rs/tokio/latest/src/tokio/io/util/write.rs.html#17>
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `Write<'_, W>`.
#[allow(clippy::mut_mut)]
struct WriteProj<'a, 'p, W: ?Sized> {
    writer: &'p mut &'a mut W,
    buf: &'p mut &'a [u8],
}

impl<'a, W: ?Sized> Write<'a, W> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> WriteProj<'a, '_, W> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        WriteProj {
            writer: &mut me.writer,
            buf: &mut me.buf,
        }
    }
}

impl<W> Future for Write<'_, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(&mut *me.writer).poll_write(cx, me.buf)
    }
}
