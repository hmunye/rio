use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::rt::task::{TaskStage, TaskState};
use crate::task;

#[non_exhaustive]
enum InnerJoinError {
    /// Task was canceled before retaining the expected output.
    Canceled,
}

/// Error indicating a task failed to execute to completion.
pub struct JoinError {
    id: task::Id,
    err: InnerJoinError,
}

impl JoinError {
    /// Returns the [`Id`] of the associated task.
    ///
    /// [`Id`]: task::Id
    #[inline]
    #[must_use]
    pub const fn id(&self) -> task::Id {
        self.id
    }

    /// Returns `true` if the error was caused by the task being canceled.
    #[inline]
    #[must_use]
    pub const fn is_canceled(&self) -> bool {
        matches!(&self.err, InnerJoinError::Canceled)
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.err {
            InnerJoinError::Canceled => write!(f, "[task #{}]: canceled", self.id),
        }
    }
}

impl fmt::Debug for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.err {
            InnerJoinError::Canceled => write!(f, "JoinError::Canceled {{ Id: Id({}) }}", self.id),
        }
    }
}

impl std::error::Error for JoinError {}

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
/// assert_eq!(handle.await.unwrap(), 2);
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
        debug_assert!(
            !self.state.is_detached(),
            "`JoinHandle` must not exist for detached task #{}",
            self.id()
        );

        self.state.id
    }

    /// Cancels the associated task.
    ///
    /// If the task has not yet completed, awaiting the handle will fail with a
    /// [`canceled`] error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[rio::main]
    /// # async fn main() {
    /// use std::time::Duration;
    ///
    /// let mut handles = Vec::new();
    ///
    /// handles.push(rio::spawn(async {
    ///     rio::time::sleep(Duration::from_secs(100000)).await;
    ///     true
    /// }));
    ///
    /// handles.push(rio::spawn(async {
    ///     rio::time::sleep(Duration::from_secs(100000)).await;
    ///     false
    /// }));
    ///
    /// for handle in &handles {
    ///     handle.cancel();
    /// }
    ///
    /// for handle in handles {
    ///     assert!(handle.await.unwrap_err().is_canceled());
    /// }
    /// # }
    /// ```
    ///
    /// [`canceled`]: JoinError::is_canceled
    #[inline]
    pub fn cancel(&self) {
        debug_assert!(
            !self.state.is_detached(),
            "`JoinHandle` must not exist for detached task #{}",
            self.id()
        );

        if self.state.is_incomplete() {
            self.state.set_stage(TaskStage::Canceled);
        }
    }
}

impl<T: 'static> JoinHandle<T> {
    /// # Panics
    ///
    /// Panics if the task's output cannot be downcast to `T` or if there is no
    /// output retained by the current task stage.
    pub(crate) fn take_output(&self) -> Result<T, JoinError> {
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

                Ok(*out)
            }
            TaskStage::Canceled => Err(JoinError {
                id: self.id(),
                err: InnerJoinError::Canceled,
            }),
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
    type Output = Result<T, JoinError>;

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
