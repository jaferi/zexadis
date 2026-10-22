//! Client connection handling and request processing.

use std::io::Write;
use std::{io::Read, net::TcpStream, sync::Arc};

use zexadis_core::commands::CommandResult;
use zexadis_protocol::command::{decode_request, encode_response};
use zexadis_protocol::encoding::{decode_frame, encode_frame};
use zexadis_protocol::framing::{DEFAULT_MAX_FRAME_LENGTH, FRAME_HEADER_SIZE, FRAME_LENGTH_SIZE};
use zexadis_protocol::Frame;

use crate::runtime::Runtime;

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

pub fn handle(mut stream: TcpStream, runtime: Arc<Runtime>) {
    println!("New client connected: {:?}", stream.peer_addr());

    loop {
        let request_frame = match read_frame(&mut stream) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(e) => {
                eprintln!("Error reading frame: {e}");
                break;
            }
        };

        let request = match decode_request(&request_frame) {
            Ok(command) => command,
            Err(e) => {
                eprintln!("Error decoding request: {e}");
                break;
            }
        };

        println!("Received request: {:?}", request);

        match runtime.execute(request) {
            Ok(result) => {
                println!("Command result: {:?}", result);

                if let Err(e) = write_response(&mut stream, &result) {
                    eprintln!("Error sending response: {e}");
                    break;
                }
            }
            Err(e) => {
                eprintln!("Command execution failed: {e}");
                break;
            }
        }
    }
}

fn write_response(
    stream: &mut TcpStream,
    result: &CommandResult,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_frame = encode_response(result)?;
    let response_bytes = encode_frame(&response_frame)?;
    stream.write_all(&response_bytes)?;

    Ok(())
}
