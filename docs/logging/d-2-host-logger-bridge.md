# D2 host-owned logger bridge

`attach_logger` installs the `log` facade over an existing
`Arc<sc_observability::Logger>`. The attachment is deliberately non-owning:
it does not construct a writer, acquire a `LevelOwner`, flush the host as a
shutdown operation, or change the host logger's level.

The process slot has six operational states: `Empty`, `Owned`, `Attached`,
`Closing`, `Detached`, and `Stopped`. `Owned` remains the existing `LogGuard`
lifecycle. One mutex guards the attachment mode, slot, and in-flight count;
no host callback runs under that mutex. An attachment closes admission first
and waits for entered calls. An explicit detach timeout restores `Attached`
for retry. If bounded drop times out, the last entered call transitions
`Closing` to `Detached`, allowing another attachment. Each call releases its
logger reference before decrementing the count, so successful detach permits
host-held `Arc<Logger>` ownership recovery. `Stopped` is the terminal owned
bridge state, not an attachment-control error.

An unrepresentable detach deadline (`Duration::MAX`) waits for entered calls
to drain without a deadline. Service identity comes from `Logger::service_name`,
independent of log paths and whether a file sink exists.

`BridgeEventPolicy` runs on the assembled event immediately before every
facade, macro, and attached-control `try_log`. Rejections use the existing
`DropCause::InvalidEvent` counter and never reach the sink. Their diagnostics
use `SC_OBSERVABILITY_LOG_POLICY_REJECTED` with reason-specific messages and
concrete recovery steps. `LogAttachment::last_policy_rejection` retains the
latest direct, facade, or macro rejection for inspection. Panics are caught
by the existing guarded submission boundary. Redaction remains solely the
host logger's responsibility, so the bridge does not introduce a second
redaction or accounting system.

Each attachment control carries only a weak token for its originating
attachment. Admission and bounded flush therefore remain tied to that
attachment; after detach, including after a later reattachment, stale
controls return the typed `NotInstalled` operation result and record the
canonical `DropCause::NotInstalled` accounting bucket rather than silently
routing through the current global slot, as required by ADR-019. A timed-out flush keeps its in-flight call
and attachment logger reference until the helper exits, so a successful retry
is the point at which `Arc::try_unwrap` can recover host ownership.
The public integration fixtures in `bridge_attachment.rs` and
`bridge_policy.rs`, plus the isolated foreign/owned-facade fixtures, cover
direct/macro routing, policy allowlisting, bounded payload rejection, host
redaction, policy panic, foreign-facade rejection, init/attach exclusion,
concurrent detach timeout/retry, reattachment, stale controls, and host
ownership recovery.

## Tracing-bridge ownership

The `log` facade and a `tracing` bridge are separate foreign logger owners. An
application may compose them only by selecting one process-global owner and
routing the other facade into it; `attach_logger` never installs or shuts down
a tracing subscriber. Hosts must install their tracing bridge before choosing
the owner, or explicitly detach the attachment before transferring ownership.
