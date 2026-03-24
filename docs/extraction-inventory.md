# Extraction Inventory

## Purpose

This table tracks the migration boundary from ATM-owned observability code to
the standalone `sc-observability` workspace. It supplements
[`ADR-006: ATM Adapter Boundary`](./architecture.md) by mapping each ATM-owned
surface to its destination and expected proof artifact.

## Inventory

| Area | Current ATM-owned concept | Destination | Action | Proof artifact | Notes |
| --- | --- | --- | --- | --- | --- |
| Shared neutral contracts | generic diagnostics, trace ids, shared health/value types | `sc-observability-types` | move/stay | shared workspace docs | must remain ATM-free |
| Lightweight logging | logger, file/console sinks, redaction, rotation | `sc-observability` | move/stay | shared workspace docs | no routing, no OTLP |
| Observation routing | subscriber/projector registration, fan-out, routing health | `sc-observe` | move/new | shared workspace docs | layered on logging only |
| OTLP transport | exporters, batching, retry, TelemetryConfig, OTLP protocol | `sc-observability-otlp` | move/stay | shared workspace docs | top-of-stack only |
| `LogEventV1` | ATM-specific event shape | ATM adapter | stay outside | `atm-adapter-requirements.md` | map through adapter, not shared repo |
| `LifecycleTraceRecord` | ATM lifecycle record shape | ATM adapter | stay outside | `atm-adapter-requirements.md` | neutral payload mapping documented separately |
| ATM-specific projector implementations | projector behavior that promotes ATM semantics into generic projections | ATM adapter | stay outside | `atm-adapter-architecture.md` | shared repo exposes hooks only |
| `EventFields -> LogEventV1` mapping semantics | field promotion, passthrough, generated IDs, ATM-specific envelope shaping | ATM adapter | stay outside | `atm-adapter-requirements.md` | adapter-owned source of truth |
| trace/span ID generation semantics | ATM fallback generation when upstream identifiers are missing | ATM adapter | stay outside | `atm-adapter-requirements.md` | shared repo only defines generic ID types |
| message preview / sensitive text exclusion | ATM preview behavior and exclusion from persistent logs | ATM adapter | stay outside | `atm-adapter-requirements.md` | adapter-owned redaction/compat policy |
| ATM env parsing | ATM-prefixed env decoding | ATM adapter | stay outside | `atm-adapter-requirements.md` | shared repo must not own ATM env names |
| OTEL env translation + launch inheritance | ATM-owned OTEL setup inheritance to child processes or launches | ATM adapter | stay outside | `atm-adapter-requirements.md` | generic OTLP config remains lower-level |
| Daemon fan-in / spool compatibility | ATM runtime/daemon transport behavior | ATM adapter | stay outside | `atm-adapter-architecture.md` | no daemon behavior in shared repo |
| direct spool fallback | ATM synchronous durability path near shutdown or crash-adjacent behavior | ATM adapter | stay outside | `atm-adapter-architecture.md` | boundary proof only in shared repo |
| daemon fan-in merge path | ATM replay/merge ownership for persisted fan-in artifacts | ATM adapter | stay outside | `atm-adapter-architecture.md` | not specified by shared crates |
| ATM health JSON / snapshots | ATM-specific health presentation | ATM adapter | stay outside | `atm-adapter-requirements.md` | may consume shared diagnostics, not own them here |
| status / doctor / daemon health projection | ATM parity across shipped JSON health surfaces | ATM adapter | stay outside | `atm-adapter-requirements.md` | compatibility obligation |
| GH observability ledger integration boundary | ATM-owned integration with any GitHub/ledger-specific observability surface | ATM adapter or delete | decision pending | follow-up ATM adapter review | intentionally left open pending a separate ATM adapter decision; do not silently absorb into shared crates |
| ATM proving artifact | ATM-shaped example wiring | unpublished example crate in this repo | new | `docs/atm-adapter-example.md` | boundary proof only, not migration sufficiency proof |
