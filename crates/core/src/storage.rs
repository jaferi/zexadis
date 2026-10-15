//! Storage abstractions and in-memory storage implementations.

use crate::{
    commands::{Command, CommandResult},
    shard::Shard,
};
use ahash::AHasher;
use std::hash::Hasher;

/// Errors for storage command validation and execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("Shard count must be a non-zero power of two")]
    InvalidShardCount,

    #[error("Nested batches are strictly prohibited")]
    NestedBatchNotAllowed,

    #[error("Calculated expiration timestamp exceeds maximum supported u64 bound")]
    InvalidTtl,
}

/// A sharded, in-memory key-value storage engine.
pub struct Storage {
    shards: Vec<Shard>,
    shard_mask: usize,
}

impl Storage {
    /// Creates a new storage instance with the given number of shards.
    ///
    /// The shard count must be a non-zero power of two.
    pub fn new(shard_count: usize) -> Result<Self, StorageError> {
        if shard_count == 0 || !shard_count.is_power_of_two() {
            return Err(StorageError::InvalidShardCount);
        }

        let shards = (0..shard_count).map(|_| Shard::new()).collect();

        Ok(Self {
            shards,
            shard_mask: shard_count - 1,
        })
    }

    /// Returns the total number of shards.
    #[inline]
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Returns the shard index responsible for a given key.
    #[inline]
    pub fn shard_index(&self, key: &[u8]) -> usize {
        let mut hasher = AHasher::default();
        hasher.write(key);

        // bitwise is cheaper than modulo
        (hasher.finish() as usize) & self.shard_mask
    }

    /// Returns a reference to the shard responsible for a key.
    #[inline]
    pub fn shard(&self, key: &[u8]) -> &Shard {
        &self.shards[self.shard_index(key)]
    }

    /// Executes a storage command.
    pub fn execute(&self, command: Command, now_ms: u64) -> Result<CommandResult, StorageError> {
        match command {
            Command::Get { key } => match self.shard(&key).get(&key, now_ms) {
                Some(value) => Ok(CommandResult::Entry(value)),
                None => Ok(CommandResult::NotFound),
            },

            Command::Set { key, value, ttl } => {
                let stored = self.shard(&key).set(key, value, ttl, now_ms);
                if !stored {
                    return Err(StorageError::InvalidTtl);
                }
                Ok(CommandResult::Set)
            }

            Command::Del { key } => {
                let deleted = self.shard(&key).delete(&key, now_ms);
                Ok(CommandResult::Deleted(deleted))
            }

            Command::Exists { key } => {
                let exists = self.shard(&key).exists(&key, now_ms);
                Ok(CommandResult::Exists(exists))
            }

            Command::Cas {
                key,
                expected,
                new_value,
                ttl,
            } => {
                // Check for TTL math overflow up-front if TTL is specified
                if let Some(duration) = ttl {
                    let ttl_ms = u64::try_from(duration.as_millis())
                        .map_err(|_| StorageError::InvalidTtl)?;
                    if now_ms.checked_add(ttl_ms).is_none() {
                        return Err(StorageError::InvalidTtl);
                    }
                }

                let updated = self.shard(&key).cas(key, &expected, new_value, ttl, now_ms);

                Ok(CommandResult::Cas(updated))
            }

            Command::Batch { commands } => self.execute_batch(commands, now_ms),
        }
    }

    /// Executes commands sequentially and preserves their result order.
    ///
    /// Batches are not transactional and provide no cross-shard atomicity.
    fn execute_batch(
        &self,
        commands: Vec<Command>,
        now_ms: u64,
    ) -> Result<CommandResult, StorageError> {
        if commands
            .iter()
            .any(|command| matches!(command, Command::Batch { .. }))
        {
            return Err(StorageError::NestedBatchNotAllowed);
        }

        let mut results = Vec::with_capacity(commands.len());

        for command in commands {
            results.push(self.execute(command, now_ms)?);
        }

        Ok(CommandResult::Batch(results))
    }
}
