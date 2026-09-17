---
id: B.4a
status: in_progress
branch: feature/phase-b-4a-python-packaging
base: feature/phase-b-4-python
---

# B.4a — Python distributions and platform qualification

## Goal and dependencies

Deliver installable distributions of the complete B.4 runtime API.
`must_follow` B.4 for tested owned/attached behavior and build configuration;
B.5 `must_follow` B.4a for its qualified distribution baseline. Shared package
sources, lockfiles and CI artifacts preclude parallel_safe work. Parent pushes
trigger merge-forward before every child dev/fix round; parent PR merges first.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Complete maturin packaging in `bindings/python/sc-observability-py/` using
   B.4's locked dependencies, `abi3-py310`, stubs and `py.typed`. Produce wheels
   plus a self-contained sdist with the complete Rust/Python source bundle,
   including unpublished DTO/shared-runtime/embedding dependencies and locked
   published transitive dependencies. Keep extension linking separate from the Rust embedding target.
   Use the prepublication packaging procedure below; no publication is needed
   to build an artifact and no path outside its unpacked root may be resolved.
   Normalized publishable crate manifests keep versioned dependency declarations;
   sdist-local overrides never become a registry publishing requirement.
2. Add the wheel-build/install CI matrix: GIL-enabled CPython 3.10, 3.11, 3.12,
   3.13 and 3.14 on each of macOS arm64, macOS x86_64, Linux glibc x86_64,
   Linux glibc aarch64 and Windows x86_64. Use manylinux_2_28 on Linux;
   macOS minimum deployment targets are 11.0 arm64 and 10.13 x86_64. One abi3
   wheel per platform is exercised on every listed interpreter; cross-build
   success alone is not runtime evidence. Unsupported dependencies fail the
   gate and require an explicit plan correction, never silent matrix reduction.
3. Extend `scripts/ci/validate_python_bindings.sh` to run B.4's full API/error/
   lifecycle suite and type checks against clean installed artifacts on every
   matrix cell, run the Rust embedding example with its separate link settings,
   and rebuild/install from the sdist outside the checkout. Record exact wheel
   tags, interpreter/platform versions, deployment targets, artifact hashes,
   source revision, bundle inventory/checksums and per-cell results in `docs/plans/phase-b/handoff-b-4a.md`.

B.3 implements `scripts/ci/build_binding_source_bundle.py`; this sprint reuses
that helper for sdist and embedding distributions and owns matrix qualification.
Earlier sprint validators use this procedure with their then-existing crate
graph, never wait for B.4a implementation or require nonexistent later crates.

## Artifact contract

No new trait, struct, enum or Python operation is introduced. The exact B.4
public signatures and shared binding contract remain the API. A wheel must
expose the same Result/Failure tags and stubs as a source build; packaging cannot
replace native functionality with a fallback or weaken error handling.
Distribution name remains proposed `sc-observability`; registry availability
and publication belong to B.7.

The lead-approved qualification refinement (ATM 01M2QF796PNV7EN3JN445PTD5D)
keeps one immutable production abi3 wheel per platform. Every cell runs the full
public runtime suite, including inherited B.5/B.6 tests, and asserts that private
native test hooks are absent. Fault injection alone uses a separate instrumented
companion built from the identical sdist with only the additional `test-hooks`
feature. Its explicit `fault_pytest_paths` files run in a separate installed
environment on each cell's interpreter. Both suites reject skips. Evidence
records separate hashes, features and roles; companion results cannot replace
production behavior, and companions never enter the publication inventory.
The runner supports interpreter-matched embedded-host execution in every cell
through `embedding_in_each_cell`; B.6 enables this together with asyncio debug
and warnings-as-errors for its full owned/attached qualification.

## Prepublication Rust source bundle

B.3/B.3b/B.4 artifacts are not on crates.io until B.7. Therefore B.4a cannot
require an unpublished version to resolve from that registry. Reuse B.3's sole helper
`scripts/ci/build_binding_source_bundle.py --root-manifest PATH --output DIR`
to build an isolated staging workspace
from the reviewed source and generate `rust-bundle/manifest.json`. Its inventory
contains every first-party package name/version/source revision, .crate SHA-256,
normalized manifest hash, archive member hashes and all locked target-dependent
transitive packages needed by the five platform families. Include
`sc-observability-dto`, `sc-observability-binding-runtime`, the Python Rust
embedding/extension crate and any unpublished first-party dependency they
actually resolve. Published core packages stay at their existing released versions; B.2's
staged companion candidates are referenced by exact staged version/checksum,
not a registry lookup.
A missing dependency is a packaging failure, never permission to publish early.

1. Materialize inherited workspace package/dependency/lint values in the staging
   workspace. Keep package versions and every dependency's version requirement
   equal to the reviewed publishable manifest; reject missing version entries.
   Package the mutually dependent first-party closure together with Cargo's
   multi-package `cargo package --no-verify` operation in that staged workspace.
   This produces candidate .crate files without pretending registry resolution
   is final verification. Record the exact selected package list and normalized
   manifests; the external build below is mandatory before acceptance.
2. Place candidate archives in `rust-bundle/crates/` and extract their normalized
   contents under `rust-bundle/unpublished/<name>-<version>/`. Place unmodified
   published dependencies under `rust-bundle/registry/` using `cargo vendor` and
   retain `.cargo-checksum.json`. Resolve all target-specific dependencies from
   the committed build lock, not only packages used by the packaging host.
   Record licenses and verify archive/member checksums. Reject symlinks/path
   traversal and absolute or escaping paths before creating the bundle.
