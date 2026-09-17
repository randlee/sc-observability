---
id: B.2
status: proposed
branch: feature/phase-b-2-publish-rust
base: develop
---

# B.2 — Qualify the Rust bridge and macros for phase-end publication

## Goal and dependencies

Prepare immutable bridge/macros artifacts so downstream sprints can consume the
qualified staged versions before Phase B's single live release. `must_follow`
B.1e: qualify B.1a–B.1e's additive core error API and warning-only compatibility
path, retaining B.1's accepted bridge inventory/API. B.3 `must_follow` this
sprint's staged baseline. Merge-forward follows the phase dependency rule.
Live crates.io publication and registry-only proof are deferred to B.7.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Prepare publishable bridge and macros manifests, and choose the
   initial companion release version through the workspace release train, including the
   minor core release selected by B.1e and its concrete deprecation version.
   This version is greater than B.P2's staged runtime-level version; never
   republish its versions. No legacy BTIT API/version constrains the new companion pair; the approved target contract
   is its first public baseline. Existing published core semver gates remain. Maintain an
   exact bridge → macros `=V` pin and version+path dependencies that package
   correctly outside the checkout. Use `cargo package`/dry-run and an isolated
   staged source bundle to produce immutable candidate archives, manifests and
   checksums; do not write any registry. Update release inventory, version-literal
   checks (including `=V`), package metadata/licenses, changelog and consumer
   docs. Preserve B.1's historical import manifest; record publication-only
   variations in the release handoff instead of rewriting source provenance.
2. Extend public-API tooling for the new packages, including proc-macro and
   never-published handling. Require an approval artifact specifically naming
   each affected crate; an unrelated existing approval file must not excuse
   a tool error or semver failure. Exclude the CI-only consumer package.
3. Record the planned dependency order (changed core packages before macros
   before bridge), exact staged source commit, archive/manifests/checksums and
   release-preflight results. B.7 performs the `main`-based live release, index
   wait and dependency-order publication after all Phase B artifacts qualify.
4. Add `scripts/ci/validate_log_staged_consumer.py --version V`, which creates
   a clean temporary Cargo project from the staged archives with no ambient
   workspace dependency, runs an enabled macro plus explicit flush and shutdown
   against a temporary log directory, and records the selected staged artifacts.
   Store the version, source commit, checksums, dependency resolution and
   consumer results in `docs/plans/phase-b/handoff-b-2.md`. B.7 later adds the
   separate registry-only consumer result.

## Consumer contract

The intermediate consumer manifest uses only the staged archive/source
configuration for the selected `V`; it has no ambient checkout dependency:

```toml
[dependencies]
sc-observability-log = "=V"
```

The executable fixture uses the exact accepted public initialization/lifecycle
signatures captured by B.1; it checks JSONL content after shutdown and does not
copy private bridge support code into the consumer.

## Acceptance criteria (authoritative)

- AC1: Both public packages have immutable staged archives at the recorded
  version, and the staged bridge resolves the exact matching macros package.
  The CI-only crate is absent from the publish inventory.
- AC2: Package contents are self-contained and implement the approved target contract; public
  API/semver checks give actionable crate-specific results for every package.
- AC3: The clean staged fixture passes on macOS/Linux/Windows. BTIT is given
  the exact staged version/checksum and adoption instructions without changing
  its dependency tree in this sprint. A release-ready PR, dry-run, or staged
  package alone is not live-publication closure.

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
execution failure; attach the report and scoped approvals. Follow the repository
release preflight for chained staged-package verification. B.7 later records
actual publication commands, index visibility and registry-only proof; do not
improvise tags or publish here.

## Paths to delete

None.

## Non-closure

No BTIT manifest/lockfile change or removal of its local crates. No live binding
or Rust publication, or behavior redesign. Missing registry credentials/approval
are phase-end B.7 concerns, never permission to publish early.
