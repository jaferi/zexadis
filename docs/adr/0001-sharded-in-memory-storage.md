# ADR-0001: Sharded In-Memory Storage Architecture

- **Status:** Accepted
- **Date:** 2026-09-15

## Context

Zexadis is a high-performance binary state store designed for latency-sensitive infrastructure. Its core storage engine must support concurrent access while keeping synchronization overhead and operational complexity manageable.

A single shared map can become a contention point under concurrent workloads. A sharded design partitions keys across independent storage units, allowing operations on different shards to proceed concurrently.

The initial implementation uses a fixed collection of shards, with each shard maintaining its own concurrent in-memory map.

## Decision

Zexadis uses an explicitly sharded, in-memory storage architecture with the following properties:

1. **Fixed shard collection:** `Storage` owns a fixed number of independent shards, established during initialization.
2. **Power-of-two shard count:** The shard count must be non-zero and a power of two.
3. **Hash-based routing:** Keys are hashed to select their responsible shard. A bitmask maps the hash to a shard index.
4. **Independent concurrent maps:** Each shard uses `DashMap` to manage its entries concurrently.
5. **Shard-local operations:** Each key is handled by its owning shard. Operations on different shards do not require a global storage lock.
6. **In-memory operation:** The initial storage engine does not provide persistence or replication.

The current implementation uses `AHasher` for key routing and `DashMap` for per-shard storage. These are initial implementation choices, not permanent architectural requirements.

## Consequences

### Benefits

- Reduces the scope of contention by partitioning keys across independent shards.
- Enables concurrent access to different parts of the keyspace.
- Keeps key routing and storage responsibilities separate.
- Allows the hashing algorithm and concurrent-map implementation to be replaced without changing the overall architecture.

### Trade-offs

- Hashing and shard routing introduce per-operation overhead.
- Uneven key distributions or hot keys can cause shard-level contention.
- The shard count cannot change dynamically in the current design.
- Sharding does not guarantee uniform distribution or eliminate contention.
- The engine does not provide persistence, replication, or global transactions.

## Alternatives Considered

### Single shared concurrent map

Use one concurrent map for the entire keyspace.

Rejected as the initial architecture because explicit sharding provides a clear partitioning boundary and allows us to evaluate contention and concurrency behavior independently.

### One global lock around a standard hash map

Use a single `HashMap` protected by a mutex or read-write lock.

Not selected because a global lock can serialize unrelated operations and limit concurrent access.

### Custom sharded maps with explicit locks

Implement each shard using a standard hash map and a selected locking primitive.

Deferred. This may be worth benchmarking against `DashMap` if measurements show that its synchronization overhead or contention characteristics are unsuitable.

## Validation

The design will be evaluated through:

- Unit tests for valid shard counts and invalid shard-count rejection.
- Tests confirming that shard indices remain within bounds and routing is consistent within a storage instance.
- Concurrent correctness tests.
- Benchmarks covering throughput and latency percentiles across different shard counts, payload sizes, and concurrency levels.

No performance improvement is assumed without benchmark evidence.

## Revisit When

Reconsider this decision if benchmarks show that the current sharding strategy is a bottleneck, if dynamic shard resizing becomes necessary, or if persistence, replication, or distributed routing requirements emerge.