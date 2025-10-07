use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::os::fd::AsFd;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::net::tcp::TcpStream;
use crate::rt::Runtime;

/// A TCP socket server, listening for connections.
///
/// The Transmission Control Protocol is specified in [IETF RFC 793].
///
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
#[derive(Debug)]
pub struct TcpListener {
    ln: std::net::TcpListener,
    /// In `EPOLLET` (edge-triggered mode), the listener must be fully drained,
    /// as multiple connections may be ready to accept before `accept()` would
    /// block again. To handle this, additional connections are queued.
    queue: RefCell<VecDeque<(TcpStream, SocketAddr)>>,
}

impl TcpListener {
    /// Creates a new `TcpListener`, which will be bound to the specified
    /// address.
    ///
    /// The returned listener is ready for accepting connections.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port
    /// to this listener.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let ln = std::net::TcpListener::bind(addr)?;

        // Required to make sure `listener` can be polled without blocking when
        // awaited.
        ln.set_nonblocking(true)?;

        Ok(TcpListener {
            ln,
            queue: RefCell::new(Default::default()),
        })
    }

    /// Accepts a new incoming connection from this listener.
    ///
    /// This function will yield once a new TCP connection is established. When
    /// established, the corresponding `TcpStream` and the remote peer’s address
    /// will be returned.
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.accept_one().await
    }

    /// Returns the local address that this listener is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.ln.local_addr()
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&self) -> io::Result<u32> {
        self.ln.ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.ln.set_ttl(ttl)
    }

    /// Returns a `Future` that resolves to the next incoming connection.
    #[inline]
    fn accept_one(&self) -> AcceptFut<'_> {
        AcceptFut(self)
    }

    /// Queues a TCP connection given the `TcpStream` and remote address.    
    #[inline]
    fn enqueue_connection(&self, stream: TcpStream, addr: SocketAddr) {
        self.queue.borrow_mut().push_back((stream, addr));
    }

    /// Returns a queued accepted TCP connection, or [`None`] if it is empty.
    #[inline]
    fn dequeue_connection(&self) -> Option<(TcpStream, SocketAddr)> {
        self.queue.borrow_mut().pop_front()
    }
}

impl AsFd for TcpListener {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.ln.as_fd()
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.ln.as_raw_fd()
    }
}

impl Drop for TcpListener {
    // SAFETY: The current runtime is guaranteed to be set via thread-local
    // storage when entering `Runtime::block_on`, which is the only entry point
    // for asynchronous execution, therefore, any async code, including this
    // `Drop`, must be running within a valid runtime context to be called.
    fn drop(&mut self) {
        Runtime::current()
            .scheduler
            .unregister_fd(self.ln.as_raw_fd());

        // Inner `std::net::TcpListener` and queued connections are dropped...
    }
}

/// A `Future` that resolves to the next incoming connection on a TCP listener.
struct AcceptFut<'a>(&'a TcpListener);

impl<'a> Future for AcceptFut<'a> {
    type Output = io::Result<(TcpStream, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(conn_pair) = self.0.dequeue_connection() {
            return Poll::Ready(Ok(conn_pair));
        }

        loop {
            match self.0.ln.accept() {
                Ok((stream, addr)) => match TcpStream::try_from(stream) {
                    Ok(stream) => {
                        println!("accept: accepted connection from {}", addr);
                        self.0.enqueue_connection(stream, addr);
                        continue;
                    }
                    Err(e) => return Poll::Ready(Err(e)),
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // `EPOLLET` enables edge-triggered mode, notifying only
                    // when changes occur on the monitored file descriptor,
                    // rather than if it is in the desired state. This requires
                    // non-blocking sockets and fully draining the socket of
                    // reads/writes until it would block to avoid missing
                    // events.
                    let events = libc::EPOLLIN | libc::EPOLLET;
                    Runtime::current().scheduler.register_fd(
                        self.0.ln.as_raw_fd(),
                        events as u32,
                        ctx.waker().clone(),
                    );

                    // Connection may have been queued during draining loop.
                    if let Some((stream, addr)) = self.0.dequeue_connection() {
                        println!("accept: accepted connection fd: {}", stream.as_raw_fd());
                        return Poll::Ready(Ok((stream, addr)));
                    } else {
                        return Poll::Pending;
                    }
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }
    }
}
