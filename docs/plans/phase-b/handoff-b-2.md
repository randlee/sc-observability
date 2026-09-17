# B.2 six-package qualification handoff

Candidate version: `1.4.0`, distinct from B.P2's historical staged `1.3.0`.
Status: prior candidate qualified; final integration and explicit package-selection
correction in progress. Lead completeness and independent QA pending.

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

## Historical qualification evidence (superseded by final integration)

[Run 35203709233](https://github.com/randlee/sc-observability/actions/runs/35203709233)
passed package-stage, macOS/Linux/Windows consumers and the aggregate check.
Its branch head is `bdc6270bce433183bf46b71e8d2c9b7d8bff3e2b`; GitHub tested
and packaged PR merge source `e8c05747094fb5299dfab49dc2df3629ed248cf7`.
The latter is the source attested by every archive, not the branch head.
The final parent merge was `67e715d5be90aaf60f02289bfc3dbb4255ccd94b`, carrying
integration `15ff602f06ff30725ccf316ba28fd7a3fbbb8b20` and copy repair `015a888`.
Later phase corrections require fresh B.7 qualification; this candidate is frozen.

The complete downloaded bytes are retained under
[`evidence/b2-qualification/stage`](evidence/b2-qualification/stage), including
all six `.crate` files, `cargo-package.log` and `stage-manifest.json`.
The manifest includes each normalized Cargo manifest, its hash, every archive
member hash, the clean source SHA and the exact packaging command. Its SHA-256 is
`0a18b0dc021625bfc416034e62e731e8607bf5b4a34ce1138707bf82dce0ccaa`.
All archives are version `1.4.0`, under `stage/archives/<package>-1.4.0.crate`:

| Package (publication order) | Archive SHA-256 |
| --- | --- |
| `sc-observability-types` | `679ba0eff7a612bafaf3b59626c17b37ba93544b8ef07400ee8a4723de9cde03` |
| `sc-observability` | `d89389edb10bd8daa9f3614ff6544afcbace94f207109b7dd5e975878bcb333d` |
| `sc-observe` | `91a6fd90761fa11e148245daa9475e6c6bf5c2a1027a3888c88d701fc0ea6bf6` |
| `sc-observability-otlp` | `68484f47640a4c8816da51fdd65a2aa58859a5e8fcdb973d2ccfc82140b79871` |
| `sc-observability-log-macros` | `43bbe50c30954bb2657c89357191849228688fa835664f80b5540b1768458652` |
| `sc-observability-log` | `add41c97d7fe5f9a1ec8bd71cdd6e721252ad84921b271c59cb62c60b0c7bade` |

The [`platforms`](evidence/b2-qualification/platforms) directory retains each
platform's JSON result and actual command log. Every result records the same
candidate source, manifest hash and six archive hashes, plus archive locations,
exact dependency-resolution manifests under a fresh temporary extraction,
commands and log hash. All three pass the enabled macro, explicit flush,
shutdown and JSONL payload assertions. The consumer pins Rust/Cargo `1.94.1`,
isolates Cargo home/target and rejects ambient checkout resolution. These are
staged-artifact results; they do not establish registry visibility.

Recheck the retained bytes without rebuilding or contacting the registry:

```sh
python3 scripts/ci/prepare_log_staged_packages.py --version 1.4.0 \
  --output docs/plans/phase-b/evidence/b2-qualification/stage --verify-existing
python3 scripts/ci/validate_log_platform_evidence.py --version 1.4.0 \
  --source-commit e8c05747094fb5299dfab49dc2df3629ed248cf7 \
  --stage docs/plans/phase-b/evidence/b2-qualification/stage \
  --evidence-dir docs/plans/phase-b/evidence/b2-qualification/platforms
```

## Validation and API review

[`local-gates/results.json`](evidence/b2-qualification/local-gates/results.json)
records exact commands, exits and source `67e715d5be90aaf60f02289bfc3dbb4255ccd94b`;
its numbered raw logs are retained alongside it. Format, workspace all-target
tests, doctests, clippy with warnings denied, publish order, version literals,
docs consistency, both companion package lists and release-adaptation proof
all passed. No Rust implementation changed between that source and the frozen
branch candidate. The final CI package-stage also passed all 14 integrity and
bounded-index tests. Their cases reject archive tampering, wrong version/source,
traversal, ambient paths, private consumer leakage and invalid approval scope.
The future live-publish workflow's index retries are bounded and fail visibly;
that workflow was edited but never executed.

[`b2-api`](evidence/b2-api/README.md) contains each crate's exact API stdout,
SHA-256 and semver result. The four existing crates pass minor semver against
published `1.2.0`. The two new companions have verified absent published
baselines and successful initial API generation. API diff exit 1 reports
reviewable additions; no tool error or semver failure is waived.
The actual [aobs approval](../../api-approvals/phase-b-1.4.0.json), dated
2026-09-17T09:06:54.905317+00:00, names all six API digests reviewed at `6bb5cc1`.
All six digests remain equal at the merged source and final branch candidate;
[`api-diff-final.json`](evidence/b2-qualification/api-diff-final.json) and
[`api-docs-final.log`](evidence/b2-qualification/api-docs-final.log) retain the
final successful docs-gate evidence. This approval covers API surface only;
it does not settle behavior findings, owner runtime-contract acceptance,
independent QA or publication approval.

The separate release-adaptation record proves LICENSE bytes against the root
LICENSE and the two `publish = true` transitions against historical manifests.
B.1's source import record is unchanged; post-import warning/adaptation
composition is owned by the B.1e integration layer. B.P2's four-package
inventory remains separate and its qualification workflow also passed after
the source-version-independent staging repair.

## Downstream boundary

B.3 consumes these immutable bytes for its six-package dependencies. Its new
publishable DTO and later binding crates are outside the B.2 six-package
roster; later release-governance integration must explicitly select the six
when rerunning B.2 tooling on the expanded workspace. This handoff does not
add those later crates to the frozen candidate or claim their qualification.
B.7 rebuilds the final corrected phase source, publishes only after its gates,
and executes the separate registry-only consumer proof.
