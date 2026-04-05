use std::future;
use std::task::Poll;

use crate::rt::context;
use crate::task;

/// Yields control back to the runtime, allowing other ready tasks to make
/// progress.
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
/// async fn foo() {
///     println!("task #{}", rio::task::id());
///
///     // ...
///
///     rio::task::yield_now().await;
/// }
///
/// rio::spawn(foo());
/// # }
/// ```
#[inline]
pub async fn yield_now() {
    // Ensures we only yield up to the scheduler once, to avoid blocking other
    // tasks.
    let mut yielded = false;

    future::poll_fn(|_| {
        if yielded {
            return Poll::Ready(());
        }

        yielded = true;

        context::with_handle(|handle| handle.defer_task(task::id()));

        Poll::Pending
    })
    .await;
}
