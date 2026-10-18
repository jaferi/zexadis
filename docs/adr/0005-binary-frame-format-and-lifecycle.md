# ADR-0005: Binary Frame Format and Lifecycle

- **Status:** Proposed
- **Date:** 2026-10-15

## Context

Zexadis exposes its in-memory storage engine through a network server. The server needs a compact, binary protocol that supports arbitrary binary keys and values, handles TCP stream behavior correctly, and can evolve without breaking clients unnecessarily.

TCP provides a continuous byte stream rather than discrete messages. A single read may return only part of a frame or several frames together. Therefore, the protocol must define explicit frame boundaries and a lifecycle for decoding, validating, processing, and responding to requests.

The protocol implementation belongs in `zexadis-protocol`. The `zexadis-server` crate owns TCP connections and invokes the protocol codec and `zexadis-core`. Core storage operations must remain independent of networking and frame encoding.

## Decision

### 1. Frame Format

Zexadis will use a length-prefixed binary frame format over TCP.

Each frame has the following structure:

| Field | Size | Description |
|---|---:|---|
| Frame length | 4 bytes | Unsigned big-endian integer representing the number of bytes following this field |
| Protocol version | 1 byte | Identifies the wire-protocol version |
| Opcode | 1 byte | Identifies the request or response message type |
| Payload | Variable | Encoded message-specific fields |

The minimum valid frame length is 2 bytes, accounting for the version and opcode fields. The total frame size is the four-byte length prefix plus the declared frame length.

The initial protocol version is `1`. Opcode assignments and payload schemas will be defined separately in the protocol specification.

The protocol is binary-first. Keys, values, and other binary data must not require UTF-8 encoding.

### 2. Frame Size Limits

The server must enforce a configurable maximum frame size before allocating memory for the complete frame.

The initial default maximum frame length is **8 MiB (8,388,608 bytes)**, including the version and opcode fields but excluding the four-byte length prefix.

Frames declaring a length smaller than 2 bytes or larger than the configured maximum are invalid. The server must reject them without attempting to allocate the declared payload size.

The limit applies to individual frames. Batch requests must also obey this limit; a batch cannot bypass it by containing many smaller commands.

### 3. Frame Decoding

The decoder must:

1. Read the four-byte length prefix.
2. Validate the declared length against the minimum and configured maximum.
3. Read exactly the declared number of subsequent bytes.
4. Extract and validate the protocol version and opcode.
5. Decode and validate the opcode-specific payload.
6. Produce a typed protocol message only when the complete frame is valid.

The decoder must correctly handle partial TCP reads and multiple frames received in a single read. It must not assume that a single socket read corresponds to one frame.

Malformed, truncated, unsupported, or invalid messages must not reach core storage execution.

An incomplete frame is not considered malformed merely because it arrives in multiple TCP reads. If the connection closes before the frame is complete, the server treats it as a truncated frame and closes the connection.

### 4. Frame Lifecycle

A request follows this lifecycle:

1. **Receive:** The server accepts bytes from a TCP connection.
2. **Frame:** The server reads the length prefix and obtains the complete frame.
3. **Validate:** The protocol layer validates frame size, version, opcode, and payload.
4. **Decode:** The request becomes a typed protocol message and, where applicable, a core `Command`.
5. **Execute:** The server passes the command to `zexadis-core`.
6. **Encode:** The result is converted into a versioned binary response frame.
7. **Send:** The server writes the complete response to the connection.
8. **Continue:** The server processes the next request while the connection remains open.

The initial implementation processes requests sequentially within each connection. Multiple connections may be served concurrently. Core storage retains responsibility for safe concurrent access across connections.

### 5. Response and Error Handling

Every valid request that reaches execution must produce one corresponding response, including requests whose execution returns a core error.

Protocol-level errors must be distinguishable from core execution errors. Invalid frames may be rejected by closing the connection when a safe, unambiguous response cannot be constructed.

Unsupported protocol versions, unknown opcodes, and invalid payloads must never be silently interpreted as valid commands.

If a response cannot be written because the peer disconnects or a socket error occurs, the server terminates processing for that connection. It must not retry a storage mutation automatically merely because the response was not delivered; the client may otherwise cause a duplicate operation.

### 6. Connection Lifecycle

The server accepts multiple TCP connections and processes each independently.

The initial implementation will:

- Process requests sequentially per connection.
- Support multiple frames over one connection.
- Close connections on unrecoverable framing errors, truncated frames, or transport failures.
- Apply configured limits before allocating memory for incoming frames.
- Support graceful server shutdown and release connection resources.

Authentication, TLS, connection pooling policies, request pipelining, and concurrent in-flight requests are outside the scope of this decision.

### 7. Responsibilities

