# Local desktop viewer setup

The local collector and viewer is the released `otel-desktop-viewer` v0.5.0
selected and installed by the managed installer. The approved release is pinned per platform
(`darwin_arm64`, `linux_amd64`, `windows_amd64`); the macOS Apple Silicon
archive is
`https://github.com/CtrlSpice/otel-desktop-viewer/releases/download/v0.5.0/otel-desktop-viewer_darwin_arm64.tar.gz`,
SHA-256 `e4a0051f827e6a40f52b097f490d7832af85bae577f4b33a69a986112c7618a4`.
The same source commit is recorded in
[`release.json`](../../../scripts/ci/fixtures/otlp/desktop-viewer/release.json).
The download script selects the host entry. Setup provenance is
the `obs-d9-local-viewer-setup` deliverables and
[`collector-environments.md`](../../plans/phase-d/collector-environments.md).
The `desktop-viewer-factory-conformance` job in
`otlp-conformance.yml` downloads that pinned artifact, starts an isolated
viewer on dynamically selected loopback ports, and qualifies both public
factory backends by querying their exported production records. Do not resolve
`latest` at run time.

## Installed desktop service

The user's launchd service owns its process and persistent database. The service
binds only to loopback:

| Purpose | Address |
| --- | --- |
| UI and JSON-RPC | `http://127.0.0.1:8000` |
| OTLP HTTP/protobuf | `http://127.0.0.1:4318` |
| OTLP gRPC | `127.0.0.1:4317` |
| Persistent database | `~/Library/Application Support/otel-desktop-viewer/telemetry.duckdb` |

Only the service owner may load or unload this launchd service. Other users
can inspect it without changing its lifecycle. Never claim its PID or delete
its database:

```sh
launchctl load -w ~/Library/LaunchAgents/com.ctrlspice.otel-desktop-viewer.plist
launchctl list | grep otel-desktop-viewer
# Owner-only lifecycle command:
launchctl unload ~/Library/LaunchAgents/com.ctrlspice.otel-desktop-viewer.plist
```

Logs are at `~/Library/Logs/otel-desktop-viewer.log` and
`~/Library/Logs/otel-desktop-viewer.err.log`. Never stop a process just because
it occupies one of the defaults; select explicit free port overrides instead.

## Isolated viewer invocation

The harness is standard-library Python. The commands below show a local run of
the pinned `darwin_arm64` artifact: download and verify it, then start an
isolated instance with a disposable database. The CI qualification job uses
the same pinned release and harness lifecycle, but selects free loopback ports
and invokes `assert-production` after each public-factory backend rather than
running this synthetic setup command verbatim.

```sh
VIEWER_RELEASE=scripts/ci/fixtures/otlp/desktop-viewer/release.json
VIEWER_VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$VIEWER_RELEASE")"
VIEWER_SHA256="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["binary_sha256"])' "$VIEWER_RELEASE")"
VIEWER_STATE_DIR="${TMPDIR:-/tmp}/sc-observability-d9-viewer-$$"
python3 scripts/ci/fixtures/otlp/desktop-viewer/download_pinned_release.py build/otel-desktop-viewer
python3 scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py ci \
  --binary build/otel-desktop-viewer \
  --version "$VIEWER_VERSION" \
  --binary-sha256 "$VIEWER_SHA256" \
  --state-dir "$VIEWER_STATE_DIR"
```

The downloader verifies the archive digest and extracted executable digest. The
process helper refuses occupied ports and never kills an unrecognized PID.
Its readiness deadline is 30 seconds. Stop sends TERM only when the pid file
agrees with recorded metadata and the live process executable and `--db`
argument exactly equal that metadata; otherwise it refuses. `--remove-state`
removes only the instance database and log created under that state directory.
The `ci` command runs start, status, probe and cleanup in one process, including
cleanup after a failed probe. The individual `start`, `status`, `probe`, and
`stop` commands are available for local diagnosis. The public-api-governance
job runs hermetic harness unit tests only; live viewer delivery is qualified by
the separate `desktop-viewer-factory-conformance` job after each backend emits
through its real public factory.
The probe uses bounded synthetic data (`service.name=sc-observability-d9`, a
unique `test.run_id`, and backend `setup-probe`) and has a 120-second query
deadline. It needs no Grafana credentials.

