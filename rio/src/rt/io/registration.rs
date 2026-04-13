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
    pub fd: RawFd,
    pub interest: Interest,
    pub token: PollToken,
}

impl IoHandle {
    #[must_use]
    pub fn new(fd: RawFd, interest: Interest) -> Self {
        IoHandle {
            fd,
            interest,
            token: PollToken::next(),
        }
    }

    pub fn modify(&mut self, interest: Interest) {
        self.interest = interest;
        context::with_handle(|handle| handle.modify_io(self));
    }

    fn deregister(&self) {
        context::with_handle(|handle| handle.deregister_io(self));
    }
}

impl Drop for IoHandle {
    fn drop(&mut self) {
        self.deregister();
    }
}
