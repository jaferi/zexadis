//! TCP listener and network transport for the Zexadis server.

use crate::{connection, runtime::Runtime};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

pub fn run(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;

    let runtime = Arc::new(Runtime::new(16).map_err(|e| std::io::Error::other(e.to_string()))?);

    println!("Server listening on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let runtime = Arc::clone(&runtime);
                // Spawn thread for handle the client connection
                thread::spawn(move || {
                    connection::handle(stream, runtime);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
