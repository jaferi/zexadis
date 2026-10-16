# Zexadis

**A lightweight, binary-first hot-state store for latency-sensitive infrastructure.**

Zexadis is a high-performance, in-memory state store designed for infrastructure where low latency, concurrent access, and efficient binary data handling matter.

It is designed to run alongside infrastructure such as:

- RPC nodes and blockchain clients
- MEV and searcher infrastructure
- L2 sequencers and transaction processors
- Blockchain indexers and simulation systems
- AI workers, agents, and model-serving infrastructure

Zexadis focuses on a small, purpose-built command surface and a binary-first data path. It is not intended to be a general-purpose Redis replacement.

## Status

**Early development — APIs and protocol are evolving.**

The core storage engine implements sharded in-memory storage, binary keys and values, TTL-based expiration, atomic compare-and-swap (CAS), and sequential batch execution.

The binary wire protocol and TCP server are under development. The project is not yet intended for production deployment.

## Design Goals

- **Binary-first:** Handle arbitrary binary keys and values without requiring text serialization.
- **Low overhead:** Minimize unnecessary allocations and data copies.
- **Predictable behavior:** Define expiration, concurrency, and command semantics explicitly.
- **Concurrent access:** Partition storage into independent shards.
- **Atomic operations:** Provide atomic compare-and-swap for individual keys.
- **Focused API:** Keep the command surface small and suited to hot-state workloads.
- **Network access:** Expose storage through a dedicated server and binary protocol.
- **Benchmark-driven optimization:** Validate performance decisions with reproducible measurements.

## Architecture

Zexadis is organized as a Rust Cargo workspace:

```text
crates/
├── core/       # In-memory storage engine and command execution
├── protocol/   # Binary frames, encoding, and protocol messages
└── server/     # TCP listener, connections, and request lifecycle
```

### Core

The core engine owns the in-memory state and executes storage commands.

- Sharded storage using `DashMap`
- Binary keys and values using `Bytes`
- TTL-based expiration with lazy removal
- Atomic compare-and-swap
- Sequential batch execution with ordered results
- Explicit validation for invalid TTLs and unsupported nested batches

The shard count is configured during initialization and must be a non-zero power of two.

Batch execution is ordered but not transactional. Operations completed before an execution error are not rolled back, and batches do not provide cross-shard atomicity.

### Protocol

The protocol crate defines the wire format used by network clients.

The initial design uses length-prefixed binary frames containing:

- A four-byte big-endian frame length
- A one-byte protocol version
- A one-byte opcode
- A variable-length binary payload

The initial maximum frame length is 8 MiB, excluding the four-byte length prefix.

Frame encoding, decoding, validation, and command payload schemas are being developed separately from the storage engine and TCP server.

### Server

The server crate will provide the runnable `zexadis-server` executable.

Its responsibilities include:

- Accepting TCP connections
- Reading and writing complete protocol frames
- Validating and dispatching requests
- Executing commands through the core engine
- Returning encoded responses
- Handling connection failures and graceful shutdown

The intended initial model is sequential request processing per connection, with multiple connections served concurrently.

## Development

Build the workspace:

```bash
cargo build --workspace
```

Run all tests:

```bash
cargo test --workspace
```

Check the workspace:

```bash
cargo check --workspace
```

Run formatting checks:

```bash
cargo fmt --all -- --check
```

Run Clippy:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Performance

Performance is a core design objective, but performance claims must be supported by measurements.

The project will use reproducible benchmarks to evaluate:

- GET and SET throughput
- Read and write latency, including tail latency
- CAS performance under contention
- Batch execution overhead
- Performance across different shard counts
- Concurrent-client throughput
- Protocol encoding and decoding overhead
- Allocation and memory overhead

Implementation choices such as hashing, sharding, and data representation should be revisited when benchmark results provide evidence for improvement.

## Roadmap

- [x] Initial in-memory storage engine
- [x] Sharded state
- [x] GET / SET / DEL / EXISTS
- [x] TTL-based expiration
- [x] Atomic CAS
- [x] Sequential batch operations
- [x] Core semantic tests
- [x] Initial architecture decision records
- [ ] Finalize binary frame and command specifications
- [ ] Complete protocol encoding and decoding
- [ ] Implement the `zexadis-server` TCP executable
- [ ] Add end-to-end TCP integration tests
- [ ] Add graceful shutdown and operational logging
- [ ] Add metrics and tracing
- [ ] Establish reproducible performance benchmarks
- [ ] Add containerized deployment

The roadmap describes project milestones, not guarantees about release dates.

Persistence, replication, clustering, and distributed transactions are outside the initial scope.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## Security

See [SECURITY.md](SECURITY.md) for information about reporting security vulnerabilities.

Until authentication and transport security are implemented, the server should not be exposed to untrusted networks.

## License

Zexadis is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.