use std::ops;

cfg_epoll! {
    /// Bitmask of I/O event readiness flags to be notified on.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Interest(libc::c_int);

    impl Interest {
        /// The associated file is available for `read(2)` operations.
        pub const READ: Interest = Interest(libc::EPOLLIN);
        /// The associated file is available for `write(2)` operations.
        pub const WRITE: Interest = Interest(libc::EPOLLOUT);
        /// Requests edge-triggered notification for the associated file
        /// descriptor.
        pub const EDGE_TRIGGERED: Interest = Interest(libc::EPOLLET);
        /// Requests one-shot notification for the associated file descriptor.
        pub const ONESHOT: Interest = Interest(libc::EPOLLONESHOT);

        pub const fn is_readable(self) -> bool {
            (self.0 & libc::EPOLLIN) != 0
        }

        pub const fn is_writable(self) -> bool {
            (self.0 & libc::EPOLLOUT) != 0
        }

        #[allow(unused)]
        pub const fn is_edge_triggered(self) -> bool {
            (self.0 & libc::EPOLLET) != 0
        }

        #[allow(unused)]
        pub const fn is_oneshot(self) -> bool {
            (self.0 & libc::EPOLLONESHOT) != 0
        }
    }

    impl From<Interest> for u32 {
        fn from(interest: Interest) -> Self {
            interest.0 as u32
        }
    }

    impl ops::BitOr for Interest {
        type Output = Interest;

        fn bitor(self, rhs: Self) -> Self::Output {
            Interest(self.0 | rhs.0)
        }
    }

    impl ops::BitOrAssign for Interest {
        fn bitor_assign(&mut self, rhs: Self) {
            self.0 |= rhs.0;
        }
    }
}

cfg_kqueue! {
    /// Bitmask of I/O event readiness flags to be notified on.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Interest {
        pub flags: libc::c_ushort,
        pub filter: libc::c_short,
    }

    impl Interest {
        /// The associated event is available for `read(2)` operations.
        pub const READ: Interest = Interest { flags: 0, filter: libc::EVFILT_READ };
        /// The associated event is available for `write(2)` operations.
        pub const WRITE: Interest = Interest { flags: 0, filter: libc::EVFILT_WRITE };
        /// Permit `kevent()`, `kevent64()`, and `kevent_qos()` to return the
        /// event if it is triggered.
        #[allow(unused)]
        pub const ENABLE: Interest = Interest { flags: libc::EV_ENABLE, filter: 0 };
        /// Disable the event so `kevent()`, `kevent64()`, and `kevent_qos()`
        /// will not return it. The filter itself is not disabled.
        #[allow(unused)]
        pub const DISABLE: Interest = Interest { flags: libc::EV_DISABLE, filter: 0 };
        /// After the event is retrieved, its state is reset. Useful for filters
        /// which report state transitions instead of the current state.
        pub const EDGE_TRIGGERED: Interest = Interest { flags: libc::EV_CLEAR, filter: 0 };

        pub const fn is_readable(self) -> bool {
            self.filter == libc::EVFILT_READ
        }

        pub const fn is_writable(self) -> bool {
            self.filter == libc::EVFILT_WRITE
        }

        #[allow(unused)]
        pub const fn is_enabled(self) -> bool {
            (self.flags & libc::EV_ENABLE) != 0
        }

        #[allow(unused)]
        pub const fn is_disabled(self) -> bool {
            (self.flags & libc::EV_DISABLE) != 0
        }

        #[allow(unused)]
        pub const fn is_edge_triggered(self) -> bool {
            (self.flags & libc::EV_CLEAR) != 0
        }

        fn merge_filter(a: libc::c_short, b: libc::c_short) -> libc::c_short {
            debug_assert!(
                a == 0 || b == 0,
                "`kqueue(2)` filters are mutually exclusive, not combinable"
            );

            if b != 0 { b } else { a }
        }
    }

    impl ops::BitOr for Interest {
        type Output = Interest;

        fn bitor(self, rhs: Self) -> Self::Output {
            Interest {
                flags: self.flags | rhs.flags,
                filter: Interest::merge_filter(self.filter, rhs.filter)
            }
        }
    }

    impl ops::BitOrAssign for Interest {
        fn bitor_assign(&mut self, rhs: Self) {
            self.flags |= rhs.flags;
            self.filter = Interest::merge_filter(self.filter, rhs.filter);
        }
    }
}
