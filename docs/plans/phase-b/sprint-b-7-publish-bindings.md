---
id: B.7
status: proposed
branch: feature/phase-b-7-publish-bindings
base: develop
---

# B.7 — Publish the language binding packages

## Goal and dependencies

Publish the already-working B.3a TypeScript and B.4/B.5/B.6 Python artifacts so external
consumers can install them. `must_follow` B.6 and transitively B.5/B.4a/B.4/B.3a/B.3b/B.3/B.2: preserve
the accepted runtime/schema contracts. This sprint owns distribution and
registry proof, not unfinished runtime work. Shared release/runtime artifacts
preclude parallel_safe execution; follow parent-push merge-forward before each
child development/fix round and parent PR merge before child completion.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Add `release/bindings-artifacts.toml` recording Rust DTO/native-runtime/Tauri-adapter/embedding crates,
   npm and PyPI package names, owners, versions, schema compatibility, source
   commit and artifact paths/hashes. Check registry name/control availability
   before freezing names; record any reviewed rename consistently. Preserve
   independently versioned language packages and schema-v1 compatibility.
2. Extend the existing release workflow with explicit binding artifact jobs.
   Publish prerequisite Rust DTO/native-runtime/adapter crates before packages or sdists that
   resolve them from registries. Build Python wheels from an immutable main
   release commit for the B.4a matrix and publish those tested bytes. Include
   stubs, py.typed, licenses and source distribution; npm ships generated
   declarations, client, transport and package exports tested by B.3a.
3. Create `scripts/ci/validate_binding_registry_consumers.sh` to install exact
   npm/PyPI versions into fresh external fixtures with no local path override.
   Exercise the TypeScript client with the released host adapter and the Python
   API against temporary logs. Build a registry-only Rust embedding consumer
   using sc-observability-py and the supplied shared native backend, and prove
   shared Rust/Python records without implementing new DTO mappings. Write `docs/plans/phase-b/handoff-b-7.md` with
   registry URLs, versions, hashes, source tag, public API coverage, platform
   results and adoption examples. Update root consumer/release documentation.

Proposed crates.io names: `sc-observability-dto`,
`sc-observability-binding-runtime`, `sc-observability-tauri`, and
`sc-observability-py`. B.7 verifies ownership/availability for each before release.
Publish in that dependency order; language packages consume these tested versions.

## Release compatibility record

```toml
schema_version = 1

# One entry per actual artifact; values are filled from the accepted release.
[[artifacts]]
kind = "npm" # other entries: pypi or crates-io
name = "@sc-observability/client"
version = "<approved package version>"
dto_schema = 1
source_commit = "<full immutable main commit>"

[[artifacts]]
kind = "crates-io"
name = "sc-observability-tauri"
version = "<approved crate version>"
dto_schema = 1
source_commit = "<full immutable main commit>"

[[artifacts]]
kind = "crates-io"
name = "sc-observability-py"
version = "<approved crate version>"
dto_schema = 1
source_commit = "<full immutable main commit>"
```

The record is rejected if placeholders remain at release. Package semver follows
its own public surface; incompatible wire changes require a new DTO schema.
Registry credentials and publication approval use existing release controls;
no automated job claims success before artifacts are retrievable. Never overwrite
a released version or substitute different wheel bytes after testing.

## Acceptance criteria (authoritative)

- AC1: All approved artifacts are published, downloadable, and match the recorded
  tested hashes/source commit. No source-tree path dependency escapes into
  distributable packages; Rust prerequisites resolve from crates.io.
- AC2: Registry-only TypeScript and Python examples exercise supported runtime
  operations on their promised platform matrices. Types/stubs and schema version
  agree across installed artifacts. Examples handle or deliberately ignore
  discriminated results; no published wrapper replaces them with exceptions.
- AC3: Documentation distinguishes Rust bridge adoption, Tauri frontend usage,
  Python-owned logging and deferred Go support. Missing publication access leaves
  this sprint pending; a staged package or release-ready PR is not closure.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_typescript_bindings.sh
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_binding_registry_consumers.sh
bash scripts/ci/validate_publish_order.sh
python3 scripts/ci/validate_version_literals.py
bash scripts/ci/validate_public_api_docs.sh
bash scripts/ci/validate_docs_consistency.sh
```

The registry-consumer script reads exact versions from the release record and
rejects placeholders, unavailable artifacts, version/schema mismatches or skipped
platform evidence. Reuse the accepted B.3a/B.4/B.4a/B.5/B.6 runtime/package tests on release artifacts;
do not replace them with import-only checks.

## Paths to delete

None.

## Non-closure

No BTIT rollout, Go implementation, new runtime features, expanded platform
support or unreviewed DTO changes. A future Go sprint must select its runtime
transport/ABI and ownership model separately while reusing the shared data
contract where appropriate.
