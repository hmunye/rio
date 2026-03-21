use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::runtime::context;

/// Waits until `deadline` is reached.
///
/// No work is performed while awaiting on the `Sleep` future to complete.
/// Cancellation is done by dropping the returned future. No additional cleanup
/// logic is required.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// let rt = rio::runtime::Runtime::new();
///     
/// rt.block_on(async {
///     rio::time::sleep_until(std::time::Instant::now() + std::time::Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// });
/// ```
#[inline]
#[must_use]
pub const fn sleep_until(deadline: Instant) -> Sleep {
    Sleep::new_timeout(deadline)
}

/// Waits until `duration` has elapsed.
///
/// Equivalent to calling `sleep_until(Instant::now() + duration)` and serves as
/// a non-blocking alternative to [`std::thread::sleep`].
///
/// No work is performed while awaiting on the `Sleep` future to complete.
/// Cancellation is done by dropping the returned future. No additional cleanup
/// logic is required.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// let rt = rio::runtime::Runtime::new();
///     
/// rt.block_on(async {
///     rio::time::sleep(std::time::Duration::from_millis(100)).await;
///     println!("100 ms have elapsed");
/// });
/// ```
#[inline]
#[must_use]
pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new_timeout(Instant::now() + duration)
}

/// Future returned by [`sleep`] and [`sleep_until`].
#[derive(Debug)]
pub struct Sleep {
    /// Point in time to wake the associated task.
    wake_at: Instant,
    registered: bool,
}

impl Sleep {
    /// Returns the instant at which the future will complete.
    #[inline]
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.wake_at
    }

    /// Returns `true` if `Sleep` has elapsed.
    ///
    /// A `Sleep` instance is elapsed when the requested duration has elapsed.
    #[inline]
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        Instant::now() >= self.wake_at
    }

    /// Creates a new `Sleep`.
    #[inline]
    const fn new_timeout(duration: Instant) -> Self {
        Sleep {
            wake_at: duration,
            registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_elapsed() {
            return Poll::Ready(());
        }

        if !self.registered {
            self.registered = true;

            context::with_current(|handle| {
                handle
                    .time
                    .register_timer(self.wake_at, ctx.waker().clone());
            });
        }

        Poll::Pending
    }
}
