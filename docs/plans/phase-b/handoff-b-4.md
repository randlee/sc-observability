---
id: B.4-python-handoff
status: in_progress
branch: feature/phase-b-4-python
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-4-python
parent: feature/phase-b-3a-typescript
---

# Python runtime handoff

The B.4 layer provides the locked PyO3 `abi3-py310` mixed `cdylib`/`rlib`
project, `Ok`/`Err` public facade, generated frozen DTO data, independently
owned loggers, and non-owning Rust-host attachments. Owned lifecycle and level
authority stay with `CoreLoggerOwner`; attached handles retain only a shared
`HostLoggingBackend` Arc and cannot stop or mutate the host.

Current source evidence is `7ac8c57c241922c4a7d3e8d6707187d98099e683`.
It carries frozen B.3, B.3b and telemetry/copy ancestors while retaining the
direct TypeScript parent. The facade contains no authored validation `raise`:
malformed Python values, hostile mappings/accessors, foreign native exceptions,
malformed native payloads and contained native panics return tagged failures.

## Executed gates

`bash scripts/ci/validate_python_bindings.sh` passes with CPython 3.10.21:
the generated/stub checks and strict Result narrowing fixture pass; eight
facade adversarial tests pass; a locked wheel is built and installed into a
clean environment for four owned lifecycle/isolation/synchronized-N=32,
level-transition and lifecycle-race tests; native binding tests pass; and
`cargo run -p rust-python-logging` proves a single writer accepts correlated
Rust and Python records, redacts both bearer values, supports querying and
retains attached health after host shutdown.

Native attachment tests prove missing-host tagging, immutable duplicate install,
attached admission, retained health after host shutdown, eight concurrent
install callers with one winner and seven exact duplicate-install failures, and
32 simultaneous attached PyO3 producer calls with 32 tagged admissions. The
host install check-and-install transition is serialized without storing a global
backend, so free-threaded interpreter contenders cannot replace a module slot.

This handoff remains an implementation evidence record until coordinator
completeness review and the sprint closeout update are complete.
