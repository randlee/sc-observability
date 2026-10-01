# d-32: sanity telemetry e2e

## Plan metadata

- Wave: 5.3 (wave-5 integration)
- Layer: 5 of the wave-5 stack (d-29 → d-33 → d-30 → d-31 → d-32)
- Assignee / model: cobs / terra (difficulty: normal)
- Closure: `integration`
- Target boundary: wave-5 composition (installed Python wheel + installed `sc-otel` + `DurableTelemetryClient` + pinned viewer)
- Branch: `sprint/d-32-sanity-telemetry-e2e`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-32-sanity-telemetry-e2e`
- PR target: `sprint/d-31-sc-otel-cli`
- Blocked by: `obs-d-30-sanity`, `obs-d-31-sanity`, `obs-d-33-sanity`
- Requirements: PHD-002, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-011, PHD-012, PHD-013
- ADRs: ADR-020, ADR-021
- Owned paths:
  - importer (PHD-011): `scripts/sanity-telemetry/**`
  - e2e proof (PHD-012/013): `tests/telemetry-e2e/**`, `.github/workflows/telemetry-e2e.yml`
  - shared consumer config: `.sc/telemetry.yaml`
  - user docs: `docs/telemetry-submission.md`
  - `docs/plans/phase-d/sprint-d-32-sanity-telemetry-e2e.md`

## Relations

- `must_follow` d-30: consumes the installed `sc_observability.telemetry` API
  in the release-config wheel.
- `must_follow` d-31: consumes the installed `sc-otel` binary, its exit codes
  and `sc-otel.result/v1`.
- `must_follow` d-33: consumes `DurableTelemetryClient` (the real store, drain
  and sync-http export, including `/v1development/profiles`).
- Consumed outside the DAG: `.sc/telemetry.yaml` from PR #788, present on
  `integrate/phase-d` before wave 5 (R1). The viewer harness under
  `scripts/ci/fixtures/otlp/desktop-viewer/` and the
  `otlp-conformance.yml` jobs are d-9's, consumed read-only.
- No shared files with d-18: d-32 edits nothing under `release/**` or
  `docs/api-approvals/**`. No crate is added (lead ruling P1), so there is no
  new publish row, and the approval record is d-29's closeout gate (lead
  ruling P6). No DAG edge is added (R9).

## Goal

Prove that wave 5 works through the real composition. Ship the sanity/QA
history importer as the first consumer, and re-run the D18 and D9 gates over
the wave-5 additions.

## Deliverables

Importer (root `scripts/sanity-telemetry/`):

1. Add `scripts/sanity-telemetry/import_sanity.py`. It reads
   `.sc/telemetry.yaml` `sources[]` (parsed with the repository's existing
   PyYAML `yaml.safe_load`; no new parser) and maps each row to a submission through
   the installed `sc_observability.telemetry` API, per the mapping tables
   below. It sets `service` and `team` as resource attributes and builds
   `pr.url` from `github.pr_url_template`. [PHD-011]
2. Add the modes `import` (historical, read to EOF) and `follow` (tail with a
   poll interval). Source progress is persisted in the checkpoint file
   `.sc/telemetry-state/checkpoints.json`, and is written only after `emit`
   returns a receipt. Handle partial final lines (not consumed until a
   newline arrives), rotation or replacement (inode or first-line fingerprint
   change → restart from 0, with record keys preventing duplicates),
   truncation (size < offset → restart from 0) and repeated imports (stable
   `record_key`). [PHD-011]
3. Extend `.sc/telemetry.yaml` with the core `store:` keys from d-29
   (`store.path: .sc/telemetry-state/store.sqlite`). The #788 keys stay
   unchanged. [PHD-011]

End-to-end proof (root `tests/telemetry-e2e/`, CI job `telemetry-e2e`):

4. Add the harness `tests/telemetry-e2e/conftest.py`. It builds the
   release-config wheel into a venv and installs `sc-otel` with
   `cargo install --path crates/sc-otel-cli --root $TMP --locked`. It starts
   the pinned viewer with
   `scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py start` on free
   loopback ports and an isolated state directory, and starts a loopback
   OTLP/JSON capture server for collector-capture assertions. [PHD-012]
5. Add the viewer readback tests listed in the acceptance criteria, using the
   viewer's JSON-RPC `POST /rpc` (`searchLogs`, `getLog`, `searchSpans`,
   `searchMetricSummaries`, `getMetric`). [PHD-012, PHD-013]
6. Add the collector-capture tests for representations that viewer v0.5.0
   cannot query. [PHD-012, PHD-013]
7. Add Python/CLI equivalence, offline recovery, restart, partial-delivery
   and shared-store tests. [PHD-007, PHD-008, PHD-010, PHD-012]
8. Add `.github/workflows/telemetry-e2e.yml` (`macos-14`, because the viewer
   pin is darwin_arm64 only), running deliverables 4–7 and the importer tests
   on each PR to `integrate/phase-d` that touches wave-5 paths. [PHD-012]
9. Run the D18 and D9 gates over wave 5: the public-API semver/compat checks
   and the `otlp-conformance.yml` jobs, checked against the approval record
   the user signed at d-29 closeout. [PHD-002, PHD-013]
10. Add the user docs `docs/telemetry-submission.md`: Python and CLI usage,
    config precedence, exit codes, the at-least-once note and the capability
    matrix link. [PHD-009, PHD-010]

## Design

### `.sc/telemetry.yaml` schema (consumed from #788; `store` added here)

| Key | Type | Owner | Meaning |
| --- | --- | --- | --- |
| `service` | string | core | `service.name` resource attribute |
| `team` | string | importer | `team` resource attribute |
| `github.pr_url_template` | string with `{pr_number}` | importer | builds `pr.url` |
| `otlp.endpoint` | URL | core | OTLP/HTTP base (`http://localhost:4318`) |
| `store.path` | path (relative to the YAML file's directory) | core | SQLite store (added by d-32) |
| `sources[].path` | path | importer | JSONL source |
| `sources[].kind` | `qa` \| `sanity` \| `finding-counts` | importer | row schema |
| `sources[].phase` | string | importer | `phase` attribute default |
| `sources[].reviewer` | string, optional | importer | default reviewer for rows without one |

### Source schemas (from `feat/qa-sanity-telemetry-config` and its tests)

- `kind: sanity`. Two sources: `.sc/sanity-log/sanity-llm.jsonl` (legacy
  rows) and `.sc/sanity-log/phase-d.jsonl` (current rows written by
  `sanity-run-history` via `sanity-run-record.json.j2` v1.0.0).
  - Current row: `run_id`, `reviewer` (`sanity-llm`|`sanity-jev`), `commit`
    (a 40- or 64-hex SHA), `task`, `sprint`, `phase`, `started_at`,
    `completed_at` (UTC `Z`), `duration`, `duration_seconds`, `pr_number`,
    `iteration`, `verdict` (`PASS`|`FAIL`|`CANNOT_RUN`), `findings`
    (int | null) and `error` ({code, message} | null).
  - Legacy row: `completed_at`, `duration`, `findings`, `iteration`, `phase`,
    `pr_number`, `sprint`, `task` and `verdict`. It has no `run_id`,
    `reviewer`, `commit` or `started_at`. Its reviewer is the source's
    `reviewer:` value (`sanity-llm`).
- `kind: qa` (`.sc/qa-log/phase-d.jsonl`): `blk`, `imp`, `min`, `fnd`,
  `completed_at`, `completed_local`, `duration`, `iteration`, `phase`
  (missing on some rows), `pr_number`, `sprint`, `task`, `tested` and
  `verdict`. Correction rows add `correction`, `correction_reason`,
  `correction_ref`, `corrects`, `supersedes_counts` and `valid_row`.
- `kind: finding-counts` (`.sc/qa-log/phase-d-stats.jsonl`): `blk`, `imp`,
  `min`, `open`, `tot`, `phase`, `snapshot_at`, `snapshot_local` and
  `trigger_task`, plus optional `superseded`, `superseded_reason` and `note`.

### Field mapping

| Source field | Neutral target | Missing-field behavior |
| --- | --- | --- |
| `completed_at` / `snapshot_at` | `LogPoint.time` | row rejected and counted (`skipped_invalid`); checkpoint still advances past it |
| `started_at` + `completed_at` | `SpanPoint.start_time` / `end_time`, name `sanity.review` | no span emitted (log only); no start time is fabricated |
| `duration_seconds` | attribute `review.duration_s` (int) | absent; legacy `duration` text kept as `review.duration` |
| `verdict` | `LogPoint.body` (string) and attribute `review.verdict` | row rejected |
| `findings` / `fnd` | attribute `review.findings` (int) | attribute omitted (`CANNOT_RUN` has null) |
| `blk`, `imp`, `min` | attributes `qa.blocking`, `qa.important`, `qa.minor` | omitted |
| `open`, `tot` | metrics `sc.qa.findings.open` and `sc.qa.findings.total` (Gauge, int) | point omitted |
| `reviewer` | attribute `review.reviewer`; scope name `sc-observability.sanity` | source `reviewer:` default, then `sanity-llm` for legacy rows |
| `commit` | attribute `vcs.commit.sha` | omitted (legacy rows) |
| `task`, `sprint`, `iteration`, `tested` | `review.task`, `review.sprint`, `review.iteration`, `qa.tested` | omitted |
| `phase` | attribute `phase` | source `phase:` default |
| `pr_number` | attributes `vcs.pr.number` and `pr.url` (template) | both omitted |
| `run_id` | `record_key` part, trace ID derived as sha256(`run_id`)[0:16] so the LLM and JEV spans of one run share a trace | legacy rows: no shared trace |
| `error.code` / `error.message` | attributes `review.error.code`, `review.error.message`; span status `Error` | omitted |
| `completed_local`, `snapshot_local` | not imported (local time is display only) | n/a |
| correction rows | log with attribute `qa.correction = true` and `qa.corrects` | n/a |

The record key is
`<kind>:<source path>:<run_id or sha256(canonical row JSON)>:<reviewer>`.

Local display: `scripts/sanity-telemetry/display.py::format_local(ts, tz)` is
unit tested. The viewer is never used to prove local time.

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

Importer (PHD-011):

- [ ] req:PHD-011 (D1): `python3 -m pytest scripts/sanity-telemetry/tests/test_mapping.py`
  maps fixture rows copied from the real `.sc/sanity-log` and `.sc/qa-log`
  formats (current sanity, legacy sanity, qa, qa correction, finding-counts)
  to the exact neutral attributes in the mapping table. It covers each
  missing-field rule.
- [ ] req:PHD-011 (D2): `scripts/sanity-telemetry/tests/test_follow.py` covers:
  - `partial_line_not_consumed_until_newline`
  - `truncation_restarts_without_loss`
  - `rotation_restarts_without_loss`
  - `repeated_import_admits_nothing_new` (receipts `duplicate = true`)
  - `checkpoint_written_only_after_receipt`: an injected admission failure
    leaves the checkpoint unchanged.
- [ ] req:PHD-011 (D1, D3): `scripts/sanity-telemetry/tests/test_import_viewer.py`
  (CI `telemetry-e2e`) imports a bounded sample through the installed wheel
  into the pinned viewer. It asserts by `searchLogs`:
  - team/phase filtered result sets equal the expected row IDs;
  - an LLM vs JEV paired run returns two spans with one trace ID via
    `searchSpans`;
  - the `pr.url` attribute equals the template output;
  - `vcs.commit.sha` equals the source commit;
  - the stored time equals the source UTC `completed_at` to the nanosecond.
  `test_display.py` covers `format_local`.

End-to-end (PHD-012/013):

- [ ] req:PHD-012 (D4, D5): `tests/telemetry-e2e/test_viewer_readback.py`
  submits through the installed wheel and the installed `sc-otel`. For each
  viewer-readback signal and form it asserts exact values, UTC timestamps,
  resource attributes and log/span correlation in the stored records.
- [ ] req:PHD-013 (D6): `tests/telemetry-e2e/test_collector_capture.py`
  submits ExponentialHistogram, Summary, exemplars and a profile with a
  dictionary through both installed front ends. It asserts the captured
  OTLP/JSON field values, with profiles at `/v1development/profiles`. The
  report labels these "collector capture", never "viewer verified".
- [ ] req:PHD-010 (D7): `tests/telemetry-e2e/test_equivalence.py`: for every
  d-29 golden fixture, Python `build_envelope` and `sc-otel validate` produce
  byte-identical canonical JSON, and Python `emit` and `sc-otel emit` produce
  identical captured OTLP payloads apart from generated IDs.
- [ ] req:PHD-007, req:PHD-008 (D7): `tests/telemetry-e2e/test_recovery.py`
  covers:
  - `offline_then_recover`: the collector is down at emit (exit 6), and a
    later `sc-otel flush` delivers.
  - `kill_python_mid_drain_cli_resumes`: shared store; the CLI takes the
    lease and delivers.
  - `partial_signal_delivery`: the profiles endpoint returns 503 while logs
    deliver, and status shows per-signal state.
  - `viewer_restart_no_loss`
- [ ] req:PHD-002, req:PHD-013 (D9), D18 gate re-run: `just public-api`
  (including `python3 scripts/ci/validate_public_api_semver.py` against the
  1.4.1 baseline) and `python3 scripts/ci/validate_error_migration.py` pass
  with the wave-5 surface, and every reported wave-5 public item is listed
  in the signed `docs/api-approvals/phase-d-wave5-telemetry-submission.json`.
- [ ] req:PHD-012 (D9), D9 gate re-run: the `otlp-conformance.yml` jobs
  (hermetic collector matrix and `desktop-viewer-factory-conformance`) pass on
  the d-32 PR head, and the `telemetry-e2e` job passes in CI.
- [ ] req:PHD-013 (D9): the scoped cargo-deny audit from d-29 passes again on
  the integrated graph.

Manual UI inspection is not acceptable proof for any criterion. Every
criterion above runs in CI (`telemetry-e2e`, `otlp-conformance` and `ci`).

## Required validation

```sh
just validate
just public-api
python3 scripts/ci/validate_error_migration.py
python3 -m pytest scripts/sanity-telemetry/tests tests/telemetry-e2e
cargo deny --manifest-path crates/sc-observability-otlp/Cargo.toml --features durable-store check --config policy/deny-durable-store.toml licenses bans advisories
```

CI jobs that must be green on the PR head: `ci`, `otlp-conformance`
(all jobs) and `telemetry-e2e`.
