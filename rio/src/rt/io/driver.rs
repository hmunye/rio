use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::task::Waker;

use crate::rt::io::reactor::IoReactor;
use crate::rt::io::{Interest, IoHandle, PollToken};

/// Driver for managing non-blocking I/O within the runtime.
#[derive(Debug)]
pub struct Driver {
    inner: RefCell<IoReactor>,
    registered: RefCell<HashMap<PollToken, Waker>>,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Driver {
            inner: RefCell::new(IoReactor::new()),
            registered: RefCell::default(),
        }
    }

    /// Registers an I/O resource with the driver, monitoring for the events
    /// specified by `interest`, returning an `IoHandle`.
    pub fn register_io(&self, fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
        let handle = self.inner.borrow().register(fd, interest);
        self.registered.borrow_mut().insert(handle.token, waker);
        handle
    }

    /// Updates the `interest` set for the I/O resource identified by `handle`.
    pub fn update_interest_io(&self, handle: &IoHandle) {
        self.inner.borrow().update_interest(handle);
    }

    /// Deregisters an I/O resource from the driver identified by `handle`.
    pub fn deregister_io(&self, handle: &IoHandle) {
        self.inner.borrow().deregister(handle);
        self.registered.borrow_mut().remove(&handle.token);
    }

    /// Drives the I/O resources registered with the driver.
    ///
    /// Polls the underlying system for registered I/O readiness, notifying
    /// associated `Waker`s.
    pub fn drive(&self, timeout: i32) {
        if self.registered.borrow().is_empty() {
            return;
        }

        self.drive_io(timeout);
    }

    /// Performs a readiness check on registered I/O resources.
    ///
    /// For each I/O resource that is ready, its associated `Waker` is notified.
    fn drive_io(&self, timeout: i32) {
        let mut inner = self.inner.borrow_mut();
        let tokens = inner.wait(timeout);

        for token in tokens {
            if let Some(waker) = self.registered.borrow().get(&token) {
                waker.wake_by_ref();
            }
        }
    }
}

#[cfg(all(test, not(miri)))]
impl Driver {
    /// Returns the number of I/O resources registered with the driver.
    pub fn io_resources(&self) -> usize {
        self.registered.borrow().len()
    }
}
