use rio::io::{AsyncReadExt, AsyncWriteExt};
use rio::net::TcpListener;

const DEFAULT_ADDR: &str = "127.0.0.1:3000";
const BUFFER_SIZE: usize = 4096;

#[rio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());

    let listener = TcpListener::bind(&addr).await?;
    eprintln!("[{addr}]: listening for connections...");

    loop {
        let (mut socket, addr) = listener.accept().await?;

        eprintln!("[{addr}]: client connected");

        rio::spawn(async move {
            let mut buf = vec![0; BUFFER_SIZE];

            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        eprintln!("[{addr}]: client disconnected");
                        return;
                    }
                    Ok(n) => {
                        eprintln!("[{addr}]: read {n} bytes");

                        if let Err(e) = socket.write_all(&buf[0..n]).await {
                            eprintln!("[{addr}]: failed to write to socket: {e}");
                            return;
                        }

                        eprintln!("[{addr}]: sent {n} bytes");
                    }
                    Err(e) => {
                        eprintln!("[{addr}]: failed to read from socket: {e}");
                        return;
                    }
                }
            }
        });
    }
}
