# B.2 six-package qualification handoff

Candidate version: `1.4.0`, distinct from B.P2's historical staged `1.3.0`.
Status: implementation and all three platform qualifications complete. Lead
completeness and independent QA pending. Publication remains pending B.7.

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

## Final qualification evidence

[Run 35204643293](https://github.com/randlee/sc-observability/actions/runs/35204643293)
passed package-stage, all three platform consumers and aggregate verification
for branch and packaged source `136799758a5e91aabbf064ae50b5c7c6f20d4413`.
It was explicitly dispatched on the candidate branch after automatic PR
checks were not scheduled; this is only the staged-consumer workflow.
The definitive parent `c001cc6bd72a3ab97eddbaa343de05f516318c6f` was merged,
including the B.1 copy repair and final migration evidence. The final correction
selects exactly six Cargo packages even in a later expanded workspace and
composes historical import, post-import warning and release-only proof.
Later phase corrections require fresh B.7 qualification; this candidate is frozen.

Historical [run 35203709233](https://github.com/randlee/sc-observability/actions/runs/35203709233)
at branch `bdc6270` (packaged PR merge source `e8c05747094fb5299dfab49dc2df3629ed248cf7`)
is retained unchanged under `evidence/b2-qualification`; it is superseded by
the final candidate described here.

The complete downloaded bytes are retained under
[`evidence/b2-final/stage`](evidence/b2-final/stage), including
all six `.crate` files, `cargo-package.log` and `stage-manifest.json`.
The manifest includes each normalized Cargo manifest, its hash, every archive
member hash, the clean source SHA and the exact packaging command. Its SHA-256 is
`3e599c73b43ce2b8f87f08278890dc2e15d94ec4113bf4d14452436615815ae7`.
All archives are version `1.4.0`, under `stage/archives/<package>-1.4.0.crate`:

| Package (publication order) | Archive SHA-256 |
| --- | --- |
| `sc-observability-types` | `3a076bf879bc3c95578226ba973e8d73f2c545dcdec9f68c92651aa542a25b20` |
| `sc-observability` | `c63cd7dcb62199ad51f18650a1f8029999cbfde1b91d82b49f8e71d16d8ef26d` |
| `sc-observe` | `74e52b9ebc691427d1ca33b8981a24f8def08f196fb1eee1aa1589045fb324d9` |
| `sc-observability-otlp` | `65c961f3a1392505e5bc9ae54ee554fb9f9813fd4f587198d39e87d785aab54e` |
| `sc-observability-log-macros` | `207fa60837f22069d0e85a2221c797c08aa1f2bcebf1c7186414a988adb10a0d` |
| `sc-observability-log` | `d6f2ae8459947c643628223a2e79060683ad9f1f171e454896398fceb2f4cf49` |

The [`platforms`](evidence/b2-final/platforms) directory retains each
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
  --output docs/plans/phase-b/evidence/b2-final/stage --verify-existing
python3 scripts/ci/validate_log_platform_evidence.py --version 1.4.0 \
  --source-commit 136799758a5e91aabbf064ae50b5c7c6f20d4413 \
  --stage docs/plans/phase-b/evidence/b2-final/stage \
  --evidence-dir docs/plans/phase-b/evidence/b2-final/platforms
```

## Validation and API review

[`local-gates/results.json`](evidence/b2-final/local-gates/results.json)
records exact commands, exits and source `5ea0d5284a8885ab478bc537b78abd6575d0e207`;
its numbered raw logs are retained alongside it. Format, workspace all-target
tests, doctests, clippy with warnings denied, publish order, version literals,
docs consistency, both companion package lists and release-adaptation proof
all passed. No Rust implementation changed between that source and the frozen
branch candidate. The final CI package-stage also passed all 17 integrity and
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
[`public-api-diff.json`](evidence/b2-final/public-api-diff.json) and
[`api-docs.log`](evidence/b2-final/api-docs.log) retain the
final successful docs-gate evidence. This approval covers API surface only;
it does not settle behavior findings, owner runtime-contract acceptance,
independent QA or publication approval.

The separate release-adaptation record proves LICENSE bytes against the root
LICENSE and the two `publish = true` transitions against historical manifests.
B.1's source import record is unchanged. The final full import invocation
composes the B.1e warning record and B.2 release record; 52 import tests pass,
including acceptance of both and rejection of an unrecorded extra file.
`final-checks.json`, `import-tests.log` and `final-check-0.log` retain that proof.
One exploratory import command omitted `--source-repo` and failed argument
validation; local-gates/11.log retains it, and the complete final invocation
passed. It is not represented as a successful gate. B.P2's four-package
inventory remains separate and its qualification workflow also passed after
the source-version-independent staging repair.

## Downstream boundary

B.3 consumes these immutable bytes for its six-package dependencies. Its new
publishable DTO and later binding crates are outside the B.2 six-package
roster. B.2 tooling explicitly selects the six with Cargo `-p` arguments;
extra public members cannot broaden the stage, and the CI consumer must
remain private. Later owners extend their own API/release governance. This handoff does not
add those later crates to the frozen candidate or claim their qualification.
B.7 rebuilds the final corrected phase source, publishes only after its gates,
and executes the separate registry-only consumer proof.
