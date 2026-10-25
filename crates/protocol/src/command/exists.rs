//! EXISTS command definition, request, response, encoder and decoder.

use bytes::{BufMut, BytesMut};

use crate::{CommandError, Frame, Opcode, ProtocolVersion};
use zexadis_core::commands::{Command, CommandResult};

pub fn encode_request(command: &Command) -> Result<Frame, CommandError> {
    let Command::Exists { key } = command else {
        return Err(CommandError::UnsupportedCommand);
    };

    let length = u32::try_from(key.len()).map_err(|_| CommandError::InvalidPayload)?;

    let mut payload = BytesMut::with_capacity(4 + key.len());
    payload.put_u32(length);
    payload.extend_from_slice(key);

    Ok(Frame::new(
        ProtocolVersion::V1,
        Opcode::Exists,
        payload.freeze(),
    ))
}

pub fn decode_request(frame: &Frame) -> Result<Command, CommandError> {
    if frame.opcode != Opcode::Exists {
        return Err(CommandError::UnexpectedOpcode(frame.opcode));
    }

    if frame.payload.len() < 4 {
        return Err(CommandError::InvalidPayload);
    }

    let length = u32::from_be_bytes(
        frame.payload[..4]
            .try_into()
            .map_err(|_| CommandError::InvalidPayload)?,
    ) as usize;

    if frame.payload.len() - 4 != length {
        return Err(CommandError::InvalidPayload);
    }

    Ok(Command::Exists {
        key: frame.payload.slice(4..),
    })
}

pub fn encode_response(result: &CommandResult) -> Result<Frame, CommandError> {
    let mut payload = BytesMut::new();

    match result {
        CommandResult::Exists(success) => {
            payload.put_u8(if *success { 0x01 } else { 0x00 });
        }
        _ => return Err(CommandError::UnsupportedCommand),
    };

    Ok(Frame::new(
        ProtocolVersion::V1,
        Opcode::ExistsResponse,
        payload.freeze(),
    ))
}

pub fn decode_response(frame: &Frame) -> Result<CommandResult, CommandError> {
    if frame.opcode != Opcode::ExistsResponse {
        return Err(CommandError::UnexpectedOpcode(frame.opcode));
    }

    if frame.payload.len() != 1 {
        return Err(CommandError::InvalidPayload);
    }

    let success = match frame.payload[0] {
        0x00 => false,
        0x01 => true,
        _ => return Err(CommandError::InvalidPayload),
    };

    Ok(CommandResult::Exists(success))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn exists_request_round_trip() {
        let command = Command::Exists {
            key: Bytes::from_static(b"my-key"),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
        assert_eq!(frame.opcode, Opcode::Exists);
    }

    #[test]
    fn exists_request_decodes_key_without_copying() {
        let command = Command::Exists {
            key: Bytes::from_static(b"my-key"),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        let Command::Exists { key } = decoded else {
            panic!("expected EXISTS command");
        };

        assert_eq!(key.as_ptr(), frame.payload.slice(4..).as_ptr());
    }

    #[test]
    fn exists_request_rejects_truncated_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Exists,
            Bytes::from_static(&[0x00, 0x00]),
        );

        assert_eq!(decode_request(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_request_rejects_incorrect_key_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Exists,
            Bytes::from_static(&[0x00, 0x00, 0x00, 0x05, b'a', b'b']),
        );

        assert_eq!(decode_request(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_request_supports_binary_key() {
        let command = Command::Exists {
            key: Bytes::from_static(&[0x00, 0xFF, 0x80, 0x01]),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn exists_request_supports_empty_key() {
        let command = Command::Exists { key: Bytes::new() };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn exists_response_round_trip_not_found() {
        let result = CommandResult::Exists(false);

        let frame = encode_response(&result).unwrap();
        let decoded = decode_response(&frame).unwrap();

        assert_eq!(decoded, result);
        assert_eq!(frame.payload.as_ref(), &[0x00]);
    }

    #[test]
    fn exists_response_rejects_truncated_value() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::ExistsResponse,
            Bytes::from_static(&[0x01, 0, 0, 0, 5, b'a']),
        );

        assert_eq!(decode_response(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_request_rejects_wrong_opcode() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::from_static(&[0x00, 0x00, 0x00, 0x01, b'a']),
        );

        assert_eq!(
            decode_request(&frame),
            Err(CommandError::UnexpectedOpcode(Opcode::Get))
        );
    }

    #[test]
    fn exists_request_rejects_trailing_bytes() {
        // Declares a one-byte key, but contains two key bytes.
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Exists,
            Bytes::from_static(&[0x00, 0x00, 0x00, 0x01, b'a', b'b']),
        );

        assert_eq!(decode_request(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_request_encoder_rejects_unsupported_command() {
        let command = Command::Get {
            key: Bytes::from_static(b"my-key"),
        };

        assert_eq!(
            encode_request(&command),
            Err(CommandError::UnsupportedCommand)
        );
    }

    #[test]
    fn exists_response_round_trip_success() {
        let result = CommandResult::Exists(true);

        let frame = encode_response(&result).unwrap();
        let decoded = decode_response(&frame).unwrap();

        assert_eq!(decoded, result);
        assert_eq!(frame.opcode, Opcode::ExistsResponse);
        assert_eq!(frame.payload.as_ref(), &[0x01]);
    }

    #[test]
    fn exists_response_encoder_rejects_unsupported_result() {
        let result = CommandResult::NotFound;

        assert_eq!(
            encode_response(&result),
            Err(CommandError::UnsupportedCommand)
        );
    }

    #[test]
    fn exists_response_rejects_empty_payload() {
        let frame = Frame::new(ProtocolVersion::V1, Opcode::ExistsResponse, Bytes::new());

        assert_eq!(decode_response(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_response_rejects_unknown_status() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::ExistsResponse,
            Bytes::from_static(&[0x02]),
        );

        assert_eq!(decode_response(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_response_rejects_trailing_bytes() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::ExistsResponse,
            Bytes::from_static(&[0x01, 0x00]),
        );

        assert_eq!(decode_response(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn exists_response_rejects_wrong_opcode() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::GetResponse,
            Bytes::from_static(&[0x01]),
        );

        assert_eq!(
            decode_response(&frame),
            Err(CommandError::UnexpectedOpcode(Opcode::GetResponse))
        );
    }
}
