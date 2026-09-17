# B.2 six-package qualification handoff

Candidate version: `1.4.0`, distinct from B.P2's historical staged `1.3.0`.
Status: implementation and qualification in progress; publication pending B.7.

## Candidate contract and adoption

Release order: `sc-observability-types`, `sc-observability`, `sc-observe`,
`sc-observability-otlp`, `sc-observability-log-macros`, `sc-observability-log`.
The bridge pins macros at `=1.4.0`. The CI-only consumer remains private.
BTIT can inspect the verified staged archives and use exact `=1.4.0`
dependencies plus `[patch.crates-io]` paths to fresh verified extractions;
no BTIT manifest or dependency tree changes occur in this sprint. The automated
consumer demonstrates this isolated configuration and records every resolved
first-party manifest path. Third-party dependencies resolve normally.

The qualification source SHA is embedded in every archive and the stage manifest.
It is not a publication source attestation. B.7 must rebuild and qualify the
final phase source after later fixes and separately prove registry consumption.

## Release-only adaptations

B.1's import-provenance.json and B.P2's prior staged bytes remain historical
records. The 1.4.0 workspace train and dependency pins, bridge/macros `publish`
flags, and per-crate root LICENSE copies are B.2 release adaptations, not edits
to the historical source inventory. The private consumer is still excluded.
B.P2 tools retain their four-package inventory in bp2-publish-artifacts.toml.

## Evidence

Actual immutable stage SHA, manifest/archive hashes, commands, logs, platform
run links and scoped API approvals will be recorded here after execution.
No QA PASS, API reviewer approval or three-platform result is asserted yet.
