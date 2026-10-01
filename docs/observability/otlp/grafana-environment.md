# Grafana environment preparation (D9)

## Status and ownership

This is a setup harness, not evidence that the user account is usable. As of
2026-09-30 UTC (config-agent report `01M3R4MK55FYNBJKV12BM6A7AZ`), the OTLP
receiver, per-signal query URLs, tenant identifiers, credential references, and
enabled Loki/Tempo/metrics stores were **not available**. `config-agent@hermes`
owns account configuration with the user. No credential, stack URL, tenant, or
token is stored in this repository.

Config-agent's 2026-09-30 nonsecret readiness report identifies **Grafana
Cloud** as the legacy target and expects HTTPS OTLP/HTTP ingestion plus Loki,
Tempo, and Mimir/Prometheus. Those are planning inputs, not verified account
capabilities: write/read access for every signal, the auth scheme, tenant, and
the actual routes remain pending Rand's account provisioning. The proposed
protected local environment file is `~/.config/sc-observability/grafana.env`
with mode `0600`; it is never read from or committed by this repository.

Consequently the protected remote probe is currently **BLOCKED**, not PASS. Its
offline unit tests are the only completed evidence in this layer. Once the
nonsecret configuration references arrive, record the deployment type and the
redacted contract output in D9 evidence; do not copy secret values into a bead,
Git, console capture, or artifact.

## Configuration contract

Copy `scripts/ci/fixtures/otlp/grafana/grafana-environment.example.env` into an
approved local secret store or protected CI configuration and provide:

| Purpose | Variable | Value policy |
| --- | --- | --- |
| Grafana UI/stack URL | `SC_OBS_GRAFANA_URL` | Optional nonsecret UI reference; it is not an ingest endpoint. |
| OTLP producer endpoint | `OTEL_EXPORTER_OTLP_ENDPOINT` | Actual endpoint; do not commit it if account-sensitive. |
| OTLP producer protocol | `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` only for the Grafana Cloud direct-ingest contract. |
| OTLP headers | `OTEL_EXPORTER_OTLP_HEADERS` | Secret-bearing; never print or commit. |
| Loki query route | `SC_OBS_GRAFANA_LOGS_QUERY_URL` | Full actual query URL. |
| Tempo query route | `SC_OBS_GRAFANA_TRACES_QUERY_URL` | Full actual query URL. |
| Prometheus query route | `SC_OBS_GRAFANA_METRICS_QUERY_URL` | Full actual query URL. |
| Auth reference names | `SC_OBS_GRAFANA_{LOGS,TRACES,METRICS}_AUTH_ENV` | Names of environment variables containing authorization values. |

The UI URL is not inferred to be an OTLP receiver. If the selected account does
not accept the required production backend protocol, document a version- and
digest-pinned Collector or Alloy ingress before using it. This harness neither
installs an ingress nor adds a third library transport.

Config-agent supplied `SC_OBS_GRAFANA_URL=https://randlee.grafana.net/` as a
nonsecret UI/stack reference and proposed the three
`SC_OBS_GRAFANA_*_QUERY_URL` variables above. The producer adapter keeps the
standard `OTEL_EXPORTER_OTLP_ENDPOINT` and
`OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf` contract required by the collector
plan. Ingest is contract-only in this setup layer: emission, flush, and
shutdown belong to the D9 public factory. Historical `ATM_OTEL_*` and
`ATM_LOKI_*` names are migration evidence only; they are not a generic
shared-repository contract.

## Query routes and neutral-schema mapping

The adapter accepts the actual full endpoint for each backend, so it works with
Grafana Cloud or a self-managed deployment once config-agent verifies it. The
expected routes are normally Loki `.../loki/api/v1/query_range`, Tempo's
configured TraceQL search endpoint, and Prometheus
`.../api/v1/query_range`; verify those routes rather than deriving them from a
Grafana UI URL.

Every synthetic record uses the bounded resource `service.name=sc-observability-d9`
and an attribute `test.run_id=<safe unique value>`. TraceQL retains those
neutral attributes. LogQL selects the resource-promoted `service_name` label,
then parses and filters the structured `test_run_id` attribute. PromQL uses the
conventional OTLP-promoted label normalization `service_name` and `test_run_id`;
that normalization is explicit in `grafana_probe.py`, never a claim that the
neutral attributes changed.

The generated presentation queries are:

```text
LogQL:   {service_name="sc-observability-d9"} | json | test_run_id="<run>"
TraceQL: { resource.service.name = "sc-observability-d9" && .test.run_id = "<run>" }
PromQL:  sc_observability_d9_probe_total{service_name="sc-observability-d9",test_run_id="<run>"}
```

The following is a non-destructive dashboard panel example. Replace only the
datasource UID placeholder after config-agent verifies it; do not replace a
production dashboard. This is a handoff artifact, not evidence that the panel
works against the unprovisioned account.

```json
{
  "title": "D9 protected synthetic logs",
  "type": "logs",
  "datasource": {"uid": "<loki-datasource-uid>"},
  "targets": [{"expr": "{service_name=\"sc-observability-d9\"} | json | test_run_id=\"${run_id}\"", "refId": "A"}]
}
```

Confirm the final metric name and Tempo syntax against the selected deployment
before retaining remote evidence. The synthetic producer must use the real D9
public factory, flush and shut down before querying; this setup adapter never
pretends to emit production telemetry itself.

## Offline and protected verification

Offline verification is hermetic but is not independently wired into this
preparation branch's CI. `obs-d-9` deliverable 3 owns wiring it into the
`otlp-conformance.yml` CI job; run it locally before a protected probe:

```sh
python3 scripts/ci/fixtures/otlp/grafana/test_grafana_probe.py
```

After a protected environment has emitted the bounded D9 corpus, run the remote
read-only query probe explicitly and promptly. It queries the 120 seconds
ending at invocation, and its total request budget (not each individual socket)
is 120 seconds; it makes no account-destructive request:

```sh
SC_OBS_GRAFANA_PROBE=1 python3 scripts/ci/fixtures/otlp/grafana/grafana_probe.py \
  --run-id '<run-id>' --trace-id '<known-trace-id>'
```

Use `--print-contract` with the nonsecret reference variables configured to
emit the redacted configuration contract without querying the account.

The command returns `PASS` only when a Loki stream value contains the run ID, a
Tempo `traceID` exactly equals the known 32-lowercase-hex ID, and a Prometheus
series has the expected `service_name`, `test_run_id`, and a non-empty sample.
It returns the three presentation queries. Absent configuration, credentials,
401/403 access, or connection access returns `BLOCKED` (exit 2); reachable
missing/malformed/mismatched data returns `FAIL` (exit 1); `PASS` exits 0.
Preserve redacted request/result evidence, code SHA, deployment identity,
backend/protocol, expected values and one outcome per signal in the D9
sanity/QA evidence. Hermetic-corpus recipe validation is owned by obs-d-9
deliverable 4, not by this setup harness.
