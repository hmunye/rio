use std::cell::Cell;
use std::os::fd::RawFd;

use crate::rt::context;
use crate::rt::io::Interest;

thread_local! {
    /// Monotonic counter for constructing [`PollToken`]s.
    static IDS: Cell<u64> = const { Cell::new(0) };
}

/// Opaque identifier for an I/O resource relative to all other resources.
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

/// Handle to an I/O resource returned by [`Driver::register`].
///
/// Deregisters the associated I/O resource on `Drop`. The caller is responsible
/// for closing the file descriptor of the I/O resource.
///
/// [`Driver::register`]: crate::rt::io::Driver::register
#[derive(Debug)]
pub struct IoHandle {
    pub(crate) fd: RawFd,
    pub(crate) interest: Interest,
    pub(crate) token: PollToken,
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    /// Bitmask tracking which filter types are currently registered for this
    /// handle. `kqueue(2)` supports multiple filters (e.g., `EVFILT_READ`,
    /// `EVFILT_WRITE`) per _ident_, each identified by an (_ident_, _filter_)
    /// pair.
    ///
    /// Avoids redundant `kevent(2)` registrations for filters that are already
    /// active.
    events_set: u8,
}

impl IoHandle {
    #[must_use]
    pub fn new(fd: RawFd, interest: Interest) -> Self {
        #[cfg(any(
            target_os = "macos",
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
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
            events_set,
        }
    }

    /// Combines the provided `Interest` with the current, updating its entry
    /// within the I/O driver.
    pub fn add_interest(&mut self, interest: Interest) {
        #[cfg(any(
            target_os = "macos",
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
        }

        self.interest |= interest;
        context::with_handle(|handle| handle.update_interest_io(self));
    }

    /// Returns `true` if this `IoHandle` is readable.
    ///
    /// On epoll-based platforms, this reflects the current interest value. On
    /// kqueue-based platforms (macOS, FreeBSD, etc.), this reflects whether a
    /// read filter is currently registered, since multiple filters can exist
    /// per handle.
    pub const fn is_readable(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.interest.is_readable();

        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        return self.is_read_registered();
    }

    /// Returns `true` if this `IoHandle` is writable.
    ///
    /// On epoll-based platforms, this reflects the current interest value. On
    /// kqueue-based platforms (macOS, FreeBSD, etc.), this reflects whether a
    /// write filter is currently registered, since multiple filters can exist
    /// per handle.
    pub const fn is_writable(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.interest.is_writable();

        #[cfg(any(
            target_os = "macos",
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

cfg_bsd! {
    impl IoHandle {
        const READ_MASK: u8 = 0x1;
        const WRITE_MASK: u8 = 0x2;

        pub const fn clear_events(&mut self) {
            self.events_set = 0;
        }

        const fn set_read_event(&mut self) {
            self.events_set |= Self::READ_MASK;
        }

        const fn set_write_event(&mut self) {
            self.events_set |= Self::WRITE_MASK;
        }

        const fn is_read_registered(&self) -> bool {
            (self.events_set & Self::READ_MASK) != 0
        }

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
