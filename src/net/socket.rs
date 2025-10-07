use std::cell::Cell;
use std::future::Future;
use std::net::SocketAddr;
use std::os::fd::FromRawFd;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, mem, ptr};

use crate::rt::Runtime;
use crate::rt::io::errno;

/// Raw, non-blocking socket used for initiating outbound TCP connections.
pub(crate) struct TcpSocket {
    fd: RawFd,
    sock_addr_s: libc::sockaddr_storage,
    sock_len: libc::socklen_t,
    /// Indicates whether a connection was established using this socket.
    connected: Cell<bool>,
}

impl TcpSocket {
    /// Creates a new non-blocking `TcpSocket` with a specified remote address.
    pub(crate) fn new(addr: SocketAddr) -> io::Result<Self> {
        let mut sock_addr_s: libc::sockaddr_storage = unsafe { mem::zeroed() };

        let sock_len = match addr {
            SocketAddr::V4(v4) => {
                let ipv4 = libc::sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: v4.port().to_be(), // network-byte order
                    sin_addr: libc::in_addr {
                        // Already in network-byte order.
                        s_addr: u32::from_ne_bytes(v4.ip().octets()),
                    },
                    sin_zero: [0; 8],
                };

                unsafe {
                    ptr::write(&raw mut sock_addr_s as *mut libc::sockaddr_in, ipv4);
                }

                mem::size_of_val(&ipv4) as libc::socklen_t
            }
            SocketAddr::V6(v6) => {
                let ipv6 = libc::sockaddr_in6 {
                    sin6_family: libc::AF_INET6 as u16,
                    sin6_port: v6.port().to_be(), // network-byte order
                    sin6_flowinfo: v6.flowinfo(),
                    sin6_addr: libc::in6_addr {
                        s6_addr: v6.ip().octets(),
                    },
                    sin6_scope_id: v6.scope_id(),
                };

                unsafe {
                    ptr::write(&raw mut sock_addr_s as *mut libc::sockaddr_in6, ipv6);
                }

                mem::size_of_val(&ipv6) as libc::socklen_t
            }
        };

        let fd = unsafe {
            let raw_fd = libc::socket(
                sock_addr_s.ss_family as libc::c_int,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK,
                0,
            );
            if raw_fd == -1 {
                return Err(errno!("failed to created non-blocking TcpSocket"));
            }

            raw_fd
        };

        Ok(TcpSocket {
            fd: RawFd::from(fd),
            sock_addr_s,
            sock_len,
            connected: Cell::new(false),
        })
    }

    /// Returns a `Future` that resolves to a `TcpStream` once the connection is
    /// successfully established.
    #[inline]
    pub(crate) fn connect(&self) -> ConnectFut<'_> {
        ConnectFut(self)
    }
}

impl Drop for TcpSocket {
    fn drop(&mut self) {
        if !self.connected.get() {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

/// A `Future` that resolves to a TCP connection with a remote host.
pub(crate) struct ConnectFut<'a>(&'a TcpSocket);

impl Future for ConnectFut<'_> {
    type Output = io::Result<std::net::TcpStream>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            if libc::connect(
                self.0.fd,
                &raw const self.0.sock_addr_s as *const libc::sockaddr,
                self.0.sock_len,
            ) == -1
            {
                match io::Error::last_os_error().raw_os_error() {
                    Some(libc::EAGAIN) | Some(libc::EALREADY) | Some(libc::EINPROGRESS) => {
                        // Register for write readiness notifications, as the
                        // socket becomes writable once the connection is
                        // established.
                        let events = libc::EPOLLOUT;
                        Runtime::current().scheduler.register_fd(
                            self.0.fd,
                            events as u32,
                            ctx.waker().clone(),
                        );

                        return Poll::Pending;
                    }
                    // The socket is already connected, so fallthrough.
                    Some(libc::EISCONN) => {}
                    _ => return Poll::Ready(Err(errno!("failed to connect to remote host"))),
                };
            }

            // Mark the socket as connected. If the socket is dropped before a
            // connection is established, the file descriptor will be closed.
            self.0.connected.set(true);

            let stream = std::net::TcpStream::from_raw_fd(self.0.fd);

            println!(
                "connect (in TcpSocket): connected to remote {}",
                stream.peer_addr().unwrap()
            );

            Poll::Ready(Ok(stream))
        }
    }
}
