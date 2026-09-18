# sc-observability-log

`sc-observability-log` bridges the Rust `log` facade and tracing-compatible
macros to one process-global `sc_observability::Logger` with structured JSONL
output. Its reviewed B.P3 public control contract is direct and typed: producers
submit `BridgeEvent`, never an adapter record or binding-only failure report.

## Quick start

```rust,no_run
use std::time::Duration;
use sc_observability_log::{
    ActionName, BridgeEvent, BridgeOptions, EventLevel, LoggerConfig, ServiceName, TargetCategory,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let config = LoggerConfig::default_for(ServiceName::new("my-app")?, "/var/log/my-app".into());
let guard = sc_observability_log::init(config, BridgeOptions {
    default_action: ActionName::new("log.record")?,
    parse_bracket_action: true,
})?;
let control = guard.control();
control.try_log(BridgeEvent {
    level: EventLevel::Info,
    target: TargetCategory::new("my_app.ui")?,
    action: None,
    message: Some("clicked".to_owned()),
    outcome: None,
    fields: serde_json::Map::new(),
    request_id: None,
    correlation_id: None,
    trace: None,
})?;
control.flush(Duration::from_secs(1))?;
guard.shutdown(Duration::from_secs(5))?;
# Ok(())
# }
```

## Public contract

`LogGuard` is the only shutdown owner and is not `Clone`. `LogControl` is
`Clone + Send + Sync`, has no shutdown authority, and offers `try_log`, `query`,
`flush`, `health`, `active_log_path`, `dropped_events`, and `wait_stopped`.
Facade calls, macros, and `try_log` share one guarded admission path and one
writer. A direct accepted event is queued; a filtered event is not a drop.

`BridgeEvent` supplies producer data only. The bridge supplies version,
timestamp, service, identity, routing, redaction, and ambient trace unless an
explicit trace is supplied. Invalid direct field keys return `EmitError` and are
counted exactly once. The six native operation error enums are serde data with
stable `code()`/`remediation()` methods; they retain `OperationDiagnostic`
values rather than opaque source errors.

The bridge installs once per process. After shutdown, controls retain health,
path, counters, and the single saved `ShutdownReport`; `try_log`, query, and
flush reject the non-running lifecycle. A timed-out owner shutdown remains
`stopping` until the reserved worker publishes its eventual stopped or failed
outcome. Runtime level changes are owner-only and retained in health after
shutdown.

`log::logger().flush()` is intentionally a no-op. Use bounded `LogControl` or
`LogGuard` flush. A concurrent in-flight flush returns `FlushError::InProgress`;
no second helper is started.

`docs/mapping.md` records producer mapping, field-key, lifecycle, health, and
error behavior. Historical Phase A evidence remains under `docs/plans/phase-a/`;
it describes the superseded unpublished adapter API and is not current API
documentation.
