use std::task::Poll;

use crate::runtime::{context, task};

/// Yields execution back to the `rio` runtime.
///
/// A task yields by awaiting on `yield_now()`, and may resume when that future
/// completes. No other waking is required for the task to continue.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn foo() {
///     println!("task ID: {}", rio::task::id());
///     // Yield control to the runtime to allow other tasks to run.
///     rio::task::yield_now().await;
/// }
///
/// fn main() {
///     let rt = rio::runtime::Runtime::new();
///
///     rt.block_on(async {
///         rio::spawn(foo());
///         rio::spawn(foo());
///     });
/// }
/// ```
pub async fn yield_now() {
    // Used to track whether the current task has been rescheduled to avoid
    // repeatedly returning `Poll::Pending`, which could lead to a deadlock.
    let mut yielded = false;

    std::future::poll_fn(|_| {
        if yielded {
            return Poll::Ready(());
        }

        yielded = true;

        // Push current task ID to the _back_ of the pending queue.
        context::with_current(|handle| handle.schedule_task(task::id()));

        Poll::Pending
    })
    .await;
}
