use crate::rt::{Handle, context};

/// `rio` Runtime.
///
/// Provides a single-threaded task scheduler and time driver, necessary for
/// running asynchronous [`tasks`].
///
/// [`tasks`]: crate::task
#[derive(Debug)]
pub struct Runtime {
    handle: Handle,
}

impl Runtime {
    /// Creates a new `Runtime`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rio::rt::Runtime;
    ///
    /// let rt = Runtime::new();
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Runtime {
            handle: Handle::new(),
        }
    }

    /// Runs the provided future to completion, serving as the runtime entry
    /// point. This function blocks the current thread until `fut` has resolved,
    /// returning it's output.
    ///
    /// # Panics
    ///
    /// Panics if `fut` panics or if the current thread is already within a
    /// runtime context.
    ///
    /// # Examples
    ///
    /// ```
    /// use rio::rt::Runtime;
    ///
    /// let rt = Runtime::new();
    ///
    /// let res = rt.block_on(async {
    ///     "hello, world"
    /// });
    ///
    /// assert_eq!(res, "hello, world");
    /// ```
    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        self.handle.block_on(fut)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Signals to the runtime to begin shutting down, without waiting for any
/// spawned tasks to complete.
///
/// Only the future provided to `Runtime::block_on` will be guaranteed to
/// complete before the runtime is shutdown.
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
///
/// # Examples
///
/// ```
/// # #[rio::main]
/// # async fn main() {
/// use std::time::Duration;
///
/// rio::spawn(async {
///     loop {
///         rio::task::coop::make_cooperative(std::future::ready(())).await;
///     }
/// });
///
/// rio::spawn(async {
///     rio::time::sleep(Duration::from_millis(10)).await;
///     rio::rt::shutdown();
/// });
/// # }
/// ```
#[inline]
pub fn shutdown() {
    context::with_handle(Handle::signal_shutdown);
}
