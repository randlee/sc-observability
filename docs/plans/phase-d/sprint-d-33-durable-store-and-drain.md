# d-33: durable store and drain

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 2 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: aobs / astra
- Difficulty: `hard` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `boundary` (implementer)
- Target boundary: BOUNDARY-ScObservabilityOtlp
- Branch: `sprint/d-33-durable-store-and-drain`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-33-durable-store-and-drain`
- PR target: `sprint/d-29-telemetry-submission-contract`
- Blocked by: `obs-d-29-sanity`
- Requirements: PHB-010, PHB-011, PHD-002, PHD-003, PHD-004, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-013
- ADRs: ADR-004, ADR-005, ADR-009, ADR-012, ADR-017, ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - crates/sc-observability-otlp/src/durable/**
  - crates/sc-observability-otlp/src/contracts/credits.rs
  - crates/sc-observability-otlp/src/contracts/submission.rs
  - crates/sc-observability-otlp/src/lifecycle.rs
  - crates/sc-observability-otlp/tests/fixtures/telemetry_yaml/**
  - .github/workflows/telemetry-platforms.yml

Ownership notes: `src/durable/schema.sql` is d-29's contract DDL and stays
read-only. In `src/lifecycle.rs` and `src/contracts/submission.rs` d-33
changes only the d-29 staging attribute; in `src/contracts/credits.rs` it
implements `wait_for_release`, the budget's `Condvar` and their tests. See
"Crate-private items d-33 may change".

## Relations

- `must_follow` d-29: consumes the `TelemetryClient` trait,
  `SubmissionEnvelope`, `AdmissionReceipt`, `DeliveryState`, `StoreStatus`,
  the error enums and their codes, `TelemetryClientConfig`,
  `TelemetryFileConfig` and the `load_telemetry_file` signature,
  `schema.sql`, the staged `durable/mod.rs`, `durable/adapter.rs` and
  `durable/config_file.rs`, the staged crate-private seams
  (`SignalKind::Profiles`, `AdmissionCredits::wait_for_release`,
  `SubmissionExporter`, `SubmissionExportFailure`, `exporter_for`), the
  wave-5 constants and error codes, the committed dependency set (no
  dependency is added here), and `otlp::submission::testing::conformance`
  with `ConformanceHarness` in `sc-observability-types` (feature
  `test-double`).
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-33,
  d-34, d-30 and d-31 can run in parallel. Their owned paths are disjoint,
  and all four consume only d-29 artifacts. d-33 calls the d-29
  `SubmissionExporter` seam and never d-34 code by name; d-32 composes them.
- d-18 fence: d-18 owns `src/lib.rs`, `src/assembly.rs`, `src/config.rs`
  (now `src/config/`), `src/contracts.rs`, `src/projectors.rs`,
  `tests/composition.rs` and `tests/full_stack_integration.rs`. d-33 edits
  none of them. The `contracts.rs` module lines and the `lib.rs` `durable`
  registration are added by d-29 under the P2 merge-forward rule.
  `src/lifecycle.rs` and `src/contracts/credits.rs` are outside the d-18
  fence.

## Crate-private items d-33 may change

d-33 builds on the seams d-29 staged (d-29 "Crate-private seams"; wave-5
ruling R14). Their signatures are frozen; a needed signature change is a
contract change routed to the lead. d-33 may change only:

| File | Item | Allowed change |
| --- | --- | --- |
| `src/durable/**` except `schema.sql` | `DurableTelemetryClient` method bodies, `otel_config_from` body, `load_telemetry_file` body, private items | implement; public signatures stay as in d-29 |
| `src/contracts/credits.rs` | `AdmissionCredits::wait_for_release` body, the budget `Condvar`, `CreditLease::drop` notification, their unit tests | implement (d-29 staged the signature only) and replace the staging attribute |
| `src/contracts/submission.rs` | the staging attribute on `SubmissionExporter` | replace `#[expect(dead_code, ...)]` with `#[cfg_attr(not(feature = "durable-store"), expect(dead_code, reason = "..."))]` |
| `src/lifecycle.rs` | the staging attribute on `SignalKind::Profiles` | same replacement |

Nothing else under `src/` changes: not `contracts.rs`, `ExporterSet`,
`lib.rs`, `config/**`, `runtime.rs`, `sync_http/**` (d-34) or
`contracts/profiles.rs` (d-34).

## Goal

Implement `DurableTelemetryClient` in `sc-observability-otlp` behind
`durable-store`: the SQLite store, the drain worker with lease and claims,
bounded backpressure, the `telemetry.yaml` loader and the capability check,
exporting through the d-29 `SubmissionExporter` seam.

## Deliverables

1. Implement `DurableTelemetryClient::{open, emit, flush, flush_submission,
   shutdown, status}` in `durable/` against the committed `schema.sql`.
   `emit` commits envelope and delivery rows in one `synchronous=FULL`
   transaction before returning the receipt; every submitted signal (logs,
   spans, metrics, profiles, alone or together) gets its own delivery row.
   A duplicate `RecordKey` returns the original receipt with
   `duplicate = true`. Enforce reject-newer schema and envelope versions.
   Implement the d-29 flush result rules. Implement
   `durable::adapter::otel_config_from`; `open` builds the production
   exporter with `exporter_for(worker, bounds)` from the destructured `SyncHttpConfig::from_otel(..)` result and calls the
   crate-private `open_with_exporter(config, Arc<dyn SubmissionExporter>)`.
   [PHD-005, PHD-007, PHB-010]
2. Implement the drain worker and multi-process ownership: drain lease with
   expiry, row claims, `SubmissionExportFailure::Retryable` → `retry` with
   `next_attempt_at` (bounded by `SyncHttpRetryPolicy`), `Terminal` →
   `failed`, at-least-once delivery with the duplicate window documented
   below, and resumption after a crash or exit. The transport owns the network
   retry budget: completed `RetryAttemptsExhausted` and `RetryDeadlineExhausted`
   results are terminal to the drain and must not start another transport
   sequence. Explicit transport shutdown returns interrupted rows to pending
   without charging a persisted attempt and stops that signal worker; after
   client shutdown releases the lease, a replacement client can reclaim them.
   Process interruption retains the existing lease-expiry recovery behavior.
   [PHD-008, PHB-011]
3. Implement the layering and backpressure: store → drain worker →
   `AdmissionCredits::reserve` → `SubmissionExporter::export`. Implement
   `AdmissionCredits::wait_for_release` (staged by d-29 as a signature only):
   the budget gains a `Condvar`, `CreditLease::drop` notifies, and the call
   returns `true` when a release happened before `timeout`. The worker waits
   on it when credits are exhausted, with profiles accounted under
   `SignalKind::Profiles`. Add disk-bound accounting, delivered-row and
   record-key retention, and status counters. [PHD-004, PHD-007, PHB-011]
4. Implement `durable/config_file.rs` (`load_telemetry_file`, staged by d-29
   as a signature): parse `telemetry.yaml` with `serde-saphyr` into
   `TelemetryFileConfig`, ignoring consumer keys, resolving a relative
   `store.path` against the file's directory. The CLI `--config` flag and
   Python `Telemetry.open(config=...)` call it. [PHD-009, PHD-010]
5. Enforce the ADR-021 capability matrix at construction.
   `DurableTelemetryClient::open` with `config.backend` other than
   `ExporterBackendId::SyncHttp` returns
   `TelemetryConfigError::UnsupportedCombination`. No combination silently
   omits a signal. [PHD-003, PHD-006, PHD-013]
6. Add `.github/workflows/telemetry-platforms.yml` (`workflow_dispatch`
   only, input `source_commit`), which checks the `durable-store` graph and
   builds `sc-otel` on the six platform targets in d-29 "Platform matrix".
   [PHD-003, PHD-013]

## This Sprint Does Not Close

- OTLP/JSON encoding, `ProfileExporter` and per-variant round trips. d-34
  owns them.
- Python bindings (d-30), the CLI (d-31) and the importer (d-35).
- Installed-front-end submission, viewer readback and the D18/D9 gate
  re-run (d-32).

## Design

The signatures are in the d-29 doc and are not restated here.

### Export seam and test strategy

- The drain calls only `SubmissionExporter::export(signal, envelopes)`.
  Production wiring is `exporter_for` (d-34 implements it). At the d-33
  head, before d-34 lands in the stack, the staged `exporter_for` returns an
  exporter whose `export` yields `SubmissionExportFailure::Terminal` with the
  d-29 code `SC_OBSERVABILITY_OTLP_SUBMISSION_EXPORT_UNWIRED`, so nothing
  reports false success.
- d-33's tests are lib unit tests under `src/durable/tests/` (`#[cfg(test)]`),
  because the seam is crate-private and the public surface is frozen. They
  open the client with `open_with_exporter` and a `ScriptedExporter`:
  `Deliver` returns `Ok`, `Fail` returns `Terminal`, `Stall` blocks until the
  harness releases it (after the call's deadline), and `Retry` returns
  `Retryable`. Each delivery is appended as one line to a file in the test's
  temp directory, so deliveries are counted across processes.
- Multi-process cases re-run the lib test binary
  (`std::env::current_exe()`) as a child with `--exact <child test> --ignored`
  and the store path in an environment variable. The child tests are
  `#[ignore]` so they run only when invoked that way.

### Layering and backpressure

- **Drain loop.** One drain worker runs per lease holder. It claims up to
  the d-29 `DRAIN_BATCH_SIZE` ready rows per signal, ordered by
  `admitted_at`, reserves record and byte credits
  (`AdmissionCredits::reserve`), and hands each batch to
  `SubmissionExporter::export`.
- **Backend queue full.** When credits are exhausted, the worker pauses that
  signal and blocks in `AdmissionCredits::wait_for_release` until the next
  credit release. It never drops or evicts. Store rows stay `claimed` until
  their claim expires or the batch completes.
- **Disk bound.** The store holds unexported rows until they are delivered,
  fail terminally, or are evicted under the disk bound. At
  `max_store_bytes`, `RejectNew` (default) returns
  `AdmissionError::DiskBoundExceeded` and increments `rejected_by_disk_bound`.
  `EvictOldest` deletes the oldest unexported submissions until the new one
  fits, sets their rows to `evicted`, and increments `evicted_by_disk_bound`.
  Both counters appear in `StoreStatus`.
- **Health counters.** The existing `TelemetryHealthReport` drop counters
  count only backend-level drops (a terminal backend rejection). Store-side
  rejection and eviction are counted only in `StoreStatus`, so nothing is
  counted twice.
- **Retention.** Delivered rows are purged after `delivered_retention` (24 h).
  `record_keys` rows are purged after `record_key_retention` (30 days).
  Pending rows never expire by age.
- **Constants.** Every non-trivial number (batch size, lease renewal
  divisor, busy timeout) is a d-29 entry in `src/constants.rs` (ADR-005).

### Multi-process ownership

- **Lease.** `drain_lease` holds a single row. A process acquires it when the
  row is absent or `expires_at < now`, inside an immediate transaction. The
  holder renews every `lease_duration / 3`. The holder ID is
  `<pid>:<uuid>`: `std::process::id()` and a UUIDv7 from the `uuid` crate
  (d-29 dependency set). No hostname is used.
- **Claims.** Only the lease holder claims rows. It sets `state='claimed'`,
  `claimed_by` and `claim_expires_at = lease expiry`. After a batch completes,
  rows move to `delivered`, `retry` (with `next_attempt_at`) or `failed`.
- **Retry.** The d-34 transport owns in-sequence retry; durable owns only the
  bounded, persisted retry count and backoff from named constants.
- **Takeover.** Claims held by an expired holder are reset to `pending` when
  the lease is taken over.
- **Flush and shutdown.** Scope and results follow the d-29 flush rules.
  `flush(deadline)` from a non-holder waits for the rows in its scope to
  reach a terminal state. If the lease becomes free or expires before the
  deadline, it acquires the lease and drains itself. `shutdown` flushes,
  stops the worker and deletes the lease row if it still holds it.
- **Duplicate window.** A crash or lease loss after the collector accepts a
  batch, but before the `delivered` update commits, resends that batch after
  takeover. So the window is at most one in-flight batch per signal per
  takeover. This is documented in rustdoc and ADR-021, and no exactly-once
  claim is made.

### Config loader

`load_telemetry_file(path)` reads the file, parses it with `serde-saphyr`
into `TelemetryFileConfig`, sets `base_dir` to the file's directory, and
maps I/O and parse failures to `TelemetryConfigError::ConfigFile { path }`.
Its unit tests use the d-33 fixtures in
`crates/sc-observability-otlp/tests/fixtures/telemetry_yaml/` (a full file
with consumer keys, a relative `store.path`, and a malformed file), never the
repository's `.sc/telemetry.yaml`.

## Acceptance criteria

All `durable::` tests below run with
`cargo test -p sc-observability-otlp --features durable-store --locked --lib durable::`.

- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1): `durable::tests::conformance`
  calls `sc_observability_types::otlp::submission::testing::conformance::run_all`
  against `DurableTelemetryClient` (fresh temp store per case, `tempfile`,
  and a `ScriptedExporter` implementing `ConformanceHarness::set_outcome`).
  These are the same cases the d-29 test double passes. The following cases
  also pass:
  - `receipt_after_commit`: a child process is killed after the receipt; on
    reopen the row is present.
  - `duplicate_record_key`
  - `each_signal_alone_and_combined_gets_rows` (logs, spans, metrics and
    profiles, one row per signal)
  - `schema_too_new_rejected_unmodified`
  - `newer_envelope_skipped_and_counted`
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1): `durable::adapter` unit
  tests assert that `otel_config_from` maps every `TelemetryClientConfig`
  field it carries, maps every one of the six `SyncHttpRetryPolicyDto` fields
  onto the same-named `SyncHttpRetryPolicy` field, and returns
  `TelemetryConfigError::InvalidField` for an out-of-range value.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D2): `durable::tests::drain`
  covers two concurrent drainers. `two_process_drainers_no_loss` runs two OS
  processes over one store file: every admitted submission is delivered at
  least once (counted in the delivery file), and the duplicates observed are
  at most one batch per signal per forced takeover. It also covers:
  - `lease_expiry_takeover`: an expired holder's claims are reset and
    delivered by the new holder.
  - `crash_mid_drain_resumes`
  - `non_holder_flush_waits_then_acquires`
  - `retryable_then_delivered` and `terminal_marks_failed`
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D3): `durable::tests::backpressure`
  covers backend saturation. `backend_queue_full_pauses_no_eviction`: with
  credits exhausted, selected store rows stay claimed and are never evicted, and they
  drain after release. It also covers `disk_bound_reject_new`,
  `disk_bound_evict_oldest_counted` and `retention_purges_delivered_only`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D3): with `durable-store` and
  with default features, `cargo test -p sc-observability-otlp --locked --lib contracts::credits`
  passes `wait_for_release_wakes_on_lease_drop` (a second thread drops a
  lease after a barrier; no sleeps) and
  `wait_for_release_zero_timeout_returns_false`, and the existing lifecycle
  tests pass unchanged.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D4): `durable::config_file`
  unit tests load the fixture files and show that unknown consumer keys
  (`team`, `github.*`, `sources[]`) are ignored, that a relative
  `store.path` resolves against the file's directory, that `otlp.timeout_ms`
  reaches `request_timeout` through `resolve_config`, and that malformed YAML
  returns `TelemetryConfigError::ConfigFile`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D5): `durable::tests::capability`
  asserts one case per ADR-021 matrix row. The sync-http drain accepts every
  signal and representation row. `DurableTelemetryClient::open` with
  `config.backend = ExporterBackendId::OpenTelemetrySdk` returns
  `TelemetryConfigError::UnsupportedCombination`, naming the backend. No row
  produces a silently omitted signal.
- [ ] boundary:ADR-021 (D6): `telemetry-platforms.yml` is dispatched on the
  d-33 head SHA (`gh workflow run telemetry-platforms.yml --ref sprint/d-33-durable-store-and-drain -f source_commit=<head>`),
  all six cells pass, and the run URL is recorded in the PR body.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp: no `todo!` or `unimplemented!`
  remains in `durable/`, and the boundary and dependency validators pass.
  `git diff <d-29 head> -- Cargo.toml Cargo.lock crates/sc-observability-otlp/Cargo.toml policy/`
  is empty, and the only changes outside `src/durable/**`,
  `tests/fixtures/telemetry_yaml/**` and the workflow are the items listed
  in "Crate-private items d-33 may change".
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-33 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp`
  reports the same `api_sha256` as the signed d-29 approval record, and
  `cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256`
  equals its `feature_api_sha256["durable-store"]`.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-observability-otlp --all-targets --features durable-store -- -D warnings
cargo clippy -p sc-observability-otlp --all-targets -- -D warnings
cargo test -p sc-observability-otlp --features durable-store --locked --lib durable::
cargo test -p sc-observability-otlp --features durable-store --locked --lib contracts::credits
cargo test -p sc-observability-otlp --locked
cargo check --workspace --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_dependency_bans.sh
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp
cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256
```
