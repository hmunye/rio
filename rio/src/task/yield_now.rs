use std::future;
use std::task::Poll;

use crate::rt::context;
use crate::task;

/// Yields control back to the runtime, allowing other ready tasks to make
/// progress.
///
/// Awaiting this future suspends the current task and may resume it upon
/// completion.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn foo() {
///     println!("task #{}", rio::task::id());
///     // Yield control to the runtime, allowing other ready tasks to make
///     // progress.
///     rio::task::yield_now().await;
/// }
///
/// #[rio::main]
/// async fn main() {
///     rio::spawn(foo());
///     rio::spawn(foo());
/// }
/// ```
#[inline]
pub async fn yield_now() {
    // Ensures we only `yield` once, to avoid deadlocks.
    let mut yielded = false;

    future::poll_fn(|_| {
        if yielded {
            return Poll::Ready(());
        }

        yielded = true;

        // Current task will be deferred until the next "tick".
        context::with_handle(|handle| handle.defer_task(task::id()));

        Poll::Pending
    })
    .await;
}
