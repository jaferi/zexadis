# ADR-0007: SET Command Protocol

- **Status:** Accepted
- **Date:** 2026-10-18
- **Related ADRs:** ADR-0001, ADR-0002, ADR-0003, ADR-0005, ADR-0006

## Context

Zexadis needs a binary protocol for storing values in its in-memory state store. The SET command must support arbitrary binary keys and values, optional expiration, and predictable request and response encoding.

The format should be compact, unambiguous, and consistent with the framing rules defined in ADR-0005.

## Decision

Define the SET command using request opcode `0x02` and response opcode `0x82`.

All multi-byte integers use **big-endian** byte order. Keys and values are binary data and must not be interpreted as UTF-8 strings.

### 1. SET Request

The request payload has the following layout:

| Field | Size | Description |
|---|---:|---|
| Key length | 4 bytes | Unsigned 32-bit key length |
| Value length | 4 bytes | Unsigned 32-bit value length |
| TTL flag | 1 byte | `0x00` = no TTL, `0x01` = TTL present |
| TTL | 8 bytes, optional | TTL duration in milliseconds; present only when flag is `0x01` |
| Key | Variable | Raw key bytes |
| Value | Variable | Raw value bytes |

The request uses protocol version `0x01` and opcode `0x02`.

The payload field order is fixed: key length, value length, TTL flag, optional TTL, key bytes, then value bytes.

#### TTL semantics

- `TTL flag = 0x00`: the entry does not expire due to TTL.
- `TTL flag = 0x01`: the following 8-byte unsigned integer specifies the TTL in milliseconds.
- A TTL of zero is invalid.
- TTL expiration behavior follows ADR-0003.
- The server must reject invalid TTL values rather than silently changing their meaning.

### 2. SET Response

The response uses protocol version `0x01` and opcode `0x82`.

The response payload contains a single status byte:

| Status | Meaning |
|---|---|
| `0x00` | SET succeeded |
| `0x01` | SET failed |

A failed SET response does not include additional payload fields. If more detailed error information is needed, the protocol may use the general error response defined in ADR-0005.

The server must not return success until the storage operation has completed successfully.

### 3. Validation

The decoder must reject requests when:

- The payload is shorter than the required fields.
- The TTL flag is not `0x00` or `0x01`.
- The TTL flag indicates a TTL, but the TTL field is truncated.
- The declared key or value lengths do not match the remaining payload.
- The TTL is zero when present.
- The request exceeds the maximum frame size defined in ADR-0005.

The decoder must use checked length validation to prevent integer overflow and out-of-bounds access.

The response decoder must reject an incorrect opcode, a missing status byte, an unknown status, or trailing payload bytes.

Empty keys and empty values are valid unless a separate storage-level constraint is introduced.

### 4. Binary Safety and Memory

Keys and values may contain any byte sequence, including zero bytes and invalid UTF-8.

The decoder should use `Bytes::slice` where applicable to avoid unnecessary payload copies. The encoder may use `BytesMut` to construct the payload before freezing it into `Bytes`.

The implementation must preserve the exact key and value bytes through encoding and decoding.

### 5. Responsibilities

- **Framing layer:** validates protocol version, opcode, frame length, and maximum frame size.
- **Command codec:** encodes and decodes the SET payload and validates its fields.
- **Storage layer:** applies SET semantics and TTL handling as specified by ADR-0003.
- **Server layer:** executes the command and encodes the resulting response.

## Consequences

- SET supports binary keys and values without string conversion.
- Optional TTL is represented explicitly and unambiguously.
- Length-prefixed fields allow strict payload validation.
- The protocol remains compact while supporting expiration.
- Request and response formats are versioned through the existing framing layer.

## Testing Requirements

The implementation must include tests for:

- SET request round-trip with TTL.
- SET request round-trip without TTL.
- Empty key and empty value.
- Binary keys and values containing zero and non-UTF-8 bytes.
- Truncated length fields and TTL.
- Incorrect declared key and value lengths.
- Invalid TTL flags and zero TTL.
- Successful and failed response round-trips.
- Invalid response opcode, status, and trailing bytes.
- Maximum frame size enforcement by the framing layer.
