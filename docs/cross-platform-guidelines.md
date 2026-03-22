# SC-Observability Cross-Platform Guidelines

## Required Rules

1. Do not hardcode `/tmp` in production code or tests.
2. Use explicit paths, `std::env::temp_dir()`, or `tempfile::TempDir` for local
   file outputs.
3. Do not derive paths from ATM-specific home helpers or runtime roots.
4. Use `PathBuf` and `.join()` for path construction.
5. Any OS-specific transport or file behavior must be behind explicit cfg gates.

## Test Rules

1. Tests must isolate any local file output in temporary directories.
2. Tests that start listeners, subprocesses, or servers must use bounded waits
   and explicit teardown.
3. Tests must not rely on OS-specific default paths when explicit paths can be
   injected.
