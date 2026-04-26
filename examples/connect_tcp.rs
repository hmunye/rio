use std::error::Error;

use rio::io::AsyncWriteExt;
use rio::net::TcpStream;

const DEFAULT_ADDR: &str = "127.0.0.1:3000";

#[rio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());

    let mut stream = TcpStream::connect(addr).await?;
    println!("created stream: local: {}", stream.local_addr()?);

    let result = stream.write_all(b"hello world\n").await;
    println!("wrote to stream; success={:?}", result.is_ok());

    Ok(())
}
