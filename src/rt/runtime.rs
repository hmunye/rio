use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::{ptr, thread};

/// The `rio` runtime.
#[derive(Debug)]
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
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        let waker = Runtime::noop_waker();
        let mut ctx = Context::from_waker(&waker);

        let mut pinned = future;
        loop {
            // SAFETY: The pointee `pinned` is a stack variable that will not
            // be deallocated until the function returns, which is when the
            // `Future` resolves.
            match unsafe { Pin::new_unchecked(&mut pinned) }.poll(&mut ctx) {
                Poll::Ready(out) => return out,
                Poll::Pending => thread::yield_now(),
            }
        }
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
