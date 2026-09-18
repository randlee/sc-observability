---
id: C.2
status: proposed
branch: fix/phase-c-2-release-surface-preflight
base: fix/phase-c-1-shared-pipeline-migration
---

# C.2 — Phase B release-surface preflight

## Goal and dependencies

Preflight the complete, post-merge Phase B release surface through the
pipeline migrated in C.1. `must_follow` C.1: this sprint requires the
installed shared manifest/channel contract, the resolved npm publish step,
and the reconciled `release/publish-artifacts.toml` from C.1. Merge C.1's
pushed development forward before every round of this sprint; the trigger is
C.1's development push, not its QA outcome. This sprint runs preflight only;
it never publishes and never runs BTIT repository integration tests.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item; partial
completion leaves the sprint open.

1. **Deterministic release-surface inventory (PHC-004).** Re-derive the full
   publishable surface from the actual post-Phase-B-merge `develop` tree —
   do not reuse a pre-merge snapshot. At minimum this must enumerate:
   - Every Rust crate marked `publish = true` (explicitly or by unresolved
     Cargo default, per C.1 deliverable 2) in `release/publish-artifacts.toml`.
   - The `sc-observability-py` wheel/sdist artifact set across its full
     packaging matrix (the Python distribution/platform qualification work
     landed in Phase B's B.4a/B.5/B.6 sprints).
   - The `@sc-observability/client` npm package
     (`bindings/typescript/package.json`).
   - Applicable native/Tauri artifacts (`sc-observability-tauri` and any
     Tauri plugin distribution asset Phase B's B.3a sprint produced).
   Cross-check this inventory against `cargo metadata`, the wheel build
   matrix's declared targets, and `bindings/typescript/package.json`; a
   surface item present in the tree but missing from the inventory is a
   defect that blocks this sprint's closure, not an accepted gap.
2. **Package completeness preflight.** For each inventory item, verify the
   built artifact contains what it claims to (source bundle completeness for
   sdists, `abi3` tag correctness for wheels, declared `exports`/`types`
   fields for the npm package, correct crate metadata for `cargo package
   --locked --allow-dirty` dry runs) without publishing.
3. **Authentication setup verification without exposing tokens.** Confirm
   that every secret name documented in C.1 deliverable 7
   (`PYPI_API_TOKEN`, `TEST_PYPI_API_TOKEN`, `NPM_TOKEN`, and the existing
   crates.io token) is configured as a repository/environment secret using
   `gh secret list` (which reports names only, never values); do not attempt
   to read, print, or otherwise exercise a secret's value.
4. **Platform qualification cross-check.** Confirm the preflight process can
   name, for each wheel-matrix cell and the Tauri artifact set, which Phase B
   CI run last qualified it (run ID, source SHA, conclusion), reusing Phase
   B's retained evidence rather than re-running the full matrix. Where no
   post-merge qualification run yet exists for an item (for example, if
   `develop`'s merge commit itself has not been through the full matrix),
   this sprint records that as an explicit open gap and does not claim
   preflight completeness for that item.
5. **Recovery/idempotency verification.** Confirm the shared package's
   documented retry semantics — per-crate idempotent skip-if-already-published
   behavior, immutable-GitHub-Release-then-separate-channel-upload structure
   for PyPI — extend correctly to every inventory item added in this sprint,
   including the C.1-added npm workflow (which must itself be idempotent:
   re-running it against an already-published version must not fail the
   whole run, matching the shared PyPI workflow's `--skip-existing`
   equivalent behavior for npm, e.g. checking the registry before publishing
   or treating a 409/"already published" response as success).
6. **Post-publish verification design.** Because this sprint does not
   publish, "post-publish verification" here means specifying the exact
   commands a later, separately-authorized publish step will run to confirm
   each channel actually received the expected artifact (for example,
   `npm view @sc-observability/client versions`, `cargo search`/registry API
   checks per crate, `pip index versions sc-observability` or equivalent).
   Write these as ready-to-run acceptance commands, not prose description.

## Acceptance criteria (authoritative)

- The inventory from deliverable 1 is committed to this doc (or a generated
  artifact this doc links to) and matches `cargo metadata`,
  `bindings/typescript/package.json`, and the Python packaging matrix's
  declared targets with zero unexplained discrepancies.
- Deliverables 2–5 each have recorded PASS evidence (command + output) or an
  explicit, owner-visible open-gap note per deliverable 4's exception case.
- Deliverable 6's commands are present, syntactically valid, and dry-run
  confirmed not to require write/publish credentials to execute in read-only
  form (e.g., `npm view`, `cargo search`, `pip index versions` are read-only
  by nature).
- No channel receives a publish, upload, or tag-creation call anywhere in
  this sprint's validation.

## Required validation

- `gh secret list` (names only) for the repository, cross-checked against
  the documented secret-name list.
- `cargo package --locked --allow-dirty -p <crate>` (dry packaging, no
  publish) for every crate in the inventory.
- The shared package's manifest validation commands (same as C.1) re-run
  against the post-merge `develop` state to confirm no drift was introduced
  by the merge.
- Read-only registry-check commands from deliverable 6, executed against
  already-published versions only (never the unreleased Phase B version) to
  confirm they run without error.

## Paths to delete

None. This sprint is inventory/verification only; it does not remove files.

## Non-closure

- Does not publish anything to any channel.
- Does not create or push a release tag.
- Does not run BTIT repository integration tests — those remain gated on
  authorized publication, per `plan-phase-c.md`'s sequence.
- Does not resolve an open platform-qualification gap found in deliverable
  4 by re-running CI itself; it records the gap for a separate, explicitly
  authorized follow-up.
