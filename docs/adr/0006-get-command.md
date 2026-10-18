# ADR-0006: GET Command

- **Status:** Proposed
- **Date:** 2026-10-18

## Context

Zexadis needs a protocol command that retrieves a stored value by key. The command must support arbitrary binary keys and values, distinguish missing keys from empty values, and handle expired entries consistently with the core storage semantics.

GET must remain independent of the storage implementation. The protocol layer validates and decodes the request, while `zexadis-core` executes the command.

## Decision

Zexadis V1 will define GET with request opcode `0x01` and response opcode `0x81`.

### 1. Request Format

The GET request payload contains one length-prefixed binary key.

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

The GET response payload begins with a one-byte status.

If the key exists:

```text
[status: u8 = 0x01]
[value_length: u32][value: bytes]
```

If the key does not exist:

```text
[status: u8 = 0x00]
```

| Status | Meaning | Additional fields |
|---|---|---|
| `0x00` | Key not found | None |
| `0x01` | Key found | Length-prefixed binary value |

For a successful lookup, `value_length` is an unsigned 32-bit big-endian integer representing the value length in bytes.

An existing empty value is represented by status `0x01` followed by a value length of zero. This distinguishes an empty value from a missing key.

### 3. Execution Semantics

The server decodes a valid GET request into the corresponding core command:

```rust
Command::Get { key }
```

The core storage engine determines whether the key has a live entry.

- If the key exists and has not expired, return its stored binary value.
- If the key does not exist, return a not-found result.
- If the entry has expired, treat it as not found according to the core TTL semantics.
- GET must not modify the value or extend its TTL.

The protocol layer maps the core result to the response format defined in this ADR.

### 4. Validation

The protocol decoder must reject:

- A request with a missing or truncated key-length field.
- A key length larger than the remaining payload.
- A payload containing unexpected trailing bytes.
- A response containing an unknown status.
- A successful response with a missing or truncated value-length field.
- A value length larger than the remaining response payload.
- A response containing unexpected trailing bytes.

The decoder must validate lengths before slicing or allocating buffers. Malformed requests must not reach core execution.

### 5. Binary Data and Memory

Keys and values are arbitrary byte sequences. They must not require UTF-8 encoding or text serialization.

Where possible, command decoding should use `Bytes` slices to avoid copying the key from the frame payload. The storage implementation may retain or clone reference-counted byte buffers according to its ownership requirements.

### 6. Error Handling

Malformed payloads must be rejected by the protocol layer.

A valid GET request whose key is missing is not an error; it returns the not-found status.

Unexpected core execution errors must use the shared error-response conventions defined in ADR-0005 rather than being represented as a missing key.

### 7. Consequences

**Benefits**

- Supports arbitrary binary keys and values.
- Distinguishes missing keys from existing empty values.
- Keeps storage execution separate from protocol encoding.
- Provides a compact and predictable response format.
- Allows efficient payload handling with `Bytes`.

**Trade-offs**

- The response includes a status byte and a four-byte value length.
- Clients must handle not-found results explicitly.
- The protocol must validate all length-prefixed fields.

## Validation

The implementation must include tests covering:

- Request encoding and decoding.
- Response encoding and decoding for found and not-found results.
- Empty keys and empty values.
- Binary keys and values containing zero and non-UTF-8 bytes.
- Truncated key-length and value-length fields.
- Field lengths exceeding the remaining payload.
- Unexpected trailing bytes.
- Unknown response status values.
- Expired entries being treated as not found.
- Missing keys being distinguished from empty stored values.

## References

- ADR-0001: Sharded In-Memory Storage
- ADR-0003: TTL and Expiration Semantics
- ADR-0005: Binary Frame Format and Lifecycle