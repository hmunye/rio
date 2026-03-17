use crate::runtime::{context, scheduler};

/// Handle for interacting with the runtime.
#[derive(Debug)]
pub struct Handle {
    inner: scheduler::Handle,
}

/// Runtime context guard.
///
/// Returned by [`Runtime::enter`] and [`Handle::enter`], the context guard
/// exits the runtime context on `Drop`.
///
/// [`Runtime::enter`]: fn@crate::runtime::Runtime::enter
#[derive(Debug)]
#[must_use]
pub struct EnterGuard;

impl Drop for EnterGuard {
    fn drop(&mut self) {
        context::unset_current();
    }
}

impl Handle {
    /// Creates a new runtime `Handle`.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Handle {
            inner: scheduler::Handle::new(),
        }
    }

    /// Enters the runtime context, enabling executor-related operations.
    #[inline]
    pub fn enter(&self) -> EnterGuard {
        context::set_current(&self.inner);
        EnterGuard {}
    }
}
