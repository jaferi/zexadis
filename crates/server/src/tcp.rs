//! TCP listener and network transport for the Zexadis server.

use std::net::TcpListener;

use crate::connection;

pub fn start_server(address: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(address)?;
    println!("Server listening on {}", address);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Spawn thread for handle the client connection
                std::thread::spawn(move || {
                    connection::handle(stream);
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
