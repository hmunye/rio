use crate::runtime::{EnterGuard, Handle};

/// The `rio` Runtime.
///
/// Provides a task scheduler and timers, necessary for running asynchronous
/// tasks.
#[derive(Debug)]
pub struct Runtime {
    handle: Handle,
}

impl Runtime {
    /// Creates a new `Runtime` instance.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Runtime {
            handle: Handle::new(),
        }
    }

    /// Enters the runtime context, enabling executor-related operations.
    #[inline]
    pub fn enter(&self) -> EnterGuard {
        self.handle.enter()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime::new()
    }
}
