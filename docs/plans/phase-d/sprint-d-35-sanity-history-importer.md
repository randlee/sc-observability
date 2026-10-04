# d-35: sanity history importer

## Plan metadata

- Wave: 5.3 (wave-5 integration)
- Stack / layer: `phase-d-wave5` stack, layer 6 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: cobs2 / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `integration`
- Target boundary: importer-composition
- Branch: `sprint/d-35-sanity-history-importer`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-35-sanity-history-importer`
- PR target: `sprint/d-31-sc-otel-cli` (stack order only; no code dependency on d-31)
- Blocked by: `obs-d-30-sanity`
- Requirements: PHB-010, PHB-013, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-011
- ADRs: ADR-009, ADR-014, ADR-021
- Owned paths:
  - scripts/sanity-telemetry/**
  - .github/workflows/sanity-telemetry.yml
  - .sc/telemetry.yaml
  - .gitignore

Boundary note: importer-composition is the installed sc-observability Python
wheel (`sc_observability.telemetry`, d-30) composed with
`scripts/sanity-telemetry`. d-35 is the integration owner of the PHD-011
criteria. d-29 lists PHD-011 only for the requirement text it keeps
consistent (its D7); d-32 does not claim it.

Ownership notes: `.sc/telemetry.yaml` gains only the core `store:` keys; the
#788 keys stay unchanged. `.gitignore` gains one line,
`.sc/telemetry-state/`.

## Relations

- `must_follow` d-30: consumes the `sc_observability.telemetry` API
  (`Telemetry.open`, `emit`, `flush`, `status`, `TelemetryResult`) and the
  test-hooks wheel with `Telemetry._with_test_double`, which wraps the d-29
  `InMemoryTelemetryClient` and `DoubleScript`.
- Indirectly through d-30: the d-29 `SubmissionInput` shape, record-key
  duplicate semantics and the `telemetry.yaml` core keys.
- Consumed outside the DAG: `.sc/telemetry.yaml` and the sanity/QA log
  formats from PR #788 (`feat/qa-sanity-telemetry-config`), present on
  `integrate/phase-d` before wave 5 (R1).
- d-32 does not depend on d-35: it consumes no importer output.
- Sibling note (prose only): d-35 can run in parallel with d-33, d-34 and
  d-31 once d-30 has passed sanity, because the owned paths are disjoint.

## Goal

Ship the sanity/QA history importer as the first consumer of the Python
submission API, tested against the d-29 double through the d-30 test-hooks
wheel.

## Deliverables

1. Add `scripts/sanity-telemetry/import_sanity.py`. It reads
   `.sc/telemetry.yaml` `sources[]` (parsed with the repository's existing
   PyYAML `yaml.safe_load`; no new parser) and maps each row to one
   `SubmissionInput` per the mapping tables below, submitted with
   `Telemetry.emit`. It sets `service` and `team` as resource attributes and
   builds `pr.url` from `github.pr_url_template`. It reads only repository
   files and the YAML; no ATM environment variable or runtime path (ADR-009).
   [PHD-005, PHD-006, PHD-011]
2. Add the modes `import` (historical, read to EOF) and `follow` (tail with
   a poll interval). Source progress is persisted in the checkpoint file
   `.sc/telemetry-state/checkpoints.json`, written only after `emit` returns
   `Ok(receipt)`. A `TelemetryErr` leaves the checkpoint unchanged and is
   counted and reported (`kind`, `code`); nothing raises. Handle partial
   final lines (not consumed until a newline arrives), rotation or
   replacement (inode or first-line fingerprint change → restart from 0,
   with record keys preventing duplicates), truncation (size < offset →
   restart from 0) and repeated imports (stable `record_key`).
   [PHD-007, PHD-008, PHD-011, PHB-010]
3. Extend `.sc/telemetry.yaml` with the core `store:` keys from d-29
   (`store.path: .sc/telemetry-state/store.sqlite`) and add the
   `.sc/telemetry-state/` line to `.gitignore`. [PHD-007, PHD-011]
4. Add `scripts/sanity-telemetry/display.py::format_local(ts, tz)` for local
   display only. [PHD-011]
5. Add `.github/workflows/sanity-telemetry.yml` (`ubuntu-24.04`): on
   `pull_request` to `sprint/*`, `integrate/*`, and `fix/*` (so stacked fix
   PRs run against their declared fix-layer base) with a path filter on
   `scripts/sanity-telemetry/**`, `bindings/python/sc-observability-py/**`
   and the workflow itself, plus `workflow_dispatch`. It builds the d-30
   test-hooks wheel and runs the importer tests. [PHD-009, PHD-011]

## This Sprint Does Not Close

- Real delivery of imported rows to a collector or the viewer. The real
  store, encoders and viewer readback are proven for the front ends in d-32;
  the importer is proven against the d-29 double.
- Any change to the Python API or the submission contract (d-30, d-29).
- Grafana dashboards.

## Design

### Client use

Production: `Telemetry.open(config=".sc/telemetry.yaml")`; a `TelemetryErr`
from `open` exits the importer with status 4 and the failure printed.
Tests: `Telemetry._with_test_double(script_json)` with a `DoubleScript`
(admission rejections, delivery outcomes). The importer never calls
`flush` per row; `import` mode calls `flush` once at the end and reports the
`FlushReport` or the delivery `TelemetryErr`.

### `.sc/telemetry.yaml` schema (consumed from #788; `store` added here)

| Key | Type | Owner | Meaning |
| --- | --- | --- | --- |
| `service` | string | core | `service.name` resource attribute |
| `team` | string | importer | `team` resource attribute |
| `github.pr_url_template` | string with `{pr_number}` | importer | builds `pr.url` |
| `otlp.endpoint` | URL | core | OTLP/HTTP base (`http://localhost:4318`) |
| `store.path` | path (relative to the YAML file's directory) | core | SQLite store (added by d-35) |
| `sources[].path` | path | importer | JSONL source |
| `sources[].kind` | `qa` \| `sanity` \| `finding-counts` | importer | row schema |
| `sources[].phase` | string | importer | `phase` attribute default |
| `sources[].reviewer` | string, optional | importer | default reviewer for rows without one |

### Source schemas (from `feat/qa-sanity-telemetry-config` and its tests)

- `kind: sanity`. Two sources: `.sc/sanity-log/sanity-llm.jsonl` (legacy
  rows) and `.sc/sanity-log/phase-d.jsonl` (current rows written by
  `sanity-run-history` via `sanity-run-record.json.j2` v1.0.0).
  `.sc/sanity-log/phase-d-supplemental.jsonl` (rows with `branch`, `note`,
  `finding_beads` and no `completed_at`) is not a `sources[]` entry in the
  #788 `telemetry.yaml` and is excluded; the importer never reads it.
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
| `completed_at` / `snapshot_at` | `LogInput.time` | row rejected and counted (`skipped_invalid`); checkpoint still advances past it |
| `started_at` + `completed_at` | `SpanInput.start_time` / `end_time`, name `sanity.review` | no span emitted (log only); no start time is fabricated |
| `duration_seconds` | attribute `review.duration_s` (int) | absent; legacy `duration` text kept as `review.duration` |
| `verdict` | `LogInput.body` (string) and attribute `review.verdict` | row rejected |
| `findings` / `fnd` | attribute `review.findings` (int) | attribute omitted (`CANNOT_RUN` has null) |
| `blk`, `imp`, `min` | attributes `qa.blocking`, `qa.important`, `qa.minor` | omitted |
| `open`, `tot` | metrics `sc.qa.findings.open` and `sc.qa.findings.total` (Gauge, int) | point omitted |
| `reviewer` | attribute `review.reviewer`; scope name `sc-observability.sanity` | source `reviewer:` default, then `sanity-llm` for legacy rows |
| `commit` | attribute `vcs.commit.sha` | omitted (legacy rows) |
| `task`, `sprint`, `iteration`, `tested` | `review.task`, `review.sprint`, `review.iteration`, `qa.tested` | omitted |
| `phase` | attribute `phase` | source `phase:` default |
| `pr_number` | attributes `vcs.pr.number` and `pr.url` (template) | both omitted |
| `run_id` | `record_key` part, trace ID derived as sha256(`run_id`)[0:16] so the LLM and JEV spans of one run share a trace (see the pairing note) | legacy rows: no shared trace |
| `error.code` / `error.message` | attributes `review.error.code`, `review.error.message`; span status `Error` | omitted |
| `completed_local`, `snapshot_local` | not imported (local time is display only) | n/a |
| correction rows | log with attribute `qa.correction = true` and `qa.corrects` | n/a |

The record key is
`<kind>:<source path>:<run_id or sha256(canonical row JSON)>:<reviewer>`.

Pairing note: both reviewers of one sanity run write the same `run_id`. On
`feat/qa-sanity-telemetry-config`, `sanity-split` creates one
`run_id = str(uuid.uuid4())` per run in the split manifest
(`.claude/skills/atm-bd-orchestration/scripts/sanity-split:376`),
`sanity-merge` copies the manifest `run_id` into each reviewer's report
(`sanity-merge:224`), `sanity-run-record.json.j2` v1.0.0 writes it as a field,
and `sanity-run-history` groups one run by `run_id` (`sanity-run-history:121`;
fixture pair in `scripts/tests/test_sanity_run_history.py:73-74`).

## Acceptance criteria

All importer tests run against the d-30 test-hooks wheel, using
`Telemetry._with_test_double`; the double records the envelopes.

- [ ] req:PHD-011, req:PHD-005, req:PHD-006 (D1): `scripts/sanity-telemetry/tests/test_mapping.py`
  maps fixture rows copied from the real `.sc/sanity-log` and `.sc/qa-log`
  formats (current sanity, legacy sanity, qa, qa correction, finding-counts;
  checked in under `scripts/sanity-telemetry/tests/fixtures/`) to the exact
  neutral attributes in the mapping table, asserted on the envelopes the
  double recorded. It covers each missing-field rule, the UTC timestamp to
  the nanosecond, and one LLM/JEV pair sharing a trace ID.
- [ ] req:PHD-011, req:PHD-008, req:PHB-010 (D2): `scripts/sanity-telemetry/tests/test_follow.py` covers:
  - `partial_line_not_consumed_until_newline`
  - `truncation_restarts_without_loss`
  - `rotation_restarts_without_loss`
  - `repeated_import_admits_nothing_new` (receipts `duplicate = true`)
  - `checkpoint_written_only_after_receipt`: a `DoubleScript` admission
    rejection makes `emit` return `TelemetryErr(kind="admission")`; the
    checkpoint is unchanged, the failure is counted, and nothing raises.
- [ ] req:PHD-007, req:PHD-011 (D3): `test_config.py` loads the committed
  `.sc/telemetry.yaml` with PyYAML and asserts `store.path`, the unchanged
  #788 keys, and that `git check-ignore .sc/telemetry-state/x` succeeds.
- [ ] req:PHD-011 (D4): `test_display.py` covers `format_local` for two time
  zones; no stored value is local.
- [ ] req:PHD-009, req:PHD-011 (D5): `sanity-telemetry.yml` passes on the d-35
  PR, and no test is skipped (`-rs` reports 0 skipped).
- [ ] boundary:ADR-009 (D1): `rg -n 'ATM_|\.atm' scripts/sanity-telemetry`
  finds nothing.

## Required validation

```sh
maturin build --manifest-path bindings/python/sc-observability-py/Cargo.toml \
  --features pyo3/abi3-py310,pyo3/extension-module,otlp-telemetry,test-hooks --out dist-hooks
python3 -m venv .venv-d35 && .venv-d35/bin/pip install dist-hooks/*.whl pytest pyyaml
.venv-d35/bin/python -m pytest -rs scripts/sanity-telemetry/tests
! rg -n 'ATM_|\.atm' scripts/sanity-telemetry
```
