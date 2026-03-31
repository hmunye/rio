use crate::rt::context;

/// Spawns a new asynchronous task, allowing it to execute concurrently with
/// other tasks.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
///
/// ```
/// # #[rio::main]
/// # async fn main() {
/// rio::spawn(async { 1 + 1 });
/// # }
/// ```
// TODO: Update example when JoinHandle is implemented.
#[inline]
pub fn spawn<F: Future + 'static>(fut: F) {
    context::with_handle(|handle| handle.spawn_task(fut));
}
