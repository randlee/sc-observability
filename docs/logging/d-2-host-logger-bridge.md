# D2 host-owned logger bridge

`attach_logger` installs the `log` facade over an existing
`Arc<sc_observability::Logger>`. The attachment is deliberately non-owning:
it does not construct a writer, acquire a `LevelOwner`, flush the host as a
shutdown operation, or change the host logger's level.

The process slot has four operational states: `Empty`, `Owned`, `Attached`,
and `Closing`. Owned `init` remains the existing `LogGuard` lifecycle. An
attachment closes admission first, removes the slot, and waits for calls that
already entered the shared bridge path. A timeout restores `Attached` so the
same handle can retry. Successful detach releases the attachment's logger
reference, allowing a host-held `Arc<Logger>` to recover unique ownership.

`BridgeEventPolicy` runs on the assembled event immediately before every
facade, macro, and attached-control `try_log`. Rejections use the existing
`DropCause::InvalidEvent` counter and never reach the sink. Panics are caught
by the existing guarded submission boundary. Redaction remains solely the
host logger's responsibility, so the bridge does not introduce a second
redaction or accounting system.

Controls consult the attachment slot at admission time. After detach they
return a typed not-running result rather than silently routing through an
empty slot.
The public integration fixtures in `bridge_attachment.rs` and
`bridge_policy.rs` cover direct/macro routing, policy rejection and panic,
concurrent detach timeout/retry, stale controls, and host ownership recovery.
