# H-4 D4: Protected Logging and Downstream Handoff Record

This record preserves the existing H-4 D4 evidence for review. It is not a
new validation gate, certificate, or release authorization. The original D4
implementation report is `01M4CP8M6RCFXW6FR7DBAHHA44`; its test results belong
to the executed commit `0331eddd4a500d31c3da49f111472a11f6a14d8f`.

## Protected logging and composition evidence

The original H-4 completion report records these checks at the execution head
`0331eddd4a500d31c3da49f111472a11f6a14d8f`:

- `cargo test -p sc-observability-log-consumer-check -p sc-observability-log -p sc-observability` — 253 passed, 0 failed. This is the protected logging API check.
- `scripts/integrate/suites/collector/run.py` — the collector suite ran end to end: native 5, frontends 13, and harness-timeouts 3 passed; 0 skipped. The report describes the pinned official `otelcol-contrib` 0.162.0 OTLP HTTP/protobuf receiver and file-export readback, native sync/Tokio signal export, file-only/OTel-only/both logger composition, and installed CLI/Python frontend coverage.
- Rust stock API history from 1.5.0 to 1.6.0 removed 0 lines from `sc-observability`, `sc-observability-log`, `sc-observability-log-macros`, `sc-observability-binding-runtime`, and `sc-observability-tauri`; `sc-observe` added 11 lines. This is API-history evidence, not a compile result.

The original completion report and its test receipts are historical reports,
not new executions. The report's executed commit is not an ancestor of the
offered review commit. `git diff --shortstat
0331eddd4a500d31c3da49f111472a11f6a14d8f
0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4` shows one file changed and one
insertion: `crates/sc-otel-cli/src/constants.rs` adds a comment describing
exit code 7. The 253-test and collector-suite results must not be attributed to
the offered review commit.

## Downstream cargo-check offer and response

At 2026-10-08 08:19:57Z, the same offer was sent to `team-lead@sc-compose`
(`01M4D9J00DTEXP114TVRK99WRA`) and `team-lead@atm-dev`
(`01M4D9J0ASZBMF35MZSVG8YBBA`). The offer identifies the unpublished 1.6.0
candidate as commit
`0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4` on
`sprint/h-4-remove-and-qualify`, PR #1234. It says no release is implied.

The offered check recipe is to set the workspace `sc-observability*`
dependencies to version `1.6.0`, add this patch for each crate used, in a
scratch branch, then run `cargo check --workspace --all-targets`:

```toml
[patch.crates-io]
sc-observability = { git = "https://github.com/randlee/sc-observability", rev = "0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4" }
```

The `team-lead@atm-dev` response (`01M4D9JCMF01NTETPD56WP3A32`,
2026-10-08 08:20:09Z) declined for now, explicitly “not a no.” Its stated
reasons were committed Phase BD build slots and disk/load constraints, and
that ATM's dependency plan was still being decided. The response reports only
a text search of `atm-core` for 11 removed constants; it explicitly says that
this was not a compile. The response was received by team-lead and forwarded
to aobs as receipt `01M4D9JHMX00H0WWT85KAP4QRV`; the team-lead update is
`01M4D9JQTMFVXKJYTKGQY3KTNM` (08:20:21Z). The forwarded source-status
snapshot says no `sc-compose` reply had been found as of approximately
11:1xZ and that the `wyvern` recipient was not delivered to. Neither absence
of a reply from `sc-compose` nor the deferred ATM response establishes a
downstream check.

The same status snapshot records that delivery to `cwy@wyvern` was rejected
because it was neither a known local team nor an enabled trusted peer. That
recipient was not part of the offer's delivered-recipient evidence.

The earlier proposal `01M4DK4N0DWYVZ2YHGYXHQS4YK` is a locator, not evidence
that there were no responses. Its blanket `responses:none` statement is
disproved by the recorded ATM response and team-lead receipt; the correction
is `01M4DK56SHE43XJ1G4AK23FHEY`.

