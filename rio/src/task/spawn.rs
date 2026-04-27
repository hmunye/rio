use std::marker::PhantomData;

use crate::rt::context;
use crate::task::JoinHandle;

/// Spawns a new asynchronous task, returning its [`JoinHandle`].
///
/// The task begins execution immediately, enabling it to run concurrently with
/// other ready tasks.
///
/// There is no guarantee that a spawned task will execute to completion. When a
/// runtime is [`shutdown`], outstanding tasks _may_ be dropped, regardless of
/// the lifecycle of that task.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```no_run
/// # #[rio::main]
/// # async fn main() {
/// use rio::time::{self, Duration};
///
/// let a = rio::spawn(async {
///     time::sleep(Duration::from_millis(100)).await;
///     1 + 1
/// });
///
/// let b = rio::spawn(async {
///     time::sleep(Duration::from_millis(200)).await;
///     1 + 1
/// });
///
/// assert_eq!(a.await.unwrap() + b.await.unwrap(), 4);
/// # }
/// ```
///
/// [`shutdown`]: crate::rt::shutdown
#[inline]
pub fn spawn<F: Future + 'static>(fut: F) -> JoinHandle<F::Output> {
    JoinHandle {
        state: context::with_handle(|handle| handle.spawn_task(fut)),
        _marker: PhantomData,
    }
}
