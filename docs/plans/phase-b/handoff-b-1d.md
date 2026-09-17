# B.1d typed telemetry preparation handoff

## Implementation

This layer adds the seven opt-in B.1d typed APIs: typed endpoint/header and
configuration construction, typed span assembly, and typed telemetry
construction, flush, and shutdown. Legacy public methods delegate to the typed
implementation and convert their result back to the retained wrapper. The
final revision merges B.1-copy parent `c0a4cddaede02a7e3234bc48c421cd56be01660b`.

Private log, trace, and metric exporter contracts now use `ExportFailure`.
The existing public emit boundary still returns `TelemetryError`. Typed log,
span, and metric projector inputs adapt through the B.1a explicit adapters into
the unchanged registration surface; no exporter or callback API is publicized.

## Scenario coverage

- Configuration: public typed/legacy constructor and builder parity covers
  invalid endpoint/header, missing endpoint, zero timeout/batch/interval,
  inverted retry bounds, and no enabled signals. Existing protocol/HTTP(S)
  endpoint acceptance is explicitly retained rather than inventing a new
  mismatch rejection.
- Assembly: paired `push`/`push_typed` fixtures cover event and end without a
  start plus a missing event buffer, with matching diagnostics and error-context
  chains.
- Export/lifecycle: log, trace, and metric exporters fail then recover through
  both APIs with exact call counts and health transitions. First shutdown now
  retains the final `ExportFailure` as a source, including a custom exporter
  code and its native source; flush remains fail-open. Paired fixtures cover
  incomplete spans, post-shutdown retained emit methods, and repeated shutdown.
- Projectors: typed log/span/metric projectors are the production implementations
  and retained projectors adapt once at the B.1a registration boundary. The
  full-stack fixture exercises all three through unchanged fluent registration.

## Review correction

The follow-up correction preserves the retained shutdown diagnostic when a
final exporter failure and incomplete spans occur together: the shutdown
diagnostic continues to select the post-cleanup incomplete-span summary for
its cause and `exporter_error_code`, while its source chain retains the actual
`ExportFailure`. The combined fixture pins that legacy/typed behavior and
repeated shutdown. It also adds typed parity for both empty and whitespace
endpoint validation and exercises custom exporter-code/native-source retention
through both legacy and typed shutdown entry points.

## Validation

- PASS: `cargo fmt --all -- --check`
- PASS: `cargo test --locked -p sc-observability-otlp --all-targets`
  (31 unit tests, 2 full-stack integration tests)
- PASS: `cargo test --locked --workspace --doc`
- PASS: `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- PASS: `python3 scripts/ci/validate_public_api_semver.py` (223 checks passed
  for each of the four workspace crates)
- PASS: `bash scripts/ci/validate_public_api_docs.sh`
- PASS: `git diff --check`

The reviewed `validate_public_api_diff.sh` report exits 1 because it detects
the intentional additive B.1 typed surface: the seven OTLP methods above (no
removed or changed OTLP public items). Copied-bridge integration, warning
activation, publication, and independent QA remain outside this preparation
layer. This handoff requests coordinator completeness review; it does not close
the task.

## Integration-layer status addendum (feature/phase-b-1-integration)

Implementation-complete, confirmed against merged source: the private
`LogExporter`/`TraceExporter`/`MetricExporter` traits described above
(`export_logs`/`export_spans`/`export_metrics`) are confirmed authored
directly against `Result<(), ExportFailure>` — there is no `ExportError` type
anywhere in this crate for them to be a compatibility form of (the retained
public `TelemetryError::ExportFailure` variant is an unrelated same-named
variant of that separate legacy wrapper enum). `error-api-inventory.md`'s
`## ExportError` disposition is corrected to "typed production" accordingly
(it previously read "named compatibility", which this addendum's source
inspection found to be inaccurate). `EventFailure::span_assembly` and
`ExportFailure::export`'s registry constants are both exercised by
`error_registry_parity.rs`, all passing. This crate has no copied-bridge
occurrence of any of its owned families (`FlushError`/`ShutdownError`/
`ProjectionError`/`ExportError`/part of `EventError`) — the frozen bridge
import does not call into `sc-observability-otlp`.

Independent QA/coordinator completeness PASS remains pending for both this
preparation layer and the integration layer; this addendum reports evidence,
it does not itself constitute that review.
