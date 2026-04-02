use std::future;
use std::pin::Pin;
use std::task::{Poll, ready};
use std::time::{Duration, Instant};

use crate::time::{self, Sleep};

/// Creates an `Interval` that periodically yields at a fixed `period`, with the
/// first tick completing immediately.
///
/// Equivalent to calling <code>[interval_at](Instant::now(), period)</code>.
///
/// The `Interval` is canceled by dropping it.
///
/// # Panics
///
/// Panics if `period` is zero.
///
/// # Examples
///
/// ```
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
/// ```
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

/// Creates an `Interval` that periodically yields at a fixed `period`, with the
/// first tick completing at `start`.
///
/// The `Interval` is canceled by dropping it.
///
/// # Panics
///
/// Panics if `period` is zero.
///
/// # Examples
///
/// ```
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
/// ```
/// # #[rio::main]
/// # async fn main() {
/// use std::time::{Duration, Instant};
///
/// let mut interval = rio::time::interval_at(Instant::now(), Duration::from_millis(50));
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
    /// Waits until the next interval tick, returning the `Instant` at which
    /// that tick was scheduled.
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

            // Each tick is scheduled one `period` after `scheduled`, even if
            // ticks were missed. If the interval was delayed, `scheduled` will
            // be in the past, so the next tick will return immediately.
            let next_tick = last_tick.checked_add(self.period()).unwrap_or_else(|| {
                // <https://docs.rs/tokio/latest/src/tokio/time/instant.rs.html#34-36>
                Instant::now() + Duration::from_secs(86400 * 365 * 30)
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
            delay: time::sleep_until(Instant::now()),
            period,
        }
    }

    fn new_at(start: Instant, period: Duration) -> Self {
        Interval {
            delay: time::sleep_until(start),
            period,
        }
    }
}
