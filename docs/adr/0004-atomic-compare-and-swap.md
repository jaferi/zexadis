# ADR-0004: Atomic Compare-and-Swap Semantics

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

Zexadis supports compare-and-swap (CAS) as a conditional state-update operation. CAS allows a caller to replace a stored value only if its current value matches an expected value.

This operation is useful for coordinating concurrent workers, updating shared state, and preventing updates based on stale values.

For CAS to be reliable, the comparison and replacement must be atomic with respect to competing operations on the same key.

## Decision

### 1. Existing keys only

CAS updates an existing, unexpired entry.

It does not create a new entry when the key is missing.

### 2. Comparison semantics

The current stored binary payload is compared with the expected payload using exact byte equality.

The comparison does not decode or interpret the payload.

### 3. Atomicity

The comparison and replacement occur as one atomic operation within the shard responsible for the key.

Concurrent CAS operations on the same key must not both succeed based on the same previous value when they attempt to replace it with different values.

Operations on different keys do not receive a global transaction guarantee, even when those keys happen to reside in the same shard.

### 4. Successful updates

If the key exists, is not expired, and its payload matches the expected value, CAS replaces the entry with the new payload and applies the TTL supplied by the command.

The operation returns `CommandResult::Cas(true)`.

### 5. Unsuccessful updates

CAS returns `CommandResult::Cas(false)` when:

- The key does not exist.
- The key has expired.
- The stored payload does not match the expected payload.

An expired entry encountered during CAS is removed.

An unsuccessful comparison does not change the stored payload or expiration timestamp.

### 6. Invalid TTL

The TTL must be validated before attempting the conditional update.

If TTL conversion or expiration timestamp calculation overflows, the operation returns `StorageError::InvalidTtl`.

An invalid TTL must not modify or remove an existing entry, even if that entry has expired or its value would not match the expected payload.

This validation order ensures that invalid request parameters are distinguished from ordinary CAS failures.

## Consequences

### Benefits

- Provides reliable conditional updates for concurrent access to the same key.
- Avoids separate read-then-write operations that could race.
- Works with opaque binary values without requiring serialization.
- Keeps CAS semantics simple and consistent.
- Distinguishes invalid TTL requests from unsuccessful comparisons.

### Trade-offs

- CAS cannot insert a missing key.
- The operation is atomic only for the individual key, not for a group of keys.
- Callers must handle ordinary CAS failures and decide whether to retry.
- CAS does not provide transactions, distributed coordination, or persistence guarantees.

## Alternatives Considered

### Non-atomic read followed by write

Read the current value, compare it with the expected value, and then write the replacement.

Rejected because another operation could modify the value between the read and write, invalidating the comparison.

### Insert-if-missing CAS

Allow CAS to create a new entry when the key does not exist.

Deferred to keep the initial operation's semantics unambiguous. A separate insert-if-absent command could be introduced if required.

### Multi-key conditional transactions

Atomically compare and update multiple keys.

Deferred because it would require broader coordination and potentially cross-shard locking or transaction machinery.

## Validation

The implementation will be tested for:

- Successful CAS with a matching value.
- Failure when the expected value does not match.
- Failure for a missing key.
- Failure for an expired key.
- Replacement of the TTL after successful CAS.
- Preservation of the original value and TTL after a mismatch.
- Rejection of invalid TTL values without side effects.
- Concurrent CAS attempts against the same key.

## Revisit When

Reconsider this decision if workloads require insert-if-absent semantics, multi-key atomic operations, version-based updates, or distributed compare-and-swap across multiple storage instances.