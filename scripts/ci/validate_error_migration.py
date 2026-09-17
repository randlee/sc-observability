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

    wrappers = {
        "IdentityError": "IdentityFailure",
        "InitError": "InitFailure",
        "EventError": "EventFailure",
        "FlushError": "FlushFailure",
        "ShutdownError": "ShutdownFailure",
        "ProjectionError": "ProjectionFailure",
        "SubscriberError": "SubscriberFailure",
        "LogSinkError": "LogSinkFailure",
        "ExportError": "ExportFailure",
    }
    for legacy, typed in wrappers.items():
        assert_true(legacy in text["types"], f"missing wrapper {legacy}")
        assert_true(
            f"since = \"1.4.0\"" in text["types"],
            f"{legacy} is missing the 1.4.0 warning version",
        )
        assert_true(
            f"Use sc_observability_types::typed::{typed}; see migrate-error-api.md." in text["types"],
            f"{legacy} has no exact typed replacement note",
        )

    methods = {
        ("logger", "LoggerBuilder::new", "LoggerBuilder::new_typed"),
        ("runtime", "Logger::builder", "Logger::builder_typed"),
        ("runtime", "Logger::new", "Logger::new_typed"),
        ("runtime", "Logger::log", "Logger::log_typed"),
        ("runtime", "Logger::try_log", "Logger::try_log_typed"),
        ("runtime", "Logger::try_log_with_outcome", "Logger::try_log_with_outcome_typed"),
        ("runtime", "Logger::flush", "Logger::flush_typed"),
        ("observe", "ObservabilityConfig::default_for", "ObservabilityConfig::default_for_typed"),
        ("observe", "ObservabilityConfig::service_name", "ObservabilityConfig::service_name_typed"),
        ("observe", "Observability::new", "Observability::new_typed"),
        ("observe", "Observability::flush", "Observability::flush_typed"),
        ("observe", "Observability::shutdown", "Observability::shutdown_typed"),
        ("observe", "ObservabilityBuilder::build", "ObservabilityBuilder::build_typed"),
        ("otlp_config", "OtlpEndpoint::new", "OtlpEndpoint::new_typed"),
        ("otlp_config", "AuthHeader::new", "AuthHeader::new_typed"),
        ("otlp_config", "TelemetryConfigBuilder::build", "TelemetryConfigBuilder::build_typed"),
        ("otlp_assembly", "SpanAssembler::push", "SpanAssembler::push_typed"),
        ("otlp_runtime", "Telemetry::new", "Telemetry::new_typed"),
        ("otlp_runtime", "Telemetry::flush", "Telemetry::flush_typed"),
        ("otlp_runtime", "Telemetry::shutdown", "Telemetry::shutdown_typed"),
    }
    for source, legacy, typed in methods:
        assert_true(
            f"since = \"1.4.0\"" in text[source] and f"Use {typed}(); see migrate-error-api.md." in text[source],
            f"{legacy} is missing its exact 1.4.0 migration warning",
        )

    for source in ("logger", "runtime", "observe", "otlp_config", "otlp_assembly", "otlp_runtime"):
        assert_true(
            text[source].count("since = \"1.4.0\"") > 0,
            f"{source} has no B.1e deprecation marker",
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

    skill = ROOT / ".claude" / "skills" / "sc-observability-adopting" / "references" / "migrate-error-api.md"
    assert_true(skill.exists(), "migration skill reference is missing")
    skill_text = skill.read_text()
    assert_true(
        "ClassifiedError" in skill_text and "kind()" in skill_text,
        "skill reference lacks typed matching guidance",
    )


def check_fixture(name: str, expected_notes: tuple[str, ...] = ()) -> None:
    result, diagnostics = cargo_check_json(name)
    assert_true(result.returncode == 0, f"{name} cargo check failed:\n{result.stderr}")
    deprecated = [d for d in diagnostics if d.get("code", {}).get("code") == "deprecated"]
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


def main() -> int:
    try:
        check_source_contract()
        check_fixture(
            "legacy",
            (
                "Use LoggerBuilder::new_typed(); see migrate-error-api.md.",
                "Use Logger::new_typed(); see migrate-error-api.md.",
                "Use ObservabilityConfig::default_for_typed(); see migrate-error-api.md.",
                "Use OtlpEndpoint::new_typed(); see migrate-error-api.md.",
            ),
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
    except (AssertionError, OSError) as error:
        print(f"B.1e migration validation: FAIL: {error}", file=sys.stderr)
        return 1
    print("B.1e migration validation: PASS (source contract, JSON diagnostics, and all fixtures)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
