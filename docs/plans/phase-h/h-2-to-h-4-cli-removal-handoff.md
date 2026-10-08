# H-2 to H-4 CLI removal handoff

H-4 owns removal of the obsolete durable-store CLI dependencies and the
corresponding dependency-policy references. H-2 records the current declarations
here so the integration change can update them together after both frontends
have migrated. This is an inventory only; it does not authorize H-2 to edit
manifests, CI policy, or boundary enforcement.

| Location | Current declaration or expectation | H-4 handoff |
| --- | --- | --- |
| `crates/sc-otel-cli/Cargo.toml`, `[dependencies]` | `sc-observability-types.workspace = true` | Recheck whether the CLI still needs this direct dependency after the obsolete shared models are removed; remove it only if no active CLI code uses it. |
| `crates/sc-otel-cli/Cargo.toml`, `[dependencies]` | `sc-observability-otlp = { workspace = true, features = ["durable-store", "synchronous-client"] }` | Retire the `durable-store` feature selection when H-4 removes that implementation; retain the synchronous client feature required by the CLI. |
| `crates/sc-otel-cli/Cargo.toml`, `[dev-dependencies]` | `sc-observability-types = { workspace = true, features = ["test-double"] }` | Recheck CLI tests after migration and remove the dev dependency or test-double feature if those tests no longer use them. |
| `.github/workflows/ci.yml:90` | `cargo deny --manifest-path crates/sc-otel-cli/Cargo.toml --all-features --locked check --config policy/deny-durable-store.toml licenses bans advisories` | Update the CLI audit to the surviving native feature graph and policy configuration while preserving license, advisory, and dependency-ban checks. |
| `scripts/ci/tests/test_boundary_policy.py:23` | `SC_OTEL_CLI_EDGES = {"sc-observability-types", "sc-observability-otlp"}` | Reconcile this expected CLI edge set with the final manifest and boundary record; keep the test aligned with the surviving direct dependencies. |

These declarations are part of the H-4 integration cleanup described by
[ADR-023](../../architecture.md#adr-023-native-opentelemetry-and-thin-synchronous-frontends)
and the H-4 sprint bead. The test-double feature remains listed here as a
current declaration; its eventual removal depends on the migrated CLI tests,
not on this handoff alone.

The thin `sc-otel` CLI intentionally does not read `SC_OTEL_AUTH_HEADER`; this
is not a compatibility alias. ADR-023 supersedes ADR-021's submission
architecture for Phase H. Use `OTEL_EXPORTER_OTLP_HEADERS`, per-signal
`*_HEADERS`, or `--header NAME=VALUE` for credentials. H-4 should remove the
obsolete `SC_OTEL_AUTH_HEADER` compatibility path from any remaining legacy
installer or adapter if present; do not map it to `Authorization` in the thin
CLI.
