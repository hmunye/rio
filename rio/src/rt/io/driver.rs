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
    pub fn register(&self, fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
        let handle = self.inner.borrow().register(fd, interest);
        self.registered.borrow_mut().insert(handle.token, waker);
        handle
    }

    /// Updates the `interest` set for the I/O resource identified by `handle`.
    pub fn update_interest(&self, handle: &IoHandle) {
        self.inner.borrow().update_interest(handle);
    }

    /// Deregisters an I/O resource from the driver identified by `handle`.
    pub fn deregister(&self, handle: &IoHandle) {
        self.inner.borrow().deregister(handle);
        self.registered.borrow_mut().remove(&handle.token);
    }

    /// Drives the I/O resources registered with the driver.
    ///
    /// Polls the underlying system for I/O resource readiness, notifying
    /// associated `Waker`s whose resources are ready.
    pub fn drive(&self, timeout: i32) {
        if self.registered.borrow().is_empty() {
            return;
        }

        self.drive_io(timeout);
    }

    /// Checks registered I/O resources for readiness.
    ///
    /// For each ready I/O resource, its associated `Waker` is notified.
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
