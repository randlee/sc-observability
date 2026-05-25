# Sprint Plan: Retained-Log Rotation and Pruning Maintenance

**Issue**: #70
**Branch**: `feature/retained-log-maintenance`
**Version**: 1.1.0
**Base**: `develop`

## 1. Goal

Move retained-log lifecycle management (rotation, pruning, background maintenance) out of
downstream application code and into `sc-observability` as a first-class, configurable
facility. Downstream apps choose policy values; the crate owns the machinery.

## 2. Problem

`atm-core` carries generic retained-log maintenance logic in:

- `crates/atm-daemon/bin_support/retained_sink.rs`

That wrapper owns: rotation checks, rotated-path naming, retention/prune policy, prune worker
lifecycle, prune join timeout, and maintenance scheduling. None of this is ATM-specific.

## 3. Deliverables

### D1 — Configurable Retained-Log Policy Surface

Add a `RetainedLogPolicy` struct nested in `LoggerConfig` exposing:

| Field | Type | Description |
|-------|------|-------------|
| `rotation_max_bytes` | `ByteCount` | Rotate active log when it exceeds this size |
| `rotation_max_files` | `FileCount` | Maximum number of rotated files to retain |
| `retention_max_age` | `RetentionMaxAge` | Delete rotated files older than this |
| `maintenance_cadence` | `MaintenanceCadence` | How often the background worker runs a maintenance pass |
| `maintenance_join_timeout` | `MaintenanceJoinTimeout` | Bounded shutdown join timeout for the maintenance worker |
| `maintenance_max_work_per_pass` | `Option<usize>` | Optional cap on files processed per maintenance pass |

Policy must be serialisable/deserialisable and have reasonable documented defaults.

### D2 — Background Maintenance Worker

- Runs rotation checks and prune passes on the configured cadence
- Executes off the main request/emit path — no write-path blocking
- Uses a thread or equivalent non-async execution lane (no async runtime dependency)
- Worker is owned and managed entirely by `sc-observability`; callers do not spawn or join it directly

### D3 — Health and Error Reporting

Maintenance health exposed through `LoggingHealthReport` extended with an
optional `MaintenanceHealthReport` field:

- Last maintenance pass timestamp
- Files rotated total / pruned total
- Last maintenance error (if any)
- Worker state: `Running` | `Degraded` | `Stopped`

Maintenance failures must not crash the logger or interfere with the emit path.

### D4 — Shutdown Contract

- `Logger<Running>::shutdown()` consumes the running logger, returns `Logger<Stopped>`, and joins the maintenance worker within `maintenance_join_timeout`
- If join times out, worker is abandoned and the timeout is recorded in health/error state
- Shutdown behaviour is documented in Rustdoc on the public API

### D5 — Integration Tests

Required tests:
- Rotation triggers at `rotation_max_bytes` threshold
- Rotated files pruned when count exceeds `rotation_max_files`
- Rotated files pruned when age exceeds `retention_max_age`
- Maintenance worker health reflects last pass and error state
- Shutdown joins within the configured timeout
- Emit path is not blocked during a maintenance pass

### D6 — Rustdoc

All new public items (`RetainedLogPolicy`, policy fields, health fields, shutdown behaviour)
must have complete Rustdoc.

### D7 — Consumer Documentation

Update `CONSUMING.md` and/or `README.md` with a concrete retained-log policy configuration
example showing an application integrator the expected setup.

## 4. File Targets

- `crates/sc-observability/src/lib.rs` (or new submodule `maintenance.rs`)
- `crates/sc-observability/src/sinks.rs` — rotation helpers if needed
- `crates/sc-observability-types/src/lib.rs` — `MaintenanceHealthReport` and `MaintenanceWorkerState`
- `docs/public-api-checklist.md` — mark new public items
- `CONSUMING.md` / `README.md`

## 5. Acceptance Criteria

- Downstream app configures retained-log rotation and pruning entirely through `sc-observability`
  config — no custom prune worker or rotation helper needed
- Maintenance runs off the main request path
- Maintenance timeout/degradation/failure surfaces through explicit health/error reporting
- Shutdown is bounded and documented
- `cargo test --workspace` passes
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- `cargo fmt --check --all` passes
- `bash scripts/ci/validate_repo_boundaries.sh` passes
- Version is `1.1.0` in `workspace.package.version`

## 6. Out of Scope

- ATM-specific event schema changes
- ATM-specific CLI/daemon wiring
- ATM-specific policy defaults hardcoded into `sc-observability`
- Async runtime dependency
- Windows-specific file-locking behaviour beyond stable `GetFileInformationByHandle` parity

## 7. Non-Regression

Existing `Logger`, `LoggerConfig`, `JsonlFileSink`, query/follow, and OTLP surfaces must
not change their public API shape. Retained-log policy is additive.

## 8. References

- Issue #70: https://github.com/randlee/sc-observability/issues/70
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/cross-platform-guidelines.md`
- `.claude/skills/rust-development/guidelines.txt`
