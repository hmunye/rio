use std::future;
use std::task::Poll;

use crate::task::coop;

/// Consumes a unit of execution budget, yielding control to the runtime the
/// if the task's budget was _exhausted_.
///
/// # Panics
///
/// Panics if the caller `.await` or polls the returned future outside of a
/// runtime context.
///
/// # Examples
///
/// ```
/// async fn coop_sum_squares(n: u64) -> u64 {
///     let mut sum = 0;
///
///     for i in 1..=n {
///         rio::task::coop::consume_budget().await;
///         sum += i * i;
///     }
///
///     sum
/// }
/// ```
#[inline]
pub async fn consume_budget() {
    // Only return `Poll::Pending` until the current task can proceed to avoid
    // stalling the runtime.
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
