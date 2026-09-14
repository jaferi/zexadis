//! Command definitions and execution semantics for the storage engine.

use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Retrieve a stored value by key.
    Get {
        key: Bytes,
    },

    /// Store a binary value with an optional TTL.
    Set {
        key: Bytes,
        value: Bytes,
        ttl: Option<Duration>,
    },

    /// Remove a key and its value.
    Del {
        key: Bytes,
    },

    /// Check whether a key exists and is not expired.
    Exists {
        key: Bytes,
    },

    /// Atomically update a value if the stored value matches the expected value.
    Cas {
        key: Bytes,
        expected: Bytes,
        new_value: Bytes,
        ttl: Option<Duration>,
    },

    /// Execute multiple commands in a single network round-trip.
    ///
    /// Commands are executed in order.
    /// A batch is not transactional and provides no cross-shard atomicity.
    Batch {
        commands: Vec<Command>,
    },
}

/// The result of executing a Zexadis command.
///
/// Expected operational outcomes are represented directly rather than as errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Returned by GET when the key exists and is not expired.
    Entry(Bytes),

    /// Returned by GET when the key does not exist or has expired.
    NotFound,

    /// Returned by SET after the write completes.
    Set,

    /// Returned by EXISTS: `true` if the key exists and is not expired.
    Exists(bool),

    /// Returned by DEL: `true` if the key existed and was deleted.
    Deleted(bool),

    /// Returned by CAS: `true` if the expected value matched and the write succeeded.
    Cas(bool),

    /// Returned by BATCH: results in the same order as the commands.
    Batch(Vec<CommandResult>),
}