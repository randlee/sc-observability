# Generation metadata and package history corrections

Parent PR189 remains frozen at `e1ea5fc3befe0297cba771e95b9a234db284a0ce`.
The implementation checkpoint qualified below is
`7f25a4889eb74800899f5ded162b932d82f45837`; later evidence-only commits do not
change manifests, Rust, generated outputs, validators or workflow behavior.

## Observed failures and bounded fixes

- Package-stage [35423045371](https://github.com/randlee/sc-observability/actions/runs/35423045371/job/105844082432)
  used checkout depth 1 and could not resolve the pinned metadata commit's
  parent. PR189 introduced this strict history requirement. The B.2 package-stage
  checkout now uses full history. No validator fallback or relaxation is added.
  The import-integrity job already fetches full history; the only other CI
  consumer calls `validate_log_staged_consumer.py --stage`, which consumes the
  prepared archives without invoking source staging. No other checkout changed.
- Schema-and-contract [35423045366](https://github.com/randlee/sc-observability/actions/runs/35423045366/job/105844082440)
  and binding-schema [35423045363](https://github.com/randlee/sc-observability/actions/runs/35423045363/job/105844082413)
  rejected stale generation evidence after the inherited `aec16d6` metadata edit.
  Of 15 inputs, only DTO Cargo.toml changed; deleting its three newly inherited
  authors/homepage/repository lines reproduces the former source bytes exactly.
  Its digest is updated and source_revision advances from `def5ab205c100021b499324cf4dba0ef2d02a08f`
  to the frozen parent `e1ea5fc3befe0297cba771e95b9a234db284a0ce`.
  All other input digests, all six output digests and all toolchain pins stay
  unchanged. The historical generation record remains available in Git.
- ATM-QA-001, scoped to PR186: the historical parity evidence now explicitly says
  **64 installer package outputs plus 2 rendered manifests (66 total)**.
  Its original source pin, counts and other claims remain unchanged.

## Two-pass verification

1. Reproduced the two causes from actual hosted logs. Checked all source-staging
   callers and compared every generation input/output. Full schema validation
   and four regression tests passed before commit.
2. On clean committed `7f25a4889eb74800899f5ded162b932d82f45837`, rebuilt and
   verified all six actual Cargo packages, reran the complete schema script,
   four regressions, and documentation/rustdoc consistency. All passed.

The full schema script includes 18 DTO tests, 2 Rust conformance tests, 291
frozen cases, two clean schema/language regenerations, drift/unsupported-keyword
negatives, Python 3.10 runtime/stubs, 8 source-bundle tests, immutable artifact
verification and a real isolated bundle consumer with six exact-code negatives.
The four new tests prove valid evidence passes; forged input bytes plus a
matching replacement hash still fail the immutable source check; generated
output drift fails; and an actual shallow clone fails closed before full
history is fetched and then passes the unchanged release proof.

Exact commands:

```sh
python3 -m unittest discover -s scripts/ci/tests -p 'test_generation_provenance.py' -v
bash scripts/ci/validate_binding_schema.sh
bash scripts/ci/validate_docs_consistency.sh
python3 scripts/ci/prepare_log_staged_packages.py --version 1.4.0 --output /tmp/phase-c-generation-six-package-stage
```

Generation evidence update was derived with these checks (not by skipping drift):

```python
old = json.loads(Path("bindings/generation-manifest.json").read_text())
new_revision = "e1ea5fc3befe0297cba771e95b9a234db284a0ce"
for path, expected in old["inputs"].items():
    before = subprocess.check_output(["git", "show", old["source_revision"] + ":" + path])
    after = subprocess.check_output(["git", "show", new_revision + ":" + path])
    assert Path(path).read_bytes() == after
    if before != after:
        assert path == "crates/sc-observability-dto/Cargo.toml"
        restored = after
        for field in ("repository", "homepage", "authors"):
            restored = restored.replace(f"{field}.workspace = true\n".encode(), b"", 1)
        assert restored == before
        old["inputs"][path] = hashlib.sha256(after).hexdigest()
for path, expected in old["outputs"].items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == expected
old["source_revision"] = new_revision
Path("bindings/generation-manifest.json").write_text(json.dumps(old, indent=2) + "\n")
```

Raw logs, immutable package digest manifest and bundle proof are retained under
`generation-metadata/`. The six archives remain at the command's `/tmp` output
path. These are local proofs; hosted CI and independent QA remain separate.
No publication, tag, parent edits or historical Phase B record changes occurred.
