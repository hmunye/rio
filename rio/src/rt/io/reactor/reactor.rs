use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;

use crate::rt::io::{Interest, IoHandle, PollToken};

// ============================================================================
// epoll(7) - Linux
// ============================================================================
cfg_linux! {
    /// Manages I/O resources and event monitoring with the underlying system.
    #[derive(Debug)]
    pub struct IoReactor {
        fd: OwnedFd,
        events: [libc::epoll_event; Self::EPOLL_MAX_EVENTS as usize],
    }

    impl IoReactor {
        const EPOLL_MAX_EVENTS: libc::c_int = 1024;

        /// Returns a new `IoReactor`, backed by `epoll(7)`.
        ///
        /// # Panics
        ///
        /// Panics if an `epoll(7)` instance could not be initialized.
        #[must_use]
        pub fn new() -> Self {
            IoReactor {
                fd: Self::init_epoll(),
                events: [libc::epoll_event { events: 0, u64: 0 }; Self::EPOLL_MAX_EVENTS as usize],
            }
        }

        /// Registers an I/O resource with the underlying system, returning an
        /// `IoHandle`.
        ///
        /// # Panics
        ///
        /// Panics if `epoll_ctl(2)` fails.
        pub fn register(&self, fd: RawFd, interest: Interest) -> IoHandle {
            let handle = IoHandle::new(fd, interest);
            let mut ev = libc::epoll_event {
                events: interest.into(),
                u64: handle.token.into(),
            };

            assert!(
                unsafe { libc::epoll_ctl(self.fd.as_raw_fd(), libc::EPOLL_CTL_ADD, fd, &raw mut ev) }
                    != -1,
                "{}",
                errno!("epoll_ctl(2) ADD failed")
            );

            handle
        }

        /// Updates the registration of an existing I/O resource.
        ///
        /// # Panics
        ///
        /// Panics if `epoll_ctl(2)` fails.
        pub fn update_interest(&self, handle: &IoHandle) {
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
                errno!("epoll_ctl(2) MOD failed")
            );
        }

        /// Deregisters an I/O resource from the underlying system.
        ///
        /// # Panics
        ///
        /// Panics if `epoll_ctl(2)` fails.
        pub fn deregister(&self, handle: &IoHandle) {
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
                errno!("epoll_ctl(2) DEL failed")
            );
        }

        /// Waits for events on registered I/O resources and returns an iterator
        /// of readiness tokens.
        ///
        /// `timeout` specifies the maximum duration (in milliseconds) to block.
        /// A value of `-1` will cause the function to block indefinitely, while
        /// a value of `0` will return immediately.
        ///
        /// # Panics
        ///
        /// Panics if `epoll_wait(2)` fails.
        pub fn wait(&mut self, timeout: i32) -> impl Iterator<Item = PollToken> {
            let ready = unsafe {
                #[allow(clippy::ref_as_ptr)]
                libc::epoll_wait(
                    self.fd.as_raw_fd(),
                    &mut self.events as *mut libc::epoll_event,
                    Self::EPOLL_MAX_EVENTS,
                    timeout,
                )
            };

            assert!(ready != -1, "{}", errno!("epoll_wait(2) failed"));

            self.events
                .iter()
                .take(ready as usize)
                .map(|ev| PollToken::from(ev.u64))
        }

        /// Returns an `epoll(7)` instance.
        ///
        /// # Panics
        ///
        /// Panics if `epoll_create1(2)` fails.
        fn init_epoll() -> OwnedFd {
            // `epoll(7)` is used to efficiently monitor multiple file
            // descriptors for I/O.
            //
            // Instead of blocking on each socket sequentially, this approach
            // (with non-blocking sockets) allows blocking on all
            // simultaneously, processing only the ready file descriptors.
            let epoll_fd = unsafe { libc::epoll_create1(0) };

            assert!(epoll_fd != -1, "{}", errno!("epoll_create1(2) failed"));

            // SAFETY: `epoll_fd` is a valid file descriptor and no other owner
            // exists for it at this point.
            unsafe { OwnedFd::from_raw_fd(epoll_fd) }
        }
    }
}

