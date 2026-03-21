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
    /// Returns the [`Id`] of the joined task.
    ///
    /// [`Id`]: task::Id
    #[inline]
    #[must_use]
    pub const fn id(&self) -> task::Id {
        self.task_id
    }

    #[inline]
    pub(crate) const fn new(task_id: task::Id, state: Rc<TaskState>) -> Self {
        JoinHandle {
            task_id,
            state,
            _marker: PhantomData,
        }
    }

    /// Takes the joined task's output.
    ///
    /// # Panics
    ///
    /// Panics if the joined task is not finished or has already been consumed.
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
                // Store the `LocalWaker` to be notified when output becomes
                // available.
                unsafe {
                    self.state.waker.replace(Some(std::mem::transmute::<
                        std::task::Waker,
                        task::LocalWaker,
                    >(cx.waker().clone())));
                }
            }

            return Poll::Pending;
        }

        Poll::Ready(self.take_output())
    }
}
