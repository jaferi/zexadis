# ADR-0002: Command and Batch Execution Semantics

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

Zexadis exposes storage operations through a command-based API. Supported commands include `GET`, `SET`, `DEL`, `EXISTS`, `CAS`, and `BATCH`.

The engine needs predictable execution semantics, particularly for batches. Without explicit rules, nested batches can introduce recursive execution complexity, while ambiguous failure behavior can make it difficult for clients to determine which operations have taken effect.

Zexadis prioritizes a small, predictable execution model over complex transaction semantics.

## Decision

### 1. Command execution

The storage engine executes individual commands by routing them to the shard responsible for their key.

Ordinary operational outcomes are represented by `CommandResult`. Invalid requests or execution-validation failures are represented by `StorageError`.

Examples:

- A missing key returns `NotFound` for `GET`.
- A missing key returns `Deleted(false)` for `DEL`.
- A failed compare-and-swap returns `Cas(false)`.
- An invalid TTL returns `StorageError::InvalidTtl`.

These outcomes are distinct from protocol-level errors, which will be handled by the protocol layer.

### 2. Batch execution

A batch contains a sequence of ordinary commands executed sequentially in the supplied order.

The following rules apply:

1. Commands execute in order.
2. Each command's result appears at the corresponding position in the batch result.
3. Later commands observe the effects of earlier successfully executed commands.
4. Nested batches are prohibited.
5. The complete batch is validated for nested batches before any command executes.
6. Batches are not transactional and provide no cross-shard atomicity.
7. The same `now_ms` value supplied to the outer batch is used for every command within that batch.

### 3. Error handling

If a batch contains a nested batch, execution is rejected with `NestedBatchNotAllowed` before any command executes.

If a command produces an execution error after validation, the batch returns that error. Commands successfully executed before the failure are not rolled back.

The engine does not promise transaction-like rollback or all-or-nothing execution.

### 4. Batch scope

Batches are intended to reduce request overhead by grouping commands into a single request. They do not introduce transaction boundaries or guarantee atomic execution across keys or shards.

## Consequences

### Benefits

- Provides predictable command ordering and result ordering.
- Avoids recursive batch execution.
- Prevents invalid nested batches from causing partial side effects before validation fails.
- Keeps the execution model straightforward to test and maintain.
- Allows protocol-level batching without requiring distributed transaction machinery.

### Trade-offs

- A batch can partially execute if a command fails after earlier commands have succeeded.
- Concurrent operations may interleave with batch commands.
- No atomicity is guaranteed across different keys or shards.
- Clients must inspect results and account for partial execution when handling errors.

## Alternatives Considered

### Recursive batches

Allow batches to contain other batches.

Rejected because recursion complicates validation, execution limits, and result interpretation without providing transactional guarantees.

### Transactional batches

Execute all commands atomically, rolling back changes if any command fails.

Rejected for the initial implementation because transactions introduce additional coordination, locking, and rollback complexity, particularly when a batch spans multiple shards.

### Flatten nested batches

Convert nested batches into a single sequence before execution.

Rejected because implicit flattening can obscure the original command structure and complicate client expectations.

## Validation

The implementation will be validated with tests covering:

- Execution order and result ordering.
- Visibility of earlier writes to later commands in the same batch.
- Empty batches.
- Nested-batch rejection.
- Rejection before execution, with no side effects.
- TTL behavior using a consistent `now_ms`.
- Error propagation when a command fails after earlier commands have executed.

## Revisit When

Reconsider this decision if clients require transactional operations, cross-key atomicity, nested composition, explicit per-command error results, or different failure-handling semantics.