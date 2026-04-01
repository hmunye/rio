use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::rt::task::{TaskStage, TaskState};
use crate::task;

/// An owned handle used to join on a [`spawned task`] asynchronously.
///
/// Analogous to [`std::thread::JoinHandle`], the task begins executing
/// immediately, even before awaiting the handle.
///
/// If dropped, the task continues running, but the return value is discarded.
///
/// # Examples
///
/// ```
/// # #[rio::main]
/// # async fn main() {
/// let handle = rio::spawn(async { 1 + 1 });
/// assert_eq!(handle.await, 2);
/// # }
/// ```
///
/// [`spawned task`]: crate::spawn
/// [`std::thread::JoinHandle`]: std::thread::JoinHandle
#[derive(Debug)]
pub struct JoinHandle<T> {
    pub(crate) state: Rc<TaskState>,
    pub(crate) _marker: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    /// Returns the [`Id`] of the associated task.
    ///
    /// [`Id`]: task::Id
    #[inline]
    #[must_use]
    pub fn id(&self) -> task::Id {
        self.state.id
    }
}

impl<T: 'static> JoinHandle<T> {
    /// # Panics
    ///
    /// Panics if the task's output cannot be downcast to `T` or if there is no
    /// output retained by the current task stage.
    pub(crate) fn take_output(&self) -> T {
        debug_assert!(
            !self.state.is_detached(),
            "`JoinHandle` must not exist for detached task #{}",
            self.id()
        );

        match self.state.set_stage(TaskStage::Consumed) {
            TaskStage::Finished(out) => {
                let Ok(out) = out.downcast::<T>() else {
                    panic!(
                        "[task #{}]: output type mismatch at runtime; expected `{}`, received a different type",
                        self.id(),
                        std::any::type_name::<T>(),
                    );
                };

                *out
            }
            stage => panic!(
                "[task #{}]: cannot take output; no output retained in stage: `{stage:?}`",
                self.id()
            ),
        }
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        self.state.detached.set(true);
    }
}

impl<T: 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        debug_assert!(
            !self.state.is_detached(),
            "`JoinHandle` must not exist for detached task #{}",
            self.id()
        );

        if self.state.is_incomplete() {
            self.state.set_waker(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(self.take_output())
        }
    }
}
