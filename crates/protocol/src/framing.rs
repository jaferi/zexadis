//! Binary request and response framing for the Zexadis protocol.

use thiserror::Error;
use bytes::Bytes;

/// The number of bytes used to encode the frame length.
pub const FRAME_LENGTH_SIZE: usize = 4;

/// The size of the version and opcode fields.
pub const FRAME_HEADER_SIZE: usize = 2;

/// Default maximum frame length, excluding the four-byte length prefix.
pub const DEFAULT_MAX_FRAME_LENGTH: usize = 8 * 1024 * 1024;

/// The wire protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtocolVersion {
    V1 = 1,
}

impl TryFrom<u8> for ProtocolVersion {
    type Error = FrameError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            version => Err(FrameError::UnsupportedVersion(version)),
        }
    }
}

/// Identifies a request or response message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // Request opcodes
    Get = 0x01,
    Set = 0x02,
    Del = 0x03,
    Exists = 0x04,
    Cas = 0x05,
    Batch = 0x06,

    // Response opcodes
    GetResponse = 0x81,
    SetResponse = 0x82,
    DelResponse = 0x83,
    ExistsResponse = 0x84,
    CasResponse = 0x85,
    BatchResponse = 0x86,

    ErrorResponse = 0xFF,
}

impl TryFrom<u8> for Opcode {
    type Error = FrameError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Get),
            0x02 => Ok(Self::Set),
            0x03 => Ok(Self::Del),
            0x04 => Ok(Self::Exists),
            0x05 => Ok(Self::Cas),
            0x06 => Ok(Self::Batch),
            0x81 => Ok(Self::GetResponse),
            0x82 => Ok(Self::SetResponse),
            0x83 => Ok(Self::DelResponse),
            0x84 => Ok(Self::ExistsResponse),
            0x85 => Ok(Self::CasResponse),
            0x86 => Ok(Self::BatchResponse),
            0xFF => Ok(Self::ErrorResponse),
            opcode => Err(FrameError::UnknownOpcode(opcode)),
        }
    }
}

/// A complete protocol frame, excluding its length prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub version: ProtocolVersion,
    pub opcode: Opcode,
    pub payload: Bytes,
}

impl Frame {
    pub fn new(
        version: ProtocolVersion,
        opcode: Opcode,
        payload: Bytes,
    ) -> Self {
        Self {
            version,
            opcode,
            payload,
        }
    }

    /// Length encoded in the wire-format prefix.
    ///
    /// Includes the version, opcode, and payload, but excludes
    /// the four-byte length prefix itself.
    pub fn frame_length(&self) -> usize {
        FRAME_HEADER_SIZE + self.payload.len()
    }

    /// Validates the frame against the supplied maximum frame length.
    pub fn validate(&self, max_frame_length: usize) -> Result<(), FrameError> {
        let length = self.frame_length();

        if length > max_frame_length {
            return Err(FrameError::FrameTooLarge {
                actual: length,
                maximum: max_frame_length,
            });
        }

        if length > u32::MAX as usize {
            return Err(FrameError::FrameLengthOverflow);
        }

        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u8),

    #[error("unknown opcode: {0:#04x}")]
    UnknownOpcode(u8),

    #[error("frame length {actual} exceeds maximum {maximum}")]
    FrameTooLarge {
        actual: usize,
        maximum: usize,
    },

    #[error("frame length exceeds the wire format's u32 limit")]
    FrameLengthOverflow,

    #[error("frame minimum length not satisfied {0}")]
    InvalidFrameLength(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_conversion() {
        assert_eq!(
            ProtocolVersion::try_from(1),
            Ok(ProtocolVersion::V1)
        );

        assert_eq!(
            ProtocolVersion::try_from(2),
            Err(FrameError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn test_opcode_conversion() {
        assert_eq!(Opcode::try_from(0x01), Ok(Opcode::Get));
        assert_eq!(
            Opcode::try_from(0xFF),
            Ok(Opcode::ErrorResponse)
        );
        assert_eq!(
            Opcode::try_from(0x07),
            Err(FrameError::UnknownOpcode(0x07))
        );
    }

    #[test]
    fn test_frame_length_includes_header_and_payload() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Set,
            Bytes::from_static(b"value"),
        );

        assert_eq!(frame.frame_length(), 7);
    }

    #[test]
    fn test_empty_payload_has_minimum_frame_length() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Get,
            Bytes::new(),
        );

        assert_eq!(frame.frame_length(), FRAME_HEADER_SIZE);
        assert!(frame.validate(DEFAULT_MAX_FRAME_LENGTH).is_ok());
    }

    #[test]
    fn test_frame_within_maximum_length_is_valid() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Set,
            Bytes::from(vec![0; 6]),
        );

        assert!(frame.validate(8).is_ok());
    }

    #[test]
    fn test_frame_exceeding_maximum_length_is_rejected() {
        let frame = Frame::new(
            ProtocolVersion::V1,
            Opcode::Set,
            Bytes::from(vec![0; 7]),
        );

        assert_eq!(
            frame.validate(8),
            Err(FrameError::FrameTooLarge {
                actual: 9,
                maximum: 8,
            })
        );
    }
}