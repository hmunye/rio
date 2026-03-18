use std::cell::RefCell;
use std::rc::Rc;

use crate::runtime::Scheduler;
use crate::runtime::task;

/// Handle for interacting with the scheduler.
#[derive(Debug, Clone)]
pub struct Handle {
    inner: Rc<RefCell<Scheduler>>,
}

impl Handle {
    #[inline]
    pub fn new() -> Self {
        Handle {
            inner: Rc::new(RefCell::new(Scheduler::new())),
        }
    }

    #[inline]
    pub fn schedule_task(&self, id: task::Id) {
        self.inner.borrow_mut().schedule_task(id);
    }

    #[inline]
    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        self.inner.borrow_mut().block_on_fut(self.clone(), fut)
    }
}
