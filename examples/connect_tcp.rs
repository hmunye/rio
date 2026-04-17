use rio::io::AsyncWriteExt;
use rio::net::{SocketAddr, TcpStream};

#[rio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addrs = [
        SocketAddr::from(([127, 0, 0, 1], 6141)),
        SocketAddr::from(([127, 0, 0, 1], 6142)),
    ];
    let mut stream = TcpStream::connect(&addrs[..]).await?;
    println!("created stream: local: {}", stream.local_addr()?);

    let result = stream.write_all(b"hello world\n").await;
    println!("wrote to stream; success={:?}", result.is_ok());

    Ok(())
}
