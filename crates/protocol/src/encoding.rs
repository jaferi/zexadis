//! Binary encoding and decoding utilities for protocol data.

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

use crate::framing::{
    Frame, FrameError, Opcode, ProtocolVersion, DEFAULT_MAX_FRAME_LENGTH, FRAME_HEADER_SIZE,
    FRAME_LENGTH_SIZE,
};

pub fn encode_frame(frame: &Frame) -> Result<Bytes, FrameError> {
    frame.validate(DEFAULT_MAX_FRAME_LENGTH)?;

    let frame_length = frame.frame_length();
    let frame_length = u32::try_from(frame_length).map_err(|_| FrameError::FrameLengthOverflow)?;

    let mut output = BytesMut::with_capacity(FRAME_LENGTH_SIZE + frame.frame_length());

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

    let declared_length = u32::from_be_bytes([input[0], input[1], input[2], input[3]]) as usize;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn test_frame(opcode: Opcode, payload: &[u8]) -> Frame {
        Frame::new(ProtocolVersion::V1, opcode, Bytes::copy_from_slice(payload))
    }

    #[test]
    fn test_encode_decode_round_trip() {
        let frame = test_frame(Opcode::Set, b"hello");

        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();

        assert_eq!(decoded, frame);
    }

    #[test]
    fn test_round_trip_binary_payload() {
        let payload = [0x00, 0xFF, 0x80, 0x01, 0x00];
        let frame = test_frame(Opcode::Set, &payload);

        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();

        assert_eq!(decoded.payload.as_ref(), payload);
    }

    #[test]
    fn test_empty_payload() {
        let frame = test_frame(Opcode::Get, &[]);
        let encoded = encode_frame(&frame).unwrap();

        assert_eq!(encoded.len(), FRAME_LENGTH_SIZE + FRAME_HEADER_SIZE);
        assert_eq!(
            u32::from_be_bytes(encoded[..4].try_into().unwrap()),
            FRAME_HEADER_SIZE as u32
        );

        assert_eq!(decode_frame(&encoded).unwrap(), frame);
    }

    #[test]
    fn test_minimum_valid_frame_length() {
        let frame = test_frame(Opcode::Get, &[]);
        let encoded = encode_frame(&frame).unwrap();

        assert_eq!(u32::from_be_bytes(encoded[..4].try_into().unwrap()), 2);
        assert!(decode_frame(&encoded).is_ok());
    }

    #[test]
    fn test_reject_frame_length_smaller_than_header() {
        let input = [0x00, 0x00, 0x00, 0x01, 0x01, 0x01];

        assert_eq!(
            decode_frame(&input),
            Err(DecodeError::Frame(FrameError::InvalidFrameLength(1)))
        );
    }

    #[test]
    fn test_reject_zero_frame_length() {
        let input = [0x00, 0x00, 0x00, 0x00];

        assert_eq!(
            decode_frame(&input),
            Err(DecodeError::Frame(FrameError::InvalidFrameLength(0)))
        );
    }

    #[test]
    fn test_maximum_frame_length_is_accepted() {
        let payload = vec![0xAB; DEFAULT_MAX_FRAME_LENGTH - FRAME_HEADER_SIZE];
        let frame = test_frame(Opcode::Set, &payload);

        let encoded = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&encoded).unwrap();

        assert_eq!(decoded.payload.len(), payload.len());
        assert_eq!(decoded, frame);
    }

    #[test]
    fn test_oversized_frame_is_rejected_during_encoding() {
        let payload = vec![0xAB; DEFAULT_MAX_FRAME_LENGTH - FRAME_HEADER_SIZE + 1];
        let frame = test_frame(Opcode::Set, &payload);

        assert!(matches!(
            encode_frame(&frame),
            Err(FrameError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn test_oversized_declared_length_is_rejected_before_body_read() {
        let oversized = (DEFAULT_MAX_FRAME_LENGTH as u32 + 1).to_be_bytes();
        let input = oversized;

        assert!(matches!(
            decode_frame(&input),
            Err(DecodeError::Frame(FrameError::FrameTooLarge { .. }))
        ));
    }

    #[test]
    fn test_partial_length_prefix_returns_incomplete() {
        for length in 0..FRAME_LENGTH_SIZE {
            let input = vec![0; length];

            assert_eq!(decode_frame(&input), Err(DecodeError::Incomplete));
        }
    }

    #[test]
    fn test_partial_payload_returns_incomplete() {
        let frame = test_frame(Opcode::Set, b"hello");
        let encoded = encode_frame(&frame).unwrap();

        for end in (FRAME_LENGTH_SIZE + FRAME_HEADER_SIZE)..(encoded.len()) {
            assert_eq!(decode_frame(&encoded[..end]), Err(DecodeError::Incomplete));
        }
    }

    #[test]
    fn test_unsupported_version_is_rejected() {
        let mut encoded = encode_frame(&test_frame(Opcode::Get, b"key"))
            .unwrap()
            .to_vec();

        encoded[4] = 2;

        assert_eq!(
            decode_frame(&encoded),
            Err(DecodeError::Frame(FrameError::UnsupportedVersion(2)))
        );
    }

    #[test]
    fn test_unknown_opcode_is_rejected() {
        let mut encoded = encode_frame(&test_frame(Opcode::Get, b"key"))
            .unwrap()
            .to_vec();

        encoded[5] = 0x07;

        assert_eq!(
            decode_frame(&encoded),
            Err(DecodeError::Frame(FrameError::UnknownOpcode(0x07)))
        );
    }

    #[test]
    fn test_trailing_bytes_are_rejected() {
        let frame = test_frame(Opcode::Get, b"key");
        let mut encoded = encode_frame(&frame).unwrap().to_vec();

        encoded.extend_from_slice(&[0xAA]);

        assert_eq!(decode_frame(&encoded), Err(DecodeError::TrailingBytes));
    }

    #[test]
    fn test_multiple_frames_can_be_decoded_from_one_stream() {
        let first = test_frame(Opcode::Get, b"key-1");
        let second = test_frame(Opcode::Set, b"value-2");

        let first_encoded = encode_frame(&first).unwrap();
        let second_encoded = encode_frame(&second).unwrap();

        let mut stream = Vec::new();
        stream.extend_from_slice(&first_encoded);
        stream.extend_from_slice(&second_encoded);

        // decode_frame consumes exactly one frame.
        // use the declared length to advance.
        let first_decoded = decode_frame(&stream[..first_encoded.len()]).unwrap();
        let second_decoded = decode_frame(&stream[first_encoded.len()..]).unwrap();

        assert_eq!(first_decoded, first);
        assert_eq!(second_decoded, second);
    }
}
