//! A TCP echo server with `rio`.
//!
//! This server will create a TCP listener, accept connections in a loop, and
//! echo read bytes to each TCP connection.
//!
//! Run this in the terminal using:
//!
//!     cargo run --example echo_tcp
//!
//! and in another terminal run:
//!
//!     cargo run --example client_tcp
//!
//! Each line you type in to the `client_tcp` terminal should be echo'd back.
//!
//! Multiple terminals can run the `client_tcp` example and all make progress
//! concurrently.

use std::error::Error;

use rio::io::{AsyncReadExt, AsyncWriteExt};
use rio::net::TcpListener;

const ADDR: &str = "127.0.0.1:8080";

fn main() -> Result<(), Box<dyn Error>> {
    let rt = rio::rt::Runtime::new();

    rt.block_on(async {
        // Create a TCP listener which will listen for incoming connections.
        let listener = TcpListener::bind(ADDR).await?;

        println!("listening on: {}", ADDR);

        loop {
            // Asynchronously wait for an inbound connection.
            let (mut socket, _) = listener.accept().await?;

            // Use the `rio::spawn` function to execute the work in the
            // background and have all clients make progress concurrently.
            rio::spawn(async move {
                let mut buf = vec![0; 1024];

                loop {
                    let n = socket
                        .read(&mut buf)
                        .await
                        .expect("failed to read data from socket");

                    if n == 0 {
                        return;
                    }

                    socket
                        .write_all(&buf[..n])
                        .await
                        .expect("failed to write data to socket");
                }
            });
        }

        #[allow(unreachable_code)]
        Ok(())
    })
}
