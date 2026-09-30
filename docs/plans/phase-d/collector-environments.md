# Phase D collector environments

Status: proposed planning addendum, separate from compatibility PR #499.
The lead coordinates preparation dispatch separately from the compatibility
sprints. This document does not itself install services, change account resources,
or authorize remote telemetry submission. Compatibility execution is authorized
separately and is not held by this addendum.

The user selected an existing Grafana account and authorized
`config-agent@hermes` to install the latest released `otel-desktop-viewer` and
launch it on startup. That agent owns installation, startup integration and
account configuration with the user. The dev tasks below verify and consume
that setup; they must not install a competing managed service. Prepare both environments alongside the
compatibility sprints. D9 then proves the completed production exporters against
both destinations; endpoint setup and synthetic probes are not D9 completion.
Beads hold task ownership and execution state. This document defines the shared
environment and acceptance contract.

## Verified starting point

- Monitored clone: `/Users/randlee/github/otel-desktop-viewer`, clean at
  `01ab6fa17912cb2f8d621e7d6be6eb45eed00eca`.
- Upstream: <https://github.com/CtrlSpice/otel-desktop-viewer>.
- Research: `/Users/randlee/github/github-research/otel-desktop-viewer/`, especially
  `architecture.md`, `usage.md`, `security.md`, `cli.md`, and
  `agent-metrics-howto.md`. Their recorded reference is
  `cc014c6b7313cfe75d5c1943b2e9454025556b96`; commands and RPC shapes must be checked
  against the selected newer source, not copied as already-verified evidence.
- At the selected commit, `main.go::collectorURIs` configures an OTLP receiver,
  batch processor, desktop exporter, and DuckDB extension for all three signals.
  Its default pipelines do not forward application telemetry to Grafana.
- The account's Grafana stack URL, deployment type, ingest protocols, telemetry
  query endpoints, enabled signal stores, and credential references have not yet
  been verified. Owning a Grafana account alone does not establish these.

## Parallel work and ownership

| Task | Proposed owner | Work available before D22/D26 | Owned implementation paths |
|---|---|---|---|
| `obs-d9-local-viewer-setup` | lobs2 | Reproducible harness, RPC queries, synthetic three-signal setup probe | `scripts/ci/fixtures/otlp/desktop-viewer/**`; `docs/observability/otlp/local-viewer.md` |
| `obs-d9-grafana-setup` | cobs2 | Resolve account/ingest/query configuration, secret references, synthetic three-signal setup probe and dashboard queries | `scripts/ci/fixtures/otlp/grafana/**`; `docs/observability/otlp/grafana-environment.md` |
| Existing D9 | cobs | Conformance corpus preparation; final exporter runs require D22/D26 | Existing D9 integration test, smoke entry points, workflow, and remaining documentation |

These are two preparation tasks under D9, not two new compatibility sprints.
The nine planned development sprints and their three-stage critical path stay
unchanged. Neither setup task depends on the other, D22, D26, or D18. Their file
fences are exclusively delegated by D9; D9 consumes them without concurrent edits.
No setup task may modify the monitored upstream clone, compatibility APIs,
release manifests, or the compatibility plan worktree.

The lead may dispatch setup scaffolding in parallel immediately after provisioning
its worktrees. There is no invented review-bead dependency. Account configuration
is coordinated by `config-agent@hermes` with the user; missing credentials block
only the protected live probe, not configuration adapters, local tests or docs.
Final D9 sanity/QA requires both reviewed setup results and D22/D26 artifacts;
their absence must not prevent independent D9 development or compatibility work.
Include this proposed acceptance contract in the D9 scope review and record the
setup results in D9. Environment preparation does not silently certify or close
the existing sprint.

Implementation changes join the existing single stack on `integrate/phase-d`
under the lead’s dispatch. Each task gets its own worktree from the published stack tip;
completion determines physical layer order. This planning branch is independently
based on `develop` and does not change PR #499.

## Local receiver contract

Use the installed released viewer as the local collector and viewer. Do not add a second
local Loki/Tempo/Mimir/Grafana deployment merely to duplicate its display/storage.
Config-agent selects the latest released artifact for desktop installation and
records its version, platform, checksum and release source. The monitored clone
above is a research reference, not a requirement to install an unreleased commit.
Pin the selected release artifact/version/digest in CI once known; verify all
flags and RPC shapes against that release, and never resolve `latest` on each CI run.

The launch configuration is:

```sh
otel-desktop-viewer --host 127.0.0.1 --http 4318 --grpc 4317 \
  --browser-port 8000 --open-browser=false \
  --db "$SC_OBS_VIEWER_DB" --db-max-size 2GB
```

`SC_OBS_VIEWER_DB` is a dedicated absolute database path owned by this setup.
Desktop mode retains that database until explicit cleanup; CI uses a disposable
database. Port overrides must be explicit, reflected in the producer config, and
reported on startup. Never stop a process or clear a database merely because it
already occupies a default port. The lifecycle helper owns only its own PID and
data directory, checks readiness within 30 seconds, and always cleans up its CI
instance. Local use documents the start, status and stop commands for config-agent’s
authorized startup service. The test harness uses that service without taking
ownership of its PID or deleting its persistent database; only an explicitly
created isolated CI/test instance is owned and cleaned up by the harness.

Expose the UI at `http://127.0.0.1:8000` and query `POST /rpc`. Verify OTLP HTTP
JSON, HTTP protobuf, and gRPC ingestion using the protocols needed by the two
exporters. At this source revision, useful RPC methods include `getStats`,
`searchLogs`, `getLog`, `searchTraces`, `searchSpans`, `getTraceLogs`,
`searchMetricSummaries`, and `getMetric`. Pin and exercise their actual parameter
and result shapes in the harness. UI readiness, aggregate counts, or an OTLP
HTTP 200 alone do not prove the submitted records were stored.