Accordingly, no downstream `cargo check --workspace --all-targets` result or
downstream PASS is recorded. The offered recipe and pin above are preserved
without representing the offer as a completed check.

## Independent diff accounting at the offered review commit

The original completion report's inventory at `0331eddd` says 299
changed files and `+27122/-52761` lines versus `14538ea2` (net
`-25639`). The following independent recount is for the exact offer range,
`14538ea2405fa9cf7c9d6744173be2e119a6e92f` through
`0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4`; it uses `--no-renames`, so
renames are counted as old-path deletions and new-path additions. It finds 300
path records and `+27123/-52761` (net `-25638`), one additional insertion
than the original report, as accounted for above.

| Path class | Records | Added | Deleted | Net |
| --- | ---: | ---: | ---: | ---: |
| Source | 120 | 671 | 32,186 | -31,515 |
| Test | 108 | 1,405 | 14,784 | -13,379 |
| Other | 72 | 25,047 | 5,791 | +19,256 |
| Total | 300 | 27,123 | 52,761 | -25,638 |

The deterministic path classification used for this recount is:

1. `other` takes precedence for any path containing a `generated` component,
   or under `bindings/schema/`, `bindings/conformance/`, or `schema/api/`.
2. Otherwise, `test` includes paths beneath a `tests`, `test`, or `__tests__`
   directory, and basenames matching `test_*.py`, `*_test.py`, `*.test.*`,
   or `*.spec.*`.
3. Otherwise, `source` includes `.rs`, `.py`, `.pyi`, `.ts`, `.tsx`, `.js`,
   `.jsx`, `.mjs`, `.sh`, and `.sql` files. Every remaining path is `other`.

Reproduce the table from the repository root with:

```python
import subprocess

base = "14538ea2405fa9cf7c9d6744173be2e119a6e92f"
head = "0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4"
rows = subprocess.check_output(
    ["git", "diff", "--no-renames", "--numstat", base, head], text=True
)
extensions = (".rs", ".py", ".pyi", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".sh", ".sql")
totals = {name: [0, 0, 0] for name in ("source", "test", "other")}
for row in rows.splitlines():
    added, deleted, path = row.split("\t", 2)
    added = 0 if added == "-" else int(added)
    deleted = 0 if deleted == "-" else int(deleted)
    parts = path.split("/")
    name = parts[-1]
    generated = "generated" in parts or path.startswith(
        ("bindings/schema/", "bindings/conformance/", "schema/api/")
    )
    test = (
        any(part in {"tests", "test", "__tests__"} for part in parts[:-1])
        or name.startswith("test_")
        or name.endswith("_test.py")
        or ".test." in name
        or ".spec." in name
    )
    category = "other" if generated else (
        "test" if test else "source" if name.endswith(extensions) else "other"
    )
    totals[category][0] += 1
    totals[category][1] += added
    totals[category][2] += deleted
for category, (files, added, deleted) in totals.items():
    print(category, files, added, deleted, added - deleted)
```

This is a path-based line recount, not a semantic test inventory. Inline tests
inside production `src` files remain in `source`; the classifier does not
parse Rust `#[cfg(test)]` blocks. The source and test buckets together show
44,894 net lines removed under this classification. `other` includes
generated API, schema and conformance artifacts, documentation, manifests,
and configuration; its positive line count is not a source-code growth count.

## Provenance

- Original H-4 developer completion and validation: `01M4CP8M6RCFXW6FR7DBAHHA44`.
- Exact offer, ATM response, receipt, and source-status transcription: team-lead
  source parts `01M4DKG38MH786P3686GP3YF4G` and
  `01M4DKGE36671ZZ6BTQK0MVXEP`; forwarded completion body
  `01M4DKFYMFJFB7YZFPVFDH59P3`.
- D4 finding: `obs-h-4.group-dev.1`, from `obs-h-4.group-sanity`, against
  `docs/test-strategy.md:129` at `0c0cbca0f3d7a37ea4b5ae0611c4917bcd8f2ce4`.
- This record does not claim final-phase verification, a current test run, a
  downstream compile, or a release.
