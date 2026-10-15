//! Binary encoding and decoding utilities for protocol data.

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::framing::{
    Frame, FrameError, Opcode, ProtocolVersion,
    DEFAULT_MAX_FRAME_LENGTH, FRAME_HEADER_SIZE, FRAME_LENGTH_SIZE,
};

pub fn encode_frame(frame: &Frame) -> Result<Bytes, FrameError> {
    frame.validate(DEFAULT_MAX_FRAME_LENGTH)?;

    let frame_length = frame.frame_length();
    let frame_length =
        u32::try_from(frame_length).map_err(|_| FrameError::FrameLengthOverflow)?;

    let mut output = BytesMut::with_capacity(
        FRAME_LENGTH_SIZE + frame.frame_length(),
    );

    output.put_u32(frame_length);
    output.put_u8(frame.version as u8);
    output.put_u8(frame.opcode as u8);
    output.extend_from_slice(&frame.payload);

    Ok(output.freeze())
}

pub fn decode_frame(input: &[u8]) -> Result<Frame, DecodeError> {
    if input.len() < FRAME_LENGTH_SIZE {
        return Err(DecodeError::Incomplete);
    }

    let declared_length = u32::from_be_bytes([
        input[0], 
        input[1],
        input[2],
        input[3],
    ]) as usize;

    if declared_length < FRAME_HEADER_SIZE {
        return Err(FrameError::InvalidFrameLength(declared_length).into());
    }

    if declared_length > DEFAULT_MAX_FRAME_LENGTH {
        return Err(FrameError::FrameTooLarge {
            actual: declared_length,
            maximum: DEFAULT_MAX_FRAME_LENGTH,
        }
        .into());
    }

    let total_length = FRAME_LENGTH_SIZE + declared_length;

    if input.len() < total_length {
        return Err(DecodeError::Incomplete);
    }

    if input.len() > total_length {
        return Err(DecodeError::TrailingBytes);
    }

    let version = ProtocolVersion::try_from(input[4])?;
    let opcode = Opcode::try_from(input[5])?;
    let payload = Bytes::copy_from_slice(&input[6..total_length]);

    Ok(Frame::new(version, opcode, payload))
}


#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("incomplete frame")]
    Incomplete,

    #[error("frame contains trailing bytes")]
    TrailingBytes,

    #[error(transparent)]
    Frame(#[from] FrameError),
}
