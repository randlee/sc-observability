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

## Validation

- PASS: `cargo fmt --all -- --check`
- PASS: `cargo test --locked -p sc-observability-otlp --all-targets`
  (30 unit tests, 2 full-stack integration tests)
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
