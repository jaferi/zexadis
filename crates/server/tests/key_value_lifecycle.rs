//! Verify key-value operations through the running server.

use std::time::Duration;

use bytes::Bytes;

use zexadis_core::commands::{Command, CommandResult};

mod utils;
use utils::TestServer;

#[test]
fn set_request_and_response_round_trip() {
    let mut server = TestServer::new().unwrap();

    let set_command = Command::Set {
        key: Bytes::from_static(b"user:42"),
        value: Bytes::from_static(b"test-bytes"),
        ttl: Some(Duration::from_secs(60)),
    };

    let set_result = server.send(&set_command).unwrap();
    assert_eq!(set_result, CommandResult::Set(true));

    let get_command = Command::Get {
        key: Bytes::from_static(b"user:42"),
    };

    let get_result = server.send(&get_command).unwrap();
    assert_eq!(
        get_result,
        CommandResult::Entry(Bytes::from_static(b"test-bytes"))
    );
}

#[test]
fn binary_keys_and_values_round_trip() {
    let mut server = TestServer::new().unwrap();

    let key = Bytes::from_static(b"\x00\xff\x01binary-key");
    let value = Bytes::from_static(b"\x00\xff\xfe\x80\x01");

    let result = server
        .send(&Command::Set {
            key: key.clone(),
            value: value.clone(),
            ttl: None,
        })
        .unwrap();

    assert_eq!(result, CommandResult::Set(true));

    let result = server.send(&Command::Get { key }).unwrap();

    assert_eq!(result, CommandResult::Entry(value));
}

#[test]
fn set_overwrites_existing_value() {
    let mut server = TestServer::new().unwrap();

    let key = Bytes::from_static(b"user:42");

    server
        .send(&Command::Set {
            key: key.clone(),
            value: Bytes::from_static(b"first"),
            ttl: None,
        })
        .unwrap();

    server
        .send(&Command::Set {
            key: key.clone(),
            value: Bytes::from_static(b"second"),
            ttl: None,
        })
        .unwrap();

    let result = server.send(&Command::Get { key }).unwrap();

    assert_eq!(result, CommandResult::Entry(Bytes::from_static(b"second")));
}

#[test]
fn get_missing_key_returns_not_found() {
    let mut server = TestServer::new().unwrap();

    let result = server
        .send(&Command::Get {
            key: Bytes::from_static(b"missing-key"),
        })
        .unwrap();

    assert_eq!(result, CommandResult::NotFound);
}

#[test]
fn isolation_test_multiple_keys_keep_independent_values() {
    let mut server = TestServer::new().unwrap();

    let key_a = Bytes::from_static(b"key-a");
    let key_b = Bytes::from_static(b"key-b");
    let value_a = Bytes::from_static(b"value-a");
    let value_b = Bytes::from_static(b"value-b");

    for (key, value) in [
        (key_a.clone(), value_a.clone()),
        (key_b.clone(), value_b.clone()),
    ] {
        server
            .send(&Command::Set {
                key,
                value,
                ttl: None,
            })
            .unwrap();
    }

    assert_eq!(
        server.send(&Command::Get { key: key_a }).unwrap(),
        CommandResult::Entry(value_a)
    );

    assert_eq!(
        server.send(&Command::Get { key: key_b }).unwrap(),
        CommandResult::Entry(value_b)
    );
}
