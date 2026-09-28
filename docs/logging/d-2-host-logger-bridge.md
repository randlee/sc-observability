# D2 host-owned logger bridge

`attach_logger` installs the `log` facade over an existing
`Arc<sc_observability::Logger>`. The attachment is deliberately non-owning:
it does not construct a writer, acquire a `LevelOwner`, flush the host as a
shutdown operation, or change the host logger's level.

The process slot has six operational states: `Empty`, `Owned`, `Attached`,
`Closing`, `Detached`, and `Stopped`. `Owned` remains the existing `LogGuard`
lifecycle. An attachment closes admission first, removes the slot, and waits
for calls that already entered the shared bridge path. A timeout restores
`Attached` so the same handle can retry. Successful detach transitions through
`Detached` and releases the attachment's logger reference, allowing a
host-held `Arc<Logger>` to recover unique ownership; `Stopped` is the terminal
state reported when the saved attachment control can no longer upgrade its
weak token.

`BridgeEventPolicy` runs on the assembled event immediately before every
facade, macro, and attached-control `try_log`. Rejections use the existing
`DropCause::InvalidEvent` counter and never reach the sink. Panics are caught
by the existing guarded submission boundary. Redaction remains solely the
host logger's responsibility, so the bridge does not introduce a second
redaction or accounting system.

Each attachment control carries only a weak token for its originating
attachment. Admission and bounded flush therefore remain tied to that
attachment; after detach, including after a later reattachment, stale
controls return a typed not-running result rather than silently routing
through the current global slot. A timed-out flush keeps its in-flight call
and attachment logger reference until the helper exits, so a successful retry
is the point at which `Arc::try_unwrap` can recover host ownership.
The public integration fixtures in `bridge_attachment.rs` and
`bridge_policy.rs`, plus the isolated foreign/owned-facade fixtures, cover
direct/macro routing, policy allowlisting, bounded payload rejection, host
redaction, policy panic, foreign-facade rejection, init/attach exclusion,
concurrent detach timeout/retry, reattachment, stale controls, and host
ownership recovery.
