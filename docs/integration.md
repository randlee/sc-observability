# On-demand integration

Run from an authenticated checkout with Python 3 and GitHub CLI (`gh`):

```sh
just integrate integrate/phase-e
just integrate sprint/e-1-workflow --suite wheels,collector --os windows,linux
just integrate integrate/phase-e --suite wheel-cli-viewer --os macos
```

The branch must already be pushed and contain `integration.yml` and the common
runner. The dispatcher validates comma-separated selectors before contacting
GitHub, rejects empty, unknown and duplicate entries, and prints the selected
coverage and run URL. Defaults select all six suites and `macos,windows,linux`:
18 independent suite/OS jobs. A partial selection is labelled explicitly and
cannot establish complete phase-end coverage. Matrix selection can be checked
locally without `gh`, native dependencies or completed suite implementations:

```sh
python3 scripts/integrate/dispatch.py --matrix --suite wheels,collector --os linux
python3 -m unittest scripts.integrate.tests.test_dispatch
```

| Selector | Integration boundary |
| --- | --- |
| `wheel-cli-viewer` | Installed Python wheel and CLI, pinned viewer readback, offline recovery, restart and partial delivery |
| `rust-viewer` | Public SDK and sync-http factory viewer assertions |
| `collector` | Hermetic OTLP backend conformance and canonical ingress |
| `wheels` | Candidate wheel installation, owned/attached runtime and embedding |
| `tauri` | Packaged npm/Rust artifacts through real frontend/host IPC |
| `rust-consumers` | Packaged core, binding bridge, runtime-level and log-bridge consumers outside the workspace |

## Run results and source

The workflow is `workflow_dispatch` only. The command uses
`gh workflow run integration.yml --ref <branch>`. GitHub resolves that ref when
creating the run; every job checks out its immutable `github.sha`, and the
common runner verifies HEAD equals that SHA. Suite builds use that checkout.
Branch changes after dispatch do not move the run's source.

An explicitly compatible E3/E4 Rust preparation may avoid recompiling the same
OTLP test target. It is valid only for the same immutable SHA, OS/architecture,
Rust `1.94.1`, `Cargo.lock`, target/profile/`RUSTFLAGS`,
`otlp-sdk,sync-http` feature projection, named test binary, and restored target
path. This shares compilation only: both suite cells retain their own selected
tests and assertions, and E4 compiles its additional `canonical_ingress` binary
when required. A mismatch or absent compatible target is a normal local compile,
not a fallback to another suite's artifact.

No other sibling artifact exchange is implied. E2's wheel/CLI and E6's
Tauri/npm artifacts have distinct build contracts. E5's isolated sdist wheel
and embedding checks, and E7's isolated external-consumer checks, must build in
their own constrained roots. The workflow remains `workflow_dispatch` only:
there is no automatic per-sprint preparation and no generic cache framework.

The selection job validates workflow-form inputs and emits the matrix; it is
not an aggregate status job. Matrix fail-fast is disabled. The six suite owners
preserve runnable assertions after individual assertion failures; setup/build
failure exits nonzero. Console output is captured in `runner.log`; existing
suite logs/reports under the output directory are uploaded even after failure.
GitHub job conclusions are the result authority. A failed, cancelled, skipped
or absent selected job is never successful coverage. A green selection job,
an accepted dispatch or an artifact upload is not integration success.

Open the printed URL, or use native GitHub tools (the run ID is in the URL):

```sh
gh run view <run-id>
gh run watch <run-id> --exit-status
gh run view <run-id> --json headSha,conclusion,jobs
gh run view <run-id> --log-failed
gh run download <run-id> --dir integration-logs
```

Verify every selected suite/OS job has a successful conclusion. Full phase-end
coverage requires all 18 successful suite jobs at the same SHA. There is no
custom result schema, collector or aggregate job. The command correlates its
run URL using a unique dispatch ID; if GitHub has not listed the accepted run
within a minute, it reports the ID and the native lookup command, not success.

## Suite entrypoint contract

The workflow invokes only:

```sh
python scripts/integrate/run_suite.py --suite <selector> --source-sha <40hex> --output-dir <absolute-path>
```

The common runner validates the SHA, HEAD, absolute output path and entrypoint
presence, then invokes the following from the checkout root with the same
Python interpreter:

```sh
python scripts/integrate/suites/<selector>/run.py --source-sha <40hex> --output-dir <absolute-path>
```

Only the common runner checks HEAD and runner presence. Missing implementations
fail explicitly; they never pass or silently skip. Each suite builds its own
candidate artifacts unless the narrow E3/E4 compatible-preparation conditions
above are met, runs existing assertions, writes existing logs/reports below the
output directory and returns nonzero on failure. Preserve runnable assertions,
use bounded subprocess operations and clean owned processes, files and sockets
in `finally`, including setup/assertion failure. Native viewer/webview execution
belongs on CI, not a developer desktop. No suite may consume an unqualified
sibling artifact.

## Existing viewer interface (read-only contract)

Both viewer suites reuse
`scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py` and its adjacent
`release.json`. Select the host artifact from that pinned manifest, verify its
artifact checksum and extracted binary checksum, and use its version and
binary name. The current manifest declares v0.5.0 for macOS arm64 only; e-2
owns extending platform acquisition and the manifest. This common workflow
does not download or qualify the viewer, and unavailable platform assets must
fail affected viewer cells explicitly. Viewer readiness is no prerequisite for
common dispatch or the four unrelated suites.

The existing harness CLI is:

```text
python viewer_harness.py start --binary <path> --binary-sha256 <digest> --version <version> --state-dir <dir>
python viewer_harness.py status --state-dir <dir>
python viewer_harness.py assert-production --state-dir <dir> --backend sdk|sync-http
python viewer_harness.py stop --state-dir <dir> --timeout 10
```

Use an isolated state directory per job/process and distinct available loopback
ports. `start` accepts `--host 127.0.0.1 --http <port> --grpc <port> --ui <port>`;
default ports are not concurrency isolation. Stop owned viewers in `finally`.
The Rust viewer suite can develop against this interface while e-2 implements
platform support. Viewer readback must obey PHD-012 and ADR-021; collector
capture does not stand in for stored viewer data or unsupported capabilities.

## Default-branch registration

GitHub must first know this workflow on the actual default branch, `main`.
The lead opens a normal reviewed bootstrap PR against `main` containing only
an exact copy of `.github/workflows/integration.yml` from the approved e-1
commit. Merge through normal review; do not push directly or change the default
branch. This is registration of the same workflow, not a separately maintained
implementation. Its jobs execute from the selected implementation branch,
which contains the dispatcher and runner. Dispatching on `main` before those
scripts land there is not supported.

After bootstrap merge, dispatch with `--ref` as above. Until then local
selector/runner contract tests are valid evidence, but native workflow runs
are not claimed. e-1's common contract does not require unfinished suite native
runs: selection works immediately, and missing suite runners fail explicitly.
The existing Phase D workflows and publisher qualification remain unchanged.
