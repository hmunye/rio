use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::{ptr, thread};

use crate::rt::task::Task;

/// The `rio` runtime.
#[derive(Debug, Default)]
pub struct Runtime {}

impl Runtime {
    /// Creates a new `Runtime` instance.
    #[inline]
    pub const fn new() -> Self {
        Runtime {}
    }

    /// Runs a future to completion, serving as the runtime’s entry point.
    ///
    /// This runs the given future on the current thread, blocking until it is
    /// complete, and yielding its resolved result.
    pub fn block_on<F: Future + 'static>(&self, future: F) -> F::Output {
        let waker = Runtime::noop_waker();
        let mut ctx = Context::from_waker(&waker);

        let output = Rc::new(RefCell::new(None));
        let out_clone = Rc::clone(&output);

        let mut task = Task::new(async move {
            // Ensure we can read out a possible output.
            *out_clone.borrow_mut() = Some(future.await);
        });

        loop {
            match task.poll(&mut ctx) {
                Poll::Ready(_) => break,
                Poll::Pending => thread::yield_now(),
            }
        }

        output.borrow_mut().take().unwrap()
    }

    #[inline]
    const fn noop_waker() -> Waker {
        // SAFETY: Waker only consists of no-op function, making it trivially
        // thread-safe. Data pointer is never accessed.
        unsafe { Waker::from_raw(Runtime::noop_raw_waker()) }
    }

    #[inline]
    const fn noop_raw_waker() -> RawWaker {
        let vtable = &RawWakerVTable::new(
            |_: *const ()| -> RawWaker { Runtime::noop_raw_waker() },
            |_: *const ()| {},
            |_: *const ()| {},
            |_: *const ()| {},
        );

        RawWaker::new(ptr::null(), vtable)
    }
}
