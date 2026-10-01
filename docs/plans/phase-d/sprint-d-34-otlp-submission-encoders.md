# d-34: otlp submission encoders

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 3 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: cobs2 / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `boundary` (implementer)
- Target boundary: `BOUNDARY-ScObservabilityOtlp`
- Branch: `sprint/d-34-otlp-submission-encoders`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-34-otlp-submission-encoders`
- PR target: `sprint/d-33-durable-store-and-export` (stack order only; no code dependency on d-33)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHB-010, PHD-002, PHD-003, PHD-004, PHD-005, PHD-006, PHD-008, PHD-013
- ADRs: ADR-004, ADR-005, ADR-012, ADR-017, ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - `crates/sc-observability-otlp/src/sync_http/**`
  - `crates/sc-observability-otlp/src/contracts/profiles.rs`

Ownership notes: in `src/contracts/profiles.rs` d-34 changes only the d-29
staging attribute on `ProfileExporter` (see "Crate-private items d-34 may
change"). `src/sync_http/submission.rs` is staged by d-29 and owned by d-34
from wave 5.2. d-34 adds no file outside `src/sync_http/**`.

## Relations

- `must_follow` d-29: consumes the neutral signal types, `SubmissionEnvelope`
  and its canonical JSON, the golden fixtures, the staged
  `SubmissionExporter` trait and `SubmissionExportFailure`
  (`src/contracts/submission.rs`), the staged `SyncHttpSubmissionExporter`
  and `exporter_for` (`src/sync_http/submission.rs`), the staged
  `ProfileExporter` trait (`src/contracts/profiles.rs`), the wave-5
  constants and error codes in `src/constants.rs` and `src/error_codes.rs`,
  and the committed dependency set (no dependency is added here).
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-34,
  d-33, d-30 and d-31 can run in parallel, because their owned paths are
  disjoint. d-33 drives `SubmissionExporter` through a test exporter; d-34
  proves the real exporter with its own unit tests. d-32 composes the two.
- d-18 fence: d-34 edits none of `src/lib.rs`, `src/assembly.rs`,
  `src/config/`, `src/contracts.rs`, `src/projectors.rs` or the d-18 tests.

## Crate-private items d-34 may change

| File | Item | Allowed change |
| --- | --- | --- |
| `src/sync_http/submission.rs` and new files under `src/sync_http/submission/` | `SyncHttpSubmissionExporter` body, `exporter_for` body, encoders, private items, `#[cfg(test)]` support | implement; the signatures staged by d-29 stay |
| `src/sync_http/**` (existing files) | the existing sync-http exporter and transport | implement `ProfileExporter` on the existing exporter; reuse the transport and retry policy in place (no second exporter or queue) |
| `src/contracts/profiles.rs` | the staging attribute on `ProfileExporter` | replace `#[expect(dead_code, ...)]` with `#[cfg_attr(not(feature = "durable-store"), expect(dead_code, reason = "..."))]` |

Nothing else under `src/` changes.

## Goal

Implement the sync-http OTLP/JSON export of submitted envelopes for every
signal, point form and profiles, behind the d-29 `SubmissionExporter` seam,
with per-variant round trips proven by loopback capture.

## Deliverables

1. Implement `SyncHttpSubmissionExporter` and `exporter_for` (staged by d-29
   in `src/sync_http/submission.rs`): `SubmissionExporter::export(signal,
   envelopes)` encodes the signal part of a batch of envelopes, grouped by
   resource and scope, and posts it through the existing sync-http
   transport with its `SyncHttpRetryPolicy`. A collector 4xx returns
   `SubmissionExportFailure::Terminal`; a transport failure or 5xx after the
   retry sequence returns `SubmissionExportFailure::Retryable`. [PHD-003,
   PHD-004, PHD-008, PHB-010]
2. Implement the OTLP/JSON encoders for `LogPoint`, `SpanPoint` (events,
   links, status, `trace_state`) and all five `MetricData` forms with
   exemplars, posted to `/v1/{signal}`. Encode `AnyValue::Bytes` as base64
   with a private RFC 4648 encoder, adding no dependency. Encode non-finite
   `OtlpDouble` values as the proto-JSON strings `"NaN"`, `"Infinity"` and
   `"-Infinity"`. Encode `AnyValue::StringIndex` as `stringValueStrindex` and
   `AttributeKey::Index` as `keyStrindex`. [PHD-005, PHD-006, PHD-013]
3. Implement `ProfileExporter` on the sync-http exporter: profiles are
   encoded as `ExportProfilesServiceRequest` with `dictionary` and posted to
   the d-29 constant path `/v1development/profiles`. `export` dispatches
   `Signal::Profiles` through it. [PHD-006, PHD-013]
4. Add the `#[cfg(test)]` support: the loopback capture server
   `src/sync_http/submission/tests/capture.rs` (records path and body,
   responds with a scripted status) and the proto-JSON reader
   `src/sync_http/submission/tests/proto_json.rs`. The
   `opentelemetry-proto =0.33.0` serde decoders reject non-finite doubles
   outside `ValueAtQuantile` and drop `stringValueStrindex` (d-29
   "Dependency set"), so the reader decodes base64 `bytesValue`, the three
   non-finite strings and both string-index forms itself. [PHD-013]
5. Add the per-variant round trips (`src/sync_http/submission/tests/roundtrip.rs`).
   [PHD-006, PHD-013]

## This Sprint Does Not Close

- The store, drain, lease and backpressure. d-33 owns them.
- Real collector and viewer readback through the installed front ends.
  d-32 owns them.
- Python bindings (d-30), the CLI (d-31) and the importer (d-35).

## Design

The seam signatures are in the d-29 doc ("Crate-private seams") and are not
restated here.

### Module layout

| File | Contents |
| --- | --- |
| `src/sync_http/submission.rs` | `SyncHttpSubmissionExporter { exporter: <existing sync-http exporter>, bounds }`, `exporter_for`, `impl SubmissionExporter` (dispatch by `Signal`), the 4xx/5xx → `SubmissionExportFailure` classification. |
| `src/sync_http/submission/values.rs` | `AnyValue`, `KeyValues`, `AttributeKey`, `OtlpDouble` and base64 encoding. |
| `src/sync_http/submission/resource.rs` | Grouping of `ResourceRecord<T>` by resource and scope into `resource*`/`scope*` arrays. |
| `src/sync_http/submission/logs.rs`, `spans.rs`, `metrics.rs`, `profiles.rs` | One encoder per signal; `metrics.rs` covers Gauge, Sum, Histogram, ExponentialHistogram, Summary and exemplars. |
| `src/sync_http/submission/tests/` (`#[cfg(test)]`) | `capture.rs`, `proto_json.rs`, `roundtrip.rs`, `classification.rs`. |
| existing exporter file in `src/sync_http/` | `impl ProfileExporter<ProfilesPayload>` that posts an encoded request through the existing transport. |

### Data flow

`SubmissionExporter::export(signal, envelopes)` → select the signal's
records from each envelope → group by resource/scope → encode to an
OTLP/JSON body → post through the existing transport (path from the d-29
constants) → map the transport result to `Ok(())`,
`SubmissionExportFailure::Terminal(ExportError)` (collector 4xx) or
`SubmissionExportFailure::Retryable(ExportError)` (transport error, 5xx or
timeout after the retry sequence). The bounded admission credits are taken
by the d-33 drain before `export` is called; d-34 does not reserve credits.

### Round-trip path

The store holds `SubmissionEnvelope::to_canonical_json` (d-29 `schema.sql`).
The round trip therefore starts from each golden `input.json`, canonicalizes
it with the deterministic test `IdSource`, re-reads the canonical JSON as the
store read would, exports it to the capture server, decodes the captured body
with `proto_json.rs`, and compares field by field with the envelope.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D2, D3, D4, D5):
  `cargo test -p sc-observability-otlp --features durable-store --locked --lib sync_http::submission`
  runs, for every golden fixture of each signal and point form
  (`crates/sc-observability-types/tests/fixtures/otlp_submission/golden/`,
  read through `CARGO_MANIFEST_DIR`), canonical JSON → envelope → export →
  loopback capture → decode with `proto_json.rs`. It asserts field equality
  with the input, including exemplars, profile dictionary tables,
  `stringValueStrindex`/`keyStrindex`, NaN/`Infinity`/`-Infinity` doubles
  (bitwise equality), bytes, `trace_state`, `event_name`, span events, links
  and status, observed timestamp and dropped counts. Logs, traces and
  metrics are captured at `/v1/{signal}` and profiles at
  `/v1development/profiles`.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1): the `classification` tests
  assert that a capture status of 400 returns
  `SubmissionExportFailure::Terminal`, that 503 for the whole retry sequence
  returns `SubmissionExportFailure::Retryable`, and that 200 after one 503
  returns `Ok(())` with the retry counted.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp (D1): with default features,
  `cargo test -p sc-observability-otlp --locked --lib` passes the existing
  sync-http tests unchanged.
- [ ] boundary:BOUNDARY-ScObservabilityOtlp: no `todo!` or `unimplemented!`
  remains in `src/sync_http/`. `git diff <d-29 head> -- Cargo.toml Cargo.lock crates/sc-observability-otlp/Cargo.toml policy/`
  is empty, and the only change outside `src/sync_http/**` is the one
  attribute line in `src/contracts/profiles.rs`.
- [ ] boundary:ADR-005: every non-trivial literal used by the encoders (paths,
  limits) comes from the d-29 entries in `src/constants.rs`; `rg -n '"/v1' crates/sc-observability-otlp/src/sync_http/submission*`
  finds only test assertions.
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-34 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp`
  reports the same `api_sha256` as the signed d-29 approval record, and
  `cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256`
  equals its `feature_api_sha256["durable-store"]`.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-observability-otlp --all-targets --features durable-store -- -D warnings
cargo clippy -p sc-observability-otlp --all-targets -- -D warnings
cargo test -p sc-observability-otlp --features durable-store --locked --lib sync_http::submission
cargo test -p sc-observability-otlp --locked
cargo check --workspace --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_dependency_bans.sh
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-otlp
cargo public-api --manifest-path crates/sc-observability-otlp/Cargo.toml -sss --features durable-store | shasum -a 256
```
