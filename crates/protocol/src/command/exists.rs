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
