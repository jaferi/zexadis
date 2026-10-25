//! Verify that command results survive response encoding and decoding, including edge cases and ordered batch results.

use bytes::Bytes;

use zexadis_core::commands::CommandResult;
use zexadis_protocol::command::{decode_response, encode_response};
use zexadis_protocol::encoding::{decode_frame, encode_frame};
use zexadis_protocol::Opcode;

fn assert_response_round_trip(result: CommandResult, expected_opcode: Opcode) {
    // Result -> response Frame
    let frame = encode_response(&result).unwrap();
    assert_eq!(frame.opcode, expected_opcode);

    // Response Frame -> wire bytes
    let wire_bytes = encode_frame(&frame).unwrap();

    // Wire bytes -> Frame -> Result
    let received_frame = decode_frame(wire_bytes).unwrap();
    let received_result = decode_response(&received_frame).unwrap();

    assert_eq!(received_result, result);
}

#[test]
fn get_response_preserves_binary_value() {
    let result = CommandResult::Entry(Bytes::from_static(b"\x00\xFF\x10\xAB\x00"));

    assert_response_round_trip(result, Opcode::GetResponse);
}

#[test]
fn get_response_distinguishes_missing_entry_from_empty_value() {
    assert_response_round_trip(CommandResult::NotFound, Opcode::GetResponse);

    assert_response_round_trip(CommandResult::Entry(Bytes::new()), Opcode::GetResponse);
}

#[test]
fn set_response_preserves_success_and_failure() {
    assert_response_round_trip(CommandResult::Set(true), Opcode::SetResponse);
    assert_response_round_trip(CommandResult::Set(false), Opcode::SetResponse);
}

#[test]
fn exists_response_preserves_both_outcomes() {
    assert_response_round_trip(CommandResult::Exists(true), Opcode::ExistsResponse);
    assert_response_round_trip(CommandResult::Exists(false), Opcode::ExistsResponse);
}

#[test]
fn del_response_preserves_both_outcomes() {
    assert_response_round_trip(CommandResult::Deleted(true), Opcode::DelResponse);
    assert_response_round_trip(CommandResult::Deleted(false), Opcode::DelResponse);
}
