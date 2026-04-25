use std::mem::{self, MaybeUninit};
use std::net::{SocketAddr, ToSocketAddrs};
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use std::{future, io};

use crate::io::{AsyncRead, AsyncWrite};
use crate::net::tcp::TcpSocket;
use crate::rt::context;
use crate::rt::io::{Interest, IoHandle};
use crate::task::coop;

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
    handle: Option<IoHandle>,
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
    #[inline]
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
                "could not resolve to any addresses",
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
        // FIXME: use `std::net::TcpStream::linger` when stable
        //
        // <https://github.com/rust-lang/rust/issues/88494>
        let mut opt_value = MaybeUninit::<libc::linger>::zeroed();
        let mut opt_len = mem::size_of::<libc::linger>() as libc::socklen_t;

        let ret = unsafe {
            libc::getsockopt(
                self.inner.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                opt_value.as_mut_ptr().cast(),
                &mut opt_len,
            )
        };

        if ret == -1 {
            return Err(errno!("getsockopt(2) SO_LINGER failed"));
        }

        // SAFETY: `getsockopt` call succeeded meaning `opt_value` must have
        // been initialized by the system.
        let opt_value = unsafe { opt_value.assume_init() };

        Ok((opt_value.l_onoff != 0).then(|| Duration::from_secs(opt_value.l_linger as u64)))
    }

    /// Sets the value of the `SO_LINGER` option on this socket.
    ///
    /// `SO_LINGER` controls how the socket is closed when data remains to be
    /// sent. If set, the socket will remain open for the specified duration as
    /// the system attempts to send pending data. Otherwise, the system may
    /// close the socket immediately, or wait for a default timeout.
    #[inline]
    #[allow(clippy::missing_errors_doc)]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        // FIXME: use `std::net::TcpStream::set_linger` when stable
        //
        // <https://github.com/rust-lang/rust/issues/88494>
        let opt_value = libc::linger {
            l_onoff: libc::c_int::from(linger.is_some()),
            l_linger: linger.unwrap_or_default().as_secs() as libc::c_int,
        };
        let opt_len = mem::size_of::<libc::linger>() as libc::socklen_t;

        if unsafe {
            libc::setsockopt(
                self.inner.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_LINGER,
                (&raw const opt_value).cast(),
                opt_len,
            )
        } == -1
        {
            return Err(errno!("setsockopt(2) SO_LINGER failed"));
        }

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

    pub(crate) fn from_std(
        stream: std::net::TcpStream,
        handle: Option<IoHandle>,
    ) -> io::Result<Self> {
        stream.set_nonblocking(true)?;

        Ok(TcpStream {
            inner: stream,
            handle,
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

cfg_linux! {
    impl TcpStream {
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
            use std::os::linux::net::TcpStreamExt;
            self.inner.quickack()
        }

        /// Sets the value for the `TCP_QUICKACK` option on this socket
        ///
        /// This flag causes Linux to eagerly send `ACK`s rather than delaying
        /// them. Linux may reset this flag after further operations on the
        /// socket.
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
            use std::os::linux::net::TcpStreamExt;
            self.inner.set_quickack(quickack)
        }
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use std::io::Read;

        let mut read = 0;
        let coop = ready!(coop::poll_proceed());

        loop {
            match self.inner.read(&mut buf[read..]) {
                Ok(0) => {
                    coop.made_progress();
                    return Poll::Ready(Ok(read));
                }
                Ok(n) => {
                    read += n;

                    if read == buf.len() {
                        coop.made_progress();
                        return Poll::Ready(Ok(read));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    match self.handle.as_mut() {
                        Some(handle) => {
                            if !handle.is_readable() {
                                handle.add_interest(Interest::EDGE_TRIGGERED | Interest::READ);
                            }
                        }
                        None => {
                            self.handle = Some(context::with_handle(|h| {
                                h.register_io(
                                    self.inner.as_raw_fd(),
                                    Interest::EDGE_TRIGGERED | Interest::READ,
                                    cx.waker().clone(),
                                )
                            }));
                        }
                    }

                    if read > 0 {
                        coop.made_progress();
                        return Poll::Ready(Ok(read));
                    } else {
                        return Poll::Pending;
                    }
                }
                Err(e) => {
                    coop.made_progress();
                    return Poll::Ready(Err(e));
                }
            }
        }
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        use std::io::Write;

        let mut written = 0;
        let coop = ready!(coop::poll_proceed());

        loop {
            match self.inner.write(&buf[written..]) {
                Ok(0) => {
                    coop.made_progress();
                    return Poll::Ready(Ok(written));
                }
                Ok(n) => {
                    written += n;

                    if written == buf.len() {
                        coop.made_progress();
                        return Poll::Ready(Ok(written));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // NOTE: `IoHandle` is derived from `TcpSocket::connect`,
                    // which already registers interest in EDGE_TRIGGERED and
                    // WRITE, if one was initialized.
                    if self.handle.is_none() {
                        self.handle = Some(context::with_handle(|h| {
                            h.register_io(
                                self.inner.as_raw_fd(),
                                Interest::EDGE_TRIGGERED | Interest::WRITE,
                                cx.waker().clone(),
                            )
                        }));
                    }

                    if written > 0 {
                        coop.made_progress();
                        return Poll::Ready(Ok(written));
                    } else {
                        return Poll::Pending;
                    }
                }
                Err(e) => {
                    coop.made_progress();
                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.inner.shutdown(std::net::Shutdown::Write) {
            Ok(()) => Poll::Ready(Ok(())),
            // <https://docs.rs/tokio/latest/src/tokio/net/tcp/stream.rs.html#1130>
            //
            // commit: 6c03e03898d71eca976ee1ad8481cf112ae722ba
            Err(e) if e.kind() == io::ErrorKind::NotConnected => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    cfg_bsd! {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        use std::task::Waker;
    }

    use super::*;

    use crate::io::{AsyncReadExt, AsyncWriteExt};
    use crate::net::TcpListener;
    use crate::rt::{Handle, time::clock};
    use crate::task::JoinHandle;

    const THRESHOLD_MS: u64 = 5;

    enum OnConnect {
        WriteBytes(usize),
        ReadBytes(usize),
        Shutdown,
        Wait(Duration),
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        Notify(Notify),
    }

    cfg_bsd! {
        #[derive(Clone)]
        struct Notify {
            inner: Rc<NotifyInner>,
        }

        struct NotifyInner {
            notified: Cell<bool>,
            waker: RefCell<Option<Waker>>,
        }

        impl Notify {
            fn new() -> Self {
                Self {
                    inner: Rc::new(NotifyInner {
                        notified: Cell::default(),
                        waker: RefCell::default(),
                    }),
                }
            }

            fn notify(&self) {
                self.inner.notified.set(true);
                if let Some(waker) = self.inner.waker.take() {
                    waker.wake();
                }
            }
        }

        impl Future for Notify {
            type Output = ();

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.inner.notified.get() {
                    Poll::Ready(())
                } else {
                    self.inner.waker.replace(Some(cx.waker().clone()));
                    Poll::Pending
                }
            }
        }
    }

    fn spawn_listener(
        actions: Vec<OnConnect>,
    ) -> io::Result<(JoinHandle<io::Result<()>>, SocketAddr)> {
        let ln = TcpListener::bind("127.0.0.1:0")?;
        let addr = ln.local_addr()?;

        let handle = crate::spawn(async move {
            let (mut socket, _) = ln.accept().await?;
            let mut buf = [0u8; 1024];

            for action in actions {
                match action {
                    OnConnect::WriteBytes(n) => {
                        socket.write_all("1".repeat(n).as_bytes()).await?;
                    }
                    OnConnect::ReadBytes(n) => {
                        socket.read_exact(&mut buf[..n]).await?;
                    }
                    OnConnect::Shutdown => {
                        socket.shutdown().await?;
                    }
                    OnConnect::Wait(d) => {
                        crate::time::sleep(d).await;
                    }
                    #[cfg(any(
                        target_os = "macos",
                        target_os = "freebsd",
                        target_os = "dragonfly",
                        target_os = "openbsd",
                        target_os = "netbsd"
                    ))]
                    OnConnect::Notify(n) => {
                        n.notify();
                    }
                }
            }

            Ok::<(), io::Error>(())
        });

        Ok((handle, addr))
    }

    /// Initialize a `TcpStream` connected to a listener task.
    ///
    /// The listener task is spawned with the provided `OnConnect` commands,
    /// which it will process first before returning a handle to the task and
    /// the `TcpStream`.
    #[allow(unused_mut)]
    #[allow(clippy::future_not_send)]
    async fn setup_stream(
        mut commands: Vec<OnConnect>,
    ) -> io::Result<(JoinHandle<io::Result<()>>, TcpStream)> {
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        {
            let notify_on_accept = Notify::new();
            commands.insert(0, OnConnect::Notify(notify_on_accept.clone()));
        }

        let (handle, addr) = spawn_listener(commands)?;
        let stream = TcpStream::connect(addr).await?;

        // NOTE: `kqueue(2)` notifies readiness differently than `epoll(7)`.
        //
        // With `kqueue(2)`, the first `accept(2)` may block, causing the
        // listener task to yield. `await_notify!()` ensures the listener task
        // can accept the connection before `setup_stream` returns.
        //
        // With `epoll(7)`, the first `accept(2)` does not block.
        #[cfg(any(
            target_os = "macos",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        notify_on_accept.await;

        assert!(context::with_handle(Handle::io_resources) > 0);

        Ok((handle, stream))
    }

    #[test]
    fn test_pending_read() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![OnConnect::Wait(Duration::from_millis(100))])
                .await
                .unwrap_or_else(|e| panic!("{e}"));

            let mut read = stream.read(&mut buf);

            {
                let mut pinned = unsafe { Pin::new_unchecked(&mut read) };
                let mut cx = Context::from_waker(std::task::Waker::noop());
                assert!(pinned.as_mut().poll(&mut cx).is_pending());
            }

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_read() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() * 2),
                OnConnect::Wait(Duration::from_millis(100)),
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            assert_eq!(
                stream
                    .read(&mut buf)
                    .await
                    .unwrap_or_else(|e| panic!("{e}")),
                buf.len()
            );

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_eof_read() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Shutdown,
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            assert_eq!(
                stream
                    .read(&mut buf)
                    .await
                    .unwrap_or_else(|e| panic!("{e}")),
                buf.len() / 2
            );

            assert_eq!(
                stream
                    .read(&mut buf)
                    .await
                    .unwrap_or_else(|e| panic!("{e}")),
                0
            );

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_pending_read_exact() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Wait(Duration::from_millis(100)),
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            {
                let mut read_exact = stream.read_exact(&mut buf);
                let mut pinned = unsafe { Pin::new_unchecked(&mut read_exact) };
                let mut cx = Context::from_waker(std::task::Waker::noop());
                assert!(pinned.as_mut().poll(&mut cx).is_pending());
            }

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_partial_read_exact() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Wait(Duration::from_millis(100)),
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Wait(Duration::from_millis(100)),
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            let mut read_exact = stream.read_exact(&mut buf);

            {
                let mut pinned = unsafe { Pin::new_unchecked(&mut read_exact) };
                let mut cx = Context::from_waker(std::task::Waker::noop());
                assert!(pinned.as_mut().poll(&mut cx).is_pending());
            }

            clock::advance(Duration::from_millis(100)).await;

            assert_eq!(
                read_exact.await.unwrap_or_else(|e| panic!("{e}")),
                buf.len()
            );

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_eof_read_exact() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Shutdown,
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            assert_eq!(
                stream
                    .read_exact(&mut buf)
                    .await
                    .expect_err("should be EOF")
                    .kind(),
                io::ErrorKind::UnexpectedEof
            );

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_read_after_shutdown() {
        rt! {
            let mut buf = [0u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::WriteBytes(buf.len() / 2),
                OnConnect::Shutdown,
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            assert_eq!(
                stream
                    .read(&mut buf)
                    .await
                    .unwrap_or_else(|e| panic!("{e}")),
                buf.len() / 2
            );

            assert_eq!(
                stream
                    .read(&mut buf)
                    .await
                    .unwrap_or_else(|e| panic!("{e}")),
                0
            );

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_shutdown_after_write() {
        rt! {
            let buf = [1u8; 10];

            #[cfg(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
            let notify = Notify::new();

            let (handle, mut stream) = setup_stream(vec![
                OnConnect::ReadBytes(buf.len() / 2),
                #[cfg(any(
                    target_os = "macos",
                    target_os = "freebsd",
                    target_os = "dragonfly",
                    target_os = "openbsd",
                    target_os = "netbsd"
                ))]
                OnConnect::Notify(notify.clone()),
                OnConnect::Wait(Duration::from_millis(100)),
                OnConnect::ReadBytes(buf.len() / 2),
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            stream
                .write(&buf[..buf.len() / 2])
                .await
                .unwrap_or_else(|e| panic!("{e}"));

            assert!(stream.shutdown().await.is_ok());

            #[cfg(any(
                target_os = "macos",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
            notify.await;

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert_eq!(
                handle
                    .await
                    .expect("task should complete")
                    .expect_err("stream is shutdown")
                    .kind(),
                io::ErrorKind::UnexpectedEof
            );

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }

    #[test]
    fn test_write_all() {
        rt! {
            let buf = [1u8; 10];
            let (handle, mut stream) = setup_stream(vec![
                OnConnect::ReadBytes(buf.len() * 2),
                OnConnect::Wait(Duration::from_millis(100)),
            ])
            .await
            .unwrap_or_else(|e| panic!("{e}"));

            stream
                .write_all(&buf)
                .await
                .unwrap_or_else(|e| panic!("{e}"));

            clock::advance(Duration::from_millis(100)).await;

            drop(stream);
            assert!(handle.await.is_ok());

            assert_eq!(context::with_handle(Handle::io_resources), 0);
            assert!(clock::now().elapsed() < Duration::from_millis(THRESHOLD_MS));
        }
    }
}
