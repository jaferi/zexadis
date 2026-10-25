//! Verify key expiration and TTL behavior through the running server.

use std::time::Duration;

use bytes::Bytes;

use zexadis_core::commands::{Command, CommandResult};

mod utils;
use utils::TestServer;

#[test]
fn get_expired_key_returns_not_found() {
    let mut server = TestServer::new().unwrap();

    let key = Bytes::from_static(b"temporary-key");

    server
        .send(&Command::Set {
            key: key.clone(),
            value: Bytes::from_static(b"temporary-value"),
            ttl: Some(Duration::from_millis(100)),
        })
        .unwrap();

    // Confirm the key is initially available.
    assert_eq!(
        server.send(&Command::Get { key: key.clone() }).unwrap(),
        CommandResult::Entry(Bytes::from_static(b"temporary-value"))
    );

    std::thread::sleep(Duration::from_millis(200));

    // The next access should trigger expiration if it hasn't already.
    assert_eq!(
        server.send(&Command::Get { key }).unwrap(),
        CommandResult::NotFound
    );
}
