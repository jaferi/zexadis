//! Protocol command definitions and command encoding identifiers.

use bytes::{BufMut, BytesMut};
use std::time::Duration;
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

        Command::Set { key, value, ttl } => {
            let key_length = u32::try_from(key.len()).map_err(|_| CommandError::InvalidPayload)?;
            let value_length =
                u32::try_from(value.len()).map_err(|_| CommandError::InvalidPayload)?;

            let ttl_flag = if ttl.is_some() { 1 } else { 0 };
            let ttl_len = if ttl_flag == 1 { 8 } else { 0 };

            let mut payload =
                BytesMut::with_capacity(4 + 4 + 1 + ttl_len + key.len() + 4 + value.len());

            payload.put_u32(key_length);
            payload.put_u32(value_length);
            payload.put_u8(ttl_flag);

            if let Some(ttl) = ttl {
                let ttl_ms =
                    u64::try_from(ttl.as_millis()).map_err(|_| CommandError::InvalidPayload)?;

                if ttl_ms == 0 {
                    return Err(CommandError::InvalidPayload);
                }

                payload.put_u64(ttl_ms);
            }

            payload.extend_from_slice(key);
            payload.extend_from_slice(value);

            Ok(Frame::new(
                ProtocolVersion::V1,
                Opcode::Set,
                payload.freeze(),
            ))
        }
        _ => Err(CommandError::UnsupportedCommand),
    }
}

pub fn decode_request(frame: &Frame) -> Result<Command, CommandError> {
    match frame.opcode {
        Opcode::Get => {
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
        Opcode::Set => {
            if frame.payload.len() < 9 {
                return Err(CommandError::InvalidPayload);
            }

            let key_length = u32::from_be_bytes(
                frame.payload[..4]
                    .try_into()
                    .map_err(|_| CommandError::InvalidPayload)?,
            ) as usize;

            let value_length = u32::from_be_bytes(
                frame.payload[4..8]
                    .try_into()
                    .map_err(|_| CommandError::InvalidPayload)?,
            ) as usize;

            let ttl_flag: u8 = frame.payload[8];
            match ttl_flag {
                0 => {
                    let prefix_len = 4 + 4 + 1; // key length, value length, ttl flag

                    if frame.payload.len() - prefix_len != key_length + value_length {
                        return Err(CommandError::InvalidPayload);
                    }

                    Ok(Command::Set {
                        key: frame.payload.slice(prefix_len..prefix_len + key_length),
                        value: frame.payload.slice(prefix_len + key_length..),
                        ttl: None,
                    })
                }

                1 => {
                    let prefix_len = 4 + 4 + 1 + 8; // key length, value length, ttl flag, ttl value

                    if frame.payload.len() - prefix_len != key_length + value_length {
                        return Err(CommandError::InvalidPayload);
                    }

                    Ok(Command::Set {
                        key: frame.payload.slice(prefix_len..prefix_len + key_length),
                        value: frame.payload.slice(prefix_len + key_length..),
                        ttl: Some(Duration::from_millis(u64::from_be_bytes(
                            frame.payload[9..17]
                                .try_into()
                                .map_err(|_| CommandError::InvalidPayload)?,
                        ))),
                    })
                }

                _ => Err(CommandError::InvalidPayload),
            }
        }
        _ => Err(CommandError::UnexpectedOpcode(frame.opcode)),
    }
}

pub fn encode_response(result: &CommandResult) -> Result<Frame, CommandError> {
    let mut payload = BytesMut::new();

    let opcode: Opcode = match result {
        CommandResult::NotFound => {
            payload.put_u8(0x00);
            Opcode::GetResponse
        }

        CommandResult::Set(success) => {
            payload.put_u8(if *success { 0x01 } else { 0x00 });
            Opcode::SetResponse
        }

        CommandResult::Entry(value) => {
            let length = u32::try_from(value.len()).map_err(|_| CommandError::InvalidPayload)?;

            payload.put_u8(0x01);
            payload.put_u32(length);
            payload.extend_from_slice(value);

            Opcode::GetResponse
        }
        _ => return Err(CommandError::UnsupportedCommand),
    };

    Ok(Frame::new(ProtocolVersion::V1, opcode, payload.freeze()))
}

pub fn decode_response(frame: &Frame) -> Result<CommandResult, CommandError> {
    match frame.opcode {
        Opcode::SetResponse => {
            if frame.payload.len() != 1 {
                return Err(CommandError::InvalidPayload);
            }

            let success = match frame.payload[0] {
                0x00 => false,
                0x01 => true,
                _ => return Err(CommandError::InvalidPayload),
            };

            Ok(CommandResult::Set(success))
        }

        Opcode::GetResponse => {
            if frame.payload.is_empty() {
                return Err(CommandError::InvalidPayload);
            }

            let status = frame.payload[0];

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
        _ => Err(CommandError::UnexpectedOpcode(frame.opcode)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

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
    fn set_request_round_trip() {
        let command = Command::Set {
            key: Bytes::from_static(b"my-key"),
            value: Bytes::from_static(b"my-value"),
            ttl: Some(Duration::from_millis(6000)),
        };

        let frame = encode_request(&command).unwrap();
        let decoded = decode_request(&frame).unwrap();

        assert_eq!(decoded, command);
        assert_eq!(frame.opcode, Opcode::Set);
    }

    #[test]
    fn set_request_without_ttl_round_trip() {
        let command = Command::Set {
            key: Bytes::from_static(b"key"),
            value: Bytes::from_static(b"value"),
            ttl: None,
        };

        let frame = encode_request(&command).unwrap();

        assert_eq!(frame.opcode, Opcode::Set);
        assert_eq!(decode_request(&frame).unwrap(), command);
    }

    #[test]
    fn set_request_with_empty_value_round_trip() {
        let command = Command::Set {
            key: Bytes::from_static(b"key"),
            value: Bytes::new(),
            ttl: None,
        };

        let frame = encode_request(&command).unwrap();

        assert_eq!(decode_request(&frame).unwrap(), command);
    }

    #[test]
    fn set_request_with_binary_data_round_trip() {
        let command = Command::Set {
            key: Bytes::from_static(b"\x00\xFF\x10"),
            value: Bytes::from_static(b"\xFE\x00\xAB\xFF"),
            ttl: Some(Duration::from_millis(6000)),
        };

        let frame = encode_request(&command).unwrap();

        assert_eq!(decode_request(&frame).unwrap(), command);
    }

    #[test]
    fn set_request_rejects_invalid_ttl_flag() {
        let mut payload = BytesMut::new();

        payload.put_u32(1); // key length
        payload.put_u32(1); // value length
        payload.put_u8(2); // invalid TTL flag
        payload.extend_from_slice(b"kv");

        let frame = Frame::new(ProtocolVersion::V1, Opcode::Set, payload.freeze());

        assert!(matches!(
            decode_request(&frame),
            Err(CommandError::InvalidPayload)
        ));
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
