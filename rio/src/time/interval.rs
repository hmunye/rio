use std::future;
use std::pin::Pin;
use std::task::{Poll, ready};
use std::time::{Duration, Instant};

use crate::rt::time::clock;
use crate::time::{self, Sleep};

/// Creates an `Interval` that triggers at a fixed `period`, with the first tick
/// completing immediately.
///
/// Equivalent to <code>[interval_at](Instant::now(), period)</code>.
///
/// The `Interval` is canceled by dropping it.
///
/// # Panics
///
/// Panics if `period` is zero.
///
/// # Examples
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use std::time::Duration;
///
/// let mut interval = rio::time::interval(Duration::from_millis(10));
///
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks after 10ms
/// interval.tick().await; // ticks after 10ms
///
/// // approximately 20ms have elapsed.
/// # }
/// ```
///
/// In some cases, an interval may miss ticks (e.g., if a task takes longer
/// than `period`). In such cases, the interval will "catch up" by firing ticks
/// as quickly as necessary until it reaches the expected schedule (`burst`).
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use std::time::Duration;
///
/// let mut interval = rio::time::interval(Duration::from_millis(50));
///
/// interval.tick().await; // ticks immediately
///
/// let fut = async || { rio::time::sleep(Duration::from_millis(200)).await; };
/// fut().await;
/// // missed some ticks...
///
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
///
/// interval.tick().await; // ticks after 50ms
/// # }
/// ```
#[inline]
pub fn interval(period: Duration) -> Interval {
    assert!(period > Duration::new(0, 0), "`period` must be non-zero");
    Interval::new(period)
}

/// Creates an `Interval` that triggers at a fixed `period`, with the first tick
/// completing at `start`.
///
/// The `Interval` is canceled by dropping it.
///
/// # Panics
///
/// Panics if `period` is zero.
///
/// # Examples
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use std::time::{Duration, Instant};
///
/// let start = Instant::now() + Duration::from_millis(50);
/// let mut interval = rio::time::interval_at(start, Duration::from_millis(10));
///
/// interval.tick().await; // ticks after 50ms
/// interval.tick().await; // ticks after 10ms
/// interval.tick().await; // ticks after 10ms
///
/// // approximately 70ms have elapsed.
/// # }
/// ```
///
/// In some cases, an interval may miss ticks (e.g., if a task takes longer
/// than `period`). In such cases, the interval will "catch up" by firing ticks
/// as quickly as necessary until it reaches the expected schedule (`burst`).
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use std::time::{Duration, Instant};
///
/// let start = Instant::now();
/// let mut interval = rio::time::interval_at(start, Duration::from_millis(50));
///
/// interval.tick().await; // ticks immediately
///
/// let fut = async || { rio::time::sleep(Duration::from_millis(200)).await; };
/// fut().await;
/// // missed some ticks...
///
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
/// interval.tick().await; // ticks immediately
///
/// interval.tick().await; // ticks after 50ms
/// # }
/// ```
#[inline]
pub fn interval_at(start: Instant, period: Duration) -> Interval {
    assert!(period > Duration::new(0, 0), "`period` must be non-zero");
    Interval::new_at(start, period)
}

/// Interval returned by [`interval`] and [`interval_at`].
#[derive(Debug)]
#[must_use]
pub struct Interval {
    delay: Sleep,
    period: Duration,
}

impl Interval {
    /// Waits until the next interval tick, returning the [`Instant`] it was
    /// scheduled to complete.
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
    /// use std::time::Duration;
    ///
    /// let mut interval = rio::time::interval(Duration::from_millis(10));
    ///
    /// interval.tick().await; // ticks immediately
    /// interval.tick().await; // ticks after 10ms
    /// interval.tick().await; // ticks after 10ms
    ///
    /// // approximately 20ms have elapsed.
    /// # }
    /// ```
    #[inline]
    pub async fn tick(&mut self) -> Instant {
        future::poll_fn(|cx| {
            ready!(Pin::new(&mut self.delay).poll(cx));

            let last_tick = self.delay.deadline();

            // Each tick is scheduled one `period` after `last_tick`, even if
            // ticks were missed. If the interval was delayed, `last_tick` will
            // be in the past, so `next_tick` will complete immediately.
            let next_tick = last_tick.checked_add(self.period()).unwrap_or_else(|| {
                // <https://docs.rs/tokio/latest/src/tokio/time/instant.rs.html#34-36>
                clock::now() + Duration::from_secs(86400 * 365 * 30)
            });

            self.delay.reset(next_tick);

            Poll::Ready(last_tick)
        })
        .await
    }

    /// Returns the period of this `Interval`.
    #[inline]
    #[must_use]
    pub const fn period(&self) -> Duration {
        self.period
    }

    fn new(period: Duration) -> Self {
        Interval {
            delay: time::sleep_until(clock::now()),
            period,
        }
    }

    const fn new_at(start: Instant, period: Duration) -> Self {
        Interval {
            delay: time::sleep_until(start),
            period,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::task::Context;

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
    fn test_interval_first_tick_is_immediate() {
        rt! {
            let mut interval = interval(Duration::from_millis(10));

            assert_eq!(interval.tick().await, clock::now());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_interval_multiple_ticks() {
        rt! {
            let mut interval = interval(Duration::from_millis(10));

            let t0 = interval.tick().await;
            assert_eq!(t0, clock::now());

            clock::advance(Duration::from_millis(9)).await;

            let mut pending = interval.tick();
            let mut pinned = unsafe { Pin::new_unchecked(&mut pending) };
            let mut cx = Context::from_waker(std::task::Waker::noop());
            assert!(pinned.as_mut().poll(&mut cx).is_pending());

            clock::advance(Duration::from_millis(1)).await;
            let t1 = pinned.as_mut().await;
            assert_eq!(t1, t0 + Duration::from_millis(10));

            drop(pending);

            clock::advance(Duration::from_millis(10)).await;
            let t2 = interval.tick().await;
            assert_eq!(t2, t1 + Duration::from_millis(10));

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_interval_at_start_time() {
        rt! {
            let start = clock::now() + Duration::from_millis(50);
            let mut interval = interval_at(start, Duration::from_millis(10));

            clock::advance(Duration::from_millis(49)).await;

            let mut pending = interval.tick();
            let mut pinned = unsafe { Pin::new_unchecked(&mut pending) };
            let mut cx = Context::from_waker(std::task::Waker::noop());
            assert!(pinned.as_mut().poll(&mut cx).is_pending());

            clock::advance(Duration::from_millis(1)).await;
            let t0 = pinned.as_mut().await;
            assert_eq!(t0, start);

            drop(pending);

            clock::advance(Duration::from_millis(10)).await;
            let t1 = interval.tick().await;
            assert_eq!(t1, start + Duration::from_millis(10));

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_interval_burst_after_delay() {
        rt! {
            let mut interval = interval(Duration::from_millis(50));

            let start = interval.tick().await;
            assert_eq!(start, clock::now());

            clock::advance(Duration::from_millis(200)).await;

            let t1 = interval.tick().await;
            let t2 = interval.tick().await;
            let t3 = interval.tick().await;
            let t4 = interval.tick().await;

            assert_eq!(t1, start + Duration::from_millis(50));
            assert_eq!(t2, start + Duration::from_millis(100));
            assert_eq!(t3, start + Duration::from_millis(150));
            assert_eq!(t4, start + Duration::from_millis(200));

            let mut pending = interval.tick();
            let mut pinned = unsafe { Pin::new_unchecked(&mut pending) };
            let mut cx = Context::from_waker(std::task::Waker::noop());
            assert!(pinned.as_mut().poll(&mut cx).is_pending());

            clock::advance(Duration::from_millis(50)).await;
            let t5 = pinned.as_mut().await;
            assert_eq!(t5, start + Duration::from_millis(250));

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_interval_cancellation() {
        rt! {
            let handle = crate::spawn(async {
                let mut interval = interval(Duration::from_millis(50));

                interval.tick().await;

                {
                    let mut pending = interval.tick();
                    let mut pinned = unsafe { Pin::new_unchecked(&mut pending) };
                    let mut cx = Context::from_waker(std::task::Waker::noop());
                    assert!(pinned.as_mut().poll(&mut cx).is_pending());
                }

                assert!(rt_timer_count() > 0);
                drop(interval);
                assert_eq!(rt_timer_count(), 0);
            });

            clock::advance(Duration::from_millis(50)).await;
            assert!(handle.is_finished());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }
}
