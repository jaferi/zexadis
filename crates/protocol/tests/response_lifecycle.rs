//! Verify that command results survive response encoding and decoding, including edge cases and ordered batch results.

use std::time::Duration;

use bytes::Bytes;

use zexadis_core::commands::{Command, CommandResult};
use zexadis_protocol::command::{decode_request, decode_response, encode_request, encode_response};
use zexadis_protocol::encoding::{decode_frame, encode_frame};
use zexadis_protocol::{Frame, Opcode, ProtocolVersion};

#[test]
fn get_response_preserves_binary_value() {
    let result = CommandResult::Entry(Bytes::from_static(b"\x00\xFF\x10\xAB\x00"));

    // Result -> response Frame
    let frame = encode_response(&result).unwrap();
    assert_eq!(frame.opcode, Opcode::GetResponse);

    // Response Frame -> bytes -> Frame
    let wire_bytes = encode_frame(&frame).unwrap();
    let decoded_frame = decode_frame(wire_bytes).unwrap();

    // Frame -> Result
    let decoded_result = decode_response(&decoded_frame).unwrap();

    assert_eq!(decoded_result, result);
}
