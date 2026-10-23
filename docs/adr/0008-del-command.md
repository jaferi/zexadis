# ADR-0008: DEL Command

- **Status:** Proposed
- **Date:** 2026-10-23
- **Related ADRs:** ADR-0001, ADR-0002, ADR-0003, ADR-0005, ADR-0006

## Context

Zexadis needs a protocol command that deletes a stored value by key. The command must support arbitrary binary keys and return an unambiguous result indicating whether an entry was deleted.

DEL must remain independent of the storage implementation. The protocol layer validates and decodes the request, while `zexadis-core` executes the command.

Deletion must follow the core storage semantics, including the treatment of expired entries defined by ADR-0003.

## Decision

Zexadis V1 will define DEL with request opcode `0x03` and response opcode `0x83`.

### 1. Request Format

The DEL request payload contains one length-prefixed binary key.

```text
[key_length: u32][key: bytes]
```

| Field | Size | Description |
|---|---|---|
| `key_length` | 4 bytes | Length of the key in bytes |
| `key` | Variable | Binary key |

The key length is an unsigned 32-bit big-endian integer.

The request payload does not include the opcode or protocol version; these fields are part of the outer frame defined in ADR-0005.

### 2. Response Format

The DEL response payload contains a one-byte status.

```text
[status: u8]
```

| Status | Meaning |
|---|---|
| `0x00` | No entry was deleted |
| `0x01` | An entry was successfully deleted |

The response uses opcode `0x83` and protocol version `0x01`.

A missing key is not an error. The server returns status `0x00` when no entry is deleted and status `0x01` when an entry is successfully removed.

The response payload must contain exactly one byte. It does not include the deleted key or value.

### 3. Execution Semantics

The server decodes a valid DEL request into the corresponding core command:

```rust
Command::Del { key }
```

The core storage engine determines whether a live entry exists and removes it when present.

- If the key exists and has not expired, delete the entry and return a successful deletion result.
- If the key does not exist, return a result indicating that no entry was deleted.
- If the entry has expired, treat it as absent according to the core TTL semantics.
- Repeated DEL commands for the same key are valid. After a successful deletion, a subsequent DEL returns status `0x00` unless the key has been set again.
- DEL must not affect entries associated with other keys.

The protocol layer maps the core result, `CommandResult::Deleted(bool)`, to the response status:

- `Deleted(true)` maps to `0x01`.
- `Deleted(false)` maps to `0x00`.

The protocol layer must not implement deletion or determine whether an entry has expired; these are storage responsibilities.

### 4. Validation

The protocol decoder must reject:

- A request with a missing or truncated key-length field.
- A key length larger than the remaining payload.
- A payload containing unexpected trailing bytes.
- A response containing an unknown status.
- A response with a missing status byte.
- A response containing unexpected trailing bytes.

The decoder must validate lengths before slicing or allocating buffers. Malformed requests must not reach core execution.

The framing layer is responsible for validating the protocol version, opcode, frame length, and maximum frame size according to ADR-0005.

### 5. Binary Data and Memory

Keys are arbitrary byte sequences. They must not require UTF-8 encoding or text serialization.

Where possible, command decoding should use `Bytes` slices to avoid copying the key from the frame payload. The storage implementation may retain or clone reference-counted byte buffers according to its ownership requirements.

Deletion must operate on the exact byte sequence supplied by the client. It must not perform text conversion, normalization, or other transformations.

### 6. Error Handling

Malformed payloads must be rejected by the protocol layer.

A valid DEL request for a missing key is not an error; it returns the status indicating that no entry was deleted.

Unexpected core execution errors must use the shared error-response conventions defined in ADR-0005 rather than being represented as an unsuccessful deletion.

### 7. Consequences

**Benefits**

- Supports arbitrary binary keys.
- Distinguishes successful deletion from a missing key.
- Keeps storage execution separate from protocol encoding.
- Provides a compact, predictable response format.
- Makes repeated deletion deterministic.

**Trade-offs**

- Clients must interpret the deletion status.
- The protocol does not return the deleted value or additional deletion metadata.
- The storage implementation must preserve the distinction between deleting a live entry and encountering an absent or expired entry.

## Validation

The implementation must include tests covering:

- Request encoding and decoding.
- Response encoding and decoding for successful and unsuccessful deletion.
- Deleting an existing key.
- Deleting a missing key.
- Deleting the same key twice.
- Empty keys, if supported by the core storage constraints.
- Binary keys containing zero and non-UTF-8 bytes.
- Truncated key-length fields.
- Key lengths exceeding the remaining payload.
- Unexpected trailing request bytes.
- Missing status bytes, unknown status values, and trailing response bytes.
- Expired entries being treated as absent.
- Maximum frame size enforcement by the framing layer.

## References

- ADR-0001: Sharded In-Memory Storage
- ADR-0002: Command and Batch Execution
- ADR-0003: TTL and Expiration Semantics
- ADR-0005: Binary Frame Format and Lifecycle
- ADR-0006: GET Command