# d-31: sc-otel cli

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 4 (d-29 → d-33 → d-30 → d-31 → d-32; wave-5 ruling R12)
- Assignee / model: cobs / terra (difficulty: normal)
- Closure: `boundary` (consumer)
- Target boundary: `BOUNDARY-ScOtelCli`
- Branch: `sprint/d-31-sc-otel-cli`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-31-sc-otel-cli`
- PR target: `sprint/d-30-python-telemetry-bindings` (stack order only; no code dependency on d-30)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHD-005, PHD-006, PHD-007, PHD-008, PHD-010, PHD-013
- ADRs: ADR-018, ADR-019, ADR-021
- Owned paths:
  - `crates/sc-otel-cli/src/**`
  - `crates/sc-otel-cli/tests/**`
  - `docs/plans/phase-d/sprint-d-31-sc-otel-cli.md`
- Not owned: `crates/sc-otel-cli/Cargo.toml` and `boundaries/sc-otel-cli/**`
  (d-29).

## Relations

- `must_follow` d-29: consumes the `sc-otel-cli` skeleton and manifest
  (dependencies `clap` and `serde_json` included; none is added here), the
  `TelemetryClient` trait with `flush_submission` and the flush result rules,
  `SubmissionEnvelope::from_json`, `resolve_config`, `load_telemetry_file`
  (for `--config`), the CLI contract (subcommand and flag table, exit-code
  table), the `sc-otel.result/v1` schema, `InMemoryTelemetryClient`,
  `DoubleScript` and the golden fixtures.
- `parallel_safe` with d-33 and d-30: the owned paths are disjoint.

## Goal

Ship the `sc-otel` binary as a thin consumer of the d-29 contract.

## Deliverables

1. Implement the subcommands, global flags and per-subcommand flags exactly
   as the d-29 "CLI contract" table defines them, with `clap` derive. Flags
   become `ConfigOverrides` for `resolve_config`. [PHD-010]
2. Implement `emit` and `validate` input: `--stdin` (one `SubmissionInput`
   document) or the fragment flags, assembled per the d-29 table into one
   `SubmissionInput` and passed through the same
   `SubmissionEnvelope::from_json` as stdin. Default `emit` admits, then calls
   `flush_submission(receipt.submission_id, emit_flush_deadline)`;
   `--no-flush` only admits. [PHD-005, PHD-006, PHD-010]
3. Implement the exit-code mapping in `crates/sc-otel-cli/src/exit.rs`
   (`fn exit_code(&TelemetryClientError) -> u8`, per the d-29 table; the types
   crate carries no exit policy) and print the `sc-otel.result/v1` object on
   stdout (`--output json`, the default) or a one-line summary
   (`--output text`). Credentials are redacted in every output and error
   path. [PHD-008, PHD-010]
4. Make the client backend injectable. Production opens
   `DurableTelemetryClient`; tests build the binary with the dev-only
   `test-double` feature (declared by d-29 as
   `test-double = ["sc-observability-types/test-double"]`), which opens
   `InMemoryTelemetryClient::with_script` with the `DoubleScript` JSON file
   named by `SC_OTEL_TEST_DOUBLE` (`DoubleScript::from_json`; no script file
   means the default script). That variable is read only in the
   `cfg(feature = "test-double")` build, never in release. [PHD-010]

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
  flag, a missing `store_path`, and `DoubleScript` files for an admission
  rejection (`{"admissions":[{"outcome":"reject","kind":"disk_bound_exceeded"}]}`
  → 5), a stall (→ 6), a fail (→ 7) and a fail on one signal with a stall on
  another (→ 7, the d-29 precedence). Every stdout parses against
  `sc-otel.result/v1`. An `auth_header` value never appears in stdout or
  stderr.
- [ ] boundary:BOUNDARY-ScOtelCli (D1, D2): `tests/flags.rs` covers every
  flag in the d-29 CLI table, including `--stdin` with fragment flags
  rejected (exit 2), `--record-key`, repeated `--submission`, `@file`
  fragments and a second `--profile` rejected.
- [ ] boundary:BOUNDARY-ScOtelCli: `cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'`
  lists exactly `sc-otel-cli`, `sc-observability-types`,
  `sc-observability-otlp`, `clap` and `serde_json`, and
  `cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'`
  contains no `sc-observe`, `pyo3` or `agent-team-mail`. No `todo!` or
  `unimplemented!` remains.
- [ ] boundary:BOUNDARY-ScOtelCli (no caller runtime): `rg -n 'tokio' crates/sc-otel-cli`
  finds nothing: the binary never creates a Tokio runtime. d-32 proves the
  installed binary delivers to a real collector.
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-31 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-types --crate sc-observability-otlp`
  reports the `api_sha256` values recorded in the signed d-29 approval
  record. `sc-otel-cli` is `publish = false`, so it is outside the public-API
  policy, and it has only the `sc-otel` bin target.
- [ ] boundary:BOUNDARY-ScOtelCli: `git diff <d-29 head> -- Cargo.toml Cargo.lock crates/sc-otel-cli/Cargo.toml`
  is empty.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-otel-cli --all-targets --all-features -- -D warnings
cargo test -p sc-otel-cli --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'
cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-types --crate sc-observability-otlp
```
