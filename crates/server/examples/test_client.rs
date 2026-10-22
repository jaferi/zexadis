use bytes::{Bytes, BytesMut};

use std::{
    io::{Read, Write},
    net::TcpStream,
};

use zexadis_core::commands::Command;

use zexadis_protocol::{
    command::{decode_response, encode_request},
    encoding::{decode_frame, encode_frame},
    framing::FRAME_LENGTH_SIZE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:6379")?;

    let command = Command::Get {
        key: Bytes::from_static(b"hello"),
    };

    let frame = encode_request(&command)?;
    let request_bytes = encode_frame(&frame)?;
    stream.write_all(&request_bytes)?;

    // Read the response frame's length prefix.
    let mut length_bytes = [0u8; FRAME_LENGTH_SIZE];
    stream.read_exact(&mut length_bytes)?;

    let body_length = u32::from_be_bytes(length_bytes) as usize;

    // Read the response body.
    let mut buffer = BytesMut::with_capacity(FRAME_LENGTH_SIZE + body_length);
    buffer.extend_from_slice(&length_bytes);
    buffer.resize(FRAME_LENGTH_SIZE + body_length, 0);
    stream.read_exact(&mut buffer[FRAME_LENGTH_SIZE..])?;

    let response_frame = decode_frame(buffer.freeze())?;
    let response = decode_response(&response_frame)?;

    println!("Response: {:?}", response);

    Ok(())
}
