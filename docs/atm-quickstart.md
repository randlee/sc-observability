# ATM Quickstart

**Status**: Draft for review
**Applies to**: ATM-shaped adopters of the shared `sc-observability` workspace
**Related documents**:
- [`requirements.md`](./requirements.md)
- [`architecture.md`](./architecture.md)
- [`api-design.md`](./api-design.md)
- [`atm-adapter-requirements.md`](./atm-adapter-requirements.md)
- [`atm-adapter-architecture.md`](./atm-adapter-architecture.md)
- [`atm-adapter-example.md`](./atm-adapter-example.md)

## 1. Purpose

This document answers two practical questions for the first sophisticated
consumer of the shared stack:

1. what ATM gets out of the box with zero or minimal shared-crate configuration
2. what ATM still must provide through the ATM-owned adapter boundary to be
   production-ready

This is not the ATM migration spec by itself. It is the shared-repo quickstart
for an ATM-shaped workload.

## 2. Zero-Configuration Baseline

With no ATM-specific adapter configuration beyond constructing the shared
runtime, the stack behaves as follows.

| Surface | Zero-config behavior | ATM fit |
| --- | --- | --- |
| Log format | JSONL structured `LogEvent` records | Good default |
| Built-in sink | File sink enabled | Good default for daemon-style logging |
| Console sink | Disabled | Good default for daemon-style logging |
| Log path | `<log_root>/logs/<service_name>.log.jsonl` | Good default if ATM supplies `log_root` |
| Redaction | Bearer-token redaction plus denylist-key redaction | Good default, may need ATM extensions |
| Rotation | 64 MiB max file, 10 files | Acceptable initial default |
| Retention | 7 days | Acceptable initial default |
| Logging queue | 1024 | Acceptable initial default |
| Observation queue | 1024 | Acceptable initial default |
| Process identity | Auto hostname + pid | Good default |
| Routing | Registration-order subscriber/projector fan-out | Good default |
| Health surface | In-process health objects only; no HTTP endpoint | Requires ATM projection for CLI/daemon JSON |
| OTLP | Disabled until explicitly enabled/configured | Safe default |
| Metrics/traces/log export | Disabled until configured | Safe default |

### 2.1 What ATM Gets Immediately

Out of the box, ATM can rely on the shared crates for:

- generic structured JSONL logging
- generic observation routing
- generic span, metric, and log projection contracts
- OTLP attachment once `TelemetryConfig` is provided
- generic in-process health objects

Out of the box, ATM does **not** get:

- ATM-prefixed env translation
- `LogEventV1` or ATM envelope compatibility
- ATM direct-spool or daemon fan-in durability
- ATM health JSON parity for `atm status`, `atm doctor`, or `atm daemon status`

## 3. Minimal ATM Production Configuration

The smallest production-ready ATM composition is:

1. construct `ObservabilityConfig`
2. construct `LoggerConfig` implicitly through `sc-observe`
3. construct `TelemetryConfig` independently when OTLP is desired
4. register ATM-owned projectors and subscribers
5. provide ATM-owned env/config translation and ATM-owned health/durability behavior

### 3.1 Minimal Shared-Layer Config

```rust
let observability = Observability::builder(
    ObservabilityConfig::default_for(
        ToolName::new("atm")?,
        std::path::PathBuf::from("/var/log/<service>"),
    ),
)
.register_subscriber(agent_info_subscriber_registration)
.register_projection(agent_info_projection_registration)
.build()?;
```

Shared assumptions here:

- file logging is enabled by default
- console logging is disabled by default
- routing queues use documented defaults
- no OTLP is enabled yet

### 3.2 Minimal OTLP Attachment

```rust
let telemetry_config = TelemetryConfigBuilder::new(ServiceName::new("atm")?)
    .with_transport(OtelConfig {
        enabled: true,
        endpoint: Some("https://otel.example.internal".to_string()),
        protocol: OtlpProtocol::HttpBinary,
        auth_header: None,
        ca_file: None,
        insecure_skip_verify: false,
        timeout_ms: DurationMs::from(3000),
        debug_local_export: false,
        max_retries: 3,
        initial_backoff_ms: DurationMs::from(250),
        max_backoff_ms: DurationMs::from(5000),
    })
    .enable_logs(LogsConfig { batch_size: 256 })
    .enable_traces(TracesConfig { batch_size: 256 })
    .enable_metrics(MetricsConfig {
        batch_size: 256,
        export_interval_ms: DurationMs::from(5000),
    })
    .build();

let telemetry = Telemetry::new(telemetry_config)?;
```

Attachment rule:

- `sc-observability-otlp` attaches by registering projectors with
  `ObservabilityBuilder`
- `TelemetryConfig` is not derived from `ObservabilityConfig`

### 3.3 ATM-Owned Minimal Adapter Config

For an ATM daemon to be production-ready, the ATM adapter still needs to own:

- `ATM_OTEL_*` env parsing and precedence rules
- ATM-specific redaction additions beyond the shared defaults
- ATM `LogEventV1` / envelope mapping rules
- ATM health JSON projection
- ATM direct-spool and fan-in durability policy

The shared stack intentionally does not take ownership of those.

## 4. Day-One ATM Gaps And Ownership

| Gap | Resolution | Owner |
| --- | --- | --- |
| `EventFields -> LogEventV1` semantics | Keep in ATM adapter requirements/architecture | ATM adapter |
| ATM-prefixed env translation | Keep in ATM adapter requirements | ATM adapter |
| ATM direct-spool / fan-in durability | Keep in ATM adapter architecture | ATM adapter |
| Health JSON parity | Keep in ATM adapter architecture | ATM adapter |
| Shared name newtypes and config defaults | Added to shared API design in this sprint | Shared repo |
| Public crate error/code/constant registries | Added to shared requirements and API design | Shared repo |
| Builder-time OTLP attachment model | Added to shared architecture and API design | Shared repo |

No shared-repo day-one blocker remains undocumented after this pass.

## 5. ATM-Shaped Working Pattern

The intended path is:

1. ATM-owned event types stay outside the shared crates
2. ATM adapter code maps those types to shared observations and projections
3. shared routing fans those observations into:
   - file logging
   - ATM-owned subscribers
   - OTLP projectors when configured
4. ATM-owned adapter code projects shared health and durability behavior back
   into ATM-specific operational surfaces

See [`atm-adapter-example.md`](./atm-adapter-example.md) for the boundary-proof
example and [`atm-adapter-requirements.md`](./atm-adapter-requirements.md) for
the normative ATM-owned obligations.
