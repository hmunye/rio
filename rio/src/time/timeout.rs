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
/// canceled. The `Timeout` is canceled by dropping it.
///
/// If the provided future is ready immediately, the `Timeout` is guaranteed to
/// yield the futures value no matter the provided duration.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```no_run
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
/// canceled. The `Timeout` is canceled by dropping it.
///
/// If the provided future is ready immediately, the `Timeout` is guaranteed to
/// yield the futures value no matter the provided deadline.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```no_run
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

/// Projection type providing a "view" over a `Timeout<F>`.
struct TimeoutProj<'p, F> {
    val: Pin<&'p mut F>,
    delay: Pin<&'p mut Sleep>,
}

impl<F> Timeout<F> {
    #[must_use]
    const fn project(self: Pin<&mut Self>) -> TimeoutProj<'_, F> {
        // SAFETY: We do not move out of the pinned value, only project its
        // fields.
        unsafe {
            let me = self.get_unchecked_mut();

            TimeoutProj {
                val: Pin::new_unchecked(&mut me.val),
                delay: Pin::new_unchecked(&mut me.delay),
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::rt::context;
    use crate::rt::time::clock;

    #[cfg(not(miri))]
    const THRESHOLD_MS: u64 = 5;

    fn rt_timer_count() -> usize {
        #[allow(clippy::redundant_closure_for_method_calls)]
        context::with_handle(|handle| handle.timers())
    }

    #[test]
    fn test_timeout_expires() {
        rt! {
            let handle = crate::spawn(async {
                let t = timeout(Duration::from_millis(100), async {
                    std::future::pending::<()>().await;
                });

                assert!(t.await.is_err());
            });

            clock::advance(Duration::from_millis(100)).await;

            assert!(handle.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_timeout_cancellation() {
        rt! {
            let handle = crate::spawn(async {
                let mut t = std::pin::pin!(timeout(Duration::from_millis(100_000), async {
                    std::future::pending::<()>().await;
                }));

                let mut cx = Context::from_waker(std::task::Waker::noop());

                assert!(t.as_mut().poll(&mut cx).is_pending());

                assert!(rt_timer_count() > 0);
                #[allow(clippy::drop_non_drop)]
                drop(t);
                assert_eq!(rt_timer_count(), 0);
            });

            clock::advance(Duration::from_millis(50)).await;

            assert!(handle.is_finished());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_timeout_inner_future_preempts_duration() {
        rt! {
            let t = timeout(Duration::ZERO, async {});

            clock::advance(Duration::from_millis(100)).await;

            assert!(t.await.is_ok());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_timeout_at_inner_future_preempts_deadline() {
        rt! {
            let t = timeout_at(
                clock::now()
                .checked_sub(Duration::from_millis(1))
                .expect("should not underflow"),
                async {},
            );

            clock::advance(Duration::from_millis(100)).await;

            assert!(t.await.is_ok());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_timeout_multiple_ordered() {
        rt! {
            let handle1 = crate::spawn(async {
                timeout(Duration::from_millis(100), async {
                    crate::time::sleep(Duration::from_millis(99)).await;
                }).await
            });

            let handle2 = crate::spawn(async {
                timeout(Duration::from_millis(49), async {
                    crate::time::sleep(Duration::from_millis(50)).await;
                }).await
            });

            let handle3 = crate::spawn(async {
                timeout(Duration::ZERO, async {
                    crate::time::sleep(Duration::from_millis(1)).await;
                }).await
            });

            clock::advance(Duration::from_millis(10)).await;

            assert!(!handle1.is_finished());
            assert!(!handle2.is_finished());
            assert!(handle3.await.expect("task should have completed").is_err());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(39)).await;

            assert!(handle2.await.expect("task should have completed").is_err());

            clock::advance(Duration::from_millis(1000)).await;

            assert!(handle1.await.expect("task should have completed").is_ok());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }
}
