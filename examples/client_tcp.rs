//! A simple client that opens a TCP stream, sends a message received from
//! `stdin` to the peer, and closes the connection upon receiving an "exit"
//! message.
//!
//! This is intended to show an example of working with a client.
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
use rio::net::TcpStream;

fn main() -> Result<(), Box<dyn Error>> {
    let rt = rio::rt::Runtime::new();

    rt.block_on(async {
        let mut stream = TcpStream::connect("127.0.0.1:8080").await?;

        let mut write_buf = String::new();
        let mut read_buf = Vec::new();

        loop {
            let _ = std::io::stdin().read_line(&mut write_buf)?;
            if write_buf == "exit\n" {
                break;
            }

            stream.write_all(write_buf.as_bytes()).await?;

            write_buf.clear();

            let n = stream.read(&mut read_buf).await?;

            unsafe {
                println!(
                    "received: {}",
                    std::str::from_utf8_unchecked(&read_buf[..n])
                );
            }
        }

        Ok(())
    })
}
