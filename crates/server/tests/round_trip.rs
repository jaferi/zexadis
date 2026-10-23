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
