//! I/O driver backed by `epoll(7)`.
//!
//! Handles the registering and waiting on I/O events, waking tasks when
//! file descriptors become ready.

mod driver;
pub(crate) use driver::Driver;

/// Creates an [Error::Io] with a message prefixed to the `errno` value.
macro_rules! errno {
    ($($arg:tt)+) => {{
        let errno = ::std::io::Error::last_os_error();
        let prefix = format!($($arg)+);
        ::std::io::Error::new(errno.kind(), format!("{prefix}: {errno}"))
    }};
}

pub(crate) use errno;
