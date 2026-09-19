# npm and package-only release compatibility evidence

PR: https://github.com/randlee/sc-publish/pull/95 (draft, base `develop`).
Baseline: `a4206dcf75edd047f57abebdf6e4ed4d096f2543`.
The exact tested candidate commit and GitHub CI run are recorded in the PR body;
this file is part of that candidate and does not claim a merge or qualification.

## Existing working consumers

The comparison uses read-only inputs and installs each kit into independent
scratch directories. Both initial installs and repeat `--dry-run` checks pass.
Parsed rendered artifact manifests and every pre-existing channel contract are
identical. Outputs are identical for `python-wheel-matrix`,
`python-sdist-matrix`, `list-publish-plan`, `preflight-secret-plan`, and
`release-asset-patterns`. The existing installed manifests' wheel matrices are
also compared, including sc-compose's ARM runner addition.

| Consumer | Inspected checkout HEAD | Input SHA256 |
| --- | --- | --- |
| sc-compose | `b763d2ffdaf6941bd8b375dba4e77676e357169f` | `97ec11ab3c8a9cc5ccff4ed69a97727969e80f4a3febc85eb36d53dc53f8ba2f` |
| atm-core | `849405ef76cf44937153215ea6bfe8a0ddc1f739` | `26ca2237a0e46ed14cc6dcd01d60ca1dbc45aad09be01141cfc53726735ac51b` |

Reproduce using the pinned renderer Python and isolated checkouts or archives:

```bash
python .integration/verify_consumer_compatibility.py \
  --baseline-kit /path/to/a4206dc/plugins/sc-publish \
  --candidate-kit /path/to/candidate/plugins/sc-publish \
  --consumer-input /path/to/sc-compose/release/sc-publish-install.json \
  --consumer-input /path/to/atm-core/release/sc-publish-consumer-input.json
```

The inputs' hashes are checked by the review evidence rather than assumed from
checkout HEAD (consumer files may have local edits). The script prints both
identity and comparison receipts. It performs no network or registry operations.

Existing wheels already worked: sc-compose and atm-core use runner strings,
native maturin builds, and the existing Python upload channel. sc-compose's
installed workflow additionally uses `maturin[zig]==1.9.4` and
`--compatibility manylinux2014 --zig` for `ubuntu-24.04-arm`. This change reuses
that mechanism. Explicit target objects add the missing per-architecture Rust
target, manylinux level, macOS deployment target, unique artifact ID, and wheel
platform assertions. Legacy runner-string matrix JSON remains identical.

## Validation

- Source suite: `python -m pytest plugins/sc-publish/.github/scripts/tests -q`:
  166 passed, 10 skipped on Python 3.12 with sc-compose 1.5.0 and PyYAML 6.0.3.
- Installed suite: 162 passed, 14 skipped after a clean installer overlay
  and repeat drift check. Both source and installed suites ran with ambient
  `RELEASE_TAG` removed; mocked publication fixtures set their own tag. The
  installed suite is now also a GitHub CI gate. Source tests cover
  all new npm/wheel modules instead of only historical unittest modules.
- Tests execute actual crate publish shell orchestration with a mocked Cargo
  executable and registry lookup. Both root and retry legs pass the standalone
  manifest path; dependency-aware preflight plans include standalone crates.
- npm tests cover nonpublishing preflight, matching-version skip, conflicting
  integrity rejection, accepted-upload recovery, failed-upload retry, registry
  errors, archive identities/checksums, all-artifact validation before writing,
  credential scoping, and immutable-release download gating.
- Wheel tests cover all five declared platforms, duplicate/missing/wrong
  platforms, legacy runner compatibility, and retained build failure gates.
- A temporary copy of the real TypeScript client built and packed version 1.4.0
  successfully, with `dist/index.js` and `dist/index.d.ts`; tarball SHA256 was
  `1b0c764507b7da0d8ad9dc3a750a2ac1c028c310d0b5270b202b1af7199b3c62`.
  The package was never published.

The baseline has a reproducible macOS Bash 3 failure for empty
`already_published_channels`: the preservation helper expands an unset array.
Run the baseline pytest selector
`-k test_release_preflight_registry_checks_execute_preserved_channel_exception`
to reproduce (1 failed, 2 passed). An empty-input guard now returns false before
array expansion, matching the existing supported Linux/Bash behavior. The full
candidate suite passes on macOS without excluding or waiving that test.

## Action runtime evidence and limits

Official action metadata/release notes identify the Node24 versions used:
[checkout v5](https://github.com/actions/checkout/blob/v5/action.yml),
[setup-python v6](https://github.com/actions/setup-python/blob/v6/action.yml),
[setup-node v6](https://github.com/actions/setup-node/blob/v6/action.yml),
[cache v5](https://github.com/actions/cache),
[upload-artifact v6](https://github.com/actions/upload-artifact/releases/tag/v6.0.0),
[download-artifact v7](https://github.com/actions/download-artifact/blob/v7/action.yml),
and [action-gh-release v3](https://github.com/softprops/action-gh-release/blob/master/CHANGELOG.md).
Existing composite action pins are retained.

No release tag, registry write, real publish dispatch, credential modification,
or native five-platform build was performed. The tests establish configuration,
planning, build command, archive verification, and retry behavior; they do not
replace live release qualification or the consumer's 25-cell wheel import gate.
The npm workflow requires GitHub immutable releases to be enabled and a valid
`NPM_TOKEN` in the `npm` environment; this work does not provision either.

## Consumer test portability correction

An independent sc-observability install exposed two test assumptions: scanning
caller-owned workflows applied the kit runtime floor to unrelated actions, and
Python README checks assumed `bindings/python/pyproject.toml`. The runtime test
now selects only shared workflows/composites; README checks follow declared
Python and Cargo manifests and validate optional file references where present.
A nested-package regression covers absent README metadata, a declared local
README, and a missing declared file.

An isolated copy of sc-observability at
`d75b54e10b7af8e7899aa3891c9c901ac6949140`, overlaid through the installer, passed
its full installed suite: **157 passed, 11 skipped**, with a clean repeat dry-run.
Source suite: **158 passed, 10 skipped**. Both retained peer compatibility
comparisons pass unchanged. This correction changes shared tests/documentation,
not publication behavior or consumer-owned workflows.

## Pre-tag npm source gate

Review identified that npm source versions were checked only during the later
build job. `verify-version-lockstep` now validates every declared npm source
name/version, rejects private packages, and enforces the same public registry
and publishConfig policy used for archived packages. The build reuses this
validator. Before the tag step, a separate failing gate reads the declared npm
manifest and each package.json from the exact resolved release commit, so a
valid dispatch checkout cannot conceal stale metadata in the tree being tagged.

Regression tests reject stale versions, mismatched names, private packages,
restricted access, and registry overrides. They assert exact-commit reads and
workflow ordering before tag creation and downstream build/publication jobs.
Full source: **166 passed, 10 skipped**; generic installed: **162 passed,
14 skipped**; isolated actual consumer: **165 passed, 11 skipped**. The existing
sc-compose and atm-core compatibility comparison remains unchanged. No
credential convention, registry write, tag, or live dispatch is part of this fix.
