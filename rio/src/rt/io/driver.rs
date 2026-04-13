use std::cell::RefCell;
use std::os::fd::RawFd;
use std::task::Waker;

use crate::rt::io::{Epoll, Interest, IoHandle};

/// Driver for managing non-blocking I/O within the runtime.
///
/// Registers I/O resources with the underlying system and monitors them for
/// readiness, waking tasks when progress can be made.
#[derive(Debug)]
pub struct Driver {
    reactor: RefCell<Epoll>,
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        Driver {
            reactor: RefCell::new(Epoll::new()),
        }
    }

    /// Registers an I/O resource with the driver, monitoring for the events
    /// specified by `interest`, returning its `IoHandle`.
    pub fn register(&self, fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
        self.reactor.borrow_mut().register_fd(fd, interest, waker)
    }

    /// Modifies the event mask for the I/O resource identified by `handle` to
    /// match the current state of `handle`.
    pub fn modify(&self, handle: &IoHandle) {
        self.reactor.borrow().modify_fd(handle);
    }

    /// Deregisters an I/O resource from the driver identified by `handle`.
    pub fn deregister(&self, handle: &IoHandle) {
        self.reactor.borrow_mut().deregister_fd(handle);
    }

    /// Drives the I/O resources registered with the driver.
    ///
    /// Polls the underlying system for I/O resource readiness, notifying
    /// associated `Waker`s whose resources are ready.
    pub fn drive(&self, timeout: i32) {
        self.drive_io(timeout);
    }

    /// Checks registered I/O resources for readiness.
    ///
    /// For each ready I/O resource, its associated `Waker` is notified.
    fn drive_io(&self, timeout: i32) {
        self.reactor.borrow_mut().wait(timeout);
    }
}
