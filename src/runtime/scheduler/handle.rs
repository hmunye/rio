use std::rc::Rc;

use crate::runtime::Scheduler;
use crate::runtime::task;

/// Handle for interacting with the scheduler.
#[derive(Debug, Clone)]
pub struct Handle {
    inner: Rc<Scheduler>,
}

impl Handle {
    #[inline]
    pub fn new() -> Self {
        Handle {
            inner: Rc::new(Scheduler::new()),
        }
    }

    #[inline]
    pub fn schedule_task(&self, id: task::Id) {
        self.inner.schedule_task(id);
    }

    #[inline]
    pub fn spawn_task(&self, task: task::Task) {
        self.inner.register_task(self.clone(), task);
    }

    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        self.inner.block_on_fut(self.clone(), fut)
    }
}
