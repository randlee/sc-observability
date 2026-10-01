# d-33: durable store and export

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 2 (d-29 → d-33 → d-30 → d-31 → d-32; wave-5 ruling R12)
- Assignee / model: aobs / astra
- Difficulty: `hard` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `boundary` (implementer)
- Target boundary: `BOUNDARY-ScObservabilityOtlp`
- Branch: `sprint/d-33-durable-store-and-export`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-33-durable-store-and-export`
- PR target: `sprint/d-29-telemetry-submission-contract`
- Blocked by: `obs-d-29-sanity`
- Requirements: PHD-003, PHD-004, PHD-006, PHD-007, PHD-008, PHD-013
- ADRs: ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - `crates/sc-observability-otlp/src/durable/**` (except `durable/schema.sql` and `durable/config_file.rs`, which are d-29's and read-only)
  - `crates/sc-observability-otlp/src/sync_http/**`
  - `crates/sc-observability-otlp/tests/durable_*.rs`
  - `crates/sc-observability-otlp/tests/submission_*.rs`
  - `crates/sc-observability-otlp/tests/support/**` (new: proto-JSON reader, loopback capture)
  - `crates/sc-observability-otlp/src/lifecycle.rs`, `src/contracts/profiles.rs` and `src/contracts/credits.rs`: only the d-29 staging attribute on the seam items (see "Crate-private items d-33 may change")
  - `docs/plans/phase-d/sprint-d-33-durable-store-and-export.md`

## Relations

- `must_follow` d-29: consumes the `TelemetryClient` trait,
  `SubmissionEnvelope`, `AdmissionReceipt`, `DeliveryState`, `StoreStatus`,
  the error enums and their codes, `TelemetryClientConfig`, `schema.sql`, the
  staged `durable/mod.rs` and `durable/adapter.rs`, the staged crate-private
  seams (`SignalKind::Profiles`, `ProfileExporter`,
  `AdmissionCredits::wait_for_release`), the committed dependency set (no
  dependency is added here), and `otlp::submission::testing::conformance`
  with `ConformanceHarness` in `sc-observability-types` (feature
  `test-double`).
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-33,
  d-30 and d-31 can run in parallel. Their owned paths are disjoint, and all
  three consume only d-29 artifacts.
- d-18 fence: d-18 owns `src/lib.rs`, `src/assembly.rs`, `src/config.rs`
  (now `src/config/`), `src/contracts.rs`, `src/projectors.rs`,
  `tests/composition.rs` and `tests/full_stack_integration.rs`. d-33 edits
  none of them. The seams d-33 needs in `contracts.rs` (the
  `pub(crate) mod profiles;` line) and in `lib.rs` (the `durable` module
  registration) are added by d-29 under the P2 merge-forward rule, so d-33
  has no d-18 overlap. `src/lifecycle.rs` and `src/contracts/credits.rs` are
  outside the d-18 fence.

## Crate-private items d-33 may change

d-33 builds on the seams d-29 staged (d-29 "Crate-private seams"; wave-5
ruling R14). Their signatures are frozen; a needed signature change is a
contract change routed to the lead. d-33 may change only:

| File | Item | Allowed change |
| --- | --- | --- |
| `src/durable/mod.rs`, `src/durable/adapter.rs`, new private files under `src/durable/` | `DurableTelemetryClient` method bodies, `otel_config_from` body, private items | implement; public signatures stay as in d-29 |
| `src/sync_http/**` | the sync-http exporter, encoders, private items | implement `ProfileExporter` for the sync-http exporter; extend the encoders in place (no second exporter) |
| `src/lifecycle.rs` | the staging attribute on `SignalKind::Profiles` | replace `#[expect(dead_code, ...)]` with `#[cfg_attr(not(feature = "durable-store"), expect(dead_code, reason = "..."))]` |
| `src/contracts/profiles.rs` | the staging attribute on `ProfileExporter` | same replacement |
| `src/contracts/credits.rs` | the staging attribute on `AdmissionCredits::wait_for_release` | same replacement |

Nothing else under `src/` changes: not `contracts.rs`, `ExporterSet`,
`lib.rs`, `config/**`, `runtime.rs` or `lifecycle.rs` beyond the attribute.

## Goal

Implement `DurableTelemetryClient` in `sc-observability-otlp` behind
`durable-store`: the SQLite store, the drain worker with lease and claims, and
sync-http export for every signal and point form, including profiles.

## Deliverables

1. Implement `DurableTelemetryClient::{open, emit, flush, shutdown, status}`
   in `durable/` against the committed `schema.sql`. `emit` commits envelope
   and delivery rows in one `synchronous=FULL` transaction before returning
   the receipt. A duplicate `RecordKey` returns the original receipt with
   `duplicate = true`. Enforce reject-newer schema and envelope versions.
   Implement `flush_submission` and the d-29 flush result rules. Implement
   `durable::adapter::otel_config_from` and build the sync-http exporter from
   `SyncHttpConfig::from_otel`. [PHD-007]
2. Implement the drain worker and multi-process ownership: drain lease with
   expiry, row claims, retries using `SyncHttpRetryPolicy`, at-least-once
   delivery with the duplicate window documented below, and resumption after
   a crash or exit. [PHD-008]
3. Implement the layering and backpressure: store → drain worker →
   the existing sync-http bounded admission (record and byte credits),
   waiting on `AdmissionCredits::wait_for_release` when credits are
   exhausted, and with profiles accounted under `SignalKind::Profiles`. Add
   disk-bound accounting, delivered-row and record-key retention, and status
   counters. [PHD-004, PHD-007]
4. Implement sync-http OTLP/JSON encoders for `LogPoint`, `SpanPoint`, all
   five `MetricData` forms with exemplars, and profiles
   (`ExportProfilesServiceRequest` with `dictionary`) to
   `/v1development/profiles` through a `ProfileExporter` implementation on
   the sync-http exporter. Logs, traces and metrics go to `/v1/{signal}`.
   Encode `AnyValue::Bytes` as base64 with a private RFC 4648 encoder, adding
   no new dependency. Encode non-finite `OtlpDouble` values as the proto-JSON
   strings `"NaN"`, `"Infinity"` and `"-Infinity"`. Encode
   `AnyValue::StringIndex` as `stringValueStrindex` and `AttributeKey::Index`
   as `keyStrindex`. [PHD-006, PHD-013]
5. Enforce the ADR-021 capability matrix at construction.
   `DurableTelemetryClient::open` with `config.backend` other than
   `ExporterBackendId::SyncHttp` returns
   `TelemetryConfigError::UnsupportedCombination`. No combination silently
   omits a signal. [PHD-003, PHD-013]

## This Sprint Does Not Close

- Python bindings (d-30) and the CLI (d-31).
- Installed-front-end submission, viewer readback, the sanity importer and
  the D18/D9 gate re-run (d-32).
- Cross-front-end equivalence (d-32).

## Design

The signatures are in the d-29 doc and are not restated here.

### Layering and backpressure

- **Drain loop.** One drain worker runs per lease holder. It claims up to
  `batch_size` ready rows per signal, ordered by `admitted_at`, and hands each
  batch to the sync-http admission (`contracts/credits.rs` record and byte
  credits, via `AdmissionCredits::reserve`).
- **Backend queue full.** When the backend queue is full (credits exhausted),
  the worker pauses that signal and blocks in
  `AdmissionCredits::wait_for_release` until the next credit release.
  It never drops or evicts. Store rows stay `claimed` until their claim
  expires or the batch completes.
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

### Multi-process ownership

- **Lease.** `drain_lease` holds a single row. A process acquires it when the
  row is absent or `expires_at < now`, inside an immediate transaction. The
  holder renews every `lease_duration / 3`. The holder ID is
  `<pid>:<uuid>`: `std::process::id()` and a UUIDv7 from the `uuid` crate
  (d-29 dependency set). No hostname is used.
- **Claims.** Only the lease holder claims rows. It sets `state='claimed'`,
  `claimed_by` and `claim_expires_at = lease expiry`. After a batch completes,
  rows move to `delivered`, `retry` (with `next_attempt_at`) or `failed`.
- **Takeover.** Claims held by an expired holder are reset to `pending` when
  the lease is taken over.
- **Flush and shutdown.** Scope and results follow the d-29 flush rules.
  `flush(deadline)` from a non-holder waits for the rows in its scope to
  reach a terminal state. If the lease becomes
  free or expires before the deadline, it acquires the lease and drains
  itself. `shutdown` flushes, stops the worker and deletes the lease row if
  it still holds it.
- **Duplicate window.** A crash or lease loss after the collector accepts a
  batch, but before the `delivered` update commits, resends that batch after
  takeover. So the window is at most one in-flight batch per signal per
  takeover. This is documented in rustdoc and ADR-021, and no exactly-once
  claim is made.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1):
  `cargo test -p sc-observability-otlp --features durable-store --test durable_store --locked`
  calls `sc_observability_types::otlp::submission::testing::conformance::run_all` against
  `DurableTelemetryClient`. These are the same cases the d-29 test double
  passes. The following cases also pass:
  - `receipt_after_commit`: kill the child process after the receipt; on
    reopen the row is present.
  - `duplicate_record_key`
  - `schema_too_new_rejected_unmodified`
  - `newer_envelope_skipped_and_counted`

  The harness implements d-29's `ConformanceHarness` over a fresh temp store
  (`tempfile`) and the `tests/support/capture.rs` loopback server (Deliver =
  200, Fail = 400, Stall = no response before the deadline).
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1): the `#[cfg(test)]` module in
  `src/durable/adapter.rs` (`cargo test -p sc-observability-otlp --features durable-store --locked --lib durable::adapter`)
  asserts that `otel_config_from` maps every `TelemetryClientConfig` field it
  carries, maps every one of the six `SyncHttpRetryPolicyDto` fields onto the
  same-named `SyncHttpRetryPolicy` field, and returns
  `TelemetryConfigError::InvalidField` for an out-of-range value.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D2): `--test durable_drain`
  covers two concurrent drainers. `two_process_drainers_no_loss` runs two OS
  processes over one store file and a loopback capture: every admitted
  submission is delivered at least once, and the duplicates observed are at
  most one batch per signal per forced takeover. It also covers:
  - `lease_expiry_takeover`: an expired holder's claims are reset and
    delivered by the new holder.
  - `crash_mid_drain_resumes`
  - `non_holder_flush_waits_then_acquires`
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D3): `--test durable_backpressure`
  covers backend saturation. `backend_queue_full_pauses_no_eviction`: with
  credits exhausted, store rows stay pending and are never evicted, and they
  drain after release. It also covers:
  - `disk_bound_reject_new`
  - `disk_bound_evict_oldest_counted`
  - `retention_purges_delivered_only`
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D4):
  `--test submission_roundtrip` runs, for every golden fixture of each signal
  and point form, serialize → store → read → encode → loopback HTTP capture
  → decode with the d-33-owned proto-JSON reader
  `crates/sc-observability-otlp/tests/support/proto_json.rs` (the
  `opentelemetry-proto =0.33.0` serde decoders reject non-finite doubles
  outside `ValueAtQuantile` and drop `stringValueStrindex`; see d-29
  "Dependency set"). The reader decodes base64 `bytesValue`, the three
  non-finite strings and both string-index forms. It asserts
  field equality with the input, including exemplars, profile dictionary
  tables, `stringValueStrindex`/`keyStrindex`, NaN/`Infinity`/`-Infinity`
  doubles (bitwise equality), bytes, `trace_state`, `event_name`, observed timestamp and dropped
  counts. Profiles are captured at `/v1development/profiles`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D5):
  `--test submission_capability` asserts one case per ADR-021 matrix row.
  The sync-http drain accepts every signal and representation row.
  `DurableTelemetryClient::open` with
  `config.backend = ExporterBackendId::OpenTelemetrySdk` returns
  `TelemetryConfigError::UnsupportedCombination`, naming the backend.
  No row produces a silently omitted signal.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp: no `todo!` or `unimplemented!`
  remains in `durable/` or `sync_http/`, and the boundary and dependency
  validators pass. `git diff <d-29 head> -- Cargo.toml Cargo.lock crates/sc-observability-otlp/Cargo.toml policy/`
  is empty, and the only changes outside `src/durable/**`, `src/sync_http/**`
  and the owned test paths are the three attribute lines listed in
  "Crate-private items d-33 may change".
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-33 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp`
  reports the same `api_sha256` as the signed d-29 approval record, and
  `cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256`
  equals its `feature_api_sha256["durable-store"]`.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-observability-otlp --all-targets --features durable-store -- -D warnings
cargo test -p sc-observability-otlp --features durable-store --locked
cargo test -p sc-observability-otlp --locked
cargo check --workspace --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_dependency_bans.sh
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp
cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256
```
