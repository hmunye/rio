use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{io, mem};

use crate::io::AsyncWrite;

pub const fn write_all<'a, W>(writer: &'a mut W, buf: &'a [u8]) -> WriteAll<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    WriteAll {
        writer,
        buf,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncWriteExt::write_all`].
///
/// [`AsyncWriteExt::write_all`]: crate::io::AsyncWriteExt::write_all
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct WriteAll<'a, W: ?Sized> {
    writer: &'a mut W,
    buf: &'a [u8],
    // <https://docs.rs/tokio/latest/src/tokio/io/util/write_all.rs.html#17>
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `WriteAll<'_, W>`.
#[allow(clippy::mut_mut)]
struct WriteAllProj<'a, 'p, W: ?Sized> {
    writer: &'p mut &'a mut W,
    buf: &'p mut &'a [u8],
}

impl<'a, W: ?Sized> WriteAll<'a, W> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> WriteAllProj<'a, '_, W> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        WriteAllProj {
            writer: &mut me.writer,
            buf: &mut me.buf,
        }
    }
}

// <https://docs.rs/tokio/latest/src/tokio/io/util/write_all.rs.html#34>
impl<W> Future for WriteAll<'_, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        while !me.buf.is_empty() {
            let n = ready!(Pin::new(&mut *me.writer).poll_write(cx, me.buf))?;

            {
                let (_, rest) = mem::take(&mut *me.buf).split_at(n);
                *me.buf = rest;
            }

            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }

        Poll::Ready(Ok(()))
    }
}
