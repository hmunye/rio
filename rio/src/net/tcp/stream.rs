use std::net::{SocketAddr, ToSocketAddrs};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use std::{future, io};

use crate::io::{AsyncRead, AsyncWrite};
use crate::net::tcp::TcpSocket;
use crate::rt::io::IoHandle;

/// TCP stream between a local and a remote socket.
///
/// Created by connecting to a remote endpoint via [`connect`], or by accepting
/// a connection from a [`TcpListener`].
///
/// Reading and writing to a `TcpStream` is done using the methods provided by
/// the [`AsyncReadExt`] and [`AsyncWriteExt`] traits.
///
/// [`connect`]: TcpStream::connect
/// [`TcpListener`]: crate::net::TcpListener
/// [`AsyncReadExt`]: crate::io::AsyncReadExt
/// [`AsyncWriteExt`]: crate::io::AsyncWriteExt
#[derive(Debug)]
pub struct TcpStream {
    // NOTE: Defined first to ensure it is dropped before `inner` (deregister
    // before closing FD).
    _handle: IoHandle,
    inner: std::net::TcpStream,
}

impl TcpStream {
    /// Opens a TCP connection to a remote host.
    ///
    /// `addr` is an address of the remote host. Anything which implements the
    /// [`ToSocketAddrs`] trait can be supplied for the address.
    ///
    /// # Errors
    ///
    /// Returns `Err` of the last address that could not be connected to, or if
    /// the socket's options could not be configured.
    ///
    /// # Panics
    ///
    /// Panics if the current thread is not within a runtime context.
    ///
    /// # Examples
    ///
    /// Open a TCP connection to `127.0.0.1:8080`:
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// println!("Connected to the server {:?}!", stream.peer_addr()?);
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Open a TCP connection to `127.0.0.1:8080`. If the connection fails, open
    /// a TCP connection to `127.0.0.1:8081`:
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::{SocketAddr, TcpStream};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 8080)),
    ///     SocketAddr::from(([127, 0, 0, 1], 8081)),
    /// ];
    /// let stream = TcpStream::connect(&addrs[..]).await?;
    /// println!("Connected to the server {:?}!", stream.peer_addr()?);
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
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
                "could not resolve to any address",
            )
        }))
    }

    /// Returns the socket address of the local half of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::{IpAddr, Ipv4Addr, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// assert_eq!(stream.local_addr().unwrap().ip(),
    ///            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// assert_eq!(stream.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Gets the value of the `SO_LINGER` option on this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        todo!()
    }

    /// Sets the value of the `SO_LINGER` option on this socket.
    ///
    /// `SO_LINGER` controls how the socket is closed when data remains to be
    /// sent. If set, the socket will remain open for the specified duration as
    /// the system attempts to send pending data. Otherwise, the system may
    /// close the socket immediately, or wait for a default timeout.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_linger(&self, _linger: Option<Duration>) -> io::Result<()> {
        todo!()
    }

    /// Gets the value of the `TCP_QUICKACK` option on this socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.set_quickack(true)?;
    /// assert_eq!(stream.quickack().unwrap_or(false), true);
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn quickack(&self) -> io::Result<bool> {
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::TcpStreamExt;
            self.inner.quickack()
        }

        #[cfg(not(target_os = "linux"))]
        Ok(true)
    }

    /// Sets the value for the `TCP_QUICKACK` option on this socket
    ///
    /// This flag causes Linux to eagerly send `ACK`s rather than delaying them.
    /// Linux may reset this flag after further operations on the socket.
    ///
    /// See [`man 7 tcp`](https://man7.org/linux/man-pages/man7/tcp.7.html) and
    /// [TCP delayed acknowledgement](https://en.wikipedia.org/wiki/TCP_delayed_acknowledgment)
    /// for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.set_quickack(true)?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_quickack(&self, quickack: bool) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::TcpStreamExt;
            self.inner.set_quickack(quickack)
        }

        #[cfg(not(target_os = "linux"))]
        Ok(())
    }

    /// Returns the value of the `IP_TTL` option for this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket
    ///
    /// `IP_TTL` sets the time-to-live field that is used in every packet sent
    /// from this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, `TCP_NODELAY` disables the _Nagle_ algorithm, meaning segments
    /// are always sent as soon as possible, even if there is only a small
    /// amount of data. When unset, data is buffered until there is a sufficient
    /// amount to send out, thereby avoiding the frequent sending of small
    /// packets.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    pub(crate) fn from_std(stream: std::net::TcpStream, handle: IoHandle) -> io::Result<Self> {
        stream.set_nonblocking(true)?;

        Ok(TcpStream {
            inner: stream,
            _handle: handle,
        })
    }

    async fn connect_addr(addr: SocketAddr) -> io::Result<TcpStream> {
        let sock = TcpSocket::new(addr)?;
        let mut conn = sock.connect();

        future::poll_fn(|cx| {
            let (stream, handle) = ready!(Pin::new(&mut conn).poll(cx))?;
            Poll::Ready(TcpStream::from_std(stream, handle))
        })
        .await
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        todo!()
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        todo!()
    }
}
