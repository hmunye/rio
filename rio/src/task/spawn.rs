use crate::rt::context;

/// Spawns a new asynchronous task.
///
/// The task is scheduled to run on the current runtime, allowing it to execute
/// concurrently with other tasks.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn foo() {
///     println!("hello from task #{}", rio::task::id()); // hello from task #1
/// }
///
/// #[rio::main]
/// async fn main() {
///     rio::spawn(foo());
///     println!("hello from task #{}", rio::task::id()); // hello from task #0
/// }
/// ```
#[inline]
pub fn spawn<F: Future + 'static>(fut: F) {
    context::with_handle(|handle| handle.spawn_task(fut));
}
