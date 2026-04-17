use std::net::{SocketAddr, ToSocketAddrs};
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{future, io};

use crate::net::TcpStream;
use crate::rt::context;
use crate::rt::io::{Interest, IoHandle};
use crate::task::coop;

/// Listener for accepting incoming TCP connections.
///
/// Binds to a socket address to listen for new connections, which can be
/// accepted via [`accept`].
///
/// [`accept`]: TcpListener::accept
#[derive(Debug)]
pub struct TcpListener {
    // NOTE: Defined first to ensure it is dropped before `ln` (deregister
    // before closing FD).
    _handle: IoHandle,
    ln: std::net::TcpListener,
}

/// Future returned by [`accept`].
///
/// [`accept`]: TcpListener::accept
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Accept<'a>(&'a TcpListener);

impl TcpListener {
    /// Creates a `TcpListener` bound to the specified `addr`, ready to accept
    /// connections.
    ///
    /// # Errors
    ///
    /// Returns `Err` of the last address that could not be bound to, or if the
    /// socket's options could not be configured.
    ///
    /// # Panics
    ///
    /// Panics if the current thread is not within a runtime context.
    ///
    /// # Examples
    ///
    /// Creates a `TcpListener` bound to `127.0.0.1:80`:
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").await?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Creates a `TcpListener` bound to `127.0.0.1:80`. If that fails, creates
    /// one bound to `127.0.0.1:443`:
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::{SocketAddr, TcpListener};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 80)),
    ///     SocketAddr::from(([127, 0, 0, 1], 443)),
    /// ];
    /// let listener = TcpListener::bind(&addrs[..]).await?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Creates a `TcpListener` bound to a port assigned by the operating system
    /// at `127.0.0.1`.
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:0").await?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    // <https://docs.rs/tokio/latest/src/tokio/net/tcp/listener.rs.html#103-121>
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;

        for addr in addrs {
            match TcpListener::bind_addr(addr).await {
                Ok(listener) => return Ok(listener),
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

    /// Accept a new incoming connection from this `TcpListener`, returning the
    /// corresponding [`TcpStream`] and remote peer's address.
    ///
    /// # Panics
    ///
    /// Panics if the caller `.await` or polls the returned future outside of a
    /// runtime context.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").await?;
    /// match listener.accept().await {
    ///     Ok((_socket, addr)) => println!("new client: {addr:?}"),
    ///     Err(e) => println!("couldn't get client: {e:?}"),
    /// }
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub const fn accept(&self) -> Accept<'_> {
        Accept(self)
    }

    /// Returns the local socket address of this listener.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[rio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use rio::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").await?;
    /// assert_eq!(listener.local_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.ln.local_addr()
    }

    /// Returns the value of the `IP_TTL` option for this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn ttl(&self) -> io::Result<u32> {
        self.ln.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket
    ///
    /// `IP_TTL` sets the time-to-live field that is used in every packet sent
    /// from this socket.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.ln.set_ttl(ttl)
    }

    async fn bind_addr(addr: SocketAddr) -> io::Result<TcpListener> {
        future::poll_fn(|cx| {
            // NOTE: `SO_REUSEADDR` socket option is already set here.
            let ln = std::net::TcpListener::bind(addr)?;
            let fd = ln.as_raw_fd();

            ln.set_nonblocking(true)?;

            Poll::Ready(Ok(TcpListener {
                ln,
                _handle: context::with_handle(|handle| {
                    handle.register_io(fd, Interest::READ, cx.waker().clone())
                }),
            }))
        })
        .await
    }
}

impl Future for Accept<'_> {
    type Output = io::Result<(TcpStream, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let coop = ready!(coop::poll_proceed());

        match self.0.ln.accept() {
            Ok((stream, addr)) => {
                coop.made_progress();

                match TcpStream::from_std(stream, None) {
                    Ok(stream) => Poll::Ready(Ok((stream, addr))),
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => {
                coop.made_progress();
                Poll::Ready(Err(e))
            }
        }
    }
}
