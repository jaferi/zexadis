//! Protocol command definitions and command encoding identifiers.

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::framing::{Frame, FrameError, Opcode, ProtocolVersion};

use zexadis_core::commands::Command;

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
            let length =
                u32::try_from(key.len()).map_err(|_| CommandError::InvalidPayload)?;

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

    if frame.payload.len() != 4 + length {
        return Err(CommandError::InvalidPayload);
    }

    Ok(Command::Get {
        key: frame.payload.slice(4..),
    })
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

        assert_eq!(
            decode_request(&frame),
            Err(CommandError::InvalidPayload)
        );
    }

    #[test]
    fn get_request_rejects_incorrect_key_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::from_static(&[
                0x00, 0x00, 0x00, 0x05, b'a', b'b',
            ]),
        );

        assert_eq!(
            decode_request(&frame),
            Err(CommandError::InvalidPayload)
        );
    }

    #[test]
    fn get_request_rejects_wrong_opcode() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Set,
            Bytes::new(),
        );

        assert_eq!(
            decode_request(&frame),
            Err(CommandError::UnexpectedOpcode(Opcode::Set))
        );
    }
}
