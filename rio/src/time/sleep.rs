use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use crate::rt::context;
use crate::rt::time::TimerHandle;
use crate::task::coop;

/// Waits until `duration` has elapsed.
///
/// Equivalent to <code>[sleep_until](Instant::now() + duration)</code>.
///
/// No work is performed by the task while awaiting on the `Sleep` to complete.
/// The `Sleep` is canceled by dropping it.
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
/// The `Sleep` is canceled by dropping it.
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
    handle: Option<TimerHandle>,
}

impl Sleep {
    /// Returns `true` if the deadline has elapsed.
    #[inline]
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Returns the [`Instant`] this `Sleep` will elapse.
    #[inline]
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    const fn new_timeout(deadline: Instant) -> Self {
        Sleep {
            deadline,
            handle: None,
        }
    }

    /// Updates the deadline and the underlying timer without re-registration.
    pub(crate) fn reset(&mut self, deadline: Instant) {
        self.deadline = deadline;

        if let Some(timer_handle) = &self.handle {
            context::with_handle(|handle| handle.update_timer(timer_handle.raw(), deadline));
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let coop = ready!(coop::poll_proceed());

        if self.is_elapsed() {
            coop.made_progress();
            return Poll::Ready(());
        }

        if self.handle.is_none() {
            self.handle = Some(context::with_handle(|handle| {
                handle.register_timer(self.deadline, cx.waker().clone())
            }));
        }

        Poll::Pending
    }
}
