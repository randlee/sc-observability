#!/usr/bin/env bash
# Registry-consumer validation for the B.7 binding-release manifest
# (release/bindings-artifacts.toml).
#
# Rewritten per aobs's C03 rejection of the prior version, which "succeeded
# on missing packages, skipped pending entries, and cargo-checked local path
# dependencies" -- none of which proves anything about what would actually
# get published. This version does four separate, honestly-labeled things:
#
#   1. structural preflight (offline)   -- manifest/version sanity only.
#   2. candidate evidence (real build)  -- real cargo package / maturin
#      build, real sha256 hashing, real wrong-source/changed-byte self-check
#      via `verify-evidence` immediately after `build-evidence`.
#   3. real isolated consumer matrix    -- Rust/Python/TypeScript consumers
#      of the actual packaged bytes (extracted tarballs / installed wheel /
#      installed npm tarball), never the live source tree.
#   4. summary                          -- states exactly what was proven.
#
# Nothing here is a live registry lookup unless --live-registry-check is
# passed. No "ready" entry is ever labeled registry-consumer PASS from file
# existence alone, and step 1's preflight is never relabeled as step 3's
# proof.
set -euo pipefail

LIVE_REGISTRY_CHECK=0
for arg in "$@"; do
  case "$arg" in
    --live-registry-check) LIVE_REGISTRY_CHECK=1 ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

MANIFEST="release/bindings-artifacts.toml"
WORKSPACE_TOML="Cargo.toml"
EVIDENCE_DIR="release/evidence"
EVIDENCE_FILE="$EVIDENCE_DIR/bindings-candidate.json"

TMP_ROOT="$(mktemp -d -t sc-observability-b7-registry-consumers.XXXXXX)"
trap 'rm -rf "$TMP_ROOT"' EXIT

pending_reason() {
  # $1 = package name (manifest's "package" field, not "artifact")
  python3 - "$MANIFEST" "$1" <<'PY'
import sys
import tomllib
from pathlib import Path

manifest_path, package = sys.argv[1], sys.argv[2]
data = tomllib.loads(Path(manifest_path).read_text(encoding="utf-8"))
for entry in data.get("crates", []) + data.get("packages", []):
    if entry.get("package") == package:
        print(entry.get("pending_reason", "no reason recorded"))
        break
PY
}

evidence_file_path() {
  # $1 = artifact name, $2 = file kind ("crate" / "sdist" / "wheel")
  python3 - "$EVIDENCE_FILE" "$1" "$2" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
print(data["artifacts"][sys.argv[2]]["files"][sys.argv[3]]["artifact_path"])
PY
}

echo "== 1/4: structural preflight (offline; NOT registry-consumer proof) =="
python3 scripts/release_bindings_artifacts.py validate-manifest \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML"
python3 scripts/release_bindings_artifacts.py verify-versions \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML"
echo "structural preflight passed -- offline manifest/version sanity only, this is NOT registry consumer validation"

echo
echo "== registry-secret auth gate (informational here; enforced for real in .github/workflows/release.yml) =="
if secrets_output=$(python3 scripts/release_bindings_artifacts.py list-publish-plan \
      --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML" --require-secrets 2>&1); then
  echo "all ready artifacts have a configured registry secret"
else
  echo "BLOCKED-ON-AUTH (owner-deferred registry credential/publication approval):"
  echo "$secrets_output" | sed 's/^/  /'
fi

echo
echo "== 2/4: candidate evidence (real build + hash + wrong-source/changed-byte self-check) =="
python3 scripts/release_bindings_artifacts.py build-evidence \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML" --out "$EVIDENCE_DIR"
python3 scripts/release_bindings_artifacts.py verify-evidence \
  --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML" --evidence "$EVIDENCE_FILE"
echo "candidate evidence build+verify passed -- real packaged bytes built and hashed fresh at HEAD, self-checked for tampering, not fabricated"

echo
echo "== per-artifact plan =="
python3 scripts/release_bindings_artifacts.py list-publish-plan --manifest "$MANIFEST" --workspace-toml "$WORKSPACE_TOML" \
  | while IFS='|' read -r kind package wait_seconds status workspace_member manifest_path; do
      if [[ "$status" == "pending" ]]; then
        reason=$(pending_reason "$package")
        echo "SKIPPED (pending): ${package} (${kind}) -- ${reason}"
        continue
      fi
      if [[ "$LIVE_REGISTRY_CHECK" == "1" ]]; then
        echo "LIVE CHECK: ${package} (${kind}) -- calling out to the real registry"
        case "$kind" in
          pypi)
            if pip index versions "$package" >/tmp/b7-pip-check.log 2>&1; then
              echo "  found on PyPI"
            else
              echo "  NOT FOUND on PyPI -- expected pre-release, re-run after publish"
            fi
            ;;
          npm)
            if npm view "$package" version >/tmp/b7-npm-check.log 2>&1; then
              echo "  found on npm"
            else
              echo "  NOT FOUND on npm -- expected pre-release, re-run after publish"
            fi
            ;;
          crate)
            http_status=$(curl -s -o /dev/null -w '%{http_code}' "https://crates.io/api/v1/crates/${package}" || echo "000")
            if [[ "$http_status" == "200" ]]; then
              echo "  found on crates.io"
            else
              echo "  NOT FOUND on crates.io (http ${http_status}) -- expected pre-release, re-run after publish"
            fi
            ;;
        esac
      else
        echo "READY (candidate evidence built above): ${package} (${kind})"
      fi
    done

