use std::marker::PhantomPinned;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{io, mem};

use crate::io::AsyncRead;

pub const fn read_exact<'a, R>(reader: &'a mut R, buf: &'a mut [u8]) -> ReadExact<'a, R>
where
    R: AsyncRead + Unpin + ?Sized,
{
    ReadExact {
        reader,
        cap: buf.len(),
        buf,
        _pin: PhantomPinned,
    }
}

/// Future returned by [`AsyncReadExt::read_exact`].
///
/// [`AsyncReadExt::read_exact`]: crate::io::AsyncReadExt::read_exact
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadExact<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
    cap: usize,
    // <https://docs.rs/tokio/latest/src/tokio/io/util/read_exact.rs.html#37>
    //
    // commit: 6c03e03898d71eca976ee1ad8481cf112ae722ba
    _pin: PhantomPinned,
}

/// Projection type providing a "view" over a `ReadExact<'_, R>`.
#[allow(clippy::mut_mut)]
struct ReadExactProj<'a, 'p, R: ?Sized> {
    reader: &'p mut &'a mut R,
    buf: &'p mut &'a mut [u8],
    cap: usize,
}

impl<'a, R: ?Sized> ReadExact<'a, R> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> ReadExactProj<'a, '_, R> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        let me = unsafe { self.get_unchecked_mut() };

        ReadExactProj {
            reader: &mut me.reader,
            buf: &mut me.buf,
            cap: me.cap,
        }
    }
}

// <https://docs.rs/tokio/latest/src/tokio/io/util/read_exact.rs.html#47>
//
// commit: 6c03e03898d71eca976ee1ad8481cf112ae722ba
impl<R> Future for ReadExact<'_, R>
where
    R: AsyncRead + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        loop {
            let rem = me.buf.len();

            if rem != 0 {
                let n = ready!(Pin::new(&mut *me.reader).poll_read(cx, me.buf))?;

                {
                    let (_, rest) = mem::take(&mut *me.buf).split_at_mut(n);
                    *me.buf = rest;
                }

                if me.buf.len() == rem {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "early EOF")).into();
                }
            } else {
                return Poll::Ready(Ok(me.cap));
            }
        }
    }
}
