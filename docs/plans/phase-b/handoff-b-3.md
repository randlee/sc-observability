# B.3 neutral DTO/schema handoff

Implementation and verification are complete on `feature/phase-b-3-schema` above
frozen B.2 head `1e855d93244de6561f24e39464a584195845e3f9`. This handoff requests
lead completeness acceptance; consolidated Phase B QA and ordered merges remain
separate. No crate, npm package or Python package was published.

## Delivered inventory

1. `sc-observability-dto` implements every binding-contract wire declaration,
   including the incorporated ClientStatus/ClientOutcome, checked native
   event/query/stored-event/health/level conversions, 17 binding diagnostic codes,
   bounded remote diagnostics, and additive remote-output handling. Default
   production dependencies are types/serde/serde_json. Failure owns a boxed
   diagnostic, preserving its exact wire fields without oversized Result errors.
2. The isolated, locked Schemars1.2.2 compiler emits distinct input/output
   definitions, concrete result/envelope entrypoints, embedded registry and
   conversion metadata. Canonical JSON alone drives both language generators.
   Python data/stubs share one traversal; TypeScript aliases/runtime validators
   share one traversal. Generated generic Result/WireEnvelope projections derive
   from concrete schema definitions. `bindings/API-COVERAGE.md` inventories every
   type and operation, with runtime-owned exclusions explicit.
3. The source-bundle helper emits real Cargo archives, extracts verified roots,
   reuses exact qualified B.2 archives, vendors published dependencies and freezes
   the staged workspace layout. It preserves the reviewed source-lock registry
   closure and proves every normalized dependency requirement against the source.
   The single isolation validator is reusable through `--consumer-source` and
   `--expected-marker`; B.3b/B.4a need no copy of this machinery.
4. Schema CI, dependency/core-boundary checks, rustdoc coverage, crate-specific API
   policy, an immutable generation hash record, and all positive/negative gates
   are committed. Both generator check modes and the Rust schema check leave
   mismatching files untouched and fail instead of rewriting their baseline.

## Scoped approval and ownership

Lead aobs approved the initial DTO API and scoped TYP-030 wire-only exception on
2026-09-17. The verbatim approval is
`docs/api-approvals/phase-b-dto.json`; the retained 423-line export digest is
`c03027422ae3ff9f49e5b425523c9c5eab1909e34bfac5dc39ac54295f89cc9a`. A fresh export at final verification matched that digest exactly.
Native ErrorContext/source/backtrace, logger/guard ownership, bridge runtime
conversion, native operation coordination and trusted provenance stamping stay
outside the wire crate. Core serialization was not changed.

## Tooling and deterministic artifacts

- Generation: Rust1.94.1, Python3.12.10, Schemars1.2.2, committed compiler lock.
- Schema SHA256: `6336e4542cdbf7c642505a18b2eb7dcda9c7aceb41f2ca4183382d63fdd2035c`.
- `bindings/generation-manifest.json` freezes a generator source revision,
  every generator source/toolchain/compiler-lock hash and every generated
  output hash. The gate compares source bytes to that recorded Git revision.
  This document does not pin that revision's SHA: the manifest is actively
  regenerated as fixes land (for example PHB-CI-007's generation-manifest
  refresh), so a value written here would go stale before qualification
  finishes. `bindings/generation-manifest.json`'s own `source_revision` field
  is the authoritative current value; as of this correction (still ahead of
  terminal qualification) it reads `cbcdaf8a90f3915a337beba0ca5a2916d5a1d682`,
  verified directly from that file rather than reused from an earlier report.
  The root workspace lock is excluded from generation inputs because the schema
  compiler is isolated; unrelated adapter dependencies cannot alter its selection.
  The source-bundle gate independently retains and verifies the entire root lock.
- Two fresh schema compiler runs and two fresh runs of each language generator
  are byte-identical to the committed artifacts. Deliberate drift and unsupported
  schema keywords fail without updating the expected output.
- Minimum Python support is independent of the generation pin: actual CPython
  3.10.21 runtime import/default construction and strict mypy2.3.1 targeting3.10
  pass. The NoReturn/dataclass_field generator correction resolves B3-C03.

## Exact packaged-source and sandbox proof

The tested source is `8776885a6e6e99dc9e892dae90c8f44eb2edae0a`. Commands were:

