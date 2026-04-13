use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use crate::rt::context;
use crate::rt::time::{TimerHandle, clock};
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
/// ```no_run
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
    let now = clock::now();

    let deadline = match now.checked_add(duration) {
        Some(deadline) => deadline,
        None => {
            // <https://docs.rs/tokio/latest/src/tokio/time/instant.rs.html#34-36>
            now + Duration::from_secs(86400 * 365 * 30)
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
/// ```no_run
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
    /// Returns `true` if this `Sleep`'s deadline has elapsed.
    #[inline]
    #[must_use]
    pub fn is_elapsed(&self) -> bool {
        clock::now() >= self.deadline
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

    pub(crate) fn reset(&mut self, deadline: Instant) {
        self.deadline = deadline;

        if let Some(timer_handle) = &self.handle {
            timer_handle.reset(deadline);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(miri))]
    const THRESHOLD_MS: u64 = 5;

    fn rt_timer_count() -> usize {
        #[allow(clippy::redundant_closure_for_method_calls)]
        context::with_handle(|handle| handle.timers())
    }

    #[test]
    fn test_sleep_no_early_wakeup() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            clock::advance(Duration::from_millis(50)).await;
            assert!(!handle.is_finished());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(49)).await;
            assert!(!handle.is_finished());

            clock::advance(Duration::from_millis(1)).await;
            assert!(handle.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_cancellation() {
        rt! {
            let handle = crate::spawn(async {
                let mut s = sleep(Duration::from_millis(10000));
                let mut cx = Context::from_waker(std::task::Waker::noop());

                assert!(Pin::new(&mut s).poll(&mut cx).is_pending());

                assert!(rt_timer_count() > 0);
                drop(s);
                assert_eq!(rt_timer_count(), 0);
            });

            clock::advance(Duration::from_millis(50)).await;
            assert!(handle.is_finished());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_multiple_ordered() {
        rt! {
            let handle1 = crate::spawn(async {
                sleep(Duration::from_millis(50)).await;
            });

            let handle2 = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            let handle3 = crate::spawn(async {
                sleep(Duration::from_millis(150)).await;
            });

            clock::advance(Duration::from_millis(30)).await;
            assert!(!handle1.is_finished());
            assert!(!handle2.is_finished());
            assert!(!handle3.is_finished());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(40)).await;
            assert!(handle1.is_finished());
            assert!(!handle2.is_finished());
            assert!(!handle3.is_finished());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(50)).await;
            assert!(handle2.is_finished());
            assert!(!handle3.is_finished());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(60)).await;
            assert!(handle3.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_multiple_same_deadline() {
        rt! {
            let handle1 = crate::spawn(async {
                sleep(Duration::from_millis(50)).await;
            });

            let handle2 = crate::spawn(async {
                sleep(Duration::from_millis(50)).await;
            });

            let handle3 = crate::spawn(async {
                sleep(Duration::from_millis(50)).await;
            });

            clock::advance(Duration::from_millis(30)).await;
            assert!(!handle1.is_finished());
            assert!(!handle2.is_finished());
            assert!(!handle3.is_finished());
            assert!(rt_timer_count() > 0);

            clock::advance(Duration::from_millis(20)).await;
            assert!(handle1.is_finished());
            assert!(handle2.is_finished());
            assert!(handle3.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_duration_zero() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::ZERO).await;
            });

            crate::task::yield_now().await;
            assert!(handle.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_until_deadline_past() {
        rt! {
            let handle = crate::spawn(async {
                sleep_until(
                    clock::now()
                    .checked_sub(Duration::from_millis(1))
                    .expect("should not underflow"),
                )
                .await;
            });

            crate::task::yield_now().await;
            assert!(handle.is_finished());
            assert_eq!(rt_timer_count(), 0);

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }
}
