---
id: B.1d-telemetry-prep
status: in_progress
branch: feature/phase-b-1d-telemetry-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1d-telemetry-prep
parent: feature/phase-b-1-copy
authoritative-sprint-doc: sprint-b-1d-telemetry-errors.md
contract: error-api-contract.md
---

# B.1d telemetry preparation — typed configuration and lifecycle failures

## Scope

1. Add the seven opt-in typed OTLP methods from the B.1d contract, with one
   typed production implementation and legacy conversion at retained boundaries.
2. Move private exporter and telemetry projector paths to neutral failures while
   preserving public exporter, `TelemetryError`, emit, config, and wire APIs.
3. Add paired legacy/typed fixtures for configuration, assembly, exporters,
   flush/shutdown lifecycle, source retention, and projector forwarding.

## Boundaries

No exporter redesign, callback ABI change, bridge copy, warning activation,
publication, legacy API removal, or serialization change belongs to this layer.
Flush remains fail-open; first shutdown retains its final export failure and a
repeated shutdown remains successful.

## Required matrix

| Area | Paired evidence to add or retain |
| --- | --- |
| endpoint/header/config | empty/invalid endpoint and header; protocol mismatch; zero timeout, batch, and interval; inverted retry bounds; no signals |
| construction | `Telemetry::new` and `new_typed` diagnostics/source parity |
| span assembly | event/end without start; missing event buffer; typed `push_typed` source and result parity |
| exporters | log, trace, and metric failure/recovery; custom exporter code and source preservation |
| lifecycle | fail-open flush; first shutdown final export failure; incomplete spans; post-shutdown emit; repeated shutdown |
| projectors | typed built-in forwarding through retained registration/adapters for logs, spans, and metrics |

## Validation

- `cargo fmt --all -- --check`
- `cargo test --locked -p sc-observability-otlp --all-targets`
- workspace doctests and clippy
- public API semver, docs, and reviewed diff report
- two passes: complete implementation matrix, then source/assertion and gate trace

Copied-bridge integration and independent QA are later-layer work and are not
claimed by this preparation task.
