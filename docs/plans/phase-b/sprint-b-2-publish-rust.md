---
id: B.2
status: proposed
branch: feature/phase-b-2-publish-rust
base: develop
---

# B.2 — Qualify the Rust bridge and macros for publication

## Goal and dependencies

Stage the migrated bridge and macros crates as immutable qualified candidates
so BTIT can adopt them once B.7 performs the live publication; no crate is
published in this sprint. `must_follow` B.1e: qualify and stage B.1a–B.1e's
additive core error API and warning-only compatibility path, retaining B.1's
accepted bridge inventory/API. B.3 `must_follow` this sprint's exact staged
artifacts, not a registry release. Merge-forward follows the phase dependency
rule; the consumer check in this sprint resolves the staged packages directly,
not registry visibility.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Choose the initial companion candidate version through the workspace release
   train, including the minor core release selected by B.1e and its concrete
   deprecation version. This version is greater than B.P2's staged candidate
   version; never collide with or reuse a staged version. No legacy BTIT
   API/version constrains the new companion pair; the approved target contract
   is its first public baseline. Existing published core (1.2.0) semver gates
   remain. Maintain an exact bridge → macros `=V` pin and version+path
   dependencies that package correctly outside the checkout. Update release
   inventory, version-literal checks (including `=V`), package metadata/licenses,
   changelog and consumer docs. Preserve B.1's historical import manifest;
   record staging-only variations in the release handoff instead of rewriting
   source provenance. Create immutable candidate packages and record their
   normalized manifests, source commit, versions and checksums; do not publish.
2. Extend public-API tooling for the new packages, including proc-macro and
   never-published handling. Require an approval artifact specifically naming
   each affected crate; an unrelated existing approval file must not excuse
   a tool error or semver failure. Exclude the CI-only consumer package.
3. Stage the current candidate revision through the existing release preflight
   up to but not including live publication; this sprint's own branch is
   sufficient; qualification does not require unimplemented later-phase work
   to land on `main` first. Keep the four existing packages' dependency order
   and place macros before bridge. The staged set includes the changed core
   packages from B.1a–B.1e. B.7 later publishes in that same order from
   `main`, waiting for dependency index visibility between publishes; bounded
   retries must fail visibly rather than claiming completion.
4. Add `scripts/ci/validate_log_staged_consumer.py --version V`: create a
   clean temporary Cargo project that pins only the staged bridge/macros
   candidate archives by exact version+path (no ambient workspace checkout
   and no `[patch]` override); ordinary third-party dependencies still
   resolve normally from crates.io. Run an enabled macro plus explicit flush
   and shutdown against a temporary log directory. Store version, candidate
   source commit, checksums, staged package locations, dependency resolution
   and consumer results in `docs/plans/phase-b/handoff-b-2.md`. B.7 alone
   later runs its own separate registry-only consumer proof after real
   publication.

## Consumer contract

The consumer manifest pins the staged candidate version (substitute the staged `V`):

```toml
[dependencies]
sc-observability-log = "=V"
```

This sprint's staged-consumer check resolves the first-party pin against B.2's
immutable candidate packages directly, not the registry; other, ordinary
dependencies resolve normally from crates.io. The executable fixture uses the
exact accepted public initialization/lifecycle signatures captured by B.1; it
checks JSONL content after shutdown and does not copy private bridge support
code into the consumer.

## Acceptance criteria (authoritative)

- AC1: Each selected companion package has immutable staged bytes, a checked
  manifest, and recorded source/version/checksum provenance; publication
  remains pending. The CI-only crate is absent from the staged inventory.
- AC2: Package contents are self-contained and implement the approved target
  contract; public API/semver checks give actionable crate-specific results
  for every package.
- AC3: The staged-consumer fixture passes on macOS/Linux/Windows against the
  candidate packages directly. BTIT is given the staged version and adoption
  instructions without changing its dependency tree in this sprint. A
  release-ready PR, local package, or attempted publish alone does not close
  B.2; the required package and consumer evidence must be recorded.

## Required validation (authoritative)

```sh
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets -- -D warnings
bash scripts/ci/validate_publish_order.sh
python3 scripts/ci/validate_version_literals.py
bash scripts/ci/validate_public_api_diff.sh
python3 scripts/ci/validate_public_api_semver.py
bash scripts/ci/validate_public_api_docs.sh
bash scripts/ci/validate_docs_consistency.sh
cargo package --list -p sc-observability-log-macros
cargo package --list -p sc-observability-log
python3 scripts/ci/validate_log_staged_consumer.py --version "$RELEASE_VERSION"
```

API-diff exit 1 means a diff requiring review, not permission to suppress an
execution failure; attach the report and scoped approvals. Follow the
repository release preflight for chained package verification up through
staging; do not run its live-publish steps in this sprint. Record actual
staging commands/results from that workflow; do not improvise tags here.

## Paths to delete

None.

## Non-closure

No BTIT manifest/lockfile change or removal of its local crates. No binding
publication or behavior redesign, and no live crates.io publication of any
package; that is reserved for B.7 at phase end. Missing registry
credentials/approval is not this sprint's concern.