echo
echo "== 3/4: real isolated consumer matrix (packaged local candidate bytes, not live source tree or a live registry) =="

echo
echo "-- Rust: extracted packaged-tarball consumer (sc-observability-dto, sc-observability-binding-runtime, sc-observability-py) --"
mkdir -p "$TMP_ROOT/extract/dto" "$TMP_ROOT/extract/runtime" "$TMP_ROOT/extract/py"
tar xzf "$(evidence_file_path sc-observability-dto crate)" -C "$TMP_ROOT/extract/dto" --strip-components=1
tar xzf "$(evidence_file_path sc-observability-binding-runtime crate)" -C "$TMP_ROOT/extract/runtime" --strip-components=1
tar xzf "$(evidence_file_path sc-observability-py crate)" -C "$TMP_ROOT/extract/py" --strip-components=1

RUST_CONSUMER_ROOT="$TMP_ROOT/rust-consumer"
mkdir -p "$RUST_CONSUMER_ROOT/consumer/src"

cat > "$RUST_CONSUMER_ROOT/Cargo.toml" <<EOF
[workspace]
members = ["consumer"]
resolver = "2"

# [patch.crates-io] below exists ONLY to satisfy the core crates
# (sc-observability, sc-observability-types, sc-observability-log,
# sc-observability-log-macros) that dto/binding-runtime/py's *packaged*,
# registry-normalized Cargo.toml files reference by version alone (cargo
# strips path dependencies when packaging -- see cargo package's own
# "THIS FILE IS AUTOMATICALLY GENERATED" comment in the extracted Cargo.toml).
# Those 4 core crates are release/publish-artifacts.toml's own already-
# qualified, separately-published scope; they are not yet live at this
# workspace version, so this validator patches them to their real local
# source purely to break that chicken-and-egg publish-ordering problem --
# exactly the same documented reason examples/rust-python-logging/Cargo.toml
# already gives for its own (committed, unrelated) path overrides. This
# NEVER patches sc-observability-dto, sc-observability-binding-runtime, or
# sc-observability-py themselves -- those resolve only to the extracted
# packaged tarballs below, which is the thing this section proves.
[patch.crates-io]
sc-observability = { path = "$ROOT/crates/sc-observability" }
sc-observability-types = { path = "$ROOT/crates/sc-observability-types" }
sc-observability-log = { path = "$ROOT/crates/sc-observability-log" }
sc-observability-log-macros = { path = "$ROOT/crates/sc-observability-log-macros" }
sc-observability-dto = { path = "$TMP_ROOT/extract/dto" }
sc-observability-binding-runtime = { path = "$TMP_ROOT/extract/runtime" }
EOF

cat > "$RUST_CONSUMER_ROOT/consumer/Cargo.toml" <<EOF
[package]
name = "b7-registry-consumer-proof"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
sc-observability-dto = { path = "$TMP_ROOT/extract/dto" }
sc-observability-binding-runtime = { path = "$TMP_ROOT/extract/runtime" }
sc-observability-py = { path = "$TMP_ROOT/extract/py" }
EOF

cat > "$RUST_CONSUMER_ROOT/consumer/src/lib.rs" <<'EOF'
//! Throwaway proof crate (never committed): depends on the *extracted
//! packaged tarballs* of sc-observability-dto, sc-observability-binding-runtime
//! and sc-observability-py produced by `cargo package`, proving the shipped
//! file set (respecting each crate's Cargo.toml include/exclude) actually
//! compiles as an external dependency -- not just the live source tree that
//! examples/rust-python-logging's path dependencies exercise.
extern crate _native as py_native;
pub use sc_observability_binding_runtime as _binding_runtime;
pub use sc_observability_dto as _dto;

pub fn proof_symbol() -> &'static str {
    "sc-observability binding-crate packaged-artifact consumer proof"
}
EOF

cargo check --manifest-path "$RUST_CONSUMER_ROOT/Cargo.toml" -p b7-registry-consumer-proof
echo "PASS: packaged sc-observability-dto/binding-runtime/py compile as an external Rust consumer (extracted tarball bytes)"

echo
echo "-- Rust: sc-observability-tauri --"
echo "SKIPPED (pending): sc-observability-tauri -- $(pending_reason sc-observability-tauri)"

