use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::os::fd::{FromRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{io, mem};

use crate::rt::context;
use crate::rt::io::{Interest, IoHandle};
use crate::task::coop;

/// Non-blocking TCP socket that has not yet been converted to a [`TcpStream`].
///
/// [`TcpStream`]: crate::net::TcpStream
#[derive(Debug)]
pub struct TcpSocket {
    fd: RawFd,
    sock_addr_s: libc::sockaddr_storage,
    sock_len: libc::socklen_t,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectState {
    Connecting,
    Connected,
}

/// Future returned by [`connect`].
///
/// [`connect`]: TcpSocket::connect
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Connect {
    // NOTE: Defined first to ensure it is dropped before `sock` (deregister
    // before closing FD).
    handle: Option<IoHandle>,
    sock: TcpSocket,
    state: ConnectState,
}

impl TcpSocket {
    pub fn new(addr: SocketAddr) -> io::Result<Self> {
        let mut sock_addr_s = MaybeUninit::<libc::sockaddr_storage>::uninit();

        let sock_len = match addr {
            SocketAddr::V4(v4) => {
                let ipv4 = libc::sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: v4.port().to_be(), // network-byte order
                    sin_addr: libc::in_addr {
                        s_addr: u32::from_ne_bytes(v4.ip().octets()),
                    },
                    sin_zero: [0; 8],
                };

                unsafe {
                    // SAFETY: We are writing a valid `sockaddr_in` into a
                    // `sockaddr_storage` buffer with sufficient size and
                    // alignment.
                    sock_addr_s
                        .as_mut_ptr()
                        .cast::<libc::sockaddr_in>()
                        .write(ipv4);
                }

                mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
            }
            SocketAddr::V6(v6) => {
                let ipv6 = libc::sockaddr_in6 {
                    sin6_family: libc::AF_INET6 as u16,
                    sin6_port: v6.port().to_be(), // network-byte order
                    sin6_flowinfo: v6.flowinfo().to_be(), // network-byte order
                    sin6_addr: libc::in6_addr {
                        s6_addr: v6.ip().octets(),
                    },
                    sin6_scope_id: v6.scope_id(),
                };

                unsafe {
                    // SAFETY: We are writing a valid `sockaddr_in6` into a
                    // `sockaddr_storage` buffer with sufficient size and
                    // alignment.
                    sock_addr_s
                        .as_mut_ptr()
                        .cast::<libc::sockaddr_in6>()
                        .write(ipv6);
                }

                mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t
            }
        };

        // SAFETY: `sock_addr_s` is fully initialized in all previous match
        // arms.
        let sock_addr_s = unsafe { sock_addr_s.assume_init() };

        let domain = sock_addr_s.ss_family.into();
        let fd = unsafe {
            let fd = libc::socket(domain, libc::SOCK_STREAM | libc::SOCK_NONBLOCK, 0);

            if fd == -1 {
                return Err(errno!("socket(2) failed"));
            }

            fd
        };

        Ok(TcpSocket {
            fd,
            sock_addr_s,
            sock_len,
        })
    }

    #[inline]
    pub const fn connect(self) -> Connect {
        Connect {
            sock: self,
            state: ConnectState::Connecting,
            handle: None,
        }
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        if self.fd != -1 {
            let _ = unsafe { libc::close(self.fd) };
        }
    }
}

impl Future for Connect {
    type Output = io::Result<(std::net::TcpStream, Option<IoHandle>)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let coop = ready!(coop::poll_proceed());

        loop {
            match self.state {
                ConnectState::Connecting => {
                    if unsafe {
                        libc::connect(
                            self.sock.fd,
                            (&raw const self.sock.sock_addr_s).cast::<libc::sockaddr>(),
                            self.sock.sock_len,
                        )
                    } == -1
                    {
                        // FIXME: use `io::ErrorKind::InProgress` when stable.
                        //
                        // <https://github.com/rust-lang/rust/issues/130840>
                        match io::Error::last_os_error().raw_os_error() {
                            Some(libc::EAGAIN | libc::EALREADY | libc::EINPROGRESS) => {
                                if self.handle.is_none() {
                                    // Register for `WRITE` readiness, as the
                                    // socket becomes writable once a connection
                                    // is established. `ONESHOT` disables the
                                    // FD in the interest list once an event has
                                    // been notified on it.
                                    self.handle = Some(context::with_handle(|handle| {
                                        handle.register_io(
                                            self.sock.fd,
                                            Interest::ONESHOT | Interest::WRITE,
                                            cx.waker().clone(),
                                        )
                                    }));
                                }

                                return Poll::Pending;
                            }
                            _ => {
                                coop.made_progress();
                                return Poll::Ready(Err(errno!("connect(2) failed")));
                            }
                        }
                    } else {
                        self.state = ConnectState::Connected;
                    }
                }
                ConnectState::Connected => {
                    coop.made_progress();

                    let mut err: libc::c_int = 0;
                    let mut err_len = mem::size_of_val(&err) as libc::socklen_t;

                    // Ensure no errors occurred when connecting.
                    let ret = unsafe {
                        libc::getsockopt(
                            self.sock.fd,
                            libc::SOL_SOCKET,
                            libc::SO_ERROR,
                            (&raw mut err).cast(),
                            &raw mut err_len,
                        )
                    };

                    if ret == -1 || err != 0 {
                        return Poll::Ready(Err(errno!("getsockopt(2) SO_ERROR failed")));
                    }

                    // SAFETY: `sock.fd` is guaranteed to be open at this point,
                    // and `stream` will be responsible for closing it.
                    let stream = unsafe { std::net::TcpStream::from_raw_fd(self.sock.fd) };

                    // If `connect(2)` doesn't block, we return `None`, and
                    // `stream` lazily registers one.
                    let handle = self.handle.take();

                    // Invalidate the file descriptor for `sock` to avoid
                    // premature close.
                    self.sock.fd = -1;

                    return Poll::Ready(Ok((stream, handle)));
                }
            }
        }
    }
}
