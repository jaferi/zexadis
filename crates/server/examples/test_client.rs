use bytes::Bytes;
use std::io::Write;
use std::net::TcpStream;

use zexadis_core::commands::Command;
use zexadis_protocol::command::encode_request;
use zexadis_protocol::encoding::encode_frame;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:6379")?;

    let command = Command::Get {
        key: Bytes::from_static(b"hello"),
    };

    let frame = encode_request(&command)?;
    let bytes = encode_frame(&frame)?;

    stream.write_all(&bytes)?;

    println!("Sent GET request");

    Ok(())
}
