//! GET command definition, request, response, encoder and decoder.

use bytes::{BufMut, BytesMut};
use std::time::Duration;

use crate::{CommandError, Frame, Opcode, ProtocolVersion};
use zexadis_core::commands::{Command, CommandResult};

pub fn encode_request(command: &Command) -> Result<Frame, CommandError> {
    let Command::Set { key, value, ttl } = command else {
        return Err(CommandError::UnsupportedCommand);
    };

    let key_length = u32::try_from(key.len()).map_err(|_| CommandError::InvalidPayload)?;
    let value_length = u32::try_from(value.len()).map_err(|_| CommandError::InvalidPayload)?;

    let ttl_flag = if ttl.is_some() { 1 } else { 0 };
    let ttl_len = if ttl_flag == 1 { 8 } else { 0 };

    let mut payload = BytesMut::with_capacity(4 + 4 + 1 + ttl_len + key.len() + 4 + value.len());

    payload.put_u32(key_length);
    payload.put_u32(value_length);
    payload.put_u8(ttl_flag);

    if let Some(ttl) = ttl {
        let ttl_ms = u64::try_from(ttl.as_millis()).map_err(|_| CommandError::InvalidPayload)?;

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

pub fn decode_request(frame: &Frame) -> Result<Command, CommandError> {
    if frame.opcode != Opcode::Set {
        return Err(CommandError::UnexpectedOpcode(frame.opcode));
    }

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

pub fn encode_response(result: &CommandResult) -> Result<Frame, CommandError> {
    let mut payload = BytesMut::new();

    match result {
        CommandResult::Set(success) => {
            payload.put_u8(if *success { 0x01 } else { 0x00 });
        }

        _ => return Err(CommandError::UnsupportedCommand),
    };

    Ok(Frame::new(
        ProtocolVersion::V1,
        Opcode::SetResponse,
        payload.freeze(),
    ))
}

pub fn decode_response(frame: &Frame) -> Result<CommandResult, CommandError> {
    if frame.opcode != Opcode::SetResponse {
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

    Ok(CommandResult::Set(success))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

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
}
