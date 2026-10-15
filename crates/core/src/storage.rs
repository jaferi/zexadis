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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::time::Duration;

    fn storage() -> Storage {
        Storage::new(4).unwrap()
    }

    fn bytes(value: &[u8]) -> Bytes {
        Bytes::copy_from_slice(value)
    }

    // Initialization and shard routing
    #[test]
    fn test_create_with_valid_shard_counts() {
        for count in [1, 2, 4, 8] {
            let storage = Storage::new(count).unwrap();

            assert_eq!(storage.shard_count(), count);
        }
    }

    #[test]
    fn test_reject_invalid_shard_counts() {
        for count in [0, 3, 6, 10] {
            assert!(matches!(
                Storage::new(count),
                Err(StorageError::InvalidShardCount)
            ));
        }
    }

    #[test]
    fn test_shard_index_is_in_bounds() {
        let storage = storage();

        let keys: &[&[u8]] = &[
            b"account:1",
            b"account:2",
            b"transaction:123",
            b"\x00\x01\xFF",
            b"",
        ];

        for key in keys {
            assert!(
                storage.shard_index(key) < storage.shard_count(),
                "Shard index out of bounds for key: {key:?}"
            );
        }
    }

    #[test]
    fn test_same_key_maps_to_same_shard() {
        let storage = storage();
        let key = b"account:123";

        let first = storage.shard_index(key);

        for _ in 0..100 {
            assert_eq!(storage.shard_index(key), first);
        }
    }

    // Basic command execution
    #[test]
    fn test_set_and_get() {
        let storage = storage();

        let set_result = storage.execute(
            Command::Set {
                key: bytes(b"key"),
                value: bytes(b"value"),
                ttl: None,
            },
            1000,
        );

        assert_eq!(set_result, Ok(CommandResult::Set));

        let get_result = storage.execute(Command::Get { key: bytes(b"key") }, 1000);

        assert_eq!(get_result, Ok(CommandResult::Entry(bytes(b"value"))));
    }

    #[test]
    fn test_get_missing_key() {
        let storage = storage();

        let result = storage.execute(
            Command::Get {
                key: bytes(b"missing"),
            },
            1000,
        );

        assert_eq!(result, Ok(CommandResult::NotFound));
    }

    #[test]
    fn test_set_overwrites_existing_value() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"first"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"second"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let result = storage.execute(Command::Get { key: bytes(b"key") }, 1000);

        assert_eq!(result, Ok(CommandResult::Entry(bytes(b"second"))));
    }

    #[test]
    fn test_delete_existing_key() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"value"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let delete_result = storage.execute(Command::Del { key: bytes(b"key") }, 1000);

        assert_eq!(delete_result, Ok(CommandResult::Deleted(true)));

        let get_result = storage.execute(Command::Get { key: bytes(b"key") }, 1000);

        assert_eq!(get_result, Ok(CommandResult::NotFound));
    }

    #[test]
    fn test_delete_missing_key() {
        let storage = storage();

        let result = storage.execute(
            Command::Del {
                key: bytes(b"missing"),
            },
            1000,
        );

        assert_eq!(result, Ok(CommandResult::Deleted(false)));
    }

    #[test]
    fn test_exists_for_existing_and_missing_keys() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"existing"),
                    value: bytes(b"value"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let existing = storage.execute(
            Command::Exists {
                key: bytes(b"existing"),
            },
            1000,
        );

        let missing = storage.execute(
            Command::Exists {
                key: bytes(b"missing"),
            },
            1000,
        );

        assert_eq!(existing, Ok(CommandResult::Exists(true)));
        assert_eq!(missing, Ok(CommandResult::Exists(false)));
    }

    // Compare-and-swap
    #[test]
    fn test_cas_matching_value() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"old"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let cas_result = storage.execute(
            Command::Cas {
                key: bytes(b"key"),
                expected: bytes(b"old"),
                new_value: bytes(b"new"),
                ttl: None,
            },
            1000,
        );

        assert_eq!(cas_result, Ok(CommandResult::Cas(true)));

        let get_result = storage.execute(Command::Get { key: bytes(b"key") }, 1000);

        assert_eq!(get_result, Ok(CommandResult::Entry(bytes(b"new"))));
    }

    #[test]
    fn test_cas_mismatched_value() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"original"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let cas_result = storage.execute(
            Command::Cas {
                key: bytes(b"key"),
                expected: bytes(b"wrong"),
                new_value: bytes(b"new"),
                ttl: None,
            },
            1000,
        );

        assert_eq!(cas_result, Ok(CommandResult::Cas(false)));

        let get_result = storage.execute(Command::Get { key: bytes(b"key") }, 1000);

        assert_eq!(get_result, Ok(CommandResult::Entry(bytes(b"original"))));
    }

    // TTL validation
    #[test]
    fn test_set_rejects_ttl_overflow() {
        let storage = storage();

        let result = storage.execute(
            Command::Set {
                key: bytes(b"key"),
                value: bytes(b"value"),
                ttl: Some(Duration::from_millis(1)),
            },
            u64::MAX,
        );

        assert_eq!(result, Err(StorageError::InvalidTtl));

        // The failed SET must not create the key.
        assert_eq!(
            storage.execute(Command::Get { key: bytes(b"key") }, u64::MAX,),
            Ok(CommandResult::NotFound)
        );
    }

    #[test]
    fn test_cas_rejects_ttl_overflow() {
        let storage = storage();

        storage
            .execute(
                Command::Set {
                    key: bytes(b"key"),
                    value: bytes(b"original"),
                    ttl: None,
                },
                1000,
            )
            .unwrap();

        let result = storage.execute(
            Command::Cas {
                key: bytes(b"key"),
                expected: bytes(b"original"),
                new_value: bytes(b"replacement"),
                ttl: Some(Duration::from_millis(1)),
            },
            u64::MAX,
        );

        assert_eq!(result, Err(StorageError::InvalidTtl));

        // Failed CAS must preserve the existing value.
        assert_eq!(
            storage.execute(Command::Get { key: bytes(b"key") }, 1000,),
            Ok(CommandResult::Entry(bytes(b"original")))
        );
    }

    // Batch execution
    #[test]
    fn test_batch_executes_in_order() {
        let storage = storage();

        let result = storage.execute(
            Command::Batch {
                commands: vec![
                    Command::Set {
                        key: bytes(b"key"),
                        value: bytes(b"first"),
                        ttl: None,
                    },
                    Command::Get { key: bytes(b"key") },
                    Command::Set {
                        key: bytes(b"key"),
                        value: bytes(b"second"),
                        ttl: None,
                    },
                    Command::Get { key: bytes(b"key") },
                ],
            },
            1000,
        );

        assert_eq!(
            result,
            Ok(CommandResult::Batch(vec![
                CommandResult::Set,
                CommandResult::Entry(bytes(b"first")),
                CommandResult::Set,
                CommandResult::Entry(bytes(b"second")),
            ]))
        );
    }

    #[test]
    fn test_batch_empty_commands() {
        let storage = storage();

        let result = storage.execute(Command::Batch { commands: vec![] }, 1000);

        assert_eq!(result, Ok(CommandResult::Batch(vec![])));
    }

    #[test]
    fn test_batch_rejects_nested_batch() {
        let storage = storage();

        let result = storage.execute(
            Command::Batch {
                commands: vec![Command::Batch { commands: vec![] }],
            },
            1000,
        );

        assert_eq!(result, Err(StorageError::NestedBatchNotAllowed));
    }

    #[test]
    fn test_batch_rejects_nested_batch_before_execution() {
        let storage = storage();

        let result = storage.execute(
            Command::Batch {
                commands: vec![
                    Command::Set {
                        key: bytes(b"key"),
                        value: bytes(b"value"),
                        ttl: None,
                    },
                    Command::Batch { commands: vec![] },
                ],
            },
            1000,
        );

        assert_eq!(result, Err(StorageError::NestedBatchNotAllowed));

        // Validation must happen before any command executes.
        assert_eq!(
            storage.execute(Command::Get { key: bytes(b"key") }, 1000,),
            Ok(CommandResult::NotFound)
        );
    }

    #[test]
    fn test_batch_preserves_ttl_semantics() {
        let storage = storage();

        let result = storage.execute(
            Command::Batch {
                commands: vec![
                    Command::Set {
                        key: bytes(b"temporary"),
                        value: bytes(b"value"),
                        ttl: Some(Duration::from_millis(500)),
                    },
                    Command::Get {
                        key: bytes(b"temporary"),
                    },
                ],
            },
            1000,
        );

        assert_eq!(
            result,
            Ok(CommandResult::Batch(vec![
                CommandResult::Set,
                CommandResult::Entry(bytes(b"value")),
            ]))
        );

        // The entry expires exactly at 1500ms.
        assert_eq!(
            storage.execute(
                Command::Get {
                    key: bytes(b"temporary"),
                },
                1500,
            ),
            Ok(CommandResult::NotFound)
        );
    }
}
