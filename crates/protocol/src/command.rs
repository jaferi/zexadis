//! Protocol command definitions and command encoding identifiers.

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::framing::{Frame, FrameError, Opcode, ProtocolVersion};
use zexadis_core::commands::{Command, CommandResult};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("unsupported command")]
    UnsupportedCommand,

    #[error("invalid command payload")]
    InvalidPayload,

    #[error("unexpected opcode: {0:?}")]
    UnexpectedOpcode(Opcode),

    #[error(transparent)]
    Frame(#[from] FrameError),
}

pub fn encode_request(command: &Command) -> Result<Frame, CommandError> {
    match command {
        Command::Get { key } => {
            let length = u32::try_from(key.len()).map_err(|_| CommandError::InvalidPayload)?;

            let mut payload = BytesMut::with_capacity(4 + key.len());
            payload.put_u32(length);
            payload.extend_from_slice(key);

            Ok(Frame::new(
                ProtocolVersion::V1,
                Opcode::Get,
                payload.freeze(),
            ))
        }
        _ => Err(CommandError::UnsupportedCommand),
    }
}

pub fn decode_request(frame: &Frame) -> Result<Command, CommandError> {
    if frame.opcode != Opcode::Get {
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

    Ok(Command::Get {
        key: frame.payload.slice(4..),
    })
}

pub fn encode_response(result: &CommandResult) -> Result<Frame, CommandError> {
    let mut payload = BytesMut::new();

    match result {
        CommandResult::NotFound => payload.put_u8(0x00),
        CommandResult::Entry(value) => {
            let length = u32::try_from(value.len()).map_err(|_| CommandError::InvalidPayload)?;

            payload.put_u8(0x01);
            payload.put_u32(length);
            payload.extend_from_slice(value);
        }
        _ => return Err(CommandError::UnsupportedCommand),
    }

    Ok(Frame::new(
        ProtocolVersion::V1,
        Opcode::GetResponse,
        payload.freeze(),
    ))
}

pub fn decode_response(frame: &Frame) -> Result<CommandResult, CommandError> {
    if frame.opcode != Opcode::GetResponse {
        return Err(CommandError::UnexpectedOpcode(frame.opcode));
    }

    let Some(&status) = frame.payload.first() else {
        return Err(CommandError::InvalidPayload);
    };

    match status {
        0x00 if frame.payload.len() == 1 => Ok(CommandResult::NotFound),
        0x01 if frame.payload.len() >= 5 => {
            let length = u32::from_be_bytes(
                frame.payload[1..5]
                    .try_into()
                    .map_err(|_| CommandError::InvalidPayload)?,
            ) as usize;

            if frame.payload.len() - 5 != length {
                return Err(CommandError::InvalidPayload);
            }

            Ok(CommandResult::Entry(frame.payload.slice(5..)))
        }
        _ => Err(CommandError::InvalidPayload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_request_round_trip() {
        let command = Command::Get {
            key: Bytes::from_static(b"my-key"),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
        assert_eq!(frame.opcode, Opcode::Get);
    }

    #[test]
    fn get_request_decodes_key_without_copying() {
        let command = Command::Get {
            key: Bytes::from_static(b"my-key"),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        let Command::Get { key } = decoded else {
            panic!("expected GET command");
        };

        assert_eq!(key.as_ptr(), frame.payload.slice(4..).as_ptr());
    }

    #[test]
    fn get_request_rejects_truncated_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::from_static(&[0x00, 0x00]),
        );

        assert_eq!(decode_request(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn get_request_rejects_incorrect_key_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::from_static(&[0x00, 0x00, 0x00, 0x05, b'a', b'b']),
        );

        assert_eq!(decode_request(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn get_request_rejects_wrong_opcode() {
        let frame = Frame::new(ProtocolVersion::V1, Opcode::Set, Bytes::new());

        assert_eq!(
            decode_request(&frame),
            Err(CommandError::UnexpectedOpcode(Opcode::Set))
        );
    }

    #[test]
    fn get_request_supports_binary_key() {
        let command = Command::Get {
            key: Bytes::from_static(&[0x00, 0xFF, 0x80, 0x01]),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn get_request_supports_empty_key() {
        let command = Command::Get { key: Bytes::new() };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn get_response_round_trip_with_value() {
        let result = CommandResult::Entry(Bytes::from_static(b"value"));

        let frame = encode_response(&result).unwrap();
        let decoded = decode_response(&frame).unwrap();

        assert_eq!(decoded, result);
        assert_eq!(frame.opcode, Opcode::GetResponse);
    }

    #[test]
    fn get_response_round_trip_not_found() {
        let result = CommandResult::NotFound;

        let frame = encode_response(&result).unwrap();
        let decoded = decode_response(&frame).unwrap();

        assert_eq!(decoded, result);
        assert_eq!(frame.payload.as_ref(), &[0x00]);
    }

    #[test]
    fn get_response_supports_empty_value() {
        let result = CommandResult::Entry(Bytes::new());

        let frame = encode_response(&result).unwrap();
        let decoded = decode_response(&frame).unwrap();

        assert_eq!(decoded, result);
        assert_eq!(frame.payload.as_ref(), &[0x01, 0, 0, 0, 0]);
    }

    #[test]
    fn get_response_rejects_truncated_value() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::GetResponse,
            Bytes::from_static(&[0x01, 0, 0, 0, 5, b'a']),
        );

        assert_eq!(decode_response(&frame), Err(CommandError::InvalidPayload));
    }

    #[test]
    fn get_response_rejects_wrong_opcode() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::from_static(&[0x00]),
        );

        assert_eq!(
            decode_response(&frame),
            Err(CommandError::UnexpectedOpcode(Opcode::Get))
        );
    }
}
