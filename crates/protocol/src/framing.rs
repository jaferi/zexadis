//! Binary request and response framing for the Zexadis protocol.

use thiserror::Error;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u8),
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
}