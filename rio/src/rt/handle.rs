use std::rc::Rc;

use crate::rt::task::{TaskStage, TaskState};
use crate::rt::{Scheduler, Task, context};
use crate::task;

cfg_time! {
    use std::task::Waker;
    use std::time::{Duration, Instant};

    use crate::rt::time::{self, TimerHandle};

    cfg_test! {
        use crate::rt::time::Clock;
    }
}

cfg_io! {
    use std::os::fd::RawFd;

    use crate::rt::io::{self, Interest, IoHandle};
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

/// Internal shared handle to the runtime.
#[derive(Debug, Clone)]
pub struct Handle {
    scheduler: Rc<Scheduler>,
    #[cfg(feature = "time")]
    time: Rc<time::Driver>,
    #[cfg(feature = "io")]
    io: Rc<io::Driver>,
}

impl Handle {
    #[must_use]
    pub fn new() -> Self {
        Handle {
            scheduler: Rc::new(Scheduler::new()),
            #[cfg(feature = "time")]
            time: Rc::new(time::Driver::new()),
            #[cfg(feature = "io")]
            io: Rc::new(io::Driver::new()),
        }
    }

    pub fn block_on<F: Future + 'static>(&self, fut: F) -> F::Output {
        let _guard = self.enter();
        self.scheduler
            .spawn_blocking(fut, Rc::downgrade(&self.scheduler))
    }

    pub fn spawn_task<F: Future + 'static>(&self, fut: F) -> Rc<TaskState> {
        let task = Task::new_with_unwind(fut, |res, weak| {
            if let Some(state) = weak.upgrade() {
                match res {
                    Ok(out) => {
                        if state.is_detached() {
                            // No handle exists; mark as consumed and drop
                            // output.
                            state.set_stage(TaskStage::Consumed);
                        } else {
                            // Handle exists; retain the output.
                            state.set_stage(TaskStage::Finished(Box::new(out)));
                        }
                    }
                    Err(panic) => {
                        state.set_stage(TaskStage::Panic(panic));
                    }
                }

                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
        });

        // NOTE: Returns an `Rc` (not `Weak`) so the task’s output remains
        // accessible even when the task is dropped.
        let state = Rc::clone(&task.state);

        self.scheduler.spawn(task, Rc::downgrade(&self.scheduler));

        state
    }

    pub fn defer_task(&self, id: task::Id) {
        self.scheduler
            .defer_task(id, context::with_snapshot(context::Snapshot::used_since));
    }

    pub fn signal_shutdown(&self) {
        self.scheduler.shutdown_background();
    }

    fn enter(&self) -> EnterGuard {
        context::set_handle(self);
        EnterGuard {}
    }
}

cfg_time! {
    impl Handle {
        pub fn drive_timers(&self) -> Option<Duration> {
            self.time.drive()
        }

        pub fn register_timer(&self, deadline: Instant, waker: Waker) -> TimerHandle {
            self.time.register_timer(deadline, waker)
        }

        pub fn update_timer(&self, handle: &TimerHandle, deadline: Instant) -> bool {
            self.time.update_timer(handle, deadline)
        }

        pub fn cancel_timer(&self, handle: &TimerHandle) {
            self.time.cancel_timer(handle);
        }

    }

    cfg_test! {
        impl Handle {
            pub fn clock(&self) -> &Clock {
                self.time.clock()
            }

            pub fn timers(&self) -> usize {
                self.time.timers()
            }
        }
    }
}

cfg_io! {
    impl Handle {
        pub fn drive_io(&self, timeout: i32) {
            self.io.drive(timeout);
        }

        pub fn register_io(&self, fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
            self.io.register(fd, interest, waker)
        }

        pub fn modify_io(&self, handle: &IoHandle) {
            self.io.modify(handle);
        }

        pub fn deregister_io(&self, handle: &IoHandle) {
            self.io.deregister(handle);
        }
    }
}
