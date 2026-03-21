use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use crate::runtime::task::{self, TaskState};

/// Errors that can occur when awaiting a task's completion.
#[derive(Debug)]
#[non_exhaustive]
pub enum JoinError {
    /// Task is complete, but its output type could not be converted to the
    /// expected type.
    Downcast {
        task_id: task::Id,
        expected: &'static str,
        actual: &'static str,
    },
    /// Task was canceled before producing the expected output.
    Canceled { task_id: task::Id },
}

impl JoinError {
    /// Returns `true` if the error was caused by a task cancellation.
    #[inline]
    #[must_use]
    pub const fn is_canceled(&self) -> bool {
        matches!(&self, JoinError::Canceled { .. })
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            JoinError::Downcast {
                task_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "[Task {task_id}]: task output type mismatch - expected `{expected}`, got `{actual}`"
                )
            }
            JoinError::Canceled { task_id } => {
                write!(
                    f,
                    "[Task {task_id}]: task was canceled before producing the expected output"
                )
            }
        }
    }
}

impl std::error::Error for JoinError {}

/// An owned handle for joining on a task (`await` its termination).
///
/// Equivalent to [`std::thread::JoinHandle`] but for a `rio` task. The
/// background task associated with this `JoinHandle` starts running
/// immediately, even if you have not yet awaited the `JoinHandle`.
///
/// If a `JoinHandle` is dropped, the task continues running in the background
/// and its return value is lost.
///
/// # Examples
///
/// ```
/// let rt = rio::runtime::Runtime::new();
///
/// let val = rt.block_on(async {
///     let handle = rio::spawn(async { 1 + 1 });
///     handle.await
/// });
///
/// println!("yielded: {val:?}");
/// ```
///
/// [`std::thread::JoinHandle`]: std::thread::JoinHandle
#[derive(Debug)]
pub struct JoinHandle<T: 'static> {
    task_id: task::Id,
    state: Rc<TaskState>,
    _marker: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    /// Returns the [`Id`] of task associated with the handle.
    ///
    /// [`Id`]: task::Id
    #[inline]
    #[must_use]
    pub const fn id(&self) -> task::Id {
        self.task_id
    }

    /// Cancels the task associated with the handle.
    ///
    /// Awaiting a canceled task might complete if the task was already finished
    /// at the time of cancellation, but most likely it will fail with a
    /// [`JoinError::Canceled`].
    ///
    /// # Examples
    ///
    /// ```
    /// let rt = rio::runtime::Runtime::new();
    ///
    /// rt.block_on(async {
    ///     let mut handles = Vec::new();
    ///
    ///     handles.push(rio::spawn(async {
    ///         rio::time::sleep(std::time::Duration::from_secs(10)).await;
    ///         true
    ///     }));
    ///
    ///     handles.push(rio::spawn(async {
    ///         rio::time::sleep(std::time::Duration::from_secs(10)).await;
    ///         false
    ///     }));
    ///
    ///     for handle in &handles {
    ///         handle.cancel();
    ///     }
    ///
    ///     for handle in handles {
    ///         assert!(handle.await.unwrap_err().is_canceled());
    ///     }
    /// });
    /// ```
    #[inline]
    pub fn cancel(&self) {
        if matches!(
            self.state.stage.replace(task::Stage::Canceled),
            task::Stage::Running
        ) && let Some(waker) = self.state.waker.take()
        {
            waker.wake();
        }
    }

    #[inline]
    pub(crate) const fn new(task_id: task::Id, state: Rc<TaskState>) -> Self {
        JoinHandle {
            task_id,
            state,
            _marker: PhantomData,
        }
    }

    pub(crate) fn take_output(&self) -> Result<T, JoinError> {
        match self.state.stage.replace(task::Stage::Consumed) {
            task::Stage::Finished(out) => match out.downcast::<T>() {
                Ok(val) => Ok(*val),
                Err(err) => Err(JoinError::Downcast {
                    task_id: self.task_id,
                    expected: std::any::type_name::<T>(),
                    actual: std::any::type_name_of_val(&err),
                }),
            },
            task::Stage::Canceled => Err(JoinError::Canceled {
                task_id: self.task_id,
            }),
            _ => panic!("JoinHandle polled after completion"),
        }
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        self.state.stage.replace(task::Stage::Consumed);
    }
}

impl<T: std::fmt::Debug> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if matches!(*self.state.stage.borrow(), task::Stage::Running) {
            if self.state.waker.borrow().is_none() {
                self.state.waker.replace(Some(cx.waker().clone()));
            }

            return Poll::Pending;
        }

        Poll::Ready(self.take_output())
    }
}
