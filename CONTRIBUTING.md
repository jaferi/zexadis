# Contributing to Zexadis

Thank you for your interest in contributing to Zexadis.

Zexadis is an early-stage performance-focused project. During this stage, keeping the architecture small and understandable is especially important.

## Development

Make sure you have a recent stable Rust toolchain installed.

Check the workspace:

```bash
cargo check --workspace
```

Run tests:

```bash
cargo test --workspace
```

Format the code:

```bash
cargo fmt --all
```

Run Clippy:

```bash
cargo clippy --workspace --all-targets --all-features
```

## Architecture

Before introducing a new abstraction, dependency, or subsystem, consider whether it is required by an actual workload or benchmark.

In particular:

* Keep the core independent from networking.
* Keep protocol concerns separate from storage.
* Avoid unnecessary serialization and copying.
* Prefer simple implementations before specialized optimizations.
* Use benchmarks to justify performance-sensitive changes.

## Tests

New functionality should include appropriate tests.

Concurrency-sensitive behavior should have tests covering the relevant concurrent scenarios.

## Performance

Performance is a core project goal.

If a change is intended to improve performance, include benchmarks or measurements when practical.

Avoid optimizing based solely on assumptions. Prefer:

```text
measure → identify bottleneck → change → measure again
```

## Commits

Use clear, conventional commit messages.

Examples:

```text
feat(core): implement in-memory storage
fix(protocol): reject malformed frames
test(core): add concurrent CAS tests
bench(core): add storage throughput benchmark
docs: document binary value semantics
refactor(server): simplify connection handling
```

Keep commits focused on one coherent change.

## Pull Requests

Pull requests should explain:

* What changed
* Why it changed
* How it was tested
* Any performance impact
* Any architectural trade-offs

Large architectural changes should be discussed before implementation where practical.

## Architecture Decisions

Significant architectural decisions should be documented as ADRs under:

```text
docs/adr/
```

Keep the project intentionally small. New features should have a clear reason to exist.
