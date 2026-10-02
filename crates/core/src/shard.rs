//! Sharding primitives for concurrent state access.

use crate::entry::Entry;
use bytes::Bytes;
use dashmap::{DashMap, Entry as DashEntry};
use std::time::Duration;

/// An isolated concurrent in-memory shard holding key-value entries.
///
/// Shard is completely routing-agnostic; it does not know about hash rings,
/// key distributions, or other shards.
pub struct Shard {
    data: DashMap<Bytes, Entry>,
}

impl Shard {
    /// Creates a new empty shard.
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }

    /// Retrieves the binary payload for a key if it exists and has not expired.
    ///
    /// Performs lazy eviction if an expired entry is encountered.
    pub fn get(&self, key: &[u8], now_ms: u64) -> Option<Bytes> {
        let entry = self.data.get(key)?;

        if entry.is_expired(now_ms) {
            drop(entry);
            // Remove conditionally to prevent race conditions across threads
            self.data.remove_if(key, |_, e| e.is_expired(now_ms));
            return None;
        }

        Some(entry.payload.clone())
    }

    /// Stores a binary payload with an optional TTL.
    ///
    /// Returns `false` if the TTL duration calculation overflows.
    pub fn set(
        &self,
        key: Bytes,
        value: Bytes,
        ttl: Option<Duration>,
        now_ms: u64,
    ) -> bool {
        let entry = match Entry::new(value, ttl, now_ms) {
            Some(entry) => entry,
            None => return false,
        };

        self.data.insert(key, entry);
        true
    }

    /// Removes a key from the shard.
    ///
    /// Returns `true` if an active (non-expired) entry was removed.
    pub fn delete(&self, key: &[u8], now_ms: u64) -> bool {
        match self.data.remove(key) {
            Some((_, entry)) => !entry.is_expired(now_ms),
            None => false,
        }
    }

    /// Returns `true` if a key exists and has not expired.
    ///
    /// Performs lazy eviction if an expired entry is encountered.
    pub fn exists(&self, key: &[u8], now_ms: u64) -> bool {
        let entry = match self.data.get(key) {
            Some(entry) => entry,
            None => return false,
        };

        if entry.is_expired(now_ms) {
            drop(entry);
            self.data.remove_if(key, |_, e| e.is_expired(now_ms));
            return false;
        }

        true
    }

    /// Atomically compares the current payload and replaces it if it matches `expected`.
    ///
    /// - Returns `false` if the key is absent or expired.
    /// - Returns `false` if the current payload does not match `expected`.
    /// - Removes expired entries encountered during evaluation.

    pub fn cas(
        &self,
        key: Bytes,
        expected: &[u8],
        new_value: Bytes,
        ttl: Option<Duration>,
        now_ms: u64,
    ) -> bool {
        let new_entry = match Entry::new(new_value, ttl, now_ms) {
            Some(entry) => entry,
            None => return false,
        };
    
        match self.data.entry(key) {
            DashEntry::Occupied(mut occupied) => {
                if occupied.get().is_expired(now_ms) {
                    occupied.remove();
                    return false;
                }
    
                if occupied.get().payload.as_ref() != expected {
                    return false;
                }
    
                occupied.insert(new_entry);
                true
            }
    
            DashEntry::Vacant(_) => false,
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// if the shard contains no entries.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Removes all entries
    pub fn clear(&self) {
        self.data.clear();
    }
}

impl Default for Shard {
    fn default() -> Self {
        Self::new()
    }
}