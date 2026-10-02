# d-32: telemetry e2e proof

## Plan metadata

- Wave: 5.3 (wave-5 integration)
- Stack / layer: `phase-d-wave5` stack, layer 7 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: cobs / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `integration`
- Target boundary: wave5-composition
- Branch: `sprint/d-32-telemetry-e2e-proof`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-32-telemetry-e2e-proof`
- PR target: `sprint/d-35-sanity-history-importer` (stack order only; no code dependency on d-35)
- Blocked by: `obs-d-30-sanity`, `obs-d-31-sanity`, `obs-d-33-sanity`, `obs-d-34-sanity`
- Requirements: PHB-010, PHB-013, PHD-001, PHD-002, PHD-003, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-012, PHD-013
- ADRs: ADR-009, ADR-014, ADR-017, ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - tests/telemetry-e2e/**
  - .github/workflows/telemetry-e2e.yml
  - docs/telemetry-submission.md

Boundary note: wave5-composition is the installed Python wheel, the installed
`sc-otel`, `DurableTelemetryClient`, the sync-http submission encoders and the
pinned viewer, composed end to end.

## Relations

- `must_follow` d-30: consumes the installed `sc_observability.telemetry` API
  (tagged results) in the release-config wheel.
- `must_follow` d-31: consumes the installed `sc-otel` binary, its exit codes
  and `sc-otel.result/v1`.
- `must_follow` d-33: consumes `DurableTelemetryClient` (the real store,
  drain, lease and backpressure).
- `must_follow` d-34: consumes the sync-http submission encoders and
  `ProfileExporter` (`/v1/{signal}` and `/v1development/profiles`).
- No edge to d-35: d-32 consumes no importer output. The importer has its
  own tests and workflow (d-35).
- Consumed outside the DAG: the viewer harness under
  `scripts/ci/fixtures/otlp/desktop-viewer/` and the `otlp-conformance.yml`
  jobs are d-9's, consumed read-only. `telemetry-platforms.yml` is d-33's,
  re-dispatched here.
- No shared files with d-18: d-32 edits nothing under `release/**` or
  `docs/api-approvals/**`. No crate is added (lead ruling P1), so there is no
  new publish row, and the approval record is d-29's closeout gate (lead
  ruling P6). No DAG edge is added (R9).

## Goal

Prove that wave 5 works through the real composition of both installed
front ends, the durable store and the encoders, and re-run the D18 and D9
gates over the wave-5 additions.

## Deliverables

1. End-to-end: add the harness `tests/telemetry-e2e/conftest.py`. It builds
   the release-config wheel into a venv and installs `sc-otel` with
   `cargo install --path crates/sc-otel-cli --root $TMP --locked`. It writes
   a temporary `telemetry.yaml` (endpoint and `store.path` in the test
   directory). It starts the pinned viewer with
   `scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py start` on free
   loopback ports and an isolated state directory, and starts a loopback
   OTLP/JSON capture server for collector-capture assertions. [PHD-012]
2. End-to-end: add the viewer readback tests listed in the acceptance
   criteria, using the viewer's JSON-RPC `POST /rpc` (`searchLogs`, `getLog`,
   `searchSpans`, `searchMetricSummaries`, `getMetric`). Python submissions
   run from a plain script that creates no event loop or runtime. [PHD-003,
   PHD-005, PHD-006, PHD-012, PHD-013]
3. End-to-end: add the collector-capture tests for representations that
   viewer v0.5.0 cannot query. [PHD-006, PHD-012, PHD-013]
4. End-to-end: add Python/CLI equivalence, offline recovery, restart,
   partial-delivery and shared-store tests. [PHD-007, PHD-008, PHD-009,
   PHD-010, PHD-012]
5. End-to-end: add `.github/workflows/telemetry-e2e.yml` (`macos-14`,
   because the viewer pin is darwin_arm64 only), running deliverables 1–4.
   Triggers: `pull_request` to `sprint/*` and `integrate/*` with a path
   filter on the wave-5 paths (`crates/sc-observability-types/src/otlp/**`,
   `crates/sc-observability-otlp/**`, `crates/sc-otel-cli/**`,
   `bindings/python/sc-observability-py/**`, `tests/telemetry-e2e/**`,
   `Cargo.toml`, `Cargo.lock` and the workflow itself), plus
   `workflow_dispatch`. So it runs on the d-32 PR, whose base is
   `sprint/d-35-sanity-history-importer`. [PHD-012]
6. Gate re-run: run the D18 and D9 gates over wave 5. D18 (ADR-018 dual
   backends, ADR-017 error surface): `just public-api` and
   `validate_error_migration.py`, run locally on the d-32 head with captured
   logs (the `ci.yml` public-API steps are `continue-on-error` for
   non-develop bases, so CI is not the evidence). The error-migration run
   checks that the new d-29 error enums keep the canonical diagnostic shape
   (PHD-001). D9: `otlp-conformance.yml` dispatched on the d-32 head SHA (its
   path filter does not match d-32's owned paths, so the PR alone would not
   run it). Also re-dispatch `telemetry-platforms.yml` on the d-32 head.
   [PHD-001, PHD-002, PHD-003, PHD-013]
7. Docs: add the user docs `docs/telemetry-submission.md`: Python and CLI
   usage (including Python tagged results), config precedence, exit codes,
   the at-least-once note and the capability matrix link. [PHD-009, PHD-010]

## This Sprint Does Not Close

- Any change to the wave-5 public surface, the submission contract or the
  store schema. Those are frozen at d-29 (wave-5 ruling R17).
- The sanity-history importer and `.sc/telemetry.yaml`. d-35 owns them.
- Exactly-once delivery. Delivery stays at-least-once, with the duplicate
  window documented in d-33 and ADR-021.
- Viewer queries for representations that viewer v0.5.0 cannot query. Those
  are proven by collector capture (item 3), not by viewer readback.
- Grafana dashboards and Grafana testing, which are outside wave 5.
- Merging the phase into `develop`. The user reviews the phase result first.

## Design

### Python usage in the tests

Every Python call returns a d-30 `TelemetryResult`. Tests match on
`Ok`/`TelemetryErr` and assert `error.kind`, `error.variant` and
`error.code`; no test expects an exception. A test asserts that
`Telemetry.open(...)` is `Ok`, uses its value in a `with` block, and then
asserts on `last_shutdown`.

### Viewer pin and supported-signal list

- Viewer: `otel-desktop-viewer` v0.5.0, `darwin_arm64`, SHA-256
  `e4a0051f827e6a40f52b097f490d7832af85bae577f4b33a69a986112c7618a4`
  (`scripts/ci/fixtures/otlp/desktop-viewer/release.json`).
- Started by `viewer_harness.py start` with free ports and a disposable
  `--state-dir`, and stopped by `viewer_harness.py stop --remove-state`.
- Query endpoint: JSON-RPC 2.0 `POST http://127.0.0.1:<ui port>/rpc`.
- Viewer readback (fixed for v0.5.0): logs, spans, Gauge, Sum and explicit
  Histogram.
- Collector capture (fixed for v0.5.0): ExponentialHistogram, Summary,
  exemplars and profiles. v0.5.0 has no profile query method.

If the pinned viewer is found to return a collector-capture form, that is
reported to the lead. The lists change only by plan amendment.

## Acceptance criteria

- [ ] req:PHD-012, req:PHD-003 (D1, D2): `tests/telemetry-e2e/test_viewer_readback.py`
  submits through the installed wheel (from a plain Python script that never
  creates an event loop or runtime) and the installed `sc-otel`. For each
  viewer-readback signal and form it asserts exact values, UTC timestamps,
  resource attributes and log/span correlation in the stored records. This
  is also the proof that neither front end needs a caller-owned runtime.
- [ ] req:PHD-013, req:PHD-006 (D3): `tests/telemetry-e2e/test_collector_capture.py`
  submits ExponentialHistogram, Summary, exemplars and a profile with a
  dictionary through both installed front ends. It asserts the captured
  OTLP/JSON field values, with profiles at `/v1development/profiles`. The
  report labels these "collector capture", never "viewer verified".
- [ ] req:PHD-010 (D4): `tests/telemetry-e2e/test_equivalence.py`: for every
  d-29 golden fixture, the `Ok` value of Python
  `build_envelope(json.load(input.json))` and the stdout `envelope` of
  `sc-otel validate --stdin < input.json` are byte-identical canonical JSON
  (error fixtures: the same `code` from both), and Python `emit(input)` and
  `sc-otel emit --stdin` produce identical captured OTLP payloads apart from
  generated IDs. Both front ends take the one d-29 `SubmissionInput` shape.
- [ ] req:PHD-007, req:PHD-008 (D4): `tests/telemetry-e2e/test_recovery.py`
  covers:
  - `offline_retry_exhaustion_is_terminal_and_a_recovered_collector_accepts_a_new_submission`:
    the collector is down at emit (exit 7) after the released synchronous
    HTTP retry budget is exhausted; that terminal record remains failed, and
    a new CLI submission succeeds after the collector recovers. Durable
    replay is instead proven by the interrupted mid-drain lease-takeover case
    below, before its retry budget can be exhausted.
  - `kill_python_mid_drain_cli_resumes`: shared store; the CLI takes the
    lease and delivers.
  - `partial_signal_delivery`: the profiles endpoint returns 503 while logs
    deliver, and status shows per-signal state.
  - `context_exit_retains_delivery_failure`: Python `last_shutdown` holds
    `TelemetryErr(kind="delivery")` after a `with` block while the collector
    is down; nothing is raised.
  - `viewer_restart_no_loss`
- [ ] req:PHD-001, req:PHD-002, req:PHD-013 (D6), D18 gate re-run: on the
  d-32 head, `just public-api` (including
  `python3 scripts/ci/validate_public_api_semver.py` against the 1.4.1
  baseline) and `python3 scripts/ci/validate_error_migration.py` pass, and
  `approval_for` passes for every changed crate: the docs step
  (`python3 scripts/ci/validate_public_api.py docs`) reports "all affected
  crates explicitly approved" against the signed
  `docs/api-approvals/phase-d-wave5-telemetry-submission.json`. Logs:
  `target/telemetry-e2e/evidence/public-api.log` and
  `target/telemetry-e2e/evidence/error-migration.log`, each ending with an
  `exit=<code>` line.
- [ ] boundary:ADR-018, req:PHD-003, req:PHD-012 (D6), D9 gate re-run:
  `otlp-conformance.yml` is dispatched on the d-32 head SHA
  (`gh workflow run otlp-conformance.yml --ref sprint/d-32-telemetry-e2e-proof`),
  its jobs (hermetic collector matrix for both backends and
  `desktop-viewer-factory-conformance`) pass, and the run URL is recorded in
  the PR body. The `telemetry-e2e` job passes on the d-32 PR.
- [ ] req:PHD-013 (D6): the two scoped cargo-deny audits from d-29 pass again
  on the integrated graph. Log: `target/telemetry-e2e/evidence/cargo-deny.log`,
  ending with an `exit=<code>` line per audit.
- [ ] boundary:ADR-021 (D6): `telemetry-platforms.yml` is dispatched on the
  d-32 head SHA, all six cells pass, and the run URL is recorded in the PR
  body.
- [ ] boundary:ADR-021 (D7): `docs/telemetry-submission.md` exists, shows the
  Python tagged-result usage and the CLI exit-code table, and
  `bash scripts/ci/validate_docs_consistency.sh` passes.

Manual UI inspection is not acceptable proof for any criterion. Where each
criterion runs: the e2e criteria in the `telemetry-e2e` PR job; the D9
conformance and platform criteria in the dispatched runs named above; the
D18 gate and cargo-deny in Required validation, with the captured logs as
evidence.

## Required validation

```sh
E=target/telemetry-e2e/evidence; mkdir -p "$E"
just validate
{ just public-api; echo "exit=$?"; } > "$E/public-api.log" 2>&1
{ python3 scripts/ci/validate_error_migration.py; echo "exit=$?"; } > "$E/error-migration.log" 2>&1
python3 -m pytest tests/telemetry-e2e
bash scripts/ci/validate_docs_consistency.sh
{ cargo deny --manifest-path crates/sc-observability-otlp/Cargo.toml --features durable-store check --config policy/deny-durable-store.toml licenses bans advisories; echo "exit=$?"; \
  cargo deny --manifest-path crates/sc-otel-cli/Cargo.toml --all-features check --config policy/deny-durable-store.toml licenses bans advisories; echo "exit=$?"; } > "$E/cargo-deny.log" 2>&1
```

Evidence that must exist at the PR head: the `ci` and `telemetry-e2e` PR
jobs green; the dispatched `otlp-conformance.yml` and
`telemetry-platforms.yml` run URLs, all green; and the three logs above, with
every `exit=` line `exit=0`.
