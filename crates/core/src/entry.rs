//! In-memory state storage unit for Zexadis.

use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub payload: Bytes,
    /// Unix timestamp in milliseconds when this entry expires.
    /// `None` means the entry does not expire.
    pub expires_at: Option<u64>,
}

impl Entry {
    /// Creates a new entry with an optional TTL.
    ///
    /// Returns `None` if the TTL cannot be represented in milliseconds
    /// or if the resulting expiration timestamp overflows `u64`.
    #[inline]
    pub fn new(
        payload: Bytes,
        ttl: Option<Duration>,
        now_ms: u64,
    ) -> Option<Self> {
        let expires_at = match ttl {
            Some(ttl) => {
                let ttl_ms = u64::try_from(ttl.as_millis()).ok()?;
                Some(now_ms.checked_add(ttl_ms)?)
            }
            None => None,
        };

        Some(Self {
            payload,
            expires_at,
        })
    }

    /// Returns `true` when the entry has reached its expiration time.
    #[inline]
    pub fn is_expired(&self, now_ms: u64) -> bool {
        match self.expires_at {
            Some(expires_at) => now_ms >= expires_at,
            None => false,
        }
    }
}