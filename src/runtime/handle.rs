use crate::runtime::{context, scheduler, time};

/// Handle for interacting with the runtime.
#[derive(Debug, Clone)]
pub struct Handle {
    pub(crate) scheduler: scheduler::Handle,
    pub(crate) time: time::Handle,
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
    fn new(handle: &Handle) -> Self {
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
            scheduler: scheduler::Handle::new(),
            time: time::Handle::new(),
        }
    }

    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        let _guard = self.enter();
        self.scheduler.block_on(fut)
    }

    #[inline]
    fn enter(&self) -> EnterGuard {
        EnterGuard::new(self)
    }
}
