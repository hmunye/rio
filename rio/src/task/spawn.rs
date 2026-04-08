use crate::rt::context;
use crate::task::JoinHandle;

/// Spawns a new asynchronous task, returning its [`JoinHandle`].
///
/// The task begins execution immediately, enabling it to run concurrently with
/// other ready tasks.
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
/// use std::time::Duration;
///
/// let a = rio::spawn(async {
///     rio::time::sleep(Duration::from_millis(100)).await;
///     1 + 1
/// });
///
/// let b = rio::spawn(async {
///     rio::time::sleep(Duration::from_millis(200)).await;
///     1 + 1
/// });
///
/// assert_eq!(a.await.unwrap() + b.await.unwrap(), 4);
/// # }
/// ```
#[inline]
pub fn spawn<F: Future + 'static>(fut: F) -> JoinHandle<F::Output> {
    JoinHandle {
        state: context::with_handle(|handle| handle.spawn_task(fut)),
        _marker: std::marker::PhantomData,
    }
}
