# sc-observability v1.4.0 + v1.4.1 — Typed Error APIs, First-Class Language Bindings, and Publication-Train Recovery

**Released:** September 20, 2026 · **Install:** add `sc-observability = "1.4"` to `Cargo.toml` (nine crates on [crates.io](https://crates.io/search?q=sc-observability)); Python bindings via `pip install sc-observability` from [PyPI](https://pypi.org/project/sc-observability/)

[Changelog](https://github.com/randlee/sc-observability/blob/main/CHANGELOG.md) · [Release notes](https://github.com/randlee/sc-observability/releases/tag/v1.4.0)

---

## Rust Developer

**As a Rust developer, I want reusable observability crates with standard consistent patterns, so that I can instrument my SC component without bespoke logging glue.**

v1.4.0 lands an additive typed error API across the whole surface. Logging, observation, and telemetry operations now return concrete, discriminated failure types instead of loose boxed errors — `Logger::log`/`try_log`, sink and projector registration, and OTLP export all carry typed results you can match exhaustively. The change is strictly additive: existing `emit()` callers keep a warning-only compatibility path, and the queue-admission/durability distinction from v1.2.0 is unchanged. `docs/migration-guide.md` walks through each typed replacement.

The Rust surface grows from four core crates to six, adding `sc-observability-log` (the macros-free logging core) and `sc-observability-log-macros` (the first public macros baseline, pinned at `=1.4.0`). Behind them sit the binding-surface crates `sc-observability-dto`, `sc-observability-binding-runtime`, and `sc-observability-tauri`, so the logging core and every language binding share one contract instead of per-crate glue. All nine crates publish to crates.io at 1.4.0, and every archive ships MIT license bytes.

---

## Python Developer

**As a Python developer, I want Python bindings for the same observability crates, so that Python-side SC tooling instruments consistently with the Rust side.**

The Python bindings ship. The PyO3 extension publishes to PyPI as `sc-observability` 1.4.0 — `pip install sc-observability` — with a wheel/sdist matrix and Python ≥3.10 support. You get both binding modes: owned (the Python process drives its own logger runtime) and attached (Python instruments an existing Rust host). A logging handler and scoped request context tie Python-side records into the same typed contract the Rust side uses.

Admission and durability carry over faithfully: `log()`/`try_log()` return resolved receipts, and a bounded async flush lets Python coroutines await durability without blocking the event loop. The distribution matrix is qualified across platforms, with private fault companions isolated from production wheels so failure-injection never leaks into the installed artifact.

---

## Observability Developer

**As an observability developer, I want shared contracts (identifiers, diagnostics, health reports) stable across components, so that telemetry from different SC products composes cleanly.**

`sc-observability-dto` introduces neutral wire types with checked core conversions — the single interchange contract that the Rust, Python, and TypeScript/Tauri surfaces all serialize through, so telemetry from different SC products composes cleanly instead of drifting per language. The typed-failure work (B.1a–B.1e) is the contract side of the same change: a checked error inventory with registry-parity tests keeps diagnostic shapes identical across `sc-observability`, `sc-observe`, and `sc-observability-otlp`.

A neutral binding schema drives deterministic language projections, and CI now enforces immutable import provenance — every adaptation of the upstream bridge is validated against approved source hashes, so the shared contracts can't be mutated in place. Phase C hardens the release surface itself: workspace metadata and dependency versions inherit from a single source, `sc-lint` preflight runs from an immutable pinned installation, and built wheels are verified byte-for-byte against their build provenance.

---

## Skipped / noted

- **TypeScript client (`@sc-observability/client`)** — generated and qualified, but its npm publication is deferred pending Phase C `sc-publish` migration and owner credential authorization; the `sc-observability-tauri` adapter crate is published to crates.io. The SSOT (`user-stories.md`) has no dedicated TypeScript/Tauri persona, so no section is included this release.

---

## v1.4.1 — bug-fix note (shipped September 20, 2026)

**v1.4.0 shipped with publication defects — prefer 1.4.1.**

v1.4.1, released about a day after v1.4.0, carries the same qualified 1.4.0 public API surface into a corrected, coordinated publication train. The 1.4.0 artifacts went out with broken wheel-build recovery and API baselines recorded against stale metadata; 1.4.1 regenerates release manifests and lockfiles, aligns the Rust, Python, and TypeScript package metadata and dependency pins at 1.4.1, re-records API baselines against the published 1.4.0 surface, and adopts shared wheel-release recovery.

There are no runtime or API changes — the additive typed-error and binding contracts are identical to 1.4.0. If you already installed 1.4.0, upgrading is a metadata-only bump. ([v1.4.1 release notes](https://github.com/randlee/sc-observability/releases/tag/v1.4.1))

---

## What's Next

The TypeScript client's npm publication and the remaining Phase C `sc-publish` migration for the binding channels are the next surface. `Logger::emit()` compatibility remains and is still planned for removal in a future release.
