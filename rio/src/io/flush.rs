use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::io::AsyncWrite;

pub const fn flush<F>(f: &mut F) -> Flush<'_, F>
where
    F: AsyncWrite + Unpin + ?Sized,
{
    Flush {
        f,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncWriteExt::flush`].
///
/// [`AsyncWriteExt::flush`]: crate::io::AsyncWriteExt::flush
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Flush<'a, F: ?Sized> {
    f: &'a mut F,
    // <https://docs.rs/tokio/latest/src/tokio/io/util/flush.rs.html#20>
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `Flush<'_, F>`.
#[allow(clippy::mut_mut)]
struct FlushProj<'a, 'p, F: ?Sized> {
    f: &'p mut &'a mut F,
}

impl<'a, F: ?Sized> Flush<'a, F> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> FlushProj<'a, '_, F> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        FlushProj { f: &mut me.f }
    }
}

impl<F> Future for Flush<'_, F>
where
    F: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(me.f).poll_flush(cx)
    }
}
