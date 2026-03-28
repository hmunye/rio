use std::rc::Rc;

use crate::rt::{Scheduler, context};
use crate::task;

/// Internal shared handle to the runtime.
#[derive(Debug, Clone)]
pub struct Handle {
    scheduler: Rc<Scheduler>,
}

/// Runtime context guard.
///
/// Returned by [`Handle::enter`], the context guard exits the runtime context
/// on `Drop`.
#[derive(Debug)]
#[must_use]
struct EnterGuard;

impl Drop for EnterGuard {
    fn drop(&mut self) {
        context::drop_current();
    }
}

impl Handle {
    #[must_use]
    pub fn new() -> Self {
        Handle {
            scheduler: Rc::new(Scheduler::new()),
        }
    }

    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        let _guard = self.enter();
        self.scheduler.block_on_fut(self.clone(), fut)
    }

    pub fn schedule_task(&self, id: task::Id) {
        self.scheduler.schedule_task(id);
    }

    fn enter(&self) -> EnterGuard {
        context::set_current(self);
        EnterGuard {}
    }
}
