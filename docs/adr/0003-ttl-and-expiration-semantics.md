# ADR-0003: TTL and Expiration Semantics

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

Zexadis stores binary values that may need to expire after a specified duration. TTL behavior must be consistent across storage operations and predictable for latency-sensitive workloads.

The engine needs to define how expiration timestamps are calculated, when an entry becomes expired, how expired entries are removed, and how invalid TTL values are handled.

Expiration must not require a background worker or introduce unpredictable work into every storage operation.

## Decision

### 1. Expiration representation

Each entry stores an optional absolute expiration timestamp in Unix milliseconds.

- `Some(expires_at)` means the entry expires at the specified timestamp.
- `None` means the entry does not expire.

The expiration timestamp is calculated by adding the TTL, expressed in milliseconds, to the supplied current timestamp.

### 2. Expiration boundary

An entry is expired when:

`now_ms >= expires_at`

An entry remains valid only while `now_ms < expires_at`.

The storage engine receives `now_ms` from its caller rather than reading the system clock independently in each operation. This makes expiration behavior deterministic and testable.

The caller is responsible for supplying a timestamp in the expected Unix-millisecond format.

### 3. TTL application

The following commands accept an optional TTL:

- `SET`
- `CAS`

A successful `SET` creates or replaces an entry with the specified expiration policy.

A successful `CAS` replaces an existing entry and applies the TTL supplied with that command.

A failed `CAS` does not update the entry or its expiration timestamp.

There is no separate TTL-update command in the initial implementation.

### 4. Expiration and deletion

The initial implementation uses lazy eviction.

When `GET` or `EXISTS` encounters an expired entry, it treats the entry as absent and attempts to remove it.

`DEL` physically removes the entry and reports whether the removed entry was logically valid at the supplied timestamp.

No background expiration worker is required.

Expired entries that are never accessed may remain physically stored. Consequently, physical entry counts may include expired entries until they are removed.

### 5. Invalid TTL values

TTL calculations must not overflow the supported `u64` expiration timestamp representation.

If the duration cannot be represented in milliseconds or adding it to `now_ms` overflows `u64`, the operation must return `StorageError::InvalidTtl`.

An invalid TTL must not create or replace an entry.

A failed `CAS` caused by an invalid TTL must not modify the existing entry.

A zero TTL is valid and creates an entry that is expired immediately at its creation timestamp.

Durations shorter than one millisecond are currently truncated to whole milliseconds. Therefore, a positive sub-millisecond TTL is equivalent to a zero-millisecond TTL.

### 6. Maximum TTL policy

The initial implementation does not impose a separate maximum TTL duration beyond the representable timestamp limit.

A fixed maximum TTL may be introduced later if workload requirements justify it. If introduced, the engine should reject values above the configured or defined limit rather than silently clamp them.

A missing TTL must continue to mean that the entry does not expire; it must not be interpreted as the maximum TTL.

A maximum TTL would limit retention duration, not total memory consumption. Memory limits and eviction policies require separate decisions.

## Consequences

### Benefits

- Provides deterministic expiration behavior.
- Avoids mandatory background cleanup and its associated scheduling overhead.
- Keeps expiration metadata compact.
- Makes TTL behavior straightforward to test using explicit timestamps.
- Prevents invalid timestamp calculations from silently producing incorrect expiration times.

### Trade-offs

- Expired entries may consume memory until accessed or otherwise removed.
- Millisecond precision truncates shorter TTL durations.
- Callers must provide correct timestamps.
- Entries without a TTL may remain indefinitely until explicitly removed or the storage instance is cleared.
- TTL alone does not impose a memory limit.

## Alternatives Considered

### Background expiration worker

Periodically scan or schedule entries for expiration.

Deferred because background cleanup adds scheduling and synchronization complexity. It may be reconsidered if lazy eviction causes unacceptable memory retention.

### Immediate scheduled expiration

Schedule a timer for every entry.

Rejected for the initial implementation because per-entry scheduling adds bookkeeping and operational overhead.

### Silent TTL clamping

Clamp TTLs above a maximum to the maximum allowed duration.

Rejected because silently changing the requested expiration policy can cause unexpected data retention behavior.

### Treat missing TTL as a maximum TTL

Apply a default maximum expiration duration when no TTL is supplied.

Rejected because persistent entries and expiring entries have different semantics and should remain distinguishable.

## Validation

The implementation will be tested for:

- Entries without TTL.
- Normal TTL calculation.
- Expiration immediately before, at, and after the expiration boundary.
- Zero and sub-millisecond TTLs.
- Overflow in duration conversion and timestamp addition.
- Invalid TTL propagation through `SET` and `CAS`.
- Preservation of existing values after failed CAS operations.
- Lazy removal of expired entries through supported access operations.

## Revisit When

Reconsider this decision if memory-retention measurements show that lazy eviction is insufficient, if sub-millisecond precision becomes necessary, or if workloads require a fixed maximum TTL or explicit memory-capacity controls.