//! Networking Utilities for `rio`.
//!
//! Provides high-level asynchronous networking types similar to [`std::net`],
//! including:
//!
//! - [`TcpListener`] and [`TcpStream`] for communication over TCP.

pub use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

mod tcp;
pub use tcp::{TcpListener, TcpStream};
