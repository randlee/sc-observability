# d-31: sc-otel cli

## Plan metadata

- Wave: 5.2 (wave-5 layer)
- Stack / layer: `phase-d-wave5` stack, layer 5 (d-29 → d-33 → d-34 → d-30 → d-31 → d-35 → d-32; wave-5 ruling R12)
- Assignee / model: cobs / terra
- Difficulty: `normal` (`docs/plans/phase-d/difficulty.csv`)
- Closure: `boundary` (consumer)
- Target boundary: `BOUNDARY-ScOtelCli`
- Branch: `sprint/d-31-sc-otel-cli`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-31-sc-otel-cli`
- PR target: `sprint/d-30-python-telemetry-bindings` (stack order only; no code dependency on d-30)
- Blocked by: `obs-d-29-sanity`
- Requirements: PHB-010, PHD-002, PHD-003, PHD-005, PHD-006, PHD-007, PHD-008, PHD-010, PHD-013
- ADRs: ADR-002, ADR-005, ADR-009, ADR-012, ADR-014, ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - `crates/sc-otel-cli/src/**`
  - `crates/sc-otel-cli/tests/**`

Ownership notes: `crates/sc-otel-cli/src/main.rs` is staged by d-29 and owned
by d-31 from wave 5.2. Not owned: `crates/sc-otel-cli/Cargo.toml` and
`boundaries/sc-otel-cli/**` (d-29).

## Relations

- `must_follow` d-29: consumes the `sc-otel-cli` skeleton and manifest
  (dependencies `clap` and `serde_json` included; none is added here), the
  `TelemetryClient` trait with `flush_submission` and the flush result rules,
  `SubmissionEnvelope::from_json`, `SystemIds`, the cross-crate
  constructors, `resolve_config`, the `load_telemetry_file` signature (for
  `--config`; d-33 implements its body, and d-31 tests only that `--config`
  reaches it), the CLI contract (subcommand and flag table, exit-code
  table), the `sc-otel.result/v1` schema, `InMemoryTelemetryClient`,
  `DoubleScript` and the golden fixtures.
- Sibling note (prose only; the bead relation is `must_follow` d-29): d-31,
  d-33, d-34 and d-30 can run in parallel, because their owned paths are
  disjoint. The CLI's edges follow the linear order (ADR-002): it depends on
  `sc-observability-types` and `sc-observability-otlp` only, and reads no ATM
  environment variable or path (ADR-009).

## Goal

Ship the `sc-otel` binary as a thin consumer of the d-29 contract.

## Deliverables

1. Implement the subcommands, global flags and per-subcommand flags exactly
   as the d-29 "CLI contract" table defines them, with `clap` derive. Flags
   become `ConfigOverrides` for `resolve_config`. [PHD-010]
2. Implement `emit` and `validate` input: `--stdin` (one `SubmissionInput`
   document) or the fragment flags, assembled per the d-29 table into one
   `SubmissionInput` and passed through the same
   `SubmissionEnvelope::from_json` as stdin. Default `emit` admits, then calls
   `flush_submission(receipt.submission_id, emit_flush_deadline)`;
   `--no-flush` only admits. [PHD-005, PHD-006, PHD-010]
3. Implement the exit-code mapping in `crates/sc-otel-cli/src/exit.rs`, with the
   exit-code numbers in `crates/sc-otel-cli/src/constants.rs` (ADR-005)
   (`fn exit_code(&TelemetryClientError) -> u8`, per the d-29 table; the types
   crate carries no exit policy) and print the `sc-otel.result/v1` object on
   stdout (`--output json`, the default) or a one-line summary
   (`--output text`). Credentials are redacted in every output and error
   path. [PHD-008, PHD-010]
4. Make the client backend injectable. Production opens
   `DurableTelemetryClient`; tests build the binary with the dev-only
   `test-double` feature (declared by d-29 as
   `test-double = ["sc-observability-types/test-double"]`), which opens
   `InMemoryTelemetryClient::with_script` with the `DoubleScript` JSON file
   named by `SC_OTEL_TEST_DOUBLE` (`DoubleScript::from_json`; no script file
   means the default script). That variable is read only in the
   `cfg(feature = "test-double")` build, never in release. [PHD-010]

## This Sprint Does Not Close

- Real collector delivery, offline recovery against the real store, shared
  store access with Python, and viewer readback. d-32 owns them.
- Python/CLI equivalence. d-32 owns it.

## Design

The contract types and signatures, the subcommand and flag table, the
exit-code table and the `sc-otel.result/v1` schema are in the d-29 doc and
are not restated here. This section covers only what d-31 writes.

### Module layout

| File | Contents |
| --- | --- |
| `src/main.rs` | `fn main() -> ExitCode`: runs `run()` inside `std::panic::catch_unwind`. A panic prints one line on stderr and exits 1. |
| `src/cli.rs` | The clap derive types below. |
| `src/input.rs` | Reads `--stdin` or assembles the fragments (`@file` read, `serde_json::Value` objects appended to `logs`/`spans`/`metrics`, `profiles` set, `version` = CURRENT, `record_key` from the flag) into one `SubmissionInput` JSON string. |
| `src/config.rs` | Global flags → `ConfigOverrides::default()` with `store_path` and `endpoint` set. `--config` → `load_telemetry_file`. Then `resolve_config(ConfigSources::new(&overrides, file, &env))` with `env = \|k\| std::env::var(k).ok()`. |
| `src/client.rs` | `open_client(config) -> Result<Box<dyn TelemetryClient>, TelemetryClientError>`. In release builds it opens `DurableTelemetryClient::open`. In `cfg(feature = "test-double")` builds it opens `InMemoryTelemetryClient::with_script` with the script read from `SC_OTEL_TEST_DOUBLE`, or `DoubleScript::default()` if the variable is unset. |
| `src/run.rs` | `run(cli) -> Outcome` per subcommand (table below). |
| `src/output.rs` | `ResultObject`, a serde struct with the schema fields and `schema = "sc-otel.result/v1"`. It writes JSON (`--output json`) or one text line, `<state> exit=<n>[ submission=<id>][ error=<code>]`. Only contract fields are printed, never the config, so `auth_header` (a `Secret`) cannot reach output. |
| `src/constants.rs` | `EXIT_OK` … `EXIT_DELIVERY_FAILED` (0–7) and the `sc-otel.result/v1` schema string (ADR-005: the crate's one constants module). |
| `src/exit.rs` | The exit-code mapping below. |

### clap command tree

```rust
#[derive(Parser)]
#[command(name = "sc-otel", version)]
struct Cli {
    #[arg(long, global = true)] config: Option<PathBuf>,
    #[arg(long, global = true)] store: Option<PathBuf>,
    #[arg(long, global = true)] endpoint: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)] output: OutputFormat,
    #[command(subcommand)] command: Command,
}
#[derive(Subcommand)]
enum Command { Emit(EmitArgs), Validate(InputArgs), Flush(FlushArgs), Status(StatusArgs) }

#[derive(Args)]
#[group(id = "source", required = true, multiple = true)]
struct InputArgs {
    #[arg(long, group = "source", conflicts_with_all = ["log", "span", "metric", "profile", "record_key"])]
    stdin: bool,
    #[arg(long, group = "source", value_name = "JSON|@FILE")] log: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")] span: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")] metric: Vec<String>,
    #[arg(long, group = "source", value_name = "JSON|@FILE")] profile: Option<String>, // a second --profile is a clap error
}
#[derive(Args)]
struct EmitArgs {
    #[command(flatten)] input: InputArgs,
    #[arg(long)] record_key: Option<String>,
    #[arg(long)] no_flush: bool,
}
#[derive(Args)]
struct FlushArgs { #[arg(long, value_name = "SECONDS", value_parser = parse_seconds)] timeout: Option<Duration> }
#[derive(Args)]
struct StatusArgs {
    #[arg(long, conflicts_with = "record_key")] submission: Vec<String>,
    #[arg(long)] record_key: Vec<String>,
}
```

Parsing uses `Cli::try_parse()`. Help and version print to stdout and exit 0.
Every other clap error prints clap's message to stderr and exits 2, with
nothing on stdout.

### Subcommand → TelemetryClient calls

| Subcommand | Calls | `state` / exit |
| --- | --- | --- |
| `validate` | `SubmissionEnvelope::from_json(&json, &mut SystemIds)`, then `to_canonical_json`. No config resolution and no client. | `validated`, 0; on `Submission` error, `rejected`, 3 |
| `emit` | `from_json` → resolve config → `open_client` → `emit`. Unless `--no-flush`: `flush_submission(&receipt.submission_id, config.emit_flush_deadline)`. Then `shutdown(Duration::ZERO)`. | `admitted_delivered` 0; `--no-flush`: `admitted_pending` 0, `flush: null`; deadline: `admitted_pending` 6; terminal: `admitted_failed` 7; error before admission: `rejected` 3/4/5 |
| `flush` | resolve → `open_client` → `flush(timeout.unwrap_or(config.flush_deadline))` → `shutdown(Duration::ZERO)` | `admitted_delivered` 0, `admitted_pending` 6, `admitted_failed` 7, `rejected` 4/5 |
| `status` | resolve → `open_client` → `status(query)`, where the query is `Submissions` (parsed with `SubmissionId::from_str`), `RecordKeys` (`RecordKey::from_str`) or `Summary` → `shutdown(Duration::ZERO)` | `status` 0, `rejected` 3/4/5 |

The final `shutdown(Duration::ZERO)` only stops the drain worker and releases
the lease (d-29: it does both even when it returns `Err`). Its `Delivery`
result is ignored, because rows left pending stay in the store for the next
drain. The exit code reflects only the subcommand's own call. The 7-over-6
precedence comes from the d-29 flush result rule inside the client, so the
CLI does no aggregation.

### Exit-code mapping (`src/exit.rs`)

```rust
// src/constants.rs
pub(crate) const EXIT_OK: u8 = 0;
pub(crate) const EXIT_INTERNAL: u8 = 1;
pub(crate) const EXIT_USAGE: u8 = 2;
pub(crate) const EXIT_INVALID_INPUT: u8 = 3;
pub(crate) const EXIT_CONFIG: u8 = 4;
pub(crate) const EXIT_ADMISSION: u8 = 5;
pub(crate) const EXIT_DELIVERY_PENDING: u8 = 6;
pub(crate) const EXIT_DELIVERY_FAILED: u8 = 7;

// src/exit.rs
pub(crate) fn exit_code(error: &TelemetryClientError) -> u8 {
    match error {
        TelemetryClientError::Submission(_) => EXIT_INVALID_INPUT,
        TelemetryClientError::Config(_) => EXIT_CONFIG,
        TelemetryClientError::Admission(_) => EXIT_ADMISSION,
        TelemetryClientError::Delivery(DeliveryError::DeadlineExceeded { .. }) => EXIT_DELIVERY_PENDING,
        TelemetryClientError::Delivery(DeliveryError::TerminalFailure { .. }) => EXIT_DELIVERY_FAILED,
        // #[non_exhaustive]: an unmapped future variant is an internal error.
        _ => EXIT_INTERNAL,
    }
}
```

Input read failures (`@file` not readable, stdin not UTF-8) are reported as
`SubmissionError::InvalidJson` through the same path (exit 3). `exit.rs` has
a unit test with one case per arm.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScOtelCli (D1, D2): `crates/sc-otel-cli/tests/installed_golden.rs`
  installs the binary with
  `cargo install --path crates/sc-otel-cli --root <tmp> --locked` and runs
  `<tmp>/bin/sc-otel validate --stdin` from a temp directory outside the
  workspace, once per d-29 golden fixture. Stdout `envelope` equals
  `expected.envelope.json`, or the exit code and `error.code` match
  `expected.error.json`. Fixtures that have a `flags.args` file produce the
  same envelope through `--log/--span/--metric/--profile`.
- [ ] boundary:BOUNDARY-ScOtelCli (D2, D4), **direct CLI emission**:
  `tests/direct_emission.rs` runs the test-double build of `sc-otel emit` once
  each with a log, a span, a metric and a profile flag, and once with combined
  stdin. Each prints `state: admitted_delivered` and exit 0. The double
  records exactly the envelope that `validate` printed. This is the
  assignment for "direct CLI emission".
- [ ] boundary:BOUNDARY-ScOtelCli (D3): `tests/exit_codes.rs` asserts every row
  of the exit-code table (0, 2, 3, 4, 5, 6, 7) using malformed stdin, a bad
  flag, a missing `store_path`, and `DoubleScript` files for an admission
  rejection (`{"admissions":[{"outcome":"reject","kind":"disk_bound_exceeded"}]}`
  → 5), a stall (→ 6), a fail (→ 7) and a fail on one signal with a stall on
  another (→ 7, the d-29 precedence). Exit 2 leaves stdout empty; every other
  stdout parses against `sc-otel.result/v1`. An `auth_header` value never appears in stdout or
  stderr.
- [ ] boundary:BOUNDARY-ScOtelCli (D1, D2): `tests/flags.rs` covers every
  flag in the d-29 CLI table, including `--stdin` with fragment flags
  rejected (exit 2), `--record-key`, repeated `--submission`, `@file`
  fragments and a second `--profile` rejected. `--config <missing path>`
  exits 4 with `SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE`, which holds for both
  the d-29 staged loader and the d-33 implementation.
- [ ] boundary:BOUNDARY-ScOtelCli: `cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'`
  lists exactly `sc-otel-cli`, `sc-observability-types`,
  `sc-observability-otlp`, `clap` and `serde_json`, and
  `cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'`
  contains no `sc-observe`, `pyo3` or `agent-team-mail`. No `todo!` or
  `unimplemented!` remains.
- [ ] boundary:BOUNDARY-ScOtelCli (no caller runtime): `rg -n 'tokio' crates/sc-otel-cli`
  finds nothing: the binary never creates a Tokio runtime. d-32 proves the
  installed binary delivers to a real collector.
- [ ] boundary:ADR-020 (public surface frozen; wave-5 ruling R17): at the
  d-31 head, `python3 scripts/ci/validate_public_api.py diff --crate sc-observability-types --crate sc-observability-otlp`
  reports the `api_sha256` values recorded in the signed d-29 approval
  record. `sc-otel-cli` is `publish = false`, so it is outside the public-API
  policy, and it has only the `sc-otel` bin target.
- [ ] boundary:BOUNDARY-ScOtelCli: `git diff <d-29 head> -- Cargo.toml Cargo.lock crates/sc-otel-cli/Cargo.toml`
  is empty.

## Required validation

```sh
cargo fmt --check --all
cargo clippy -p sc-otel-cli --all-targets --all-features -- -D warnings
cargo test -p sc-otel-cli --all-features --locked
bash scripts/ci/validate_repo_boundaries.sh
cargo tree -p sc-otel-cli -e normal --depth 1 --prefix none --format '{p}'
cargo tree -p sc-otel-cli -e normal,build --all-features --prefix none --format '{p}'
python3 scripts/ci/validate_public_api.py diff --crate sc-observability-types --crate sc-observability-otlp
```
