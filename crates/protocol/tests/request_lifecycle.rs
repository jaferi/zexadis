//! Verify that every supported request type can be encoded into wire bytes and reconstructed correctly.

use std::time::Duration;

use bytes::Bytes;

use zexadis_core::commands::{Command, CommandResult};
use zexadis_protocol::command::{decode_request, decode_response, encode_request, encode_response};
use zexadis_protocol::encoding::{decode_frame, encode_frame};
use zexadis_protocol::{Frame, Opcode, ProtocolVersion};

#[test]
fn set_request_survives_frame_round_trip() {
    let command = Command::Set {
        key: Bytes::from_static(b"user:42"),
        value: Bytes::from_static(b"\x00\xFF\x10\xAB"),
        ttl: Some(Duration::from_secs(60)),
    };

    // Command -> Frame
    let frame = encode_request(&command).unwrap();
    assert_eq!(frame.opcode, Opcode::Set);

    // Frame -> binary bytes
    let wire_bytes = encode_frame(&frame).unwrap();

    // Binary bytes -> Frame
    let decoded_frame = decode_frame(wire_bytes).unwrap();

    // Frame -> Command
    let decoded_command = decode_request(&decoded_frame).unwrap();

    assert_eq!(decoded_command, command);
}

#[test]
fn set_request_and_response_round_trip() {
    let command = Command::Set {
        key: Bytes::from_static(b"session"),
        value: Bytes::from_static(b"active"),
        ttl: None,
    };

    // Client encodes its request.
    let request_frame = encode_request(&command).unwrap();
    let request_bytes = encode_frame(&request_frame).unwrap();

    // Server-side request decoding.
    let received_frame = decode_frame(request_bytes).unwrap();
    let received_command = decode_request(&received_frame).unwrap();

    assert_eq!(received_command, command);

    // Simulate successful command execution.
    let result = CommandResult::Set(true);

    // Server encodes its response.
    let response_frame = encode_response(&result).unwrap();
    let response_bytes = encode_frame(&response_frame).unwrap();

    // Client decodes the response.
    let received_response_frame = decode_frame(response_bytes).unwrap();
    let received_result = decode_response(&received_response_frame).unwrap();

    assert_eq!(received_result, CommandResult::Set(true));
}

#[test]
fn truncated_frame_returns_error() {
    let command = Command::Set {
        key: Bytes::from_static(b"key"),
        value: Bytes::from_static(b"value"),
        ttl: None,
    };

    let frame = encode_request(&command).unwrap();
    let mut wire_bytes = encode_frame(&frame).unwrap().to_vec();

    // Remove the final byte, leaving the frame incomplete.
    wire_bytes.pop();

    assert!(decode_frame(Bytes::from(wire_bytes)).is_err());
}

#[test]
fn truncated_set_request_returns_error() {
    // Too short to contain the complete SET header:
    // key length (4) + value length (4) + TTL flag (1).
    let frame = Frame::new(
        ProtocolVersion::V1,
        Opcode::Set,
        Bytes::from_static(&[0x00, 0x00, 0x00, 0x01, 0x00]),
    );

    assert!(decode_request(&frame).is_err());
}

fn assert_request_round_trip(command: Command, expected_opcode: Opcode) {
    // Command -> Frame
    let frame = encode_request(&command).unwrap();
    assert_eq!(frame.opcode, expected_opcode);

    // Frame -> wire bytes
    let wire_bytes = encode_frame(&frame).unwrap();

    // Wire bytes -> Frame -> Command
    let received_frame = decode_frame(wire_bytes).unwrap();
    let received_command = decode_request(&received_frame).unwrap();

    assert_eq!(received_command, command);
}

#[test]
fn get_request_preserves_binary_key() {
    let command = Command::Get {
        key: Bytes::from_static(b"user:\x00\xFF\x10"),
    };

    assert_request_round_trip(command, Opcode::Get);
}

#[test]
fn set_request_preserves_binary_value_and_ttl() {
    let command = Command::Set {
        key: Bytes::from_static(b"user:42"),
        value: Bytes::from_static(b"\x00\xFF\x10\xAB"),
        ttl: Some(Duration::from_secs(60)),
    };

    assert_request_round_trip(command, Opcode::Set);
}

#[test]
fn set_request_preserves_absence_of_ttl() {
    let command = Command::Set {
        key: Bytes::from_static(b"session"),
        value: Bytes::from_static(b"active"),
        ttl: None,
    };

    assert_request_round_trip(command, Opcode::Set);
}

#[test]
fn del_request_preserves_key() {
    let command = Command::Del {
        key: Bytes::from_static(b"cache:\x00\xFF"),
    };

    assert_request_round_trip(command, Opcode::Del);
}

#[test]
fn exists_request_preserves_key() {
    let command = Command::Exists {
        key: Bytes::from_static(b"session:42"),
    };

    assert_request_round_trip(command, Opcode::Exists);
}
