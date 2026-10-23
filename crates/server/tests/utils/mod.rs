use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use zexadis_core::commands::{Command, CommandResult};
use zexadis_protocol::command::{decode_response, encode_request};
use zexadis_protocol::encoding::{decode_frame, encode_frame};
use zexadis_protocol::framing::{DEFAULT_MAX_FRAME_LENGTH, FRAME_HEADER_SIZE, FRAME_LENGTH_SIZE};
use zexadis_server::connection;
use zexadis_server::runtime::Runtime;

pub struct TestServer {
    pub stream: TcpStream,
}

impl TestServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();

        let runtime = Arc::new(Runtime::new(16).map_err(|e| std::io::Error::other(e.to_string()))?);

        let server_runtime = Arc::clone(&runtime);

        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let runtime = Arc::clone(&server_runtime);

                        thread::spawn(move || {
                            connection::handle(stream, runtime);
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {e}");
                    }
                }
            }
        });

        let stream = TcpStream::connect(&address)?;

        Ok(Self { stream })
    }

    pub fn send(&mut self, command: &Command) -> Result<CommandResult, Box<dyn std::error::Error>> {
        let request_bytes = {
            let request_frame = encode_request(command)?;
            let request_bytes = encode_frame(&request_frame)?;

            request_bytes
        };

        self.stream.write_all(&request_bytes)?;

        let response_bytes = {
            let mut length_bytes = [0u8; FRAME_LENGTH_SIZE];
            self.stream.read_exact(&mut length_bytes)?;

            let body_length = u32::from_be_bytes(length_bytes) as usize;

            if !(FRAME_HEADER_SIZE..=DEFAULT_MAX_FRAME_LENGTH).contains(&body_length) {
                return Err("Invalid response frame length".into());
            }

            let mut buffer = Vec::with_capacity(FRAME_LENGTH_SIZE + body_length);
            buffer.extend_from_slice(&length_bytes);
            buffer.resize(FRAME_LENGTH_SIZE + body_length, 0);

            self.stream.read_exact(&mut buffer[FRAME_LENGTH_SIZE..])?;

            buffer
        };

        let response_frame = decode_frame(response_bytes.into())?;

        Ok(decode_response(&response_frame)?)
    }
}
