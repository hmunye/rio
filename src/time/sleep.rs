use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::rt::Runtime;

/// Future returned by [`sleep`] and [`sleep_until`].
#[derive(Debug)]
pub struct SleepFut {
    /// Point in time to wake the associated task.
    wake_at: Instant,
    /// Indicates whether the future has been registered with the scheduler.
    registered: bool,
}

impl SleepFut {
    /// Creates a new `Sleep`.
    pub(crate) const fn new_timeout(duration: Instant) -> Self {
        SleepFut {
            wake_at: duration,
            registered: false,
        }
    }
}

impl Future for SleepFut {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        // Compare the current time with the timeout duration set when the
        // future was created. If the current time is `>=` to the `wake_at`
        // time, timeout has been reached or expired.
        if Instant::now() >= self.wake_at {
            return Poll::Ready(());
        }

        if !self.registered {
            self.registered = true;

            Runtime::current()
                .scheduler
                .register_timer(self.wake_at, ctx.waker().clone());
        }

        Poll::Pending
    }
}

/// Waits until `duration` has elapsed.
///
/// This is equivalent to calling `sleep_until(Instant::now() + duration)`, and
/// functions as an asynchronous alternative to [`std::thread::sleep`].
#[must_use]
pub fn sleep(duration: Duration) -> SleepFut {
    // Wait for a relative amount of time starting from now.
    SleepFut::new_timeout(Instant::now() + duration)
}

/// Waits until `deadline` is reached.
#[must_use]
pub const fn sleep_until(deadline: Instant) -> SleepFut {
    // Wait until the specific absolute time.
    SleepFut::new_timeout(deadline)
}