## Verified JSON-RPC contract

JSON-RPC 2.0 requests are POSTed to `/rpc`. The v0.5.0 service accepts positional
arrays and named objects; named forms are normalized to the positional schema.
The harness exercises:

| Method | Parameters | Result check |
| --- | --- | --- |
| `searchLogs` | `[startTime, endTime, query?]` | exact synthetic log body and returned row id |
| `getLog` | `[logID]` | full record for the returned id |
| `searchSpans` | `[traceID, query?]` | exact synthetic span name for known trace id |
| `searchMetricSummaries` | `[startTime, endTime, query?]` | exact synthetic metric name and stream id |
| `getMetric` | `[streamID, startTime, endTime, targetBuckets?, seriesIDs?, quantiles?, tzOffsetNs?, fitToData?, viewBuckets?, sparklineBuckets?, selectedSeriesIDs?, datapointSeriesIDs?, datapointSeriesLimit?, tzName?]` | retrieved metric detail and point value |

Times are Unix nanoseconds encoded as decimal strings. Search queries use a
query tree. The log probe filters on a log attribute using
`{"id":"run-id","type":"condition","query":{"field":{"name":"test.run_id","searchScope":"attribute","attributeScope":"log"},"fieldOperator":"=","value":"<run id>"}}`.
The metric query uses `{"field":{"name":"name","searchScope":"field"},"fieldOperator":"=","value":"sc_observability.d9.probe"}` in the same condition node form.
The probe constrains searches to its 61-second window and compares the exact
log body, known trace and span IDs, correlated run ID, metric name, and gauge
point. Metrics use a bounded `test.case=setup-probe` series and time window;
the unique run ID stays on log and span attributes.

The release-installed v0.5.0 service does not implement `getTraceLogs` or
`getFieldValues`
(JSON-RPC `-32601`). The probe verifies log/trace correlation using `getLog`
and `searchSpans` with the same trace ID. Newer source code may expose
`getTraceLogs`; do not assume a source-tree method exists in the pinned release.

The synthetic setup probe verifies three-signal record retrieval over OTLP
HTTP/JSON, sends empty OTLP protobuf requests for each signal and requires
success responses, and checks that the OTLP gRPC listener is reachable. The
gRPC check proves the listener only; production exporter qualification across
each backend and protocol remains part of D9 and is not certified by this setup
probe.

## Captured D9 setup evidence

The service owner attested that the managed desktop service passed the
synthetic probe at 2026-09-29 20:14 PDT (2026-09-30 03:14 UTC), after the
owner's launchd reload. Run ID was `d9-live-post-restart-final` and trace ID
was `8bdf25589e974a06b0c4291f1b2b5be8`. The owner reported that `searchLogs`
returned the exact log body, `getLog` returned its trace ID, `searchSpans`
returned the matching span, and `searchMetricSummaries` plus `getMetric`
returned gauge value `42`. The owner also reported that the same release
artifact passed a local isolated `ci`-subcommand lifecycle with run ID
`d9-ci-query-final`, that all three empty protobuf requests returned HTTP 200,
and that the gRPC listener was reachable. The isolated PID, database, and run
directory were reported removed by the harness. Raw command output was not
retained, so these owner-run results are attested only. The retained,
reproducible verification is the CI job `desktop-viewer-factory-conformance`
(ubuntu, macOS and Windows matrix) in `.github/workflows/otlp-conformance.yml`. It starts the pinned
desktop viewer, runs both public factory tests (SDK and sync-http), and asserts
the production logs, spans and metrics with `viewer_harness.py assert-production`.
A passing run on macOS arm64 is run 36813560097.

The managed launchd agent was reloaded by its owner to apply the requested
five-minute post-login delay. During that intentional delay, a probe attempt
found no listener; the owner confirmed its restart, and the post-restart probe
above passed. The persistent database and managed process were not modified by
this harness.
