use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::rt::CURRENT_RUNTIME;

/// Waits until `duration` has elapsed.
///
/// This is equivalent to calling `sleep_until(Instant::now() + duration)`,
/// and functions as an asynchronous alternative to `std::thread::sleep`.
pub fn sleep(duration: Duration) -> Sleep {
    // Wait for a relative amount of time from `Instant::now`.
    Sleep::new_timeout(Instant::now() + duration)
}

/// Waits until `deadline` is reached.
pub fn sleep_until(deadline: Instant) -> Sleep {
    // Wait until the specific absolute time.
    Sleep::new_timeout(deadline)
}

/// Future returned by `sleep` and `sleep_until`.
#[derive(Debug)]
pub struct Sleep {
    /// Point in time to wake the associated `Task`.
    wake_at: Instant,
    /// Indicates whether the `Sleep` has been registered with the scheduler.
    registered: bool,
}

impl Sleep {
    /// Creates a new `Sleep` which waits until `duration` has elapsed.
    #[inline]
    pub(crate) fn new_timeout(duration: Instant) -> Self {
        Sleep {
            wake_at: duration,
            registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        // Compare the current time with the timeout duration set when the
        // `Sleep` future was created. If the current time is `>=` to the
        // `wake_at` time, the timeout has been reached or passed.
        if Instant::now() >= self.wake_at {
            return Poll::Ready(());
        }

        if !self.registered {
            self.registered = true;

            CURRENT_RUNTIME.with(|rt| {
                if let Some(ptr) = rt.get() {
                    // SAFETY: The thread-local holds a raw pointer to a
                    // `Runtime`. This pointer is only set via the entry point
                    // `Runtime::block_on`, and cleared when the associated
                    // `EnterGuard` is dropped. Polling a `Sleep` is only
                    // possible within the context of a runtime.
                    let rt = unsafe { &*ptr };
                    rt.scheduler.register_timer(self.wake_at);
                } else {
                    panic!("`sleep/sleep_until` called outside of a rutime context");
                }
            });
        }

        Poll::Pending
    }
}
