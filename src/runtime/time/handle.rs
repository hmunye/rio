use std::rc::Rc;
use std::task::Waker;
use std::time::Instant;

use crate::runtime::time;

/// Handle for interacting with the time driver.
#[derive(Debug, Clone)]
pub struct Handle {
    inner: Rc<time::Driver>,
}

impl Handle {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Handle {
            inner: Rc::new(time::Driver::new()),
        }
    }

    #[inline]
    pub fn register_timer(&self, deadline: Instant, waker: Waker) {
        self.inner.register_timer(deadline, waker);
    }

    #[inline]
    pub fn process_timers(&self) {
        self.inner.process_timers();
    }
}
