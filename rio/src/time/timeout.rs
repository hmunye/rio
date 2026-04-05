use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::task::coop;
use crate::time::{self, Sleep};

/// Error returned by [`Timeout`] when it has elapsed.
#[derive(Debug)]
pub struct Elapsed(());

impl fmt::Display for Elapsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "timeout has elapsed".fmt(f)
    }
}

impl std::error::Error for Elapsed {}

/// Wraps a `Future`, restricting its execution time to `duration`.
///
/// If the provided future completes before `duration` has elapsed, its value is
/// yielded; otherwise, an [`error`](Elapsed) is returned and the future is
/// canceled.
///
/// The `Timeout` is canceled by dropping it.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```
/// # #[rio::main]
/// # async fn main() {
/// use std::time::{Duration, Instant};
///
/// let t = rio::time::timeout(Duration::from_millis(500), async {
///     rio::time::sleep(Duration::from_millis(800)).await;
/// });
///
/// assert!(t.await.is_err()); // timeout will elapse before future completes
/// # }
/// ```
#[inline]
pub fn timeout<F>(duration: Duration, fut: F) -> Timeout<F::IntoFuture>
where
    F: IntoFuture,
{
    Timeout {
        val: fut.into_future(),
        delay: time::sleep(duration),
    }
}

/// Wraps a `Future`, restricting its execution time until `deadline`.
///
/// If the provided future completes before `deadline` is reached, its value is
/// yielded; otherwise, an [`error`](Elapsed) is returned and the future is
/// canceled.
///
/// The `Timeout` is canceled by dropping it.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```
/// # #[rio::main]
/// # async fn main() {
/// use std::time::{Duration, Instant};
///
/// let t = rio::time::timeout_at(Instant::now() + Duration::from_millis(500), async {
///     rio::time::sleep(Duration::from_millis(800)).await;
/// });
///
/// assert!(t.await.is_err()); // timeout will elapse before future completes
/// # }
/// ```
#[inline]
pub fn timeout_at<F>(deadline: Instant, fut: F) -> Timeout<F::IntoFuture>
where
    F: IntoFuture,
{
    Timeout {
        val: fut.into_future(),
        delay: time::sleep_until(deadline),
    }
}

/// Future returned by [`timeout`] and [`timeout_at`].
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Timeout<F> {
    val: F,
    delay: Sleep,
}

impl<F> Unpin for Timeout<F> where F: Unpin {}

/// Projection type providing a "view" over a `Timeout<F>`, where each field is
/// a pinned mutable reference of itself.
struct TimeoutProj<'p, F> {
    val: Pin<&'p mut F>,
    delay: Pin<&'p mut Sleep>,
}

impl<F> Timeout<F> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> TimeoutProj<'_, F> {
        // SAFETY: `self` is a pinned mutable reference to `Timeout<F>`, making
        // it safe to pin the fields, since `Pin<T>` guarantees that the memory
        // address of this instance will not change.
        unsafe {
            let mut_self = self.get_unchecked_mut();

            TimeoutProj {
                val: Pin::new_unchecked(&mut mut_self.val),
                delay: Pin::new_unchecked(&mut mut_self.delay),
            }
        }
    }
}

// <https://docs.rs/tokio/latest/src/tokio/time/timeout.rs.html#212>
impl<F: Future> Future for Timeout<F> {
    type Output = Result<F::Output, Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();
        let budget_before = coop::has_budget_remaining();

        if let Poll::Ready(out) = me.val.poll(cx) {
            return Poll::Ready(Ok(out));
        }

        poll_delay(budget_before, me.delay, cx).map(Err)
    }
}

// <https://docs.rs/tokio/latest/src/tokio/time/timeout.rs.html#212>
fn poll_delay(budget_before: bool, delay: Pin<&mut Sleep>, cx: &mut Context<'_>) -> Poll<Elapsed> {
    let poll = || match delay.poll(cx) {
        Poll::Ready(()) => Poll::Ready(Elapsed(())),
        Poll::Pending => Poll::Pending,
    };

    if budget_before && !coop::has_budget_remaining() {
        // `delay` is cooperative, so it should be polled with an unconstrained
        // execution budget, since the wrapped future has already exhausted the
        // current "tick"'s budget. This ensures it has a chance to actually
        // execute.
        coop::with_unconstrained(poll)
    } else {
        poll()
    }
}
