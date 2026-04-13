use std::collections::HashMap;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;
use std::task::Waker;

use crate::rt::io::{IoHandle, PollToken};

/// Bitmask of I/O event readiness flags to be notified on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interest(std::ffi::c_int);

impl Interest {
    /// The associated file is available for `read(2)` operations.
    pub const READ: Interest = Interest(libc::EPOLLIN);
    /// The associated file is available for `write(2)` operations.
    pub const WRITE: Interest = Interest(libc::EPOLLOUT);
    /// Requests edge-triggered notification for the associated file descriptor.
    pub const EDGE_TRIGGERED: Interest = Interest(libc::EPOLLET);

    pub const fn is_readable(self) -> bool {
        (self.0 & libc::EPOLLIN) != 0
    }

    pub const fn is_writable(self) -> bool {
        (self.0 & libc::EPOLLOUT) != 0
    }

    pub const fn is_edge_triggered(self) -> bool {
        (self.0 & libc::EPOLLET) != 0
    }
}

impl From<Interest> for u32 {
    fn from(interest: Interest) -> Self {
        interest.0 as u32
    }
}

/// `epoll(7)` is used to efficiently monitor multiple file descriptors for I/O.
/// Instead of blocking on each socket sequentially, this approach
/// (with non-blocking sockets) allows blocking on all simultaneously,
/// processing only the file descriptors that are ready for I/O.
#[derive(Debug)]
pub struct Epoll {
    fd: OwnedFd,
    events: [libc::epoll_event; Self::EPOLL_MAX_EVENTS as usize],
    registered: HashMap<PollToken, Waker>,
}

impl Epoll {
    const EPOLL_MAX_EVENTS: i32 = 1024;

    #[must_use]
    pub fn new() -> Self {
        Epoll {
            fd: Self::init_epoll_fd(),
            events: [libc::epoll_event { events: 0, u64: 0 }; Self::EPOLL_MAX_EVENTS as usize],
            registered: HashMap::default(),
        }
    }

    /// Waits for I/O readiness on all registered file descriptors, blocking the
    /// current thread until one or more events occur, a signal interrupts the
    /// call, or `timeout` elapses.
    ///
    /// `timeout` specifies the maximum duration (in milliseconds) to block. A
    /// value of `-1` will cause the function to block indefinitely, while a
    /// value of `0` will return immediately.
    ///
    /// # Panics
    ///
    /// Panics if `epoll_wait(2)` fails.
    pub fn wait(&mut self, timeout: i32) {
        if self.registered.is_empty() {
            return;
        }

        let ready = unsafe {
            #[allow(clippy::ref_as_ptr)]
            libc::epoll_wait(
                self.fd.as_raw_fd(),
                &mut self.events as *mut libc::epoll_event,
                Self::EPOLL_MAX_EVENTS,
                timeout,
            )
        };

        assert!(ready != -1, "{}", errno!("epoll_wait failed"));

        for event in self.events.iter().take(ready as usize) {
            if let Some(waker) = self.registered.get(&PollToken::from(event.u64)) {
                waker.wake_by_ref();
            }
        }
    }

    /// Add an entry to the interest list of the `epoll(7)` instance with an
    /// associated `Waker`, returning its `IoHandle`.
    ///
    /// # Panics
    ///
    /// Panics if `epoll_ctl(2)` fails.
    pub fn register_fd(&mut self, fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
        let handle = IoHandle::new(fd, interest);
        let mut ev = libc::epoll_event {
            events: interest.into(),
            u64: handle.token.into(),
        };

        assert!(
            unsafe { libc::epoll_ctl(self.fd.as_raw_fd(), libc::EPOLL_CTL_ADD, fd, &raw mut ev) }
                != -1,
            "{}",
            errno!("epoll_ctl (ADD) failed")
        );

        self.registered.insert(handle.token, waker);

        handle
    }

    /// Updated the events to monitor for the entry within the interest list of
    /// the `epoll(7)` instance identified by `handle`.
    ///
    /// # Panics
    ///
    /// Panics if `epoll_ctl(2)` fails.
    pub fn modify_fd(&self, handle: &IoHandle) {
        let mut ev = libc::epoll_event {
            events: handle.interest.into(),
            u64: handle.token.into(),
        };

        assert!(
            unsafe {
                libc::epoll_ctl(
                    self.fd.as_raw_fd(),
                    libc::EPOLL_CTL_MOD,
                    handle.fd,
                    &raw mut ev,
                )
            } != -1,
            "{}",
            errno!("epoll_ctl (MOD) failed")
        );
    }

    /// Removes an entry from the interest list of the `epoll(7)` instance
    /// identified by `handle`.
    ///
    /// # Panics
    ///
    /// Panics if `epoll_ctl(2)` fails.
    pub fn deregister_fd(&mut self, handle: &IoHandle) {
        assert!(
            unsafe {
                libc::epoll_ctl(
                    self.fd.as_raw_fd(),
                    libc::EPOLL_CTL_DEL,
                    handle.fd,
                    ptr::null_mut(),
                )
            } != -1,
            "{}",
            errno!("epoll_ctl (DEL) failed")
        );

        self.registered.remove(&handle.token);
    }

    /// Returns an `epoll(7)` instance.
    ///
    /// # Panics
    ///
    /// Panics if an `epoll_create1(2)` fails.
    fn init_epoll_fd() -> OwnedFd {
        let epoll_fd = unsafe { libc::epoll_create1(0) };

        assert!(epoll_fd != -1, "{}", errno!("epoll_create1 failed"));

        // SAFETY: `epoll_fd` is a valid file descriptor and no other owner
        // exist for it at this point.
        unsafe { OwnedFd::from_raw_fd(epoll_fd) }
    }
}
