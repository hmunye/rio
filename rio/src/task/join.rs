use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, ready};

use crate::rt::task::{TaskStage, TaskState};
use crate::task::{self, coop};

#[non_exhaustive]
enum InnerJoinErr {
    /// Task was canceled before retaining the expected output.
    Canceled,
    /// Task panicked before retaining the expected output.
    Panic(Box<dyn Any + Send>),
}

/// Error indicating a task failed to execute to completion.
pub struct JoinError {
    id: task::Id,
    err: InnerJoinErr,
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

    /// Consumes `self`, returning the _panic_ payload.
    ///
    /// # Panics
    ///
    /// Panics if the `JoinError` does not represent a _panic_.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// # #[rio::main]
    /// # async fn main() {
    /// use std::panic;
    ///
    /// let err = rio::spawn(async {
    ///     panic!("boom");
    /// })
    /// .await
    /// .unwrap_err();
    ///
    /// if err.is_panic() {
    ///     // Resume the panic on the current thread.
    ///     panic::resume_unwind(err.into_panic());
    /// }
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn into_panic(self) -> Box<dyn Any + Send> {
        self.try_into_panic()
            .expect("`JoinError` reason is not a panic")
    }

    /// Consumes `self`, returning the _panic_ payload if the task panicked,
    /// otherwise returns `self`.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// # #[rio::main]
    /// # async fn main() {
    /// use std::panic;
    ///
    /// let err = rio::spawn(async {
    ///     panic!("boom");
    /// })
    /// .await
    /// .unwrap_err();
    ///
    /// if let Ok(reason) = err.try_into_panic() {
    ///     // Resume the panic on the current thread.
    ///     panic::resume_unwind(reason);
    /// }
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn try_into_panic(self) -> Result<Box<dyn Any + Send>, JoinError> {
        match self.err {
            InnerJoinErr::Panic(reason) => Ok(reason),
            InnerJoinErr::Canceled => Err(self),
        }
    }

    /// Returns `true` if the error was caused by the task panicking.
    #[inline]
    #[must_use]
    pub const fn is_panic(&self) -> bool {
        matches!(&self.err, InnerJoinErr::Panic(_))
    }

    /// Returns `true` if the error was caused by the task being canceled.
    #[inline]
    #[must_use]
    pub const fn is_canceled(&self) -> bool {
        matches!(&self.err, InnerJoinErr::Canceled)
    }

    /// # Panics
    ///
    /// Panics if the `JoinError` does not represent a _panic_.
    fn get_panic_message(&self) -> &str {
        if let InnerJoinErr::Panic(reason) = &self.err {
            reason.downcast_ref::<&'static str>().map_or_else(
                || {
                    reason
                        .downcast_ref::<String>()
                        .map_or("<non-string payload>", |msg| msg.as_str())
                },
                |msg| *msg,
            )
        } else {
            panic!("`JoinError` reason is not a panic");
        }
    }
}

impl fmt::Display for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.err {
            InnerJoinErr::Canceled => write!(f, "[task #{}]: canceled", self.id),
            InnerJoinErr::Panic(_) => write!(
                f,
                "[task #{}]: panicked with: {}",
                self.id,
                self.get_panic_message()
            ),
        }
    }
}

impl fmt::Debug for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.err {
            InnerJoinErr::Canceled => write!(f, "JoinError::Canceled {{ Id: Id({}) }}", self.id),
            InnerJoinErr::Panic(_) => write!(
                f,
                "JoinError::Panic {{ Id: Id({}), Reason: {:?} }}",
                self.id,
                self.get_panic_message()
            ),
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

    /// Returns `true` if the task associated with this `JoinHandle` has
    /// finished.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main()]
    /// # async fn main() {
    /// use std::time::Duration;
    ///
    /// let handle1 = rio::spawn(async {
    ///     // ...
    /// });
    ///
    /// let handle2 = rio::spawn(async {
    ///     // ...
    ///     rio::time::sleep(Duration::from_secs(1000)).await;
    /// });
    ///
    /// handle2.cancel();
    ///
    /// rio::time::sleep(Duration::from_millis(100)).await;
    ///
    /// assert!(handle1.is_finished());
    /// assert!(handle2.is_finished());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        debug_assert!(
            !self.state.is_detached(),
            "`JoinHandle` must not exist for detached task #{}",
            self.id()
        );

        !self.state.is_incomplete()
    }
}

impl<T: 'static> JoinHandle<T> {
    /// # Panics
    ///
    /// Panics if the task's output cannot be downcast to `T` or if there is no
    /// output in the current task stage.
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
                err: InnerJoinErr::Canceled,
            }),
            TaskStage::Panic(reason) => Err(JoinError {
                id: self.id(),
                err: InnerJoinErr::Panic(reason),
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

        let coop = ready!(coop::poll_proceed());

        if self.state.is_incomplete() {
            self.state.set_waker(cx.waker().clone());
            Poll::Pending
        } else {
            coop.made_progress();
            Poll::Ready(self.take_output())
        }
    }
}
