#!/usr/bin/env python3
"""Validate the B.1e warning contract and standalone downstream fixtures."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FIXTURE_ROOT = ROOT / "scripts" / "ci" / "fixtures" / "error-migration"

WRAPPERS = (
    ("IdentityError", "IdentityFailure"),
    ("InitError", "InitFailure"),
    ("EventError", "EventFailure"),
    ("FlushError", "FlushFailure"),
    ("ShutdownError", "ShutdownFailure"),
    ("ProjectionError", "ProjectionFailure"),
    ("SubscriberError", "SubscriberFailure"),
    ("LogSinkError", "LogSinkFailure"),
    ("ExportError", "ExportFailure"),
)

METHODS = (
    ("logger", "LoggerBuilder::new", "LoggerBuilder::new_typed"),
    ("runtime", "Logger::builder", "Logger::builder_typed"),
    ("runtime", "Logger::new", "Logger::new_typed"),
    ("runtime", "Logger::log", "Logger::log_typed"),
    ("runtime", "Logger::try_log", "Logger::try_log_typed"),
    (
        "runtime",
        "Logger::try_log_with_outcome",
        "Logger::try_log_with_outcome_typed",
    ),
    ("runtime", "Logger::flush", "Logger::flush_typed"),
    (
        "observe",
        "ObservabilityConfig::default_for",
        "ObservabilityConfig::default_for_typed",
    ),
    ("observe", "ObservabilityConfig::service_name", "ObservabilityConfig::service_name_typed"),
    ("observe", "Observability::new", "Observability::new_typed"),
    ("observe", "Observability::flush", "Observability::flush_typed"),
    ("observe", "Observability::shutdown", "Observability::shutdown_typed"),
    ("observe", "ObservabilityBuilder::build", "ObservabilityBuilder::build_typed"),
    ("otlp_config", "OtlpEndpoint::new", "OtlpEndpoint::new_typed"),
    ("otlp_config", "AuthHeader::new", "AuthHeader::new_typed"),
    (
        "otlp_config",
        "TelemetryConfigBuilder::build",
        "TelemetryConfigBuilder::build_typed",
    ),
    ("otlp_assembly", "SpanAssembler::push", "SpanAssembler::push_typed"),
    ("otlp_runtime", "Telemetry::new", "Telemetry::new_typed"),
    ("otlp_runtime", "Telemetry::flush", "Telemetry::flush_typed"),
    ("otlp_runtime", "Telemetry::shutdown", "Telemetry::shutdown_typed"),
)


def migration_notes() -> tuple[str, ...]:
    wrapper_notes = tuple(
        f"Use sc_observability_types::typed::{typed}; see migrate-error-api.md."
        for _, typed in WRAPPERS
    )
    method_notes = tuple(
        f"Use {typed}(); see migrate-error-api.md." for _, _, typed in METHODS
    )
    return wrapper_notes + method_notes


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(args),
        cwd=ROOT,
        text=True,
        capture_output=True,
        env={**os.environ, "CARGO_TERM_COLOR": "never"},
        check=False,
    )


def cargo_check_json(name: str) -> tuple[subprocess.CompletedProcess[str], list[dict]]:
    manifest = FIXTURE_ROOT / name / "Cargo.toml"
    # Force rustc to emit fresh JSON diagnostics. Fixture targets are standalone
    # generated build directories, so cleaning them does not touch workspace
    # artifacts or source state.
    clean = run("cargo", "clean", "--manifest-path", str(manifest))
    assert_true(clean.returncode == 0, f"{name} cargo clean failed:\n{clean.stderr}")
    result = run(
        "cargo",
        "check",
        "--manifest-path",
        str(manifest),
        "--message-format=json",
    )
    diagnostics: list[dict] = []
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-message":
            continue
        diagnostic = message.get("message", {})
        if diagnostic.get("level") != "warning":
            continue
        target = message.get("target", {})
        if target.get("name") != f"error-migration-{name}":
            continue
        diagnostics.append(diagnostic)
    return result, diagnostics


def assert_true(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def rendered(diagnostic: dict) -> str:
    return diagnostic.get("rendered", diagnostic.get("message", ""))


def item_window(source: str, marker: str, note: str) -> str:
    lines = source.splitlines()
    candidates = []
    for index, line in enumerate(lines):
        stripped = line.strip()
        if marker.startswith("pub fn "):
            matches = stripped.startswith(marker)
        else:
            matches = stripped == marker or stripped.startswith(f"pub struct {marker}")
        if matches:
            candidates.append(index)
    for index in candidates:
        window = "\n".join(lines[max(0, index - 18) : index + 1])
        if "#[deprecated(" in window and note in window:
            return window
    raise AssertionError(f"{marker} has no local deprecation attribute/note")


def check_source_contract() -> None:
    sources = {
        "types": ROOT / "crates" / "sc-observability-types" / "src" / "errors.rs",
        "logger": ROOT / "crates" / "sc-observability" / "src" / "builder.rs",
        "runtime": ROOT / "crates" / "sc-observability" / "src" / "runtime.rs",
        "observe": ROOT / "crates" / "sc-observe" / "src" / "lib.rs",
        "otlp_config": ROOT / "crates" / "sc-observability-otlp" / "src" / "config.rs",
        "otlp_assembly": ROOT / "crates" / "sc-observability-otlp" / "src" / "assembly.rs",
        "otlp_runtime": ROOT / "crates" / "sc-observability-otlp" / "src" / "lib.rs",
    }
    text = {name: path.read_text() for name, path in sources.items()}

    for legacy, typed in WRAPPERS:
        item_window(
            text["types"],
            legacy,
            f"Use sc_observability_types::typed::{typed}; see migrate-error-api.md.",
        )

    assert_true(len(WRAPPERS) + len(METHODS) == 29, "B.1e target inventory is not 29 items")
    for source, legacy, typed in METHODS:
        item_window(
            text[source],
            f"pub fn {legacy.rsplit('::', 1)[1]}(",
            f"Use {typed}(); see migrate-error-api.md.",
        )

    for source in ("logger", "runtime", "observe", "otlp_config", "otlp_assembly", "otlp_runtime"):
        assert_true(
            text[source].count("since = \"1.4.0\"") > 0,
            f"{source} has no B.1e deprecation marker",
        )

    for path in (
        ROOT / "crates" / "sc-observability" / "src" / "runtime.rs",
        ROOT / "crates" / "sc-observability" / "src" / "builder.rs",
        ROOT / "crates" / "sc-observability" / "src" / "sinks.rs",
    ):
        source = path.read_text()
        assert_true(
            re.search(r"#!\[allow\(\s*deprecated", source, re.S) is None,
            f"{path.name} hides compatibility warnings with a whole-module allowance",
        )
        for match in re.finditer(r"#\[allow\(\s*deprecated", source, re.S):
            assert_true(
                re.search(r"reason\s*=", source[match.start() : match.start() + 400]) is not None,
                f"{path.name} has an unexplained deprecated allowance",
            )

    # These methods are the documented supported-method exemptions.
    for source, method in (("logger", "pub fn build(self)"), ("logger", "pub fn build_with_level_owner("), ("runtime", "pub fn new_with_level_owner(")):
        lines = text[source].splitlines()
        indices = [i for i, line in enumerate(lines) if method in line]
        assert_true(indices, f"missing supported method {method}")
        for index in indices:
            window = "\n".join(lines[max(0, index - 8) : index])
            assert_true("#[deprecated(" not in window, f"supported method {method} was deprecated")

    emit_lines = text["runtime"].splitlines()
    emit_indices = [i for i, line in enumerate(emit_lines) if "pub fn emit(&self" in line]
    assert_true(emit_indices, "Logger::emit is missing")
    emit_window = "\n".join(emit_lines[max(0, emit_indices[0] - 8) : emit_indices[0]])
    assert_true('since = "1.2.0"' in emit_window, "Logger::emit existing 1.2.0 warning changed")

    migrated = (FIXTURE_ROOT / "migrated" / "src" / "main.rs").read_text()
    matrix = (FIXTURE_ROOT / "migrated" / "src" / "compatibility_matrix.rs").read_text()
    for required in (
        "new_with_level_owner(",
        "new_with_level_owner_typed(",
        "build_with_level_owner()",
        "build_with_level_owner_typed()",
    ):
        assert_true(required in migrated, f"migrated fixture misses owner scenario {required}")
    for required in (
        "legacy_identity",
        "typed_identity",
        "legacy_subscriber",
        "typed_subscriber",
        "legacy_log_projector",
        "typed_log_projector",
        "legacy_span_projector",
        "typed_span_projector",
        "legacy_metric_projector",
        "typed_metric_projector",
        "legacy_sink",
        "typed_sink",
        "ProjectionFailureKind::Unclassified",
        "std::error::Error::source",
        "runtime.emit",
    ):
        assert_true(required in matrix, f"adapter matrix misses {required}")

    skill = ROOT / ".claude" / "skills" / "sc-observability-adopting" / "references" / "migrate-error-api.md"
    assert_true(skill.exists(), "migration skill reference is missing")
    skill_text = skill.read_text()
    assert_true(
        "ClassifiedError" in skill_text and "kind()" in skill_text,
        "skill reference lacks typed matching guidance",
    )


def check_fixture(name: str, expected_notes: tuple[str, ...] = ()) -> list[dict]:
    result, diagnostics = cargo_check_json(name)
    assert_true(result.returncode == 0, f"{name} cargo check failed:\n{result.stderr}")
    deprecated = [d for d in diagnostics if d.get("code", {}).get("code") == "deprecated"]
    assert_true(len(deprecated) == len(diagnostics), f"{name} emitted an unexpected non-deprecation warning")
    for diagnostic in deprecated:
        primary = [span for span in diagnostic.get("spans", []) if span.get("is_primary")]
        assert_true(
            len(diagnostic.get("spans", [])) == 1 and len(primary) == 1,
            f"{name} deprecation has an unexpected secondary source span",
        )
        assert_true(primary[0].get("file_name") == "src/main.rs", f"{name} deprecation span escaped fixture source")
        diagnostic_notes = [note for note in expected_notes if note in rendered(diagnostic)]
        assert_true(
            len(diagnostic_notes) == 1,
            f"{name} deprecation has an unexpected or ambiguous migration note",
        )
    messages = "\n".join(rendered(d) for d in deprecated)
    for note in expected_notes:
        assert_true(note in messages, f"{name} lacks expected deprecation diagnostic: {note}")
    return_code = run(
        "cargo",
        "run",
        "--manifest-path",
        str(FIXTURE_ROOT / name / "Cargo.toml"),
        "--quiet",
    ).returncode
    assert_true(return_code == 0, f"{name} cargo run failed")
    return deprecated


def check_partial_allow() -> None:
    source = (FIXTURE_ROOT / "partial" / "src" / "main.rs").read_text()
    assert_true(
        re.search(r"#\[allow\(\s*deprecated\s*,\s*reason\s*=", source, re.S) is not None,
        "partial fixture lacks a reason-bearing local compatibility allow",
    )
    assert_true(
        re.search(r"#!\[allow\(\s*(deprecated|warnings)", source, re.S) is None,
        "partial fixture contains a broad crate-level allow",
    )


def check_negative_controls() -> None:
    source = (FIXTURE_ROOT / "partial" / "src" / "main.rs").read_text()
    broad = source.replace(
        "#[allow(\n    deprecated,",
        "#![allow(deprecated)]\n\n#[allow(\n    deprecated,",
    )
    assert_true(
        re.search(r"#!\[allow\(\s*(deprecated|warnings)", broad, re.S) is not None,
        "negative broad-allow control did not trigger",
    )
    bad_diagnostic = {
        "code": {"code": "unused_imports"},
        "spans": [{"is_primary": True, "file_name": "src/main.rs"}],
        "rendered": "warning: unexpected warning",
    }
    try:
        assert_true(bad_diagnostic.get("code", {}).get("code") == "deprecated", "unexpected warning")
    except AssertionError:
        return
    raise AssertionError("negative unexpected-warning control did not trigger")


def main() -> int:
    try:
        check_source_contract()
        legacy_diagnostics = check_fixture("legacy", migration_notes())
        assert_true(
            len({note for note in migration_notes() if any(note in rendered(d) for d in legacy_diagnostics)})
            == 29,
            "legacy fixture did not exercise every wrapper and mapped-method warning note",
        )
        wrapper_notes = {
            f"Use sc_observability_types::typed::{typed}; see migrate-error-api.md."
            for _, typed in WRAPPERS
        }
        for diagnostic in legacy_diagnostics:
            notes = [note for note in migration_notes() if note in rendered(diagnostic)]
            assert_true(len(notes) == 1, "legacy diagnostic is not isolated to one contract target")
            if notes[0] in wrapper_notes:
                assert_true(
                    "deprecated method" not in diagnostic["message"]
                    and "deprecated associated function" not in diagnostic["message"],
                    "legacy wrapper warning was misclassified as a method warning",
                )
            else:
                assert_true(
                    "deprecated method" in diagnostic["message"]
                    or "deprecated associated function" in diagnostic["message"],
                    "legacy method warning was misclassified as a wrapper warning",
                )
        migrated_result, migrated_diagnostics = cargo_check_json("migrated")
        assert_true(migrated_result.returncode == 0, "migrated fixture cargo check failed")
        assert_true(
            not any(d.get("code", {}).get("code") == "deprecated" for d in migrated_diagnostics),
            "migrated fixture emitted a deprecated diagnostic",
        )
        assert_true(
            run(
                "cargo",
                "run",
                "--manifest-path",
                str(FIXTURE_ROOT / "migrated" / "Cargo.toml"),
                "--quiet",
            ).returncode
            == 0,
            "migrated fixture cargo run failed",
        )
        check_fixture("partial")
        check_partial_allow()
        check_negative_controls()
    except (AssertionError, OSError) as error:
        print(f"B.1e migration validation: FAIL: {error}", file=sys.stderr)
        return 1
    print("B.1e migration validation: PASS (source contract, JSON diagnostics, and all fixtures)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
