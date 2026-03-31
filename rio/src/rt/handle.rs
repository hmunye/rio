use std::rc::Rc;
use std::task::Waker;
use std::time::Instant;

use crate::rt::{Scheduler, Task, context, time};
use crate::task;

/// Internal shared handle to the runtime.
#[derive(Debug, Clone)]
pub struct Handle {
    scheduler: Rc<Scheduler>,
    time: Rc<time::Driver>,
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
        context::drop_handle();
    }
}

impl Handle {
    #[must_use]
    pub fn new() -> Self {
        Handle {
            scheduler: Rc::new(Scheduler::new()),
            time: Rc::new(time::Driver::new()),
        }
    }

    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        let _guard = self.enter();
        self.scheduler.spawn_blocking(self.clone(), fut)
    }

    pub fn spawn_task<F: Future + 'static>(&self, fut: F) {
        self.scheduler
            .spawn(Task::new_with(fut, |_| {}), self.clone());
    }

    pub fn schedule_task(&self, id: task::Id) {
        self.scheduler.schedule_task(id);
    }

    pub fn defer_task(&self, id: task::Id) {
        self.scheduler
            .defer_task(id, context::with_snapshot(context::Snapshot::used_since));
    }

    pub fn drive_timers(&self) {
        self.time.drive();
    }

    pub fn register_timer(&self, deadline: Instant, waker: Waker) {
        self.time.register_timer(deadline, waker);
    }

    fn enter(&self) -> EnterGuard {
        context::set_handle(self);
        EnterGuard {}
    }
}
