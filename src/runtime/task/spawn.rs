use std::rc::Rc;

use crate::runtime::context;
use crate::runtime::task::{JoinHandle, Task};
use crate::task;

/// Spawns a new asynchronous task, returning its [`JoinHandle`].
///
/// The provided future will start running in the background immediately
/// when `spawn` is called, enabling it to execute concurrently with other
/// tasks.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// async fn counter() {
///     for i in 0..10 {
///         println!("task {}: {i}", rio::task::id());
///         // Yield control to the runtime to allow other tasks to run.
///         rio::task::yield_now().await;
///     }
/// }
///
/// fn main() {
///     let rt = rio::runtime::Runtime::new();
///
///     rt.block_on(async {
///         rio::spawn(counter());
///         rio::spawn(counter());
///     });
/// }
/// ```
pub fn spawn<F: Future + 'static>(fut: F) -> JoinHandle<F::Output> {
    let task = Task::new_with(|weak| async move {
        let res = fut.await;

        if let Some(state) = weak.upgrade() {
            state.stage.replace(task::Stage::Finished(Box::new(res)));

            if let Some(waker) = &*state.waker.borrow() {
                waker.wake_by_ref();
            }
        }
    });

    let id = task.id;
    let state = Rc::clone(&task.state);

    context::with_current(|handle| handle.scheduler.spawn_task(task));

    JoinHandle::new(id, state)
}
