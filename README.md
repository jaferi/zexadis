# Zexadis

> A lightweight, binary-first hot-state store for latency-sensitive infrastructure.

Zexadis is a high-performance, in-memory state store designed for workloads where low latency, predictable behavior, and efficient binary data handling matter.

It is intended for infrastructure such as:

* RPC nodes
* MEV and searcher infrastructure
* L2 sequencers
* Indexers
* Transaction and simulation systems
* AI workers and agents

Zexadis is **not intended to be a general-purpose Redis replacement**. It focuses on a smaller command surface and a binary-first data path for latency-sensitive workloads.

## Status

🚧 **Early development**

The project is currently in the architecture and core-engine development stage. APIs, protocol formats, and implementation details are subject to change.

## Design Goals

* Binary-first data handling
* Low allocation and copy overhead
* Predictable latency
* High concurrency
* Atomic operations
* Small and focused command surface
* Simple containerized deployment
* Benchmark-driven optimization

## Architecture

Zexadis is organized as a Cargo workspace:

```text
crates/
├── core/       # Storage engine and state management
├── protocol/   # Binary protocol and framing
└── server/     # TCP server and connection handling
```

The initial implementation is in-memory and uses sharded state for concurrent access.

## Development

Build the workspace:

```bash
cargo build --workspace
```

Run tests:

```bash
cargo test --workspace
```

Check the workspace:

```bash
cargo check --workspace
```

## Benchmarking

Performance is a core part of the project.

Optimization decisions should be supported by reproducible benchmarks rather than assumptions about which technologies or implementations are faster.

Benchmarking infrastructure will be added as the core engine develops.

## Roadmap

The initial roadmap includes:

* [ ] In-memory storage engine
* [ ] Sharded state
* [ ] GET / SET / DEL / EXISTS
* [ ] CAS / INCR
* [ ] Batch operations
* [ ] Binary protocol
* [ ] Tokio TCP server
* [ ] Metrics and tracing
* [ ] Performance benchmarks
* [ ] Containerized deployment

Persistence, replication, clustering, and other distributed features are intentionally outside the initial scope.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## Security

For information about reporting security vulnerabilities, see [SECURITY.md](SECURITY.md).

## License

Zexadis is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.
