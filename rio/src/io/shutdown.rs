use std::io;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::io::AsyncWrite;

pub const fn shutdown<S>(s: &mut S) -> Shutdown<'_, S>
where
    S: AsyncWrite + Unpin + ?Sized,
{
    Shutdown {
        s,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncWriteExt::shutdown`].
///
/// [`AsyncWriteExt::shutdown`]: crate::io::AsyncWriteExt::shutdown
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Shutdown<'a, S: ?Sized> {
    s: &'a mut S,
    // <https://docs.rs/tokio/latest/src/tokio/io/util/shutdown.rs.html#19>
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `Shutdown<'_, S>`.
#[allow(clippy::mut_mut)]
struct ShutdownProj<'a, 'p, S: ?Sized> {
    s: &'p mut &'a mut S,
}

impl<'a, S: ?Sized> Shutdown<'a, S> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> ShutdownProj<'a, '_, S> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        ShutdownProj { s: &mut me.s }
    }
}

impl<S> Future for Shutdown<'_, S>
where
    S: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        Pin::new(me.s).poll_shutdown(cx)
    }
}
