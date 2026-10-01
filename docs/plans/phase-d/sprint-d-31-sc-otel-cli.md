# d-31: sc-otel cli

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Layer: 4 of the wave-5 stack (d-29 → d-33 → d-30 → d-31 → d-32)
- Assignee / model: lobs / luna (difficulty: fast)
- Closure: `boundary` (consumer)
- Target boundary: `BOUNDARY-ScOtelCli`
- Branch: `sprint/d-31-sc-otel-cli`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-31-sc-otel-cli`
- PR target: `sprint/d-30-python-telemetry-bindings` (stack order only; no code dependency on d-30)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHD-005, PHD-006, PHD-007, PHD-008, PHD-010, PHD-013
- ADRs: ADR-021
- Owned paths:
  - `crates/sc-otel-cli/src/**`
  - `crates/sc-otel-cli/tests/**`
  - `docs/plans/phase-d/sprint-d-31-sc-otel-cli.md`
- Not owned: `crates/sc-otel-cli/Cargo.toml` and `boundaries/sc-otel-cli/**`
  (d-29).

## Relations

- `must_follow` d-29: consumes the `sc-otel-cli` skeleton and manifest, the
  `TelemetryClient` trait, `SubmissionEnvelope::from_json`, `resolve_config`,
  `TelemetryClientError::exit_code`, the exit-code table, the
  `sc-otel.result/v1` schema, `InMemoryTelemetryClient` and the golden
  fixtures.
- `parallel_safe` with d-33 and d-30: the owned paths are disjoint.

## Goal

Ship the `sc-otel` binary as a thin consumer of the d-29 contract.

## Deliverables

1. Add the subcommands `emit`, `validate`, `flush` and `status`, plus the
   global flags `--config`, `--store`, `--endpoint` and `--output json|text`.
   Flags become `ConfigOverrides` for `resolve_config`. [PHD-010]
2. Make `emit` accept structured stdin (`--stdin`, a `SubmissionInput` JSON
   document) and the convenience flags `--log`, `--span`, `--metric` and
   `--profile` (each a JSON fragment or `@file`). Flags are assembled into one
   `SubmissionInput` and go through the same `SubmissionEnvelope::from_json`
   as stdin. Default emit admits, then flushes within `emit_flush_deadline`;
   `--no-flush` only admits. [PHD-005, PHD-006, PHD-010]
3. Map results to the d-29 exit-code table and print the
   `sc-otel.result/v1` object on stdout (`--output json`, the default) or a
   one-line summary (`--output text`). Credentials are redacted in every
   output and error path. [PHD-008, PHD-010]
4. Make the client backend injectable. Production opens
   `DurableTelemetryClient`; tests build the binary with the dev-only
   `test-double` wiring, which opens `InMemoryTelemetryClient` with a scripted
   outcome file passed through `SC_OTEL_TEST_DOUBLE`. That variable is read
   only in the `cfg(test-double)` build, never in release. [PHD-010]

## This Sprint Does Not Close

- Real collector delivery, offline recovery against the real store, shared
  store access with Python, and viewer readback. d-32 owns them.
- Python/CLI equivalence. d-32 owns it.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScOtelCli (D1, D2): `crates/sc-otel-cli/tests/installed_golden.rs`
  installs the binary with
  `cargo install --path crates/sc-otel-cli --root <tmp> --locked` and runs
  `<tmp>/bin/sc-otel validate --stdin` from a temp directory outside the
  workspace, once per d-29 golden fixture. Stdout `envelope` equals
  `expected.envelope.json`, or the exit code and `error.code` match
  `expected.error.json`. Fixtures that have a `flags.args` file produce the
  same envelope through `--log/--span/--metric/--profile`.
- [ ] boundary:BOUNDARY-ScOtelCli (D2, D4), **direct CLI emission**:
  `tests/direct_emission.rs` runs the test-double build of `sc-otel emit` once
  each with a log, a span, a metric and a profile flag, and once with combined
  stdin. Each prints `state: admitted_delivered` and exit 0. The double
  records exactly the envelope that `validate` printed. This is the
  assignment for "direct CLI emission".
- [ ] boundary:BOUNDARY-ScOtelCli (D3): `tests/exit_codes.rs` asserts every row
  of the exit-code table (0, 2, 3, 4, 5, 6, 7) using malformed stdin, a bad
  flag, a missing `store_path`, a scripted admission failure, a scripted
  deadline and a scripted terminal failure. Every stdout parses against
  `sc-otel.result/v1`. An `auth_header` value never appears in stdout or
  stderr.
- [ ] boundary:BOUNDARY-ScOtelCli: `cargo tree -p sc-otel-cli -e normal` has no
  `sc-observe` or `pyo3`. No `todo!` or `unimplemented!` remains.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-otel-cli --all-targets --all-features -- -D warnings
cargo test -p sc-otel-cli --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
```
