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
/// Panics if the current thread is not within a runtime context or the caller
/// `.await` or polls the returned future outside of a runtime context.
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
    /// Returns `true` if the deadline has elapsed.
    ///
    /// # Panics
    ///
    /// Panics if the current thread is not within a runtime context.
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

    /// Updates the deadline and the underlying timer without re-registration.
    pub(crate) fn reset(&mut self, deadline: Instant) {
        self.deadline = deadline;

        if let Some(timer_handle) = &self.handle {
            context::with_handle(|handle| handle.update_timer(timer_handle, deadline));
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
    use std::future;

    use super::*;

    #[cfg(not(miri))]
    const THRESHOLD_MS: u64 = 5;

    #[test]
    fn test_sleep_wakeup() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            clock::advance(Duration::from_millis(50)).await;
            assert!(!handle.is_finished());

            clock::advance(Duration::from_millis(50)).await;
            assert!(handle.is_finished());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_until_wakeup() {
        rt! {
            let handle = crate::spawn(async {
                // `clock::now()` internally would return `Instant::now()` in
                // non-test environments.
                sleep_until(clock::now() + Duration::from_millis(10)).await;
            });

            clock::advance(Duration::from_millis(5)).await;
            assert!(!handle.is_finished());

            clock::advance(Duration::from_millis(5)).await;
            assert!(handle.is_finished());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_immediate() {
        rt! {
            let handle = crate::spawn(async {
                let mut sleep = sleep(Duration::ZERO);
                assert!(sleep.is_elapsed());

                future::poll_fn(move |cx| {
                    let poll = Pin::new(&mut sleep).poll(cx);
                    assert!(poll.is_ready());
                    poll
                }).await;
            });

            assert!(handle.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_until_immediate() {
        rt! {
            let handle = crate::spawn(async {
                let past_deadline = clock::now().checked_sub(Duration::from_millis(100)).unwrap();

                let mut sleep = sleep_until(past_deadline);
                assert!(sleep.is_elapsed());

                future::poll_fn(move |cx| {
                    let poll = Pin::new(&mut sleep).poll(cx);
                    assert!(poll.is_ready());
                    poll
                }).await;
            });

            assert!(handle.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_no_early_wakeup() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            clock::advance(Duration::from_millis(99)).await;
            assert!(!handle.is_finished());

            // Since the clock starts paused, this ensures the test can finish.
            clock::resume();

            assert!(handle.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_is_elapsed() {
        rt! {
            let s = sleep(Duration::from_millis(100));

            clock::advance(Duration::from_millis(99)).await;
            assert!(!s.is_elapsed());

            clock::advance(Duration::from_millis(1)).await;
            assert!(s.is_elapsed());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_concurrent_order() {
        rt! {
            let x = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            let y = crate::spawn(async {
                sleep(Duration::from_millis(200)).await;
            });

            let z = crate::spawn(async {
                sleep(Duration::from_millis(150)).await;
            });

            clock::advance(Duration::from_millis(140)).await;
            assert!(x.is_finished());
            assert!(!y.is_finished());
            assert!(!z.is_finished());

            clock::advance(Duration::from_millis(40)).await;
            assert!(!y.is_finished());
            assert!(z.is_finished());

            clock::advance(Duration::from_millis(20)).await;
            assert!(y.is_finished());

            assert!(x.await.is_ok());
            assert!(y.await.is_ok());
            assert!(z.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_single_advance() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            clock::advance(Duration::from_millis(100)).await;
            assert!(handle.is_finished());

            assert!(handle.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_large_jump_multiple_timers() {
        rt! {
            let x = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            let y = crate::spawn(async {
                sleep(Duration::from_millis(200)).await;
            });

            let z = crate::spawn(async {
                sleep(Duration::from_millis(150)).await;
            });

            clock::advance(Duration::from_millis(300)).await;
            assert!(x.is_finished());
            assert!(y.is_finished());
            assert!(z.is_finished());

            assert!(x.await.is_ok());
            assert!(y.await.is_ok());
            assert!(z.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_duplicate_deadlines() {
        rt! {
            let x = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            let y = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            let z = crate::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });

            clock::advance(Duration::from_millis(100)).await;
            assert!(x.is_finished());
            assert!(y.is_finished());
            assert!(z.is_finished());

            assert!(x.await.is_ok());
            assert!(y.await.is_ok());
            assert!(z.await.is_ok());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_sleep_handle_cancel() {
        rt! {
            let handle = crate::spawn(async {
                sleep(Duration::ZERO).await;
                panic!("boom");
            });

            handle.cancel();
            clock::advance(Duration::from_millis(100)).await;

            assert!(handle.await.unwrap_err().is_canceled());

            #[cfg(not(miri))]
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }
}
