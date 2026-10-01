# Python and CLI telemetry consumers

Status: proposed implementation plan; no implementation or task dispatch.

Parent: `feat/qa-sanity-telemetry-config` at
`151abd307795730ae68ba99f292a13ea783c51a1` (PR #788, targets develop).

## Outcome

A customer maps their data once into logs, completed spans, metrics and profiles, then submits
through either Python bindings or a Rust CLI. Both call the same Rust code for
validation, correlation, durable admission, OTLP export and lifecycle handling.
The first consumer is the repository's LLM/JEV sanity history and live results.

Plan for **four additional implementation sprints**, beyond PR #788 and the
existing Phase D work. These are delivery boundaries, not time estimates.
Sprints 2 and 3 can run concurrently once sprint 1's interface is stable.

## Existing code and prerequisite

At this plan's develop-derived base (`0ca4fe24`),
`sc-observability-types` already provides `LogEvent`, `SpanRecord<SpanEnded>`,
`TraceContext`, timestamps and structured attributes.
`sc-observability-otlp` owns telemetry configuration and span assembly, but
`Telemetry::new_typed` still selects no-op exporters at this base. Its
`TelemetryRuntime` buffers records in memory. This is not a durable export queue.

The working Phase D transport implementation was exercised separately against
the viewer; it has not reached this develop base. Its landing is a prerequisite
for end-to-end implementation, not a new transport sprint or authorization to
copy an arbitrary integration head into this branch. Reconcile API names and
feature flags when that work lands.

The inspected Python API exposes LoggerConfig, LogEvent, Logger and lifecycle
operations for logging. It does not expose OTLP endpoint configuration,
completed-span emission or an original timestamp on its LogEvent input.

## Proposed shared API

An emission accepts logs, completed spans, metrics and profiles, separately or together. Existing neutral Rust
signal types remain the data model; do not introduce a QA-specific telemetry
type or implement OTLP JSON/protobuf in Python or the CLI. A small versioned
submission envelope holds any combination of signal payloads and persistence metadata.

For a pair, preserve supplied valid trace/span IDs or generate shared IDs when
absent. Reject inconsistent supplied correlation. Preserve original UTC event
timestamps and actual span start/end. Accept start plus duration as an input
convenience; reject conflicting end/duration. Historical summaries without exact
start times remain logs with a duration attribute.

Illustrative front ends (names and error types finalized in sprint 1):

```python
with Telemetry(config=".sc/telemetry.yaml") as telemetry:
    receipt = telemetry.emit(log=log_fields, span=span_fields)
    telemetry.flush()
```

```sh
sc-otlp emit --config .sc/telemetry.yaml --stdin < submission.json
sc-otlp flush --config .sc/telemetry.yaml
```

The CLI's `--log`, `--span`, `--metric` and `--profile` conveniences must feed the same validation as
structured stdin. Python retains the package's established typed error/result
conventions. Neither interface may equate local admission with remote delivery.

## Sprint 1 — Shared emission and durable records

Deliver one shared Rust submission/configuration API over the landed OTLP
facade. Reuse existing types and additive metric/profile contracts; expose event timestamps, resource
attributes and instrumentation scope. Keep repository-specific source paths and
field mapping outside the exporter; the YAML can hold exporter settings and
consumer settings without the core interpreting sanity fields.

Provide a durable local store used by both front ends. Recommended candidate:
SQLite with a versioned serialized payload and per-signal pending/delivered
state. Confirm dependency/platform compatibility before choosing it. The format
is internal, not a public OTLP wire schema or a requirement to use JSONL.

Persist and commit before returning an admitted receipt. Signals in one
submission share correlation but export on separate OTLP signal endpoints;
track their delivery independently. Retain failures for bounded retry and
expose them through receipts/status. Explicitly define disk limits, retention,
flush timeout, process shutdown, concurrent ownership and unsupported payload
versions. Never silently discard pending data to make room.

Delivery is at-least-once: a crash after remote acceptance but before local
acknowledgement can duplicate an export. Do not claim exactly-once. A stable
caller record key prevents repeated imports from admitting the same local
record; it does not guarantee collector-side deduplication.

Acceptance: real loopback OTLP capture verifies values, original timestamps and
shared IDs. Process termination/restart preserves admitted records. An offline
collector later recovers; partial multi-signal delivery and concurrent writers do
not lose pending records. Invalid input and failed durable admission return
errors without success receipts.

## Sprint 2 — Python telemetry bindings

Expose shared configuration, submission, receipts, flush/shutdown and recovery
through the existing Python package. Add log-only, span-only, metric-only, profile-only and combined input,
timestamp support and matching type stubs. Keep current logger APIs compatible.
Release the GIL around blocking persistence/export operations as appropriate.
Specify context-manager flushing and observable errors; never hide a failed
flush during exit.

Acceptance: build/install the wheel into a clean environment and exercise the
public Python API against a loopback collector, including validation failures,
historical timestamps, correlated records and restart recovery. Test installed
bindings, not just mocked Python objects.

## Sprint 3 — Rust CLI

Add the `sc-otlp` executable as a thin consumer of sprint 1. Support configured
emit from stdin and log/span/metric/profile flags, explicit flush and a small delivery-status
view. Machine output identifies admitted versus delivered state; exit codes
distinguish invalid input, admission failure and requested delivery failure.
An ordinary emit durably admits and attempts a bounded flush. Failure to reach
the collector leaves pending data available to a later flush.

Use the same configuration precedence and validation as Python. Keep credentials
out of checked-in YAML and error output. Deliver an installable binary; do not
require users to run it from the workspace.

Acceptance: invoke the actual binary with piped and flag input. Python and CLI
produce equivalent neutral records for the same fixture. Test malformed stdin,
offline recovery, shared-store access and timeout/exit behavior.

## Sprint 4 — Sanity importer and viewer use case

Add a small Python consumer using the installed bindings. Read the telemetry
YAML, the historical `sanity-llm.jsonl`, and new phase logs. Map completion,
duration, verdict, findings, reviewer, sprint, task, iteration and commit;
attach configured service/team and construct PR URLs from the YAML template.
Only new records with actual start/end timing produce review spans. Distinct
LLM/JEV spans can share a trace using the paired run ID; their result logs link
to their respective spans. Preserve missing historical fields as missing.

Support an explicit historical import and a follow mode with persisted source
position and stable record keys. Checkpoint only after durable admission.
Handle partial final lines, file replacement/truncation and repeated imports
without skipping records or silently reusing an unrelated checkpoint.
Keep the original JSONL as input; this sprint does not replace agent logging
with direct CLI emission. Direct CLI emission is demonstrated separately.

Acceptance: load a bounded sample through Python and CLI into the pinned local
viewer and query actual stored records. Demonstrate filtering by team/phase,
LLM/JEV comparison, PR URL attributes, exact commit, UTC timestamps with local
display and correlated log/span records. Restart the consumer and collector
without losing durably admitted data. Do not run deferred Grafana testing.

## Scope boundaries and decisions

- Dashboards, remote configuration and a general mapping DSL are outside this
  delivery. Profile submission and export are included in the same four sprints;
  this consumer API accepts collected profiles and does not implement a profiler.
- Generic telemetry validation belongs in Rust; customer field mapping belongs
  in the Python consumer or caller of the CLI.
- The store format, admission receipt API and flush/exit behavior are finalized
  with sprint 1 before either front end duplicates those decisions.
- Plan depends on Phase D transport landing. If it remains unavailable, report
  that prerequisite separately rather than inflating these four sprints with
  duplicate exporter work.

## Signal coverage (user scope expansion)

The shared API, durable queue, Python bindings and CLI cover all four
OTLP signal families, including the development-version profiles protocol. Stats are metrics, not a fourth payload. Events use a log
record or span event. Trace context, resources, instrumentation scope and baggage
are metadata/context; baggage is not exported as a separate OTLP signal and is
not automatically copied into attributes.

- Logs: structured body, severity, event and observed timestamps, attributes,
  resource/scope and optional trace/span correlation; named events where the
  pinned protocol supports them.
- Traces: completed spans, parentage, kind, status, start/end, attributes, events,
  links and trace state. A trace is a set of spans, not a fourth submission type.
- Profiles: samples, stack/location/function/mapping dictionaries, sample types,
  units, timestamps, attributes and trace/span links; preserve all fields in the
  pinned upstream schema and validate dictionary references.
- Metrics: gauge, sum (monotonic/non-monotonic; delta/cumulative), explicit-bucket
  histogram, exponential histogram, and summary import compatibility. Preserve
  integer versus floating values, start/end timestamps, units, attributes,
  temporality, bucket/count/sum data and supported exemplars. Do not flatten
  distributions to a single value. Summary is a transport compatibility form,
  not a new summary instrument. Counter, up/down counter, gauge and histogram
  measurements map into these forms; already-collected observable measurements
  are submitted as values rather than requiring CLI callbacks.

Existing MetricRecord only exposes Counter/Gauge/Histogram with one f64 value.
Sprint 1 must add compatible neutral contracts for the additional representations;
wrapping that record alone does not satisfy metric coverage. Additive APIs preserve
released struct literals and enums per ADR-020.

Sprint 4 must exercise profiles and every supported metric form through both installed front
ends. For each signal supported by the pinned viewer, query its stored records and
assert exact payload values, UTC times, resource attributes and correlation. HTTP
success, empty responses and mock-only tests are not viewer-write proof. Record
viewer version/capabilities; collector wire capture covers formats the viewer
cannot expose, but must not be reported as viewer verification. The current viewer
write gap remains open until this test actually passes. No Grafana testing.

Use landed Phase D names: sync-http, sync_http, SyncHttpRetryPolicy,
OtelConfig.sync_http_retry, and native_operation; reconcile against the landing
commit instead of reviving the legacy names.

Governing additions: PHD-005–013 and ADR-021. Wave 5 sprint content and execution
state live in beads obs-d-29 through obs-d-32; this document records the design
proposal and scope discussion, not a parallel execution tracker.

References: [signals](https://opentelemetry.io/docs/concepts/signals/),
[metric data model](https://opentelemetry.io/docs/specs/otel/metrics/data-model/),
[profiles](https://opentelemetry.io/docs/concepts/signals/profiles/).

Full support is defined against one pinned upstream opentelemetry-proto revision.
D29 enumerates every payload variant and field from that revision, including
AnyValue alternatives and nested signal metadata. D30/D31 expose the same full
contract; D32 verifies serialization/export round trips for every variant and
representative non-default fields. Unsupported backend/version combinations must
fail explicitly; they cannot silently omit a signal or pass full-support acceptance.
Any proposed product exclusion requires a user decision before implementation.
Profiles remain in scope despite their upstream development status; version this
API appropriately rather than removing it. No additional sprint is assumed merely
for adding a parallel signal path.
