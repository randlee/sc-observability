---
id: B.4-python-handoff
status: complete
branch: feature/phase-b-4-python
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-4-python
parent: feature/phase-b-3a-typescript
---

# Python runtime handoff

The B.4 layer provides the locked PyO3 `abi3-py310` mixed `cdylib`/`rlib`
project, `Ok`/`Err` public facade, generated frozen DTO data, independently
owned loggers, and non-owning Rust-host attachments. Owned lifecycle and level
authority stay with `CoreLoggerOwner`; attached handles retain only a shared
`HostLoggingBackend` Arc and cannot stop or mutate the host.

Tested runtime implementation evidence is
`47d48b17759c60eb18936196b53d14930e02c086`.
It carries frozen B.3, B.3b and telemetry/copy ancestors while retaining the
direct TypeScript parent. The facade contains no authored validation `raise`:
malformed Python values, hostile mappings/accessors, foreign native exceptions,
malformed native payloads and contained native panics return tagged failures.

## Executed gates

`bash scripts/ci/validate_python_bindings.sh` passes with CPython 3.10.21:
the generated/stub checks and strict Result narrowing fixture pass; ten
facade adversarial tests pass; a locked feature-gated source-validation wheel
is built and installed into a clean environment for eleven owned and attached
lifecycle/isolation/synchronized-N=32,
level-transition, zero-deadline retained-shutdown and lifecycle-race tests;
native binding tests pass; and
`cargo run -p rust-python-logging` proves a single writer accepts correlated
Rust and Python records, redacts both bearer values, supports querying and
retains attached health after host shutdown.

Native attachment tests prove missing-host tagging, immutable duplicate install,
attached admission, retained health after host shutdown, eight concurrent
install callers with one winner and seven exact duplicate-install failures, and
32 simultaneous attached PyO3 producer calls with 32 tagged admissions. The
host install check-and-install transition is serialized without storing a global
backend, so free-threaded interpreter contenders cannot replace a module slot.
An owned-handle subprocess verifies GC/interpreter teardown exits without a
hang, while the module-collection fixture proves a live attached handle retains
only the backend: it remains usable after module collection, does not transfer
host ownership, and retains health after the host closes.

The install mutex uses PyO3 0.29.2's `MutexExt::lock_py_attached`, which detaches
before a contended Rust-mutex wait. The module state transition reads and writes
the concrete module dictionary directly rather than calling `hasattr`, so a
user-defined module `__getattr__` cannot run while the once-only lock is held.
A five-second subprocess regression races two installers with such a
GIL-releasing hook and retains the exact one-winner/one-duplicate outcome.
The immutable-slot fixture rejects both a repeat using the exact same backend
`Arc` and a separate independently owned backend, leaving the first slot intact.
The attached fixture mutates the host-owned level through its owner and proves
the attached Python health payload observes `debug` at revision one without
gaining any mutation authority.

Facade provenance fixtures reject both exact trusted names, arbitrary reserved
suffixes, normalized `::` and space aliases, and nested/mixed forged maps before
owned or attached dispatch. An installed-wheel fixture replaces the active JSONL
file with a directory before construction, then verifies a real lazy sink write
fault is retained in tagged health rather than escaping from `log` or `flush`.
That source-validation wheel alone enables private native test hooks; they force
a tagged native internal failure before both factories and every owned or
attached public operation, proving the real PyO3 boundary returns `Err` while
ordinary package builds expose neither hook nor test host fixture.
The ordinary locked wheel is also installed independently without that feature:
its seven production-runtime tests pass and the four source-hook fixtures skip,
which proves the companion coverage does not become a package API requirement.

The retained B.3b coordinator operation matrix was also run one named process
case at a time: timer/bootstrap rollback, worker rollback, waiters/callbacks,
query/flush slot ordering and timeout, held sink/shutdown, N=32 admission and
close, helper failure, bridge timeout/churn, teardown, and native-diagnostic
fidelity all pass. Python uses these shared operations rather than introducing
another worker or conversion path.

## Active completeness checklist — pass 1: implementation paths

| Requirement | Retained evidence | State |
| --- | --- | --- |
| Owned and attached lifecycle, N=32 admission, host once-only, surviving handles | Installed wheel tests plus six native PyO3 tests | covered |
| Python input/result containment and protected provenance | Ten facade tests and strict Failure narrowing fixture | covered |
| Host level authority and attached revision observation | Installed owned-level test and native attached-owner fixture | covered |
| Embedding correlation/redaction/query/health/stop | `cargo run -p rust-python-logging` | covered |
| GC/interpreter teardown | Installed owned-handle subprocess and native module-collection fixture | covered |
| Shared helper rollback, held sink, slot/timeout and late-result mechanics | all named B.3b coordinator cases | covered in supplied backend |
| Python-bound blocked-sink heartbeat and every detailed operation interleaving | feature-only builder registers a real held sink after helper reservation; installed Python wheel proves health/query progress and tagged flush timeout before release | covered |
| Native fault injection through every Python public Result method | feature-gated source wheel forces tagged faults for both factories and all owned/attached methods | covered |
| Diagnostic/remediation conversion through Python | exact unavailable/IO messages, codes, recoverable steps and terminal justification | covered |
| Revision-overflow through Python | feature-only real `LevelOwner` terminal revision produces `SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED`; health retains `u64::MAX` and `info` | covered |

## Active completeness checklist — pass 2: validation paths

| Gate | Result | State |
| --- | --- | --- |
| `validate_python_bindings.sh` (CPython 3.10.21) | green: 10 facade, 11 installed runtime, 6 native binding tests, complete `contract_matrix`, embedding example | covered |
| `cargo clippy --locked -p sc-observability-py --all-targets -- -D warnings` | green; included in the Python validator | covered |
| `validate_dependency_bans.sh` | green | covered |
| `validate_docs_consistency.sh` | green | covered |
| Linux x86_64 source CI | [run 35212196675](https://github.com/randlee/sc-observability/actions/runs/35212196675) passed at `f5975f9` | covered |
| B.4a wheel/sdist matrix | owned by B.4a and not a B.4 completion substitute | external qualification |

The approved default-feature public API digest is
`d4d01640af4a8a7a980fd86df7b4a844ad65c1fa66f5c364a4ce930f91bd12fd` for
the six re-exports, `install_host_logger`, and the `PyO3` initializer. Private
test seams are excluded. B.4a retains distribution qualification ownership.