- **`zexadis-protocol`:** Frame representation, encoding, decoding, validation, opcode definitions, and protocol error types.
- **`zexadis-server`:** TCP listener, connection management, frame read/write lifecycle, execution dispatch, logging, and shutdown.
- **`zexadis-core`:** Storage commands, results, TTL, CAS, batches, and concurrency semantics. It must not depend on the network protocol.

### 8. Frame Representation and Codec Implementation

The `zexadis-protocol` crate will separate frame representation from frame encoding and decoding.

#### `framing.rs`

This module defines:

- `ProtocolVersion`: supported wire-protocol versions, starting with V1 (`0x01`).
- `Opcode`: explicitly assigned numeric opcodes for requests and responses.
- `Frame`: the protocol version, opcode, and binary payload.
- `FrameError`: frame construction and validation errors.
- Frame size constants and validation logic.

The frame representation is:

```rust
pub struct Frame {
    pub version: ProtocolVersion,
    pub opcode: Opcode,
    pub payload: Bytes,
}
```

`Frame::frame_length()` returns the encoded body length, including the version and opcode but excluding the four-byte length prefix. `Frame::validate(max_frame_length)` validates the configured size limit and the constraints of the wire-format length field.

#### `encoding.rs`

This module implements frame-level encoding and decoding.

The intended public API is:

```rust
pub fn encode_frame(frame: &Frame) -> Result<Bytes, FrameError>;

pub fn decode_frame(input: Bytes) -> Result<Frame, DecodeError>;
```

The concrete decoding error type may distinguish incomplete input from invalid frames.

`encode_frame` writes the four-byte big-endian length prefix, version byte, opcode byte, and payload into a contiguous byte buffer.

`decode_frame` validates the declared length before accepting the frame, then validates the protocol version and opcode. It must reject input that does not represent exactly one complete frame.

The decoder accepts an owned `Bytes` buffer and extracts the payload using a shared slice, such as `input.slice(payload_start..payload_end)`. This avoids copying the payload during frame decoding.

Zero-copy payload extraction does not eliminate copies that may be required when accumulating TCP input or constructing an encoded output frame.

#### Command encoding

Command-specific payload encoding and decoding belongs in `command.rs`, separate from frame-level encoding.

The command codec converts between valid request frames and core command types. It must validate the complete payload before returning a command for execution.

Command-specific request and response layouts are defined in their respective ADRs.

### 9. Opcode and Payload Specification

Opcode values and command payload layouts must be explicitly defined and stable within a protocol version.

Each command specification must document:

- Request and response opcodes.
- Request payload field order, sizes, and byte order.
- Response payload field order, sizes, and byte order.
- Validation requirements and error behavior.
- Command execution semantics relevant to protocol clients.

Unknown opcodes must be rejected. Payloads with truncated fields, invalid lengths, or unexpected trailing bytes must not be accepted as valid commands.

## Consequences

### Benefits

- Explicit frame boundaries over TCP.
- Compact binary encoding without mandatory text serialization.
- A defined maximum frame size limits per-frame memory consumption.
- Protocol versioning creates a path for future evolution.
- Clear separation between wire format, networking, and storage.
- Multiple requests can reuse a connection without introducing request concurrency initially.

### Trade-offs

- A length-prefixed protocol requires careful handling of partial reads and writes.
- The frame size limit constrains individual requests and responses.
- Sequential processing within a connection limits concurrency on that connection.
- Version and opcode compatibility must be maintained deliberately.
- Closing a connection after an ambiguous failure may require clients to reconnect and determine whether an operation took effect.

## Alternatives Considered

### Newline-delimited JSON

Rejected because binary values require additional encoding, message framing is less natural for arbitrary binary payloads, and serialization adds unnecessary overhead to the intended binary-first protocol.

### Fixed-size frames

Rejected because command and value sizes vary significantly, making fixed-size frames wasteful or restrictive.

### Unframed messages

Rejected because TCP does not preserve message boundaries and the receiver cannot reliably determine where one request ends and another begins.

### One connection per request

Rejected because connection setup and teardown would add unnecessary overhead for latency-sensitive clients.

## Validation

The implementation must include tests covering:

- Correct encoding and decoding of valid frames.
- Minimum and maximum permitted frame lengths.
- Rejection of undersized and oversized frames.
- Partial length-prefix and payload reads.
- Multiple consecutive frames in one byte stream.
- Unsupported versions and unknown opcodes.
- Malformed payloads rejected before core execution.
- Truncated frames when a connection closes.
- Response encoding for successful operations and core errors.
- Connection closure after unrecoverable protocol or transport errors.
- Multiple clients operating concurrently.
- A request mutation followed by a failed response write must not trigger an automatic retry by the server.

## Revisit Criteria

Revisit this decision if benchmarks show framing or allocation overhead is material, if clients require concurrent in-flight requests, if large-value workloads require a different transfer strategy, or if protocol evolution requires negotiation beyond a single version byte.