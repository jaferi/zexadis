# ADR-0009: EXISTS Command

- **Status:** Proposed
- **Date:** 2026-10-23
- **Related ADRs:** ADR-0001, ADR-0002, ADR-0003, ADR-0005, ADR-0006, ADR-0008

## Context

Zexadis needs a protocol command that checks whether a key has a live entry in its in-memory state store. The command must support arbitrary binary keys and distinguish an existing entry from a missing or expired entry.

EXISTS must remain independent of the storage implementation. The protocol layer validates and decodes the request, while `zexadis-core` executes the command.

The command must follow the core storage and TTL semantics defined in ADR-0001 and ADR-0003.

## Decision

Zexadis V1 will define EXISTS with request opcode `0x04` and response opcode `0x84`.

### 1. Request Format

The EXISTS request payload contains one length-prefixed binary key.

```text
[key_length: u32][key: bytes]
```

| Field | Size | Description |
|---|---:|---|
| `key_length` | 4 bytes | Length of the key in bytes |
| `key` | Variable | Binary key |

The key length is an unsigned 32-bit big-endian integer.

The request payload does not include the opcode or protocol version; these fields are part of the outer frame defined in ADR-0005.

The key length specifies the number of bytes in the key, not the number of characters.

Empty keys are valid unless a separate storage-level constraint is introduced.

### 2. Response Format

The EXISTS response payload contains a one-byte status.

```text
[status: u8]
```

| Status | Meaning |
|---|---|
| `0x00` | Key does not exist |
| `0x01` | Key exists |

The response uses opcode `0x84` and protocol version `0x01`.

The response payload must contain exactly one byte. No additional fields are required.

An existing key returns status `0x01`, regardless of whether its stored value is empty or non-empty. A missing key returns status `0x00`.

### 3. Execution Semantics

The server decodes a valid EXISTS request into the corresponding core command:

```rust
Command::Exists { key }
```

The core storage engine determines whether the key has a live entry.

- If the key exists and has not expired, return `true`.
- If the key does not exist, return `false`.
- If the entry has expired, treat it as absent according to the core TTL semantics.
- EXISTS must not modify the stored value or extend its TTL.
- EXISTS must not create an entry when the key is missing.

The protocol layer maps the core result, `CommandResult::Exists(bool)`, to the response status:

- `Exists(true)` maps to `0x01`.
- `Exists(false)` maps to `0x00`.

The protocol layer must not implement storage lookup or expiration behavior.

### 4. Validation

The protocol decoder must reject:

- A request with a missing or truncated key-length field.
- A key length larger than the remaining payload.
- A payload containing unexpected trailing bytes.
- A response with a missing status byte.
- A response containing an unknown status value.
- A response containing unexpected trailing payload bytes.

The decoder must validate lengths before slicing or allocating buffers. Malformed requests must not reach core execution.

The framing layer is responsible for validating the protocol version, opcode, frame length, and maximum frame size according to ADR-0005.

### 5. Binary Data and Memory

Keys are arbitrary byte sequences. They must not require UTF-8 encoding or text serialization.

Where possible, command decoding should use `Bytes` slices to avoid copying the key from the frame payload.

The implementation must compare the exact key bytes supplied by the client. It must not perform text conversion, normalization, or other transformations.

EXISTS only returns a boolean result and does not return the stored value.

### 6. Error Handling

Malformed payloads must be rejected by the protocol layer.

A valid EXISTS request for a missing or expired key is not an error; it returns status `0x00`.

Unexpected core execution errors must use the shared error-response conventions defined in ADR-0005 rather than being represented as a missing key.

### 7. Consequences

**Benefits**

- Supports arbitrary binary keys.
- Distinguishes existing entries from missing or expired entries.
- Distinguishes key existence from value content, including empty values.
- Keeps storage execution separate from protocol encoding.
- Provides a compact, predictable response format.
- Does not expose stored values unnecessarily.

**Trade-offs**

- Clients receive only a boolean result, without the associated value.
- Clients requiring both existence and value must issue separate EXISTS and GET commands.
- The storage implementation must ensure expired entries are treated as absent.

## Validation

The implementation must include tests covering:

- Request encoding and decoding.
- Response encoding and decoding for both boolean values.
- Checking an existing key.
- Checking a missing key.
- Checking a key whose value is empty.
- Checking binary keys containing zero and non-UTF-8 bytes.
- Empty key handling, subject to core storage constraints.
- Truncated key-length fields.
- Key lengths exceeding the remaining payload.
- Unexpected trailing request bytes.
- Missing response status, unknown status values, and trailing response bytes.
- Expired entries being treated as absent.
- EXISTS not modifying values or extending TTL.
- Maximum frame size enforcement by the framing layer.

## References

- ADR-0001: [Sharded In-Memory Storage](./0001-sharded-in-memory-storage.md)
- ADR-0002: [Command and Batch Execution](./0002-command-and-batch-execution.md)
- ADR-0003: [TTL and Expiration Semantics](./0003-ttl-and-expiration-semantics.md)
- ADR-0005: [Binary Frame Format and Lifecycle](./0005-binary-frame-format-and-lifecycle.md)
- ADR-0006: [GET Command](./0006-get-command.md)
- ADR-0008: [DEL Command](./0008-del-command.md)