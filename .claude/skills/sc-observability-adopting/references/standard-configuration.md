# Standard Configuration

This reference defines the `sc-observability` house style used by these
skills.

## Scope

This is a skill-defined downstream convention. It is not a built-in
`sc-observability` crate default.

`CONSUMING.md` shows `PathBuf::from("./observability")` as the minimal crate
usage example. This skill layers the `~/.<app>` house style on top of that
baseline for teams that want one consistent convention across apps.

## Recommended Dependencies

```toml
[dependencies]
sc-observability = "1"
serde_json = "1"
dirs = "6"
```

Add `sc-observability-types` directly only when:

- implementing custom sinks
- extending the shared types layer directly

## Default Log Root

Use `~/.<app>` as the default log root convention.

With the built-in file sink, the resulting active file path becomes:

```text
~/.<app>/logs/<service>.log.jsonl
```

This works by setting `log_root` to the expanded home-relative directory. The
crate's actual file layout rule remains:

```text
<log_root>/logs/<service>.log.jsonl
```

## Home Directory Expansion

Generated starter code should use the `dirs` crate:

```rust
dirs::home_dir().map(|p| p.join(format!(".{app}")))
```

If a home directory cannot be resolved:

- prefer an explicit app-provided path
- or return a configuration error

Do not silently assume an OS-specific fallback path.

## Light Logging Baseline

- built-in file sink enabled
- built-in console sink disabled
- warnings and errors always emitted
- informational logging remains sparse and intentional
- long-running apps emit startup and shutdown events
- CLIs emit one success event per successful command
- use stable `target` and `action` names

## Runtime Verification

Use `logger.health()` for runtime checks.

At minimum, downstream apps should be able to inspect:

- aggregate logger state
- active log path
- sink status

`logger.health().active_log_path` is the standard way to confirm the resolved
path at runtime.
