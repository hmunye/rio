use std::cell::Cell;
use std::os::fd::RawFd;
use std::task::Waker;

use crate::io::Interest;
use crate::rt::context;

thread_local! {
    /// Monotonic counter for constructing [`PollToken`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for I/O resource readiness.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct PollToken(u64);

impl PollToken {
    #[must_use]
    fn next() -> Self {
        PollToken(IDS.replace(IDS.get() + 1))
    }
}

impl From<u64> for PollToken {
    fn from(val: u64) -> Self {
        PollToken(val)
    }
}

impl From<PollToken> for u64 {
    fn from(token: PollToken) -> Self {
        token.0
    }
}

/// Handle to an I/O resource returned by [`register_io_source`].
///
/// Deregisters the associated I/O resource on `Drop`. The caller is responsible
/// for closing the file descriptor of the I/O resource **after** dropping the
/// handle.
#[derive(Debug)]
pub struct IoHandle {
    pub(crate) fd: RawFd,
    pub(crate) interest: Interest,
    pub(crate) token: PollToken,
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    /// Bitmask of active `kqueue(2)` filters for this handle. Each filter is
    /// uniquely identified by its (_ident_, _filter_) pair.
    ///
    /// Prevents redundant `kevent(2)` registrations for filters that are
    /// already active.
    events_set: u8,
}

impl IoHandle {
    #[must_use]
    pub(crate) fn new(fd: RawFd, interest: Interest) -> Self {
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        let events_set = {
            let mut events = 0;
            if interest.is_readable() {
                events |= Self::READ_MASK;
            }
            if interest.is_writable() {
                events |= Self::WRITE_MASK;
            }
            events
        };

        IoHandle {
            fd,
            interest,
            token: PollToken::next(),
            #[cfg(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "visionos",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
            events_set,
        }
    }

    /// Combines the provided `Interest` with the current set, updating its I/O
    /// driver entry.
    #[inline]
    pub fn add_interest(&mut self, interest: Interest) {
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            if interest.is_readable() {
                self.set_read_event();
            }

            if interest.is_writable() {
                self.set_write_event();
            }

            // Store most recent filter applied.
            if interest.filter != 0 {
                self.interest.filter = interest.filter;
            }
            self.interest.flags |= interest.flags;
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.interest |= interest;
        }

        context::with_handle(|handle| handle.update_interest_io(self));
    }

    /// Returns `true` if this `IoHandle` is readable.
    ///
    /// On epoll-based platforms (Linux), this reflects the current interest
    /// set. On kqueue-based platforms (macOS, FreeBSD, etc.), this reflects
    /// if a read filter is currently active.
    #[inline]
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        return self.interest.is_readable();

        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        return self.is_read_registered();
    }

    /// Returns `true` if this `IoHandle` is writable.
    ///
    /// On epoll-based platforms (Linux), this reflects the current interest
    /// set. On kqueue-based platforms (macOS, FreeBSD, etc.), this reflects
    /// if a write filter is currently active.
    #[inline]
    #[must_use]
    #[allow(unused)]
    pub const fn is_writable(&self) -> bool {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        return self.interest.is_writable();

        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "watchos",
            target_os = "visionos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        return self.is_write_registered();
    }

    fn deregister(&self) {
        context::with_handle(|handle| handle.deregister_io(self));
    }
}

cfg_kqueue! {
    impl IoHandle {
        const READ_MASK: u8 = 0x1;
        const WRITE_MASK: u8 = 0x2;

        const fn set_read_event(&mut self) {
            self.events_set |= Self::READ_MASK;
        }

        const fn set_write_event(&mut self) {
            self.events_set |= Self::WRITE_MASK;
        }

        const fn is_read_registered(&self) -> bool {
            (self.events_set & Self::READ_MASK) != 0
        }

        #[allow(unused)]
        const fn is_write_registered(&self) -> bool {
            (self.events_set & Self::WRITE_MASK) != 0
        }
    }
}

impl Drop for IoHandle {
    fn drop(&mut self) {
        self.deregister();
    }
}

/// Registers a raw file descriptor with the runtime for asynchronous I/O,
/// returning an [`IoHandle`] that tracks the registration state.
///
/// This is a low-level primitive for integrating external or pre-existing file
/// descriptors into the reactor. The descriptor is monitored for the events
/// specified by [`Interest`] (readable, writable, etc.). The FD must be open
/// and non-blocking when registered. Updates to monitored events should be
/// done via [`IoHandle::add_interest`], and the caller retains full ownership
/// of the FD, which must only be closed **after** dropping the `IoHandle`.
///
/// # Drop Semantics
///
/// The returned `IoHandle` automatically deregisters the FD from the runtime
/// when dropped. To avoid closing the FD prematurely, declare the handle before
/// the inner resource in your type:
///
/// ```rust,ignore
/// pub struct IoStream {
///     // Ensures it is dropped first
///     handle: Option<IoHandle>,
///     fd: OwnedFd
/// }
/// ```
///
/// # Panics
///
/// Panics if the current thread is not within a runtime context.
#[inline]
#[must_use]
pub fn register_io_source(fd: RawFd, interest: Interest, waker: Waker) -> IoHandle {
    context::with_handle(|handle| handle.register_io(fd, interest, waker))
}
