use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, RawFd};
use std::task::Waker;
use std::{io, ptr};

use crate::rt::io::errno;

/// I/O driver backed by `epoll(7)`.
///
/// Handles the registering and waiting on I/O events, waking tasks when
/// file descriptors become ready.
#[derive(Debug)]
pub(crate) struct Driver {
    /// File descriptor of the `epoll(7)` instance.
    epoll_fd: RawFd,
    /// Stores events for ready file descriptors.
    events: [libc::epoll_event; Self::EPOLL_MAX_EVENTS as usize],
    /// Associates file descriptors with their corresponding [`Waker`].
    registered_fds: HashMap<RawFd, Waker>,
}

impl Driver {
    /// Total number of events returned each tick (event loop cycle).
    const EPOLL_MAX_EVENTS: i32 = 1024;

    /// Creates a new `Reactor` instance.
    ///
    /// # Panics
    ///
    /// This function panics if the `epoll(7)` instance could not be created.
    pub(crate) fn new() -> Self {
        Driver {
            epoll_fd: Self::init_epoll_fd(),
            events: [libc::epoll_event { events: 0, u64: 0 }; Self::EPOLL_MAX_EVENTS as usize],
            registered_fds: Default::default(),
        }
    }

    /// Waits for events on the `epoll(7)` instance, blocking until either a
    /// file descriptor delivers an event, the call is interrupted by a signal
    /// handler, or the timeout expires.
    ///
    /// `timeout` specifies the maximum duration (in milliseconds) to block. A
    /// timeout of `-1` will cause the function to block indefinitely, while a
    /// timeout of `0` will not wait on any file descriptors to be ready before
    /// returning.
    ///
    /// # Panics
    ///
    /// This function panics if it fails to wait on file descriptor readiness.
    pub(crate) fn poll(&mut self, timeout: i32) {
        if self.registered_fds.is_empty() {
            return;
        }

        unsafe {
            // Returns 0 if no file descriptors became ready during the
            // timeout duration, if `timeout` is a value other than `-1`.
            let rdfs = libc::epoll_wait(
                self.epoll_fd,
                &mut self.events as *mut libc::epoll_event,
                Self::EPOLL_MAX_EVENTS,
                timeout,
            );

            if rdfs == -1 {
                panic!("{}", errno!("failed to wait on epoll"));
            }

            for event in self.events.iter().take(rdfs as usize) {
                let fd = event.u64 as i32;

                if let Some(waker) = self.registered_fds.remove(&fd) {
                    waker.wake();
                }
            }
        }
    }

    /// Add an entry to the interest list of the `epoll(7)` file descriptor.
    /// Each event is associated to a given [`Waker`].
    ///
    /// `events` is a bit mask of event types (`epoll_ctl(2)`).
    ///
    /// # Panics
    ///
    /// This function panics if the entry could not be added to the interest
    /// list.
    pub(crate) fn register(&mut self, fd: RawFd, events: u32, waker: Waker) {
        let mut ev = libc::epoll_event {
            events,
            u64: fd as u64,
        };

        if unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_ADD, fd, &raw mut ev) } == -1 {
            // The supplied file descriptor is already registered with this
            // `epoll` instance.
            if io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                return;
            }

            panic!("{}", errno!("failed to add to epoll interest list"));
        }

        self.registered_fds.insert(fd, waker);
    }

    /// Remove (unregister) the target file descriptor from the `epoll(7)`
    /// interest list.
    ///
    /// # Panics
    ///
    /// This function panics if the file descriptor could not be unregistered.
    pub(crate) fn unregister(&mut self, fd: RawFd) {
        if unsafe { libc::epoll_ctl(self.epoll_fd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut()) } == -1
        {
            // The supplied file descriptor is not registered with this `epoll`
            // instance.
            if io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                return;
            }

            panic!("{}", errno!("failed to remove from epoll interest list"));
        }

        self.registered_fds.remove(&fd);
    }

    /// Creates a non-blocking `epoll(7)` instance.
    ///
    /// # Panics
    ///
    /// This function panics if an `epoll(7)` instance could not be created.
    fn init_epoll_fd() -> RawFd {
        unsafe {
            // `epoll(7)` used to efficiently monitor multiple file descriptors
            // for I/O. Instead of blocking on each socket sequentially, this
            // approach (with non-blocking sockets) allows blocking on all
            // simultaneously, processing only the file descriptors that are
            // ready for I/O.
            let epoll_fd = libc::epoll_create1(0);
            if epoll_fd == -1 {
                panic!("{}", errno!("failed to create epoll_fd"));
            }

            epoll_fd.as_raw_fd()
        }
    }
}
