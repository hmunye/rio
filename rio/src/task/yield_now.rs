use std::future;
use std::task::Poll;

use crate::rt::context;
use crate::task;

/// Yields control back to the scheduler, allowing other ready tasks to make
/// progress. No other waking is required for the task to continue.
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
/// use rio::task;
///
/// async fn foo() {
///     println!("task #{}", task::id());
///
///     //...
///
///     task::yield_now().await;
/// }
///
/// rio::spawn(foo());
/// rio::spawn(foo());
/// # }
/// ```
#[inline]
pub async fn yield_now() {
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
