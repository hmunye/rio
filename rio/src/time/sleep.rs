use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::rt::context;

/// Waits until `duration` has elapsed.
///
/// Equivalent to calling <code>[sleep_until](Instant::now() + duration)</code>.
///
/// No work is performed by the task while awaiting on the `Sleep` to complete.
/// The returned `Sleep` is canceled by dropping it.
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
/// rio::time::sleep(Duration::from_millis(100)).await;
/// println!("100ms have elapsed");
/// # }
/// ```
#[inline]
pub fn sleep(duration: Duration) -> Sleep {
    let deadline = match Instant::now().checked_add(duration) {
        Some(deadline) => deadline,
        None => {
            // <https://docs.rs/tokio/latest/src/tokio/time/instant.rs.html#34-36>
            Instant::now() + Duration::from_secs(86400 * 365 * 30)
        }
    };

    Sleep::new_timeout(deadline)
}

/// Waits until `deadline` is reached.
///
/// No work is performed by the task while awaiting on the `Sleep` to complete.
/// The returned `Sleep` is canceled by dropping it.
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
/// rio::time::sleep_until(Instant::now() + Duration::from_millis(100)).await;
/// println!("100ms have elapsed");
/// # }
/// ```
#[inline]
pub const fn sleep_until(deadline: Instant) -> Sleep {
    Sleep::new_timeout(deadline)
}

/// Future returned by [`sleep`] and [`sleep_until`].
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Sleep {
    deadline: Instant,
    registered: bool,
}

impl Sleep {
    const fn new_timeout(deadline: Instant) -> Self {
        Sleep {
            deadline,
            registered: false,
        }
    }

    /// Returns `true` if this `Sleep` has elapsed.
    ///
    /// A `Sleep` is elapsed when the requested duration/deadline has elapsed.
    #[inline]
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Returns the instant at which this `Sleep` will elapse.
    #[inline]
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) const fn reset(&mut self, deadline: Instant) {
        self.deadline = deadline;
        self.registered = false;
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_elapsed() {
            return Poll::Ready(());
        }

        if !self.registered {
            self.registered = true;

            context::with_handle(|handle| {
                handle.register_timer(self.deadline, cx.waker().clone());
            });
        }

        Poll::Pending
    }
}
