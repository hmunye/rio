//! Networking bindings for `rio`.

mod tcp;
pub use tcp::{TcpListener, TcpStream};

pub(crate) mod socket;