```sh
python3.12 scripts/ci/build_binding_source_bundle.py --root-manifest crates/sc-observability-dto/Cargo.toml --output /tmp/b3-retained-8776885
python3.12 scripts/ci/validate_binding_bundle.py --bundle /tmp/b3-retained-8776885 --evidence target/b3-retained-consumer.json
```

The actual complete bundle remains at `/tmp/b3-retained-8776885` in this session.
`evidence/b3-final/` retains its manifest, both locks, exact two consumed archives,
consumer proof and gate logs; it is an evidence subset, not a second bundle.
The manifest lists every extracted/vendor/config file hash. Qualified types come
from B.2 candidate `136799758a5e91aabbf064ae50b5c7c6f20d4413`, with no reconstruction
of its archive bytes. Registry identities for all 25 selected packages match
name/version/source/checksum between the reviewed source lock and staged lock.
A separate real target-specific fixture preserves serde_json1.0.149 and a
Windows-only libc0.2.189 dependency while running on macOS.

| Artifact | SHA256 |
| --- | --- |
| DTO .crate | `efcffb9c8e70abeb330331d7c3865a8ee19e7523650f8e5669bc83ed482826a3` |
| Qualified types .crate | `3a076bf879bc3c95578226ba973e8d73f2c545dcdec9f68c92651aa542a25b20` |
| Staged Cargo.lock | `d947700f3fc9ccb6252c02a8178d51d8579fbdd06ba187da9bc731b88b8478ed` |
| Reviewed source Cargo.lock | `3d39c58624038f74b5f19b9d2067007fc6ee4405b58dc1cfdec97f9756ed516a` |

The retained `isolated-consumer.json` contains exact commands, sandbox profile,
denied checkout roots, new CARGO_HOME/CARGO_TARGET_DIR, dependency provenance and
consumer stdout. Only the artifact was copied into a fresh external directory.
Before execution, independent probes proved checkout reads, existing Cargo-cache
reads and network access fail. Then actual `cargo metadata --locked --offline`
and `cargo run --locked --offline` resolved every manifest inside the artifact and
executed public event/query/snapshot/health/diagnostic/decimal conversions:

```text
BINDING_CONSUMER_OK: event query snapshot health diagnostic decimal
BUNDLE_ISOLATED_CONSUMER_PASSED: positive and six exact-code negatives
```

Missing unpublished member -> BUNDLE_MISSING_MEMBER; stale frozen lock ->
BUNDLE_STALE_LOCK; escaping manifest/archive -> BUNDLE_ESCAPING_PATH. Changed
registry selection and changed normalized dependency requirements are separately
rejected as BUNDLE_REGISTRY_DRIFT and BUNDLE_REQUIREMENT_DRIFT even after the
negative fixture updates its generic file-integrity hashes. Direct and inherited
path-only dependencies fail before staging with BUNDLE_MISSING_VERSION.

B3-C01 and B3-C02 were implemented and verified separately in the worktree
checklist. The tests exercise the real helper/output and the real offline
consumer, with no skipped stage or mocked package path.

The older sprint sentence requiring only already-published core dependencies
conflicts with required runtime-level types supplied by staged types1.4.0 and
the user-directed B.7 publication delay. Lead aobs confirmed that AC5's exact
staged DTO + staged types source bundle is the applicable prepublication proof.
This handoff makes no registry-only claim; B.7 must perform that later proof.

## Verification pass

`evidence/b3-final/gates/results.json` records all nine commands and exit0:

- Required schema validator, DTO tests, dependency bans, repository boundaries,
  and documentation consistency validators.
- Workspace/compiler formatting and DTO/compiler clippy with warnings denied.

The schema validator runs 17 focused Rust conversion/boundary tests, 291 frozen
schema/Serde/TypeScript/Python cases, 13 semantic exact-kind/code negative cases,
six actual source-boundary/target-lock tests, Python3.10 typing/runtime checks,
deterministic generation/hash checks and the full isolated bundle gate. It ends
with `binding schema validation passed`. Public API docs validation additionally
passed with all affected crates explicitly approved. The repository-boundary
example emits its pre-existing legacy deprecation warnings; DTO/compiler clippy
and all crate missing-docs gates pass without suppressing new warnings.

Live cyclic Python/JavaScript objects and adapter-native provenance/admission
races are exercised by the language/runtime sprints; JSON wire inputs and owned
Rust JSON trees cannot contain object-reference cycles. No runtime acceptance is
inferred from this schema-only proof. No deletion target was specified.
