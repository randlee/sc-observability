# d-1: Shared startup `LogSettings` (#96)

## Plan metadata

- Wave: 4
- Branch: `sprint/d-1-log-settings`
- PR target: `sprint/d-10-windows-arm64-wheel`
- Blocked by: `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability/src/runtime.rs`
  - `crates/sc-observability/tests/log_settings.rs`
  - `docs/logging/d-1-log-settings.md`

## Goal and dependency

Create the single serde-stable, binding-friendly configuration value in
`sc-observability` that applications resolve before constructing a `Logger`.
It is parallel-safe with D.2 and D.3 because neither consumes this type. This
is additive 1.x work checked against the published 1.4.1 API/semver baseline.


## Deliverables

1. Implement deterministic environment parsing for the complete inventory and
   field-wise resolution in the documented order including the LOG-009 root
   exception. Parsing uses a named `EnvSnapshot` so one resolution cannot mix
   process states.

2. Convert the resolved value to `LoggerConfig` and its strong policy types,
   preserving all non-inventory defaults and introducing no post-construction
   mutation.

3. Document the table, precedence, null/unset behavior, prefix rules, failure
   codes, and startup-only lifecycle. Add a public example embedding settings
   under an application's `logging` JSON key. Document the compact stable-error
   table: `PrefixCollision`/`LOG-001`, `InvalidEnvironment`/`LOG-002`,
   `UnknownKey`/`LOG-003`, `InvalidValue`/`LOG-004`, and
   `Resolution`/`LOG-005`.


## Non-closure

Do not migrate consumer applications, add fields outside the inventory,
implement dynamic reload, or plan #88 bindings/OTEL work.


## Design



## Implementation targets

 implement or update the named contract consumer and its focused test for the corresponding numbered deliverable.\n

## Acceptance criteria

## Acceptance criteria

- Empty/`null` JSON resolves exactly like `LoggerConfig::default_for` for the
  supplied service and default root.
- Every row has fixtures for default, JSON, shared environment, application
  environment, and full precedence; a supplied `retainedLogPolicy` replaces
  the complete validated policy rather than merging nested fields.
- Every invalid boundary, present-empty environment value, unknown JSON key,
  and unknown selected-prefix environment key returns its documented typed
  failure and code.
- Collision, trailing-underscore misuse, case collision, selected-namespace
  non-UTF-8, and LOG-009 root-precedence fixtures freeze prefix behavior.
- Serde uses exactly the camelCase keys and delegates level tokens to the
  existing `LevelFilter` conversion/fixtures; level spelling is defined by
  that single owner.
- Configuration is fully resolved before logger construction; no setter,
  watcher, or late reload is introduced.


## Required validation

- Table-driven unit tests generated from the authoritative inventory for JSON,
  environment, precedence, defaults, validation, and conversion parity.
- Public consumer compile fixture plus `cargo test --workspace --locked`.
- Docs consistency, rustdoc, public API, and semver gates used by the repository
  at execution time.


