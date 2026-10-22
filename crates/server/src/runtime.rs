//! Server runtime orchestration and asynchronous task management.

use std::time::{SystemTime, UNIX_EPOCH};
use zexadis_core::commands::{Command, CommandResult};
use zexadis_core::storage::{Storage, StorageError};

pub struct Runtime {
    storage: Storage,
}

impl Runtime {
    pub fn new(shard_count: usize) -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::new(shard_count)?,
        })
    }

    pub fn execute(&self, command: Command) -> Result<CommandResult, StorageError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before the Unix epoch")
            .as_millis() as u64;

        self.storage.execute(command, now_ms)
    }
}
