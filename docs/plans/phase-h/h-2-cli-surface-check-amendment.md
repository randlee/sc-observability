# Phase H h-2 acceptance amendment

Reviewed by the task assigner in ATM message `01M4CMBSXDTA9399WWKC0QD1BB`.

Remove the h-2 acceptance criterion “Run existing ADR-022 CLI surface check.”
ADR-022 defines cross-target public API parity for packages with
`publish = true` in `release/publish-artifacts.toml`. The `sc-otel-cli` crate
sets `publish = true` but has no library target, so the ADR-022 API gates skip
it and the check cannot cover its Clap surface. There is no separate ADR-022 CLI surface
check.

Keep the existing h-2 criteria for Clap parsing/help, generated manual
consistency, and CLI documentation tests unchanged. Do not add a replacement
API snapshot system or alter dependency edges or published history.
