# B.5 implementation handoff — qualification in progress

Branch: `feature/phase-b-5-python-integration`.
Direct parent: `fix/phase-b-3a-completeness`; first pushed parent `1d613bc`
merged in `b15fff9`. This is a progress record, not a completeness claim.
Final parent, installed distribution matrix, retained gate index, and lead
completeness review remain required.

## Implemented surface

- `bindings/python/sc-observability-py/python/sc_observability/logging.py`:
  explicitly created borrowed handler for owned/attached backends; complete
  Failure-kind accounting, immutable snapshots and bounded latest status;
  level/target/selected-extra mapping; single getMessage invocation, recursive
  and foreign formatter containment; bounded flush and borrowed close.
- `context.py`: validated inactive factory, immutable ContextVar stack, explicit
  Result-returning enter/close, task/thread identity and LIFO ownership, 64-scope
  bound, non-None field override, and no context-manager protocol.
- Both facade log methods apply `_inherit_event` before DTO conversion. A private
  native `_validate_event` hook uses canonical DTO/core conversion without
  backend admission. It does not construct a logger or emit validation events.
- `context.pyi` and `logging.pyi`: frozen outcome records, complete declared
  methods, opaque factory-only typed construction, Python 3.10-compatible
  `NoReturn`.
- Central `SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID` registry addition and
  regenerated schema/Python/TypeScript projections, authorized by lead. The
  final exported Rust API digest still requires scoped review.
- `examples/standard_logging.py` and `standard_logging.md`: explicit opt-in,
  owned lifecycle, observable cleanup Results and supported context behavior.
- Existing `examples/rust-python-logging` now runs a Rust/Python shared request
  through the real registered host. A unique correlation ID makes the fixture
  repeatable. It proves shared query, provenance, redaction, borrowed-handler
  close, and retained closed failure after host stop.

## Completed incremental checks

At `204a1e8`, the inherited Python suite passes 51 tests; 47 are B.5-specific.
The B.5 cases cover all level boundaries, all 13 custom Failure variants,
filtered/closed owned backends, saturating immutable counters and accounting
fault retention, concurrent drop accounting, exact recursion behavior,
exception/stack redaction, hostile foreign factory inputs, scope states and
exact diagnostics, async sibling isolation and child-token rejection, explicit
thread transfer, wrong-thread cleanup, copied-context reset failure, stack
limit, foreign activation/restoration failures, and import/shutdown opt-in.

`cargo check --locked -p sc-observability-py` and generation artifact hash
validation pass after the first parent merge. Strict mypy passes the new source
modules/example; both new stubs separately pass with Python version 3.10.

A local ABI3 macOS ARM64 wheel built at `6ab670e` passed the then-current 44 B.5
cases and the typed example from a fresh `/tmp` environment outside the
checkout. Installed module paths and all four new module/stub files were
verified. This sanity check is not the final immutable-sdist sandbox or matrix
evidence. The actual embedded host passed twice consecutively at `299c08c`.

## Findings incorporated

`B5-C01`: a hostile tuple subclass could raise during extra-field iteration
before factory containment. `299c08c` moves every foreign inspection inside the
boundary, snapshots names, and retains tuple/name/backend/trace regressions.
Lead independently accepted that correction. A follow-up review changed both
new stub constructor annotations from `Never` to Python 3.10 `NoReturn`.

## Remaining qualification

Merge the corrected direct parent carrying final B.4/B.4a/native/schema heads.
Extend its shared Python validator and qualification-suite typing paths, then
run the full immutable sdist/wheel and real embedded host suite on all 25 B.4a
interpreter/platform cells. Retain source/artifact identities and raw command
logs; update this document and the two-pass checklist from actual evidence.
Sprint closure, QA, PR merge and package publication have not been claimed.

## Corrected-parent qualification increment

Parent `0f579b65` merged in `79654a0`; integration source `87400df`, generation
provenance `df9953e`. Added root facade stub and `qualification-suite.json`,
and extended the shared validator to the complete installed suite. The shared
CPython 3.10 gate passes 60 tests, strict typing, the standard logging example
and real mixed host. The isolated macOS ARM64 source-distribution build passes
native tests, embedding and all nine negative cases; its CPython 3.12 installed
cell passes all 60 tests and strict typing with checkout/cache/network denied.
Raw local records are in `target/b5-macos-build-df9953e/build-result.json`,
`target/b5-macos-cell-df9953e/cell-result.json`, and `target/b5-validation/`.
These remain incremental local evidence pending final retained matrix artifacts.

API digest `3879489bf925437f1704bf54f7160b0d1526d73b6c0cfced076abb655d2da1fc`
adds exactly `SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID`; lead approval is
recorded in `docs/api-approvals/phase-b-python-context.json` under ATM
`01M2QF5M1GW741SY3CGCJ16N6J`. `2f11f75` corrects the inherited registry-count
assertion to 18 and explicitly requires that constant; all 17 DTO conversion
tests pass. Schema compiler, both generators and generation hashes are clean.

Platform run `35211438801` uses `df9953e` and remains incomplete; Linux wheel
jobs failed and require packaging-owner resolution. The next corrected parent
will also carry per-interpreter embedding qualification. Full matrix and lead
completeness remain outstanding; the tracked task stays open while execution
is yielded to the Tauri specialist by explicit lead request.
