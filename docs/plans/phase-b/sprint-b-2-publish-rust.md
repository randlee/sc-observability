---
id: B.2
status: proposed
branch: feature/phase-b-2-publish-rust
base: develop
---

# B.2 — Publish the Rust bridge and macros

## Goal and dependencies

Make the migrated crates available from crates.io so BTIT can adopt released
versions later. `must_follow` B.1a: publish its additive core error API and
warning-only compatibility path, retaining B.1's accepted bridge inventory/API. B.3 `must_follow` this sprint's published baseline. Merge-forward follows
the phase dependency rule; the consumer check waits for registry visibility.

## Deliverables (authoritative)

1. Remove `publish = false` only from the bridge and macros, and choose the
   initial release version through the workspace release train, including the
   minor core release required by B.1a and its concrete deprecation version. No legacy BTIT
   API/version constrains the new companion pair; the approved target contract
   is its first public baseline. Existing published core semver gates remain. Maintain an
   exact bridge → macros `=V` pin and version+path dependencies that package
   correctly outside the checkout. Update release inventory, version-literal
   checks (including `=V`), package metadata/licenses, changelog and consumer
   docs. Preserve B.1's historical import manifest; record publication-only
   variations in the release handoff instead of rewriting source provenance.
2. Extend public-API tooling for the new packages, including proc-macro and
   never-published handling. Require an approval artifact specifically naming
   each affected crate; an unrelated existing approval file must not excuse
   a tool error or semver failure. Exclude the CI-only consumer package.
3. Publish through the existing `main`-based release workflow after CI/review.
   Keep the four existing packages' dependency order and place macros before
   bridge. If the workspace release includes core packages, publish those
   first. Wait for dependency index visibility before publishing dependents;
   bounded retries must fail visibly rather than claiming completion.
4. Add `scripts/ci/validate_log_registry_consumer.py --version V`, which creates
   a clean temporary Cargo project with only a versioned bridge dependency,
   no path/git/patch overrides, and runs an enabled macro plus explicit flush
   and shutdown against a temporary log directory. Store version, immutable
   release commit/tag, checksums, registry links, dependency resolution, and
   consumer results in `docs/plans/phase-b/handoff-b-2.md`.

## Consumer contract

The consumer manifest is registry-only (substitute the published `V`):

```toml
[dependencies]
sc-observability-log = "=V"
```

The executable fixture uses the exact accepted public initialization/lifecycle
signatures captured by B.1; it checks JSONL content after shutdown and does not
copy private bridge support code into the consumer.

## Acceptance criteria (authoritative)

- AC1: Both public packages are downloadable at the recorded version, and the
  downloaded bridge resolves the exact matching macros package. The CI-only
  crate is absent from the publish inventory.
- AC2: Package contents are self-contained and implement the approved target contract; public
  API/semver checks give actionable crate-specific results for every package.
- AC3: The clean registry-only fixture passes on macOS/Linux/Windows. BTIT is
  given the version and adoption instructions without changing its dependency
  tree in this sprint. A release-ready PR alone does not close publication.

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
python3 scripts/ci/validate_log_registry_consumer.py --version "$RELEASE_VERSION"
```

API-diff exit 1 means a diff requiring review, not permission to suppress an
execution failure; attach the report and scoped approvals. Follow the repository
release preflight for chained package verification: dependent registry resolution
cannot succeed before the matching dependency is available. Record actual
publication commands/results from that workflow; do not improvise tags here.

## Paths to delete

None.

## Non-closure

No BTIT manifest/lockfile change or removal of its local crates. No binding
publication or behavior redesign. Missing registry credentials/approval leaves
publication pending, not complete.
