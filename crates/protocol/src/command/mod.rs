//! Protocol command definitions and command encoding identifiers.

use thiserror::Error;

use crate::{Frame, FrameError, Opcode};
use zexadis_core::commands::{Command, CommandResult};

pub mod del;
pub mod exists;
pub mod get;
pub mod set;

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
        Command::Get { .. } => get::encode_request(command),
        Command::Set { .. } => set::encode_request(command),
        Command::Del { .. } => del::encode_request(command),
        Command::Exists { .. } => exists::encode_request(command),
        _ => Err(CommandError::UnsupportedCommand),
    }
}

pub fn decode_request(frame: &Frame) -> Result<Command, CommandError> {
    match frame.opcode {
        Opcode::Get => get::decode_request(frame),
        Opcode::Set => set::decode_request(frame),
        Opcode::Del => del::decode_request(frame),
        Opcode::Exists => exists::decode_request(frame),
        opcode => Err(CommandError::UnexpectedOpcode(opcode)),
    }
}

pub fn encode_response(result: &CommandResult) -> Result<Frame, CommandError> {
    match result {
        CommandResult::NotFound | CommandResult::Entry(_) => get::encode_response(result),
        CommandResult::Set(_) => set::encode_response(result),
        CommandResult::Deleted(_) => del::encode_response(result),
        CommandResult::Exists(_) => exists::encode_response(result),
        _ => Err(CommandError::UnsupportedCommand),
    }
}

pub fn decode_response(frame: &Frame) -> Result<CommandResult, CommandError> {
    match frame.opcode {
        Opcode::GetResponse => get::decode_response(frame),
        Opcode::SetResponse => set::decode_response(frame),
        Opcode::DelResponse => del::decode_response(frame),
        Opcode::ExistsResponse => exists::decode_response(frame),
        _ => Err(CommandError::UnexpectedOpcode(frame.opcode)),
    }
}
