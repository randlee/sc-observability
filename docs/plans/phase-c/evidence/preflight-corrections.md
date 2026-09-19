# C.2 preflight correction evidence — historical PR185 record

Source reports: [PR184 QA report](https://github.com/randlee/sc-observability/pull/184#issuecomment-5738623032)
and the authorized non-publishing preflight run [35415396742](https://github.com/randlee/sc-observability/actions/runs/35415396742).
Correction layer: [PR185](https://github.com/randlee/sc-observability/pull/185).
The active immutable upstream `sc-publish` pin is
`cb29cb4926e09a5a4f4609e118429a2e14f69827`; upstream CI
`35416135181` passed. Its installer was applied once and the immediate
dry-run reported `Publish-kit assets are in sync.`

| Finding | Disposition | Evidence |
| --- | --- | --- |
| PHC-QA-014 | fixed-candidate | `scripts/release_bindings_artifacts.py` now uses qualified `.github/scripts/release_artifacts.py` references throughout its module docstring. |
| PHC-QA-016 | fixed-candidate | C.2 deliverable-4 text now records Linux/macOS/Windows real-IPC qualification complete, matching the closure section for run `35410593107`. |
| PHC-QA-019 | fixed-candidate/tool-open | Added a genuine `boundaries/` inventory and `planning.toml` derived from all workspace package dependency edges. The pinned `sc-lint` root smoke now discovers the repository and reaches backend analysis, which fails with `unsupported impl owner type &FieldValue<'_, T>` in the shared tool; the minimal valid source pattern is `crates/sc-observability-log/src/callsite.rs:217` (`impl<T: ?Sized + serde::Serialize> SerializeKindTag for &FieldValue<'_, T>`). This is upstream/tool-owned, not hidden with a sentinel or disabled check. |
| PHC-QA-004 | external/open | npm environment configuration and lead completeness remain external gates. |
| PHC-QA-015 | external/open | npm GitHub Environment and environment-scoped `NPM_TOKEN` require authorized repository configuration. |
| PHC-QA-017 | candidate-adopted | Immutable `sc-publish` pin `cb29cb49` adds the manifest-authorized, read-only `CARGO_REGISTRY_TOKEN` probe; local regression coverage rejects malformed, redirected, unauthorized, or transport-failed responses without disclosing credentials. Publication remains separately authorized. |
| PHC-QA-018 | candidate-adopted | The candidate package-check flow performs real local `cargo check --locked` and packages dependent archives with `--no-verify --exclude-lockfile`, while retaining full verification for independent crates. The isolated regression fixture passes and publication is not attempted. |

The real pinned smoke command used for PHC-QA-019 was:

```sh
sc-lint --json --root "$PWD" lint sc-boundary
```

It first failed root discovery because `boundaries/` was absent; after the
inventory was added, root discovery passed and the backend emitted the
unsupported-impl-owner diagnostic above.

## Current reconciliation

This table records the PR185 point in time. Current pins, fixed23 manifest
reports / retained18 cycle diagnostics, Linux/Windows source receipt PASS,
resolved npm secret scope and actual preflight35423505198 are recorded in
[final-c2-audit.md](final-c2-audit.md). Historical parser/npm-open labels above
are not current blockers. Current Python packaging qualification and explicit
lead completeness remain pending; no closure signoff is fabricated.
