# SC-Observability Cross-Platform Guidelines

## Required Rules

1. Do not hardcode `/tmp` in production code or tests.
2. Use explicit paths, `std::env::temp_dir()`, or `tempfile::TempDir` for local
   file outputs.
3. Do not derive paths from ATM-specific home helpers or runtime roots.
4. Use `PathBuf` and `.join()` for path construction.
5. Any OS-specific transport or file behavior must be behind explicit cfg gates.
6. If stable Rust lacks the platform API needed to preserve parity, the shared
   docs must state the degraded guarantee explicitly instead of implying
   cross-platform equivalence.

## Current Platform Limitations

1. Query/follow file identity is strong on Unix-family platforms through
   `(dev, ino)` metadata.
2. Windows query/follow file identity uses stable Win32 handle metadata via
   `GetFileInformationByHandle`, not nightly-only Rust metadata extensions.
3. Non-Unix, non-Windows targets still fall back to `(len, modified_nanos)`
   because stable Rust does not expose a stronger portable file identity there.

## Toolchain Baseline

1. The approved Rust toolchain baseline for this repo is `1.94.1`.
2. Toolchain bumps must update `rust-toolchain.toml`, the workspace
   `rust-version`, and every CI/release workflow toolchain pin in the same
   change.
3. Every toolchain bump must rerun `cargo fmt --check --all`,
   `cargo test --workspace`, and
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   before merge.

## Test Rules

1. Tests must isolate any local file output in temporary directories.
2. Tests that start listeners, subprocesses, or servers must use bounded waits
   and explicit teardown.
3. Tests must not rely on OS-specific default paths when explicit paths can be
   injected.
4. Logging-runtime changes that affect rotation, follow, or retained-log
   maintenance must prove parity on macOS, Linux, and Windows before merge.
5. Writer-thread queueing and batching must not change the documented
   query/follow visibility contract: queued-but-unflushed records are not
   required to appear in file-backed query/follow results.
