# Grafana environment preparation (D9)

## Status and ownership

This is a setup harness, not evidence that the user account is usable. On
2026-09-29 the deployment type, stack URL, OTLP receiver, per-signal query
URLs, tenant identifiers, credential references, and enabled Loki/Tempo/metrics
stores were **not available**. `config-agent@hermes` owns account configuration
with the user. No credential, stack URL, tenant, or token is stored in this
repository.

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
| OTLP producer endpoint | `OTEL_EXPORTER_OTLP_ENDPOINT` | Actual endpoint; do not commit it if account-sensitive. |
| OTLP producer protocol | `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `grpc`. |
| OTLP headers | `OTEL_EXPORTER_OTLP_HEADERS` | Secret-bearing; never print or commit. |
| Loki query route | `SC_OBS_GRAFANA_LOGS_QUERY_URL` | Full actual query URL. |
| Tempo query route | `SC_OBS_GRAFANA_TRACES_QUERY_URL` | Full actual query URL. |
| Prometheus query route | `SC_OBS_GRAFANA_METRICS_QUERY_URL` | Full actual query URL. |
| Auth reference names | `SC_OBS_GRAFANA_{LOGS,TRACES,METRICS}_AUTH_ENV` | Names of environment variables containing authorization values. |

The UI URL is not inferred to be an OTLP receiver. If the selected account does
not accept the required production backend protocol, document a version- and
digest-pinned Collector or Alloy ingress before using it. This harness neither
installs an ingress nor adds a third library transport.

Config-agent proposed `SC_OBS_GRAFANA_URL` for the UI/stack reference and the
three `SC_OBS_GRAFANA_*_QUERY_URL` variables above. The producer adapter keeps
the standard `OTEL_EXPORTER_OTLP_ENDPOINT` and
`OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf` contract required by the collector
plan. Historical `ATM_OTEL_*` and `ATM_LOKI_*` names are migration evidence
only; they are not a generic shared-repository contract.

## Query routes and neutral-schema mapping

The adapter accepts the actual full endpoint for each backend, so it works with
Grafana Cloud or a self-managed deployment once config-agent verifies it. The
expected routes are normally Loki `.../loki/api/v1/query_range`, Tempo's
configured TraceQL search endpoint, and Prometheus
`.../api/v1/query_range`; verify those routes rather than deriving them from a
Grafana UI URL.

Every synthetic record uses the bounded resource `service.name=sc-observability-d9`
and an attribute `test.run_id=<safe unique value>`. LogQL and TraceQL retain
those neutral attribute names. PromQL uses the conventional OTLP-promoted label
normalization `service_name` and `test_run_id`; that normalization is explicit
in `grafana_probe.py`, never a claim that the neutral attributes changed.

The generated presentation queries are:

```text
LogQL:   {service_name="sc-observability-d9",test_run_id="<run>"} |= "<run>"
TraceQL: { resource.service.name = "sc-observability-d9" && .test.run_id = "<run>" }
PromQL:  sc_observability_d9_probe_total{service_name="sc-observability-d9",test_run_id="<run>"}
```

Confirm the final metric name and Tempo syntax against the selected deployment
before retaining remote evidence. The synthetic producer must use the real D9
public factory, flush and shut down before querying; this setup adapter never
pretends to emit production telemetry itself.

## Offline and protected verification

Offline verification is hermetic and mandatory before account availability:

```sh
python3 scripts/ci/fixtures/otlp/grafana/test_grafana_probe.py
```

After a protected environment has emitted the bounded D9 corpus, run the remote
read-only query probe explicitly. It has a fixed 120-second window and makes no
account-destructive request:

```sh
SC_OBS_GRAFANA_PROBE=1 python3 scripts/ci/fixtures/otlp/grafana/grafana_probe.py \
  --run-id '<run-id>' --trace-id '<known-trace-id>'
```

The command returns `PASS` only when exact logs, the known trace ID, and the
bounded metric series all match, and it returns the three presentation queries.
Absent configuration or credentials returns `BLOCKED` before network activity;
missing data returns `FAIL`. Preserve redacted request/result evidence, code
SHA, deployment identity, backend/protocol, expected values and one outcome per
signal in the D9 sanity/QA evidence.