// ============================================================================
// kqueue(2) - macOS/BSD
// ============================================================================
cfg_bsd! {
    use std::mem;

    /// Manages I/O resources and event monitoring with the underlying system.
    #[derive(Debug)]
    pub struct IoReactor {
        fd: OwnedFd,
        events: Box<[libc::kevent]>,
    }

    impl IoReactor {
        const KQ_MAX_EVENTS: libc::c_int = 1024;

        /// Returns a new `IoReactor`, backed by `kqueue(2)`.
        ///
        /// # Panics
        ///
        /// Panics if a `kqueue(2)` instance could not be initialized.
        #[must_use]
        pub fn new() -> Self {
            IoReactor {
                fd: Self::init_kqueue(),
                events: vec![
                    libc::kevent {
                        ident: 0,
                        filter: 0,
                        flags: 0,
                        fflags: 0,
                        data: 0,
                        udata: ptr::null_mut(),
                    };
                    Self::KQ_MAX_EVENTS as usize
                ]
                .into_boxed_slice(),
            }
        }

        /// Registers an I/O resource with the underlying system, returning an
        /// `IoHandle`.
        ///
        /// # Panics
        ///
        /// Panics if `kevent(2)` fails.
        pub fn register(&self, fd: RawFd, interest: Interest) -> IoHandle {
            let handle = IoHandle::new(fd, interest);
            self.add_kevent(&handle);

            handle
        }

        /// Updates the registration of an existing I/O resource.
        ///
        /// # Panics
        ///
        /// Panics if `kevent(2)` fails.
        pub fn update_interest(&self, handle: &IoHandle) {
            // An (_ident_, _filter_, optional _udata_ value) tuple uniquely
            // identifies an event with the `kqueue(2)` instance.
            //
            // Re-adding an existing event will modify the parameters of the
            // original event, and not result in a duplicate entry.
            self.add_kevent(handle);
        }

        /// Deregisters an I/O resource from the underlying system.
        pub const fn deregister(&self, _handle: &IoHandle) {
            // When the I/O object holding `handle` is dropped, `deregister()`
            // is called, followed by the `close()` of the file descriptor,
            // making this redundant, since events which are attached to file
            // descriptors are automatically deleted on the last close of the
            // descriptor.
        }

        /// Waits for events on registered I/O resources and returns an iterator
        /// of readiness tokens.
        ///
        /// `timeout` specifies the maximum duration (in milliseconds) to block.
        /// A value of `-1` will cause the function to block indefinitely, while
        /// a value of `0` will return immediately.
        ///
        /// # Panics
        ///
        /// Panics if `kevent(2)` fails.
        pub fn wait(&mut self, timeout: i32) -> impl Iterator<Item = PollToken> {
            // SAFETY: All-zero value of the type `libc::timespec` is valid.
            let mut timespec = unsafe { mem::zeroed::<libc::timespec>() };
            let timespec_ptr = match timeout {
                -1 => {
                    // Wait indefinitely.
                    ptr::null()
                }
                0 => {
                    // To indicate an immediate timeout, the timeout argument
                    // should be non-NULL, pointing to a zero-valued `timespec`
                    // struct.
                    &raw const timespec
                }
                t => {
                    timespec.tv_sec =  (t / 1000).into();
                    timespec.tv_nsec = ((t % 1000) * 1_000_000).into();

                    &raw const timespec
                }
            };

            let ready = unsafe {
                libc::kevent(
                    self.fd.as_raw_fd(),
                    ptr::null(),
                    0,
                    self.events.as_mut_ptr(),
                    Self::KQ_MAX_EVENTS,
                    timespec_ptr,
                )
            };

            assert!(ready != -1, "{}", errno!("kevent(2) wait failed"));

            self.events
                .iter()
                .take(ready as usize)
                .map(|ev| PollToken::from(ev.udata as usize as u64))
        }

        /// Adds an `kevent` to the `kqueue(2)` instance.
        ///
        /// # Panics
        ///
        /// Panics if `kevent(2)` fails.
        fn add_kevent(&self, handle: &IoHandle) {
            let change_list: [libc::kevent; 1] = [libc::kevent {
                ident: handle.fd as usize,
                filter: handle.interest.filter,
                flags: handle.interest.flags | libc::EV_ADD,
                fflags: 0,
                data: 0,
                udata: u64::from(handle.token) as usize as *mut libc::c_void,
            }];

            assert!(
                unsafe {
                    libc::kevent(
                        self.fd.as_raw_fd(),
                        (&raw const change_list).cast(),
                        1,
                        ptr::null_mut(),
                        0,
                        ptr::null(),
                    )
                } != -1,
                "{}",
                errno!("kevent(2) EV_ADD failed")
            );
        }

        /// Returns a `kqueue(2)` instance.
        ///
        /// # Panics
        ///
        /// Panics if `kqueue(2)` fails.
        fn init_kqueue() -> OwnedFd {
            // `kqueue(2)` is a BSD event notification facility that monitors
            // the kernel for state changes in registered objects.
            //
            // Unlike `epoll(7)` (which is mask-centric: you register an fd + a
            // bitmask of events), `kqueue(2)` is filter-centric: you register a
            // (identifier, filter) pair that watches for a specific state
            // transition.
            let kq_fd = unsafe { libc::kqueue() };

            assert!(kq_fd != -1, "{}", errno!("kqueue(2) failed"));

            // SAFETY: `kq_fd` is a valid file descriptor and no other owner
            // exists for it at this point.
            unsafe { OwnedFd::from_raw_fd(kq_fd) }
        }
    }
}
