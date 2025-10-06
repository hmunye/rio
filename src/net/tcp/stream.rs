use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use crate::io::AsyncRead;
use crate::io::AsyncWrite;
use crate::net::socket::TcpSocket;
use crate::rt::Runtime;

/// A TCP stream between a local and a remote socket.
///
/// Reading and writing to a TcpStream is usually done using the methods found
/// on the `AsyncRead` and `AsyncWrite` traits.
#[derive(Debug)]
pub struct TcpStream(std::net::TcpStream);

impl TcpStream {
    /// Opens a TCP connection to a remote host.
    ///
    /// `addr` is an address of the remote host. Anything which implements the
    /// [`ToSocketAddrs`] trait can be supplied as the address. If `addr` yields
    /// multiple addresses, connect will be attempted with each of the addresses
    /// until a connection is successful. If none of the addresses result in a
    /// successful connection, the error returned from the last connection
    /// attempt (the last address) is returned.
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        // Reference: Based on Tokio's `TcpStream::connect`
        //
        // https://docs.rs/tokio/latest/src/tokio/net/tcp/stream.rs.html#115-133
        let addrs = addr.to_socket_addrs()?;

        let mut last_err = None;

        for addr in addrs {
            match TcpStream::connect_addr(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve any provided address",
            )
        }))
    }

    /// Returns the socket address of the local half of this TCP connection.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }

    /// Establishes a connection to the specified `addr`.
    async fn connect_addr(addr: SocketAddr) -> io::Result<TcpStream> {
        let sock = TcpSocket::new(addr)?;
        let remote = sock.connect().await?;
        TcpStream::try_from(remote)
    }
}

impl TryFrom<std::net::TcpStream> for TcpStream {
    type Error = io::Error;

    fn try_from(stream: std::net::TcpStream) -> Result<Self, Self::Error> {
        // Required to make sure `stream` can be polled without blocking when
        // awaited.
        stream.set_nonblocking(true)?;
        Ok(TcpStream(stream))
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        // SAFETY: The current runtime is guaranteed to be set via thread-local
        // storage when entering `Runtime::block_on`, which is the only entry
        // point for asynchronous execution, therefore, any async code,
        // including this `Drop`, must be running within a valid runtime context
        // to be called.
        Runtime::current()
            .scheduler
            .unregister_fd(self.0.as_raw_fd());

        // Inner `std::net::TcpStream` is dropped...
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match self.0.read(buf) {
            Ok(rbytes) => Poll::Ready(Ok(rbytes)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Register for read readiness notifications.
                let events = libc::EPOLLIN;

                Runtime::current().scheduler.register_fd(
                    self.0.as_raw_fd(),
                    events as u32,
                    ctx.waker().clone(),
                );

                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.0.write(buf) {
            Ok(wbytes) => Poll::Ready(Ok(wbytes)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Register for write readiness notifications.
                let events = libc::EPOLLOUT;

                Runtime::current().scheduler.register_fd(
                    self.0.as_raw_fd(),
                    events as u32,
                    ctx.waker().clone(),
                );

                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.0.shutdown(std::net::Shutdown::Write) {
            Ok(()) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Register for write readiness notifications, so shutdown can
                // be retried.
                let events = libc::EPOLLOUT;

                Runtime::current().scheduler.register_fd(
                    self.0.as_raw_fd(),
                    events as u32,
                    ctx.waker().clone(),
                );

                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
