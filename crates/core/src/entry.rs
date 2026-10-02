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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_without_ttl() {
        let payload = Bytes::from_static(b"hello");

        let entry = Entry::new(payload.clone(), None, 1000).unwrap();

        assert_eq!(entry.payload, payload);
        assert_eq!(entry.expires_at, None);
        assert!(!entry.is_expired(1000));
        assert!(!entry.is_expired(u64::MAX));
    }

    #[test]
    fn test_create_with_ttl() {
        let payload = Bytes::from_static(b"hello");
        let ttl = Duration::from_millis(500);

        let entry = Entry::new(payload.clone(), Some(ttl), 1000).unwrap();

        assert_eq!(entry.payload, payload);
        assert_eq!(entry.expires_at, Some(1500));
    }

    #[test]
    fn test_ttl_expiration_boundary() {
        let entry = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::from_millis(500)),
            1000,
        )
        .unwrap();

        // Even before the creation timestamp, the entry is not expired.
        assert!(!entry.is_expired(500));

        // Active before the expiration deadline.
        assert!(!entry.is_expired(1499));

        // Expired exactly at the deadline.
        assert!(entry.is_expired(1500));

        // Remains expired after the deadline.
        assert!(entry.is_expired(1501));
    }

    #[test]
    fn test_zero_ttl_expires_immediately() {
        let entry = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::ZERO),
            1000,
        )
        .unwrap();

        assert_eq!(entry.expires_at, Some(1000));
        assert!(entry.is_expired(1000));
        assert!(entry.is_expired(1001));
    }

    #[test]
    fn test_sub_millisecond_ttl_truncation() {
        // Durations under 1ms truncate to 0ms in millisecond representation
        let entry = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::from_micros(999)),
            1000,
        )
        .unwrap();

        assert_eq!(entry.expires_at, Some(1000));
        assert!(entry.is_expired(1000));
    }

    #[test]
    fn test_binary_payload() {
        let payload = Bytes::from_static(b"\x00\x01\xFF\xFE\xDE\xAD");

        let entry = Entry::new(payload.clone(), None, 1000).unwrap();

        assert_eq!(entry.payload, payload);
        assert_eq!(entry.payload.as_ref(), b"\x00\x01\xFF\xFE\xDE\xAD");
    }

    #[test]
    fn test_empty_payload() {
        let entry = Entry::new(Bytes::new(), None, 1000).unwrap();

        assert!(entry.payload.is_empty());
        assert_eq!(entry.expires_at, None);
        assert!(!entry.is_expired(1000));
    }

    #[test]
    fn test_ttl_overflow() {
        let result = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::from_millis(1)),
            u64::MAX,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_ttl_duration_overflow() {
        let result = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::MAX),
            1000,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_large_valid_timestamp() {
        let now_ms = u64::MAX - 1000;
        let ttl = Duration::from_millis(500);

        let entry = Entry::new(
            Bytes::from_static(b"value"),
            Some(ttl),
            now_ms,
        )
        .unwrap();

        assert_eq!(entry.expires_at, Some(u64::MAX - 500));
        assert!(!entry.is_expired(u64::MAX - 501));
        assert!(entry.is_expired(u64::MAX - 500));
    }

    #[test]
    fn test_entry_clone_and_equality() {
        let entry = Entry::new(
            Bytes::from_static(b"value"),
            Some(Duration::from_millis(100)),
            1000,
        )
        .unwrap();

        let cloned = entry.clone();

        assert_eq!(entry, cloned);
    }
}