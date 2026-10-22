//! Client connection handling and request processing.

use std::{io::Read, net::TcpStream};
use zexadis_protocol::command::decode_request;
use zexadis_protocol::encoding::decode_frame;
use zexadis_protocol::framing::{DEFAULT_MAX_FRAME_LENGTH, FRAME_HEADER_SIZE, FRAME_LENGTH_SIZE};
use zexadis_protocol::Frame;

use bytes::BytesMut;

fn read_frame(stream: &mut TcpStream) -> Result<Option<Frame>, Box<dyn std::error::Error>> {
    let mut length_bytes = [0u8; FRAME_LENGTH_SIZE];

    if stream.read(&mut length_bytes[..1])? == 0 {
        return Ok(None);
    }

    stream.read_exact(&mut length_bytes[1..])?;

    let body_length = u32::from_be_bytes(length_bytes) as usize;

    if !(FRAME_HEADER_SIZE..=DEFAULT_MAX_FRAME_LENGTH).contains(&body_length) {
        return Err("Invalid frame length".into());
    }

    let mut buffer = BytesMut::with_capacity(FRAME_LENGTH_SIZE + body_length);
    buffer.extend_from_slice(&length_bytes);
    buffer.resize(FRAME_LENGTH_SIZE + body_length, 0);

    stream.read_exact(&mut buffer[FRAME_LENGTH_SIZE..])?;

    Ok(Some(decode_frame(buffer.freeze())?))
}

pub fn handle(mut stream: TcpStream) {
    println!("New client connected: {:?}", stream.peer_addr());

    loop {
        let frame = match read_frame(&mut stream) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                eprintln!("Error reading frame: {e}");
                break;
            }
        };

        let request = match decode_request(&frame) {
            Ok(command) => command,
            Err(e) => {
                eprintln!("Error decoding request: {e}");
                break;
            }
        };

        println!("Received request: {:?}", request);
    }
}
