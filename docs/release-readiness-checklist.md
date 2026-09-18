# Release Readiness Checklist

- [x] The ten-crate Phase-B Rust publication inventory is recorded in
  `release/publish-artifacts.toml`; the three standalone/binding manifests are
  validated by path, while companion package channels remain in
  `release/bindings-artifacts.toml`.
- [x] API design, requirements, architecture, and implementation plan docs are present.
- [x] ATM adapter requirement, architecture, mapping, and example docs are present.
- [x] `docs/public-api-checklist.md` is fully marked complete.
- [x] Query/follow public API is implemented and matches `requirements.md` and `architecture.md`.
- [x] OTLP attachment is shipped as a public integration surface rather than test-only scaffolding.
- [x] UTC-only timestamp behavior is enforced in the shared public type system.
- [x] Workspace version is set and shared through `workspace.package.version`.
- [x] Shared crate Cargo manifests use the workspace version.
- [x] Duplicate plain-text release version literals are validated against `workspace.package.version`.
- [x] Publish manifest exists at `release/publish-artifacts.toml`.
- [x] Publish order validation script exists and passes.
- [x] Release preflight workflow exists.
- [x] Release workflow exists.
- [x] CI includes docs-consistency and dependency-ban validation.
- [x] Repo-boundary validation passes.
- [x] ATM adapter example compiles and runs in normal mode.
- [x] ATM adapter example compiles and runs in fail-open mode.
- [x] Migration guide for ATM consumers exists.
- [x] Performance review is documented.
- [x] Source-level inventory, manifest, dependency, boundary, and ordering
  gates have passing evidence on the candidate revision.
- [ ] Final release QA (including platform consumers and registry dry-run
  evidence) remains pending; no tag or publication is authorized by this
  checklist.
- [ ] B.1e typed-error migration warning rollout is separately qualified; the
  current preparation docs/inventory do not assert publication or removal.
