use crate::runtime::{EnterGuard, Handle};

/// `rio` Runtime.
///
/// Provides a single-threaded task scheduler for executing asynchronous
/// [`tasks`].
///
/// Enter a runtime context with [`Runtime::enter`], which uses a thread-local
/// to track the current runtime. Allows tasks spawned within it's scope to be
/// associated with the current context.
///
/// [`tasks`]: crate::runtime::task
#[derive(Debug)]
pub struct Runtime {
    handle: Handle,
}

impl Runtime {
    /// Creates a new `Runtime` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use rio::runtime::Runtime;
    ///
    /// let rt = Runtime::new();
    ///
    /// // Use the runtime...
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Runtime {
            handle: Handle::new(),
        }
    }

    /// Enters the runtime context, allowing tasks spawned within this scope
    /// to be associated with this runtime.
    ///
    /// # Panics
    ///
    /// Panics if the current thread is within a runtime context.
    ///
    /// # Examples
    ///
    /// ```
    /// use rio::runtime::Runtime;
    ///
    /// let rt = Runtime::new();
    ///
    /// // Bind subsequent asynchronous tasks to this runtime context.
    /// let _guard = rt.enter();
    ///
    /// // Will panic!
    /// // let _guard2 = rt.enter();
    ///
    /// // `_guard` is dropped, exits the runtime context...
    /// ```
    #[inline]
    pub fn enter(&self) -> EnterGuard {
        self.handle.enter()
    }

    /// Runs the provided future to completion on the `rio` runtime, serving as
    /// the runtime’s entry point.
    ///
    /// `fut` is run on the current thread, blocking until it is complete,
    /// yielding its resolved result. This function enters a runtime context,
    /// so internally spawned tasks run within the same context.
    ///
    /// # Panics
    ///
    /// Panics if the provided future panics, or if the current thread is
    /// within a runtime context.
    ///
    /// # Examples
    ///
    /// ```
    /// use rio::runtime::Runtime;
    ///
    /// let rt = Runtime::new();
    ///
    /// // Execute the future, blocking the current thread until completion.
    /// let val = rt.block_on(async {
    ///     println!("hello, world");
    ///     4
    /// });
    ///
    /// println!("yielded result: {val}");
    /// ```
    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        self.handle.block_on(fut)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime::new()
    }
}