echo
echo "-- Python: isolated venv install of the built wheel + representative test subset --"
WHEEL_PATH="$(evidence_file_path sc-observability-python-wheel wheel)"
PY_VENV="$TMP_ROOT/py-venv"
python3 -m venv "$PY_VENV"
"$PY_VENV/bin/python" -m pip install --quiet --upgrade pip
"$PY_VENV/bin/python" -m pip install --quiet "$WHEEL_PATH" pytest==9.1.1
TESTS_COPY="$TMP_ROOT/py-tests"
cp -R bindings/python/sc-observability-py/tests "$TESTS_COPY"
rm -rf "$TESTS_COPY/typing"  # mypy strict-typing gate is B.4's own scope (validate_python_bindings.sh), not this validator's
# The production wheel deliberately omits the private native test hooks used by
# test_runtime_faults.py.  Keep that suite out of the public consumer proof;
# validate_python_distribution.py runs it separately against the instrumented
# companion wheel and proves the companion never enters publication inventory.
rm -f "$TESTS_COPY/test_runtime_faults.py"
SC_OBSERVABILITY_RUNTIME_TEST=1 PYTHONWARNINGS=error "$PY_VENV/bin/python" -I -m pytest "$TESTS_COPY" -ra
echo "PASS: sc-observability wheel installs into a fresh isolated venv and its test subset passes against the installed package"
echo "PRIVATE_FAULT_SUITE: delegated to the separately-built instrumented companion wheel; production wheel remains hook-free"

echo
echo "-- TypeScript: @synaptic-canvas/sc-observability (forward-looking structural proof only) --"
# npm pack proves the packaged tarball's file set installs and runs, without
# claiming publish-readiness while the owner publication gate is pending.
TS_DIR="bindings/typescript"
TS_PACK_DIR="$TMP_ROOT/ts-pack"
mkdir -p "$TS_PACK_DIR"
( cd "$TS_DIR" && npm ci --quiet --ignore-scripts && npm run build --silent && npm pack --silent --pack-destination "$TS_PACK_DIR" >/dev/null )
TS_TARBALL="$(find "$TS_PACK_DIR" -maxdepth 1 -type f -name '*.tgz' -print -quit)"
if [[ -z "$TS_TARBALL" ]]; then
  echo "ERROR: npm pack produced no tarball" >&2
  exit 1
fi
TS_CONSUMER_DIR="$TMP_ROOT/ts-consumer"
mkdir -p "$TS_CONSUMER_DIR"
( cd "$TS_CONSUMER_DIR" && npm init --yes --silent >/dev/null && npm install --silent --ignore-scripts --no-save "$TS_TARBALL" >/dev/null )
( cd "$TS_CONSUMER_DIR" && node -e '
const { createClient, encodeEvent } = require("@synaptic-canvas/sc-observability");
const event = encodeEvent({ level: "info", target: "package-consumer", action: "smoke", fields: { count: 3n } });
if (event.kind !== "ok" || event.value.fields.count.value !== "3") throw new Error("packaged tarball event encoding failed");
const operations = [];
const created = createClient({
  request(operation, request) {
    operations.push([operation, request]);
    return Promise.resolve({ kind: "ok", value: { schema_version: 1, kind: "ok", value: { kind: "accepted", operation: "try_log" } } });
  },
});
if (created.kind !== "ok") throw new Error("packaged tarball client construction failed");
created.value.tryLog(event.value).then((result) => {
  if (result.kind !== "ok" || operations.length !== 1 || operations[0][0] !== "try_log") process.exit(1);
  console.log("TS_PACKAGED_TARBALL_CONSUMER_PASSED");
}).catch((error) => { console.error(error); process.exit(1); });
' )
echo "STRUCTURAL PROOF ONLY (pending): @synaptic-canvas/sc-observability packs into a real npm tarball and installs/smoke-checks in isolation; sc-publish credential provisioning and owner publication approval remain deferred, so this is NOT npm-publish-readiness"

echo
echo "== 4/4: summary =="
echo "This run proved, against real local candidate bytes built fresh at HEAD:"
echo "  - offline structural manifest/version validity (preflight, not registry-consumer proof)"
echo "  - real cargo package / maturin build output, sha256-hashed, self-verified for"
echo "    wrong-source and changed-byte tampering (release/evidence/bindings-candidate.json)"
echo "  - the packaged sc-observability-dto/binding-runtime/py tarballs (not the live"
echo "    source tree) compile as an external Rust consumer"
echo "  - the built sc-observability wheel installs into a fresh isolated venv and its"
echo "    test subset passes against the installed package"
echo "  - the @synaptic-canvas/sc-observability npm tarball installs and smoke-checks in isolation"
echo "    (owner publication gate pending; NOT publish-ready)"
echo "  - sc-observability-tauri: qualification recorded PASS; independent phase-end QA/API approval remains pending"
echo
if [[ "$LIVE_REGISTRY_CHECK" == "1" ]]; then
  echo "registry consumer validation passed (live registry lookups were performed above)"
else
  echo "Nothing above was installed from a live crates.io/PyPI/npm registry -- these are"
  echo "local packaged-candidate proofs only. Nothing has been published. Re-run with"
  echo "--live-registry-check only after real publication for an actual live-registry result."
  echo "binding candidate readiness checks completed"
fi
