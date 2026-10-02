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


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_set_and_get() {
        let shard = Shard::new();
        let key = Bytes::from_static(b"k1");
        let value = Bytes::from_static(b"v1");

        assert!(shard.set(key.clone(), value.clone(), None, 1000));
        assert_eq!(shard.get(b"k1", 1000), Some(value));
        assert!(shard.exists(b"k1", 1000));
    }

    #[test]
    fn test_pure_binary_non_utf8_payloads() {
        let shard = Shard::new();
        let key = Bytes::from_static(b"\x00\x01\xFF\xFE");
        let value = Bytes::from_static(b"\xDE\xAD\xBE\xEF\x00");

        assert!(shard.set(key, value.clone(), None, 1000));
        assert_eq!(shard.get(b"\x00\x01\xFF\xFE", 1000), Some(value));
        assert!(shard.exists(b"\x00\x01\xFF\xFE", 1000));
    }

    #[test]
    fn test_lazy_expiration() {
        let shard = Shard::new();
        let key = Bytes::from_static(b"k1");
        let value = Bytes::from_static(b"v1");
        let ttl = Some(Duration::from_millis(500));

        // Insert at t=1000, expires at t=1500.
        assert!(shard.set(key, value, ttl, 1000));

        // Active before expiration.
        assert!(shard.exists(b"k1", 1499));
        assert!(shard.get(b"k1", 1499).is_some());
        assert_eq!(shard.len(), 1);

        // Expired exactly at the deadline.
        assert_eq!(shard.get(b"k1", 1500), None);
        assert!(!shard.exists(b"k1", 1500));

        // Access lazily evicted the expired entry.
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_cas_operations() {
        let shard = Shard::new();
        let key = Bytes::from_static(b"cas_key");

        shard.set(
            key.clone(),
            Bytes::from_static(b"val1"),
            None,
            1000,
        );

        // CAS fails when the expected value does not match.
        assert!(!shard.cas(
            key.clone(),
            b"wrong_val",
            Bytes::from_static(b"val2"),
            None,
            1000,
        ));

        assert_eq!(
            shard.get(b"cas_key", 1000),
            Some(Bytes::from_static(b"val1"))
        );

        // CAS succeeds when the expected value matches exactly.
        assert!(shard.cas(
            key,
            b"val1",
            Bytes::from_static(b"val2"),
            None,
            1000,
        ));

        assert_eq!(
            shard.get(b"cas_key", 1000),
            Some(Bytes::from_static(b"val2"))
        );
    }

    #[test]
    fn test_cas_missing_key() {
        let shard = Shard::new();

        // CAS never creates a missing key.
        assert!(!shard.cas(
            Bytes::from_static(b"k1"),
            b"",
            Bytes::from_static(b"v1"),
            None,
            1000,
        ));

        assert_eq!(shard.get(b"k1", 1000), None);
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_cas_on_expired_key() {
        let shard = Shard::new();

        shard.set(
            Bytes::from_static(b"k1"),
            Bytes::from_static(b"v1"),
            Some(Duration::from_millis(100)),
            1000,
        );

        // Entry expired at t=1100.
        assert!(!shard.cas(
            Bytes::from_static(b"k1"),
            b"v1",
            Bytes::from_static(b"v2"),
            None,
            1200,
        ));

        // CAS also lazily removes the expired entry.
        assert_eq!(shard.len(), 0);
        assert_eq!(shard.get(b"k1", 1200), None);
    }

    #[test]
    fn test_cas_updates_ttl() {
        let shard = Shard::new();

        shard.set(
            Bytes::from_static(b"k1"),
            Bytes::from_static(b"v1"),
            None,
            1000,
        );

        assert!(shard.cas(
            Bytes::from_static(b"k1"),
            b"v1",
            Bytes::from_static(b"v2"),
            Some(Duration::from_millis(100)),
            1000,
        ));

        // New value is active before its new expiration.
        assert!(shard.exists(b"k1", 1099));

        // New value expires at t=1100.
        assert!(!shard.exists(b"k1", 1100));
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_delete() {
        let shard = Shard::new();
        let key = Bytes::from_static(b"k1");

        shard.set(
            key.clone(),
            Bytes::from_static(b"v1"),
            None,
            1000,
        );

        assert!(shard.delete(b"k1", 1000));
        assert!(!shard.delete(b"k1", 1000));

        // Deleting an expired entry returns false.
        shard.set(
            key,
            Bytes::from_static(b"v1"),
            Some(Duration::from_millis(10)),
            1000,
        );

        assert!(!shard.delete(b"k1", 1011));
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_ttl_overflow_handling() {
        let shard = Shard::new();

        let key = Bytes::from_static(b"k1");
        let value = Bytes::from_static(b"v1");

        // Duration::MAX cannot be represented as milliseconds/u64.
        assert!(!shard.set(key, value, Some(Duration::MAX), 1000));
        assert_eq!(shard.len(), 0);
    }

    #[test]
    fn test_empty_value() {
        let shard = Shard::new();

        assert!(shard.set(
            Bytes::from_static(b"k1"),
            Bytes::new(),
            None,
            1000,
        ));

        assert_eq!(
            shard.get(b"k1", 1000),
            Some(Bytes::new())
        );

        assert!(shard.exists(b"k1", 1000));
    }

    #[test]
    fn test_empty_key() {
        let shard = Shard::new();

        assert!(shard.set(
            Bytes::new(),
            Bytes::from_static(b"v1"),
            None,
            1000,
        ));

        assert_eq!(
            shard.get(b"", 1000),
            Some(Bytes::from_static(b"v1"))
        );

        assert!(shard.exists(b"", 1000));
    }

    #[test]
    fn test_concurrent_shard_access() {
        let shard = Arc::new(Shard::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let shard = Arc::clone(&shard);

            handles.push(thread::spawn(move || {
                let key = Bytes::from(format!("key_{i}"));
                let value = Bytes::from(format!("value_{i}"));

                assert!(shard.set(
                    key.clone(),
                    value.clone(),
                    None,
                    1000,
                ));

                assert_eq!(
                    shard.get(key.as_ref(), 1000),
                    Some(value)
                );

                assert!(shard.exists(key.as_ref(), 1000));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(shard.len(), 10);
    }
}