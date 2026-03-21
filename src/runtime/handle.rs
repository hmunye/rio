use crate::runtime::{context, scheduler};

/// Handle for interacting with the runtime.
#[derive(Debug)]
pub struct Handle {
    inner: scheduler::Handle,
}

/// Runtime context guard.
///
/// Returned by [`Handle::enter`], the context guard exits the runtime context
/// on `Drop`.
#[derive(Debug)]
#[must_use]
pub struct EnterGuard;

impl EnterGuard {
    #[inline]
    fn new(handle: &scheduler::Handle) -> Self {
        context::set_current(handle);
        EnterGuard {}
    }
}

impl Drop for EnterGuard {
    fn drop(&mut self) {
        context::drop_current();
    }
}

impl Handle {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Handle {
            // Initialize a single-threaded scheduler.
            inner: scheduler::Handle::new(),
        }
    }

    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        let _guard = self.enter();
        self.inner.block_on(fut)
    }

    #[inline]
    fn enter(&self) -> EnterGuard {
        EnterGuard::new(&self.inner)
    }
}