3. The sdist root is its own Cargo workspace/package root with normalized
   extension Cargo.toml, Python sources, pyproject.toml, Cargo.lock and complete
   rust-bundle. Its root `[patch.crates-io]` maps each unpublished dependency to
   that dependency's bundled extracted directory. All normal `[dependencies]`
   entries retain publishable version requirements with no checkout path.
   The root patch is deliberate prepublication build configuration; nested
   dependency patches are not relied on. No patched manifest points to a sibling
   repository, `$HOME`, a workspace checkout, or an external absolute path.
4. Vendor only published dependencies through `.cargo/config.toml` source
   replacement. Cargo forbids treating unpublished packages as copies of
   nonexistent registry entries; unpublished packages use root patches instead.
   The selected version of each patched package must satisfy its ordinary version
   requirement and match manifest.json. [Cargo patching](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html)
   and [source replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html)
   distinguish these cases.
5. Generate the sdist's own Cargo.lock against this exact self-contained layout,
   verify every package resolution against the bundle manifest, then freeze it.
   Do not claim the root workspace Cargo.lock is interchangeable after source
   identities change. Maturin includes root Cargo metadata/config, both archive
   and extracted bundle trees, license files, generated Python sources and stubs
   explicitly in sdist contents; package-data tests reject accidental omission.

Illustrative staged configuration (V is the actual reviewed package version,
substituted by the script; additional unpublished dependencies get equivalent
entries):

```toml
# sdist-root/Cargo.toml — ordinary dependency remains publishable
[dependencies]
sc-observability-dto = "=V"
sc-observability-binding-runtime = "=V"

[patch.crates-io]
sc-observability-dto = { path = "rust-bundle/unpublished/sc-observability-dto-V" }
sc-observability-binding-runtime = { path = "rust-bundle/unpublished/sc-observability-binding-runtime-V" }
```

```toml
# sdist-root/.cargo/config.toml — published registry packages only
[source.crates-io]
replace-with = "bundled-published"
[source.bundled-published]
directory = "rust-bundle/registry"
[net]
offline = true
```

Cargo normalizes package manifests and removes root patch/workspace directives
from registry artifacts. We test the actual normalized manifest rather than
assuming an sdist's local override is a public dependency.
[Cargo package documentation](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
B.7 publishes the reviewed normal versioned packages in dependency order, then
adds registry-only consumer evidence; B.4a's bundle proof is not substituted for
that gate. Release sdist contents may retain vendored sources for offline builds,
but B.7 must separately prove an override-free build from published crates.

## External build procedure

The validator creates a fresh temporary directory outside every checkout,
unpacks only the produced sdist, and sets fresh task-specific CARGO_HOME and
CARGO_TARGET_DIR locations. It provisions the pinned Rust toolchain, maturin and
Python build tools before disabling network access; these tools are not claimed
as vendored Rust dependencies. From the unpacked root it runs
`cargo metadata --locked --offline` and `maturin build --locked --offline`, then
installs the wheel into a fresh venv and runs the full B.4 suite. Read access to
the source checkout and network is denied for the build/run proof. Every metadata
manifest_path must fall beneath the unpacked artifact (or the supplied Rust
sysroot for standard library tooling), and no unpublished dependency may resolve
from cache/registry. A second fresh external Rust host fixture depends on the
bundled embedding crate using the same explicit root-patch inventory and
published-dependency source replacement, with extension-only linker features
disabled. Neither external fixture borrows a parent checkout Cargo config.

The validator also compares versioned dependency requirements in the normalized
.crate files against the source release manifest, proves patch selection through
Cargo metadata, and deliberately removes one unpublished bundle member and one
published vendor entry in separate negative fixtures: both builds must fail,
not fall back to the network or local checkout. Tampered checksums, stale locks,
missing target-specific crates and escaping paths fail before artifact acceptance.

## Acceptance criteria (authoritative)

- AC1: All 25 interpreter/platform cells install and execute B.4's full owned
  API tests and stubs outside the source checkout, including failure injection,
  integer/diagnostic conversion and lifecycle corner cases. No skipped cell or
  import-only smoke test can close platform qualification.
- AC2: Wheel tags and linked-library requirements match abi3-py310 and the stated
  deployment targets; Linux wheels pass manylinux_2_28 compliance checks. Both
  extension and embedded-host executables link and run correctly on each target.
- AC3: An sdist rebuild in a clean environment produces an installable wheel
  with matching public API and required package data before any B.7 publication.
  Metadata proves every unpublished dependency resolves inside the bundled
  artifact, offline, with no source-checkout/cache escape. Missing sources, stubs,
  py.typed, unresolved local dependencies or incorrect architecture fail CI.
- AC4: Handoff evidence identifies every tested immutable artifact and matrix
  result. Reuse of one ABI wheel across interpreters does not skip testing.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_docs_consistency.sh
```

The extended validator builds with locked maturin/PyO3, inspects wheel contents
and platform tags, runs Linux compliance inspection, installs into clean venvs,
runs B.4 tests and type checks, then rebuilds the sdist outside the checkout.
The external rebuild follows the source-bundle procedure above and uses the
sdist-specific frozen Cargo.lock. No `--no-verify` packaging result alone counts
as verification. CI must aggregate all 25 cell results and fail on absent evidence. Test missing
package data, invalid platform tags and accidental extension-only host linker
flags through packaging fixtures; do not corrupt release artifacts for tests.

## Paths to delete

None.

## Non-closure

No PyPI/crates.io publication (B.7), new runtime API, Handler/context (B.5), async
receipts (B.6), free-threaded Python, PyPy, musl or Windows arm64 support. Those
exclusions do not reduce the required GIL CPython matrix.
