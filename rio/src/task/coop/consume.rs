use std::future;
use std::task::Poll;

use crate::task::coop;

/// Consumes a unit of execution budget, yielding control to the scheduler if
/// the task's budget was exhausted.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```
/// use rio::task::coop;
///
/// async fn coop_sum_squares(n: u64) -> u64 {
///     let mut sum = 0;
///
///     for i in 1..=n {
///         coop::consume_budget().await;
///         sum += i * i;
///     }
///
///     sum
/// }
/// ```
#[inline]
pub async fn consume_budget() {
    let mut status = Poll::Pending;

    future::poll_fn(|_| {
        if status.is_ready() {
            return status;
        }

        status = coop::poll_proceed().map(|guard| {
            guard.made_progress();
        });

        status
    })
    .await;
}
