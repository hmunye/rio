use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::io::AsyncRead;

pub const fn read<'a, R>(reader: &'a mut R, buf: &'a mut [u8]) -> Read<'a, R>
where
    R: AsyncRead + Unpin + ?Sized,
{
    Read {
        reader,
        buf,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncReadExt::read`].
///
/// [`AsyncReadExt::read`]: crate::io::AsyncReadExt::read
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Read<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
    // <https://docs.rs/tokio/latest/src/tokio/io/util/read.rs.html#37>
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `Read<'_, R>`.
#[allow(clippy::mut_mut)]
struct ReadProj<'a, 'p, R: ?Sized> {
    reader: &'p mut &'a mut R,
    buf: &'p mut &'a mut [u8],
}

impl<'a, R: ?Sized> Read<'a, R> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> ReadProj<'a, '_, R> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        ReadProj {
            reader: &mut me.reader,
            buf: &mut me.buf,
        }
    }
}

impl<R> Future for Read<'_, R>
where
    R: AsyncRead + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(&mut *me.reader).poll_read(cx, me.buf)
    }
}