## Grafana destination contract

Use the user's existing deployment. Resolve and record its stack URL and product
type first. If Grafana Cloud, use its published OTLP ingestion details and
Loki/Tempo/Prometheus-compatible query endpoints. If another Grafana deployment,
identify the actual backing stores and receiver before selecting those adapters.
Grafana's UI URL is not automatically an OTLP receiver.

Keep endpoint configuration and secret references separate. Require telemetry
write and query-read access for logs, traces and metrics; a dashboard service
account is not a substitute for Cloud telemetry permissions. Store credentials
in the approved local secret store and protected CI environment, never in beads,
Git, command output or uploaded artifacts. `config-agent@hermes` works with the user to make the account name/stack ID, UI
URL, ingest URL/protocol, three query URLs, tenant IDs and credential references
available on this computer. Use standard `OTEL_EXPORTER_OTLP_ENDPOINT`,
`OTEL_EXPORTER_OTLP_PROTOCOL` and `OTEL_EXPORTER_OTLP_HEADERS` for producer
configuration; resolve query credentials independently. Document the final
query-variable names in the Grafana setup layer once the deployment is known.
If access is unavailable, report the exact missing capability without requesting
token contents in chat. Build query adapters and deterministic mocked-response
tests before those secrets arrive; never mark these substitutes as a live PASS.

Run the same corpus separately against the local viewer and Grafana. Do not
assume the desktop viewer fans out, or add dual export to the library. Select a
documented supported transport for each production backend. If the account
cannot accept a required backend protocol directly, the setup task must supply
a pinned standard Collector/Alloy ingress configuration that receives that
protocol and forwards OTLP to the account, with a three-signal setup proof. Record
that additional hop in the evidence; it is not a third library transport.

Use only bounded synthetic data with `service.name=sc-observability-d9`, a backend
identifier, and a unique `test.run_id`. Keep run IDs in structured log/trace
attributes; isolate metrics by a bounded test series and time window to avoid
unbounded label cardinality. Query with a deadline of 120 seconds. Missing access
or a missing signal is BLOCKED/FAIL, never SKIP/PASS for phase acceptance. Remote
tests run explicitly in a protected environment; ordinary PR CI remains offline
from the account. No remote outage, credential revocation, tenant deletion or
production dashboard replacement is part of negative testing.

## D9 proof matrix

| Proof | Desktop viewer | Existing Grafana deployment |
|---|---|---|
| Logs | Retrieve the exact run's records through RPC | Query exact records through the configured logs API/LogQL |
| Traces | Retrieve spans by known trace ID through RPC | Query the known trace through the configured trace API/TraceQL |
| Metrics | Retrieve the test streams and raw points through RPC | Query the test series through the configured metrics API/PromQL |
| Presentation | Open the run in the viewer UI | Run every shipped dashboard panel query and record working Explore/dashboard links |

Execute every signal row for both the SDK and legacy HTTP/JSON production
backends at the final reviewed integration SHA. The harness emits current-time
records through the actual public factory, invokes the documented flush and
shutdown lifecycle, then queries the destination. It asserts log body, severity,
resource/scope and trace correlation; parent/child spans, status/events/links and
timing; counter and gauge values plus histogram count/sum/buckets. Any backend
normalization must have an explicit semantic comparison, not a dropped field.

Use the local fault-injection receivers separately for disabled/no-network,
invalid model, redaction, timeout, retry exhaustion, partial failure, recovery,
flush and idempotent shutdown. Their protocol-level evidence cannot substitute
for either real destination. The old Grafana smoke is logs-only and ATM-specific;
D9 must replace its producer with the crate's real public API and add trace and
metric queries. Preserve pinned-source provenance and explicitly describe these
adaptations rather than claiming the original script covers all three signals.

Each run retains the code SHA, viewer source/artifact digest or remote stack
identity, backend/protocol, redacted effective configuration, generated expected
values, query requests/results, deadlines, and pass/fail per matrix row. Capture
diagnostics on failure and clean up only owned local resources. Record both the
local CI run and protected Grafana run in D9's existing sanity/QA evidence.

Environment setup passes on its own synthetic three-signal probes. D9 passes only
on production-exporter results for both destinations and both backends, with
reviewed dashboard/query evidence. The final D9 scope amendment needs review;
PR #499's compatibility approval does not certify this new acceptance contract.

## Reference material

- [Pinned viewer implementation](https://github.com/CtrlSpice/otel-desktop-viewer/blob/01ab6fa17912cb2f8d621e7d6be6eb45eed00eca/main.go)
- [Pinned viewer RPC API](https://github.com/CtrlSpice/otel-desktop-viewer/blob/01ab6fa17912cb2f8d621e7d6be6eb45eed00eca/desktopexporter/internal/server/jsonrpc_handler.go)
- [Grafana OTLP ingestion](https://grafana.com/docs/opentelemetry/ingest/)
- [Grafana OTLP format mappings](https://grafana.com/docs/grafana-cloud/send-data/otlp/otlp-format-considerations/)
- [Grafana authentication and permissions](https://grafana.com/docs/grafana-cloud/platform/security-and-account-management/security-and-access/authentication-and-permissions/)
- Existing `legacy-otlp-provenance.json` remains the single historical import
  manifest. New receiver setup files are new work, not claimed historical imports.
