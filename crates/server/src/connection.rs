//! Client connection handling and request processing.

use std::net::TcpStream;

pub fn handle(stream: TcpStream) {
    println!("New client connected: {:?}", stream.peer_addr());
}
