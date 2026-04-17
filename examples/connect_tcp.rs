use rio::io::AsyncWriteExt;
use rio::net::TcpStream;

#[rio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:3000").await?;
    println!("created stream: local: {}", stream.local_addr()?);

    let result = stream.write_all(b"hello world\n").await;
    println!("wrote to stream; success={:?}", result.is_ok());

    Ok(())
}
