# Extraction Inventory

## Purpose

This table tracks the migration boundary from ATM-owned observability code to
the standalone `sc-observability` workspace.

## Inventory

| Area | Current ATM-owned concept | Destination | Action | Notes |
| --- | --- | --- | --- | --- |
| Shared neutral contracts | generic diagnostics, trace ids, shared health/value types | `sc-observability-types` | move/stay | must remain ATM-free |
| Lightweight logging | logger, file/console sinks, redaction, rotation | `sc-observability` | move/stay | no routing, no OTLP |
| Observation routing | subscriber/projector registration, fan-out, routing health | `sc-observe` | move/new | layered on logging only |
| OTLP transport | exporters, batching, retry, TelemetryConfig, OTLP protocol | `sc-observability-otlp` | move/stay | top-of-stack only |
| `LogEventV1` | ATM-specific event shape | ATM adapter | stay outside | map through adapter, not shared repo |
| `LifecycleTraceRecord` | ATM lifecycle record shape | ATM adapter | stay outside | neutral payload mapping documented separately |
| ATM env parsing | ATM-prefixed env decoding | ATM adapter | stay outside | shared repo must not own ATM env names |
| Daemon fan-in / spool compatibility | ATM runtime/daemon transport behavior | ATM adapter | stay outside | no daemon behavior in shared repo |
| ATM health JSON / snapshots | ATM-specific health presentation | ATM adapter | stay outside | may consume shared diagnostics, not own them here |
| ATM proving artifact | ATM-shaped example wiring | unpublished example crate in this repo | new | documentation/proving only |
