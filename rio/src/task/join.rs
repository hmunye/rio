use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, ready};
use std::{any::Any, fmt};

use crate::rt::task::{TaskStage, TaskState};
use crate::task::{self, coop};

/// Error indicating a task failed to execute to completion.
pub struct JoinError {
    id: task::Id,
    err: Repr,
}

#[non_exhaustive]
enum Repr {
    /// Task was canceled before completion.
    Canceled,
    /// Task panicked before completion.
    Panic(Box<dyn Any + Send>),
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

    /// Consumes `self`, returning the _panic_ payload.
    ///
    /// # Errors
    ///
    /// Returns `self` if the associated task did not panic.
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
    ///     panic::resume_unwind(reason);
    /// }
    /// # }
    /// ```
    #[inline]
    pub fn try_into_panic(self) -> Result<Box<dyn Any + Send>, JoinError> {
        match self.err {
            Repr::Panic(reason) => Ok(reason),
            Repr::Canceled => Err(self),
        }
    }

    /// Returns `true` if the error was caused by the associated task panicking.
    #[inline]
    #[must_use]
    pub const fn is_panic(&self) -> bool {
        matches!(&self.err, Repr::Panic(_))
    }

    /// Returns `true` if the error was caused by the associated task being
    /// canceled.
    #[inline]
    #[must_use]
    pub const fn is_canceled(&self) -> bool {
        matches!(&self.err, Repr::Canceled)
    }

    /// # Panics
    ///
    /// Panics if the `JoinError` does not represent a _panic_.
    fn get_panic_message(&self) -> &str {
        if let Repr::Panic(reason) = &self.err {
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
            Repr::Canceled => write!(f, "[task #{}] canceled", self.id),
            Repr::Panic(_) => write!(
                f,
                "[task #{}] panicked: {}",
                self.id,
                self.get_panic_message()
            ),
        }
    }
}

impl fmt::Debug for JoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.err {
            Repr::Canceled => write!(f, "JoinError::Canceled {{ Id: Id({}) }}", self.id),
            Repr::Panic(_) => write!(
                f,
                "JoinError::Panic {{ Id: Id({}), Reason: {:?} }}",
                self.id,
                self.get_panic_message()
            ),
        }
    }
}

impl std::error::Error for JoinError {}

/// An owned handle used to join on a [`spawned task`].
///
/// Analogous to [`std::thread::JoinHandle`], the task begins executing
/// immediately, even before awaiting the handle.
///
/// If dropped, the task continues running, but the return value is discarded.
///
/// # Examples
///
/// ```no_run
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
    // NOTE: Stores an `Rc` (not `Weak`) so the task’s output remains accessible
    // even when the task is dropped after completion.
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
    /// If the task has not yet completed, awaiting this `JoinHandle` will
    /// resolve with a [`canceled`] error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[rio::main]
    /// # async fn main() {
    /// use rio::time::{self, Duration};
    ///
    /// let mut handles = Vec::new();
    ///
    /// handles.push(rio::spawn(async {
    ///     time::sleep(Duration::from_secs(100000)).await;
    ///     true
    /// }));
    ///
    /// handles.push(rio::spawn(async {
    ///     time::sleep(Duration::from_secs(100000)).await;
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

        // Ensures we don't overwrite a terminal stage.
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
    /// use rio::time::{self, Duration};
    ///
    /// let handle1 = rio::spawn(async {
    ///     // ...
    /// });
    ///
    /// let handle2 = rio::spawn(async {
    ///     // ...
    ///     time::sleep(Duration::from_secs(1000)).await;
    /// });
    ///
    /// handle2.cancel();
    /// time::sleep(Duration::from_millis(100)).await;
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
    /// output retained by the current stage.
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
                err: Repr::Canceled,
            }),
            TaskStage::Panic(reason) => Err(JoinError {
                id: self.id(),
                err: Repr::Panic(reason),
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
