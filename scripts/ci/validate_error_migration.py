#!/usr/bin/env python3
"""Validate the B.1e warning contract and standalone downstream fixtures."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from copy import deepcopy
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
    clean = run("cargo", "clean", "--locked", "--manifest-path", str(manifest))
    assert_true(clean.returncode == 0, f"{name} cargo clean failed:\n{clean.stderr}")
    result = run(
        "cargo",
        "check",
        "--locked",
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


def diagnostic_note(diagnostic: dict) -> str | None:
    """Extract the compiler's complete migration note from its primary message."""
    message = diagnostic.get("message", "")
    marker = ": Use "
    if message.count(marker) != 1:
        return None
    return message[message.index(marker) + 2 :]


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
        attributes = []
        attribute_index = index - 1
        while attribute_index >= 0:
            stripped = lines[attribute_index].strip()
            if (
                stripped.startswith("#[")
                or stripped in {
                    ")]",
                    "deprecated,",
                    "since = \"1.4.0\",",
                }
                or stripped.startswith(("since =", "note =", "reason ="))
            ):
                attributes.append(lines[attribute_index])
                attribute_index -= 1
                continue
            break
        block = "\n".join(reversed(attributes))
        if (
            "#[deprecated(" in block
            and 'since = "1.4.0"' in block
            and note in block
        ):
            return block
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
        "CountingTypedProjector",
        "CountingLegacyProjector",
        "fetch_add(1",
        "CUSTOM_FIXTURE_CODE",
        ".emit(observation",
    ):
        assert_true(required in matrix, f"adapter matrix misses {required}")

    skill = ROOT / ".claude" / "skills" / "sc-observability-adopting" / "references" / "migrate-error-api.md"
    assert_true(skill.exists(), "migration skill reference is missing")
    skill_text = skill.read_text()
    assert_true(
        "ClassifiedError" in skill_text and "kind()" in skill_text,
        "skill reference lacks typed matching guidance",
    )


def fixture_spans(source: str, expected_notes: tuple[str, ...]) -> dict[str, list[int]]:
    lines = source.splitlines()
    spans = {note: [] for note in expected_notes}

    def add(note: str, predicate) -> None:
        spans[note].extend(index + 1 for index, line in enumerate(lines) if predicate(index, line))

    for legacy, typed in WRAPPERS:
        note = f"Use sc_observability_types::typed::{typed}; see migrate-error-api.md."
        if legacy == "InitError":
            add(note, lambda _index, line: "InitError" in line or "legacy.0." in line)
        else:
            add(note, lambda _index, line, name=legacy: f"size_of::<sc_observability_types::{name}>" in line)

    method_markers = {
        "LoggerBuilder::new": lambda _index, line: "LoggerBuilder::new(" in line,
        "Logger::builder": lambda _index, line: "Logger::builder(" in line,
        "Logger::new": lambda _index, line: "Logger::new(" in line,
        "Logger::log": lambda _index, line: "logger.log(" in line,
        "Logger::try_log": lambda _index, line: ".try_log(" in line and ".try_log_with_outcome(" not in line,
        "Logger::try_log_with_outcome": lambda _index, line: ".try_log_with_outcome(" in line,
        "Logger::flush": lambda _index, line: "logger.flush(" in line,
        "ObservabilityConfig::default_for": lambda _index, line: "ObservabilityConfig::default_for(" in line,
        "ObservabilityConfig::service_name": lambda _index, line: "config.service_name(" in line,
        "Observability::new": lambda _index, line: "Observability::new(" in line,
        "Observability::flush": lambda _index, line: "routed.flush(" in line,
        "Observability::shutdown": lambda _index, line: "routed.shutdown(" in line,
        "OtlpEndpoint::new": lambda _index, line: "OtlpEndpoint::new(" in line,
        "AuthHeader::new": lambda _index, line: "AuthHeader::new(" in line,
        "Telemetry::new": lambda _index, line: "Telemetry::new(" in line,
        "Telemetry::flush": lambda _index, line: "telemetry.flush(" in line,
        "Telemetry::shutdown": lambda _index, line: "telemetry.shutdown(" in line,
        "SpanAssembler::push": lambda _index, line: "assembler.push(" in line,
    }
    for _source, legacy, typed in METHODS:
        note = f"Use {typed}(); see migrate-error-api.md."
        if legacy == "ObservabilityBuilder::build":
            add(
                note,
                lambda index, line: ".build()" in line
                and "Observability::builder" in "\n".join(lines[max(0, index - 4) : index + 1]),
            )
        elif legacy == "TelemetryConfigBuilder::build":
            add(
                note,
                lambda index, line: ".build()" in line
                and "TelemetryConfigBuilder::new" in "\n".join(lines[max(0, index - 4) : index + 1]),
            )
        else:
            add(note, method_markers[legacy])
    assert_true(all(spans[note] for note in expected_notes), "fixture contract has an unbound target")
    return spans


def validate_diagnostics(
    name: str,
    diagnostics: list[dict],
    source: str,
    expected_notes: tuple[str, ...],
) -> None:
    expected_spans = fixture_spans(source, expected_notes) if expected_notes else {}
    if not expected_notes:
        assert_true(not diagnostics, f"{name} emitted unexpected fixture warnings")
        return
    observed_spans = {note: [] for note in expected_notes}
    for diagnostic in diagnostics:
        assert_true(diagnostic.get("level") == "warning", f"{name} emitted a non-warning diagnostic")
        assert_true(
            diagnostic.get("code", {}).get("code") == "deprecated",
            f"{name} emitted an unexpected warning code",
        )
        spans = diagnostic.get("spans", [])
        primary = [span for span in spans if span.get("is_primary")]
        assert_true(len(spans) == 1 and len(primary) == 1, f"{name} diagnostic has unexpected spans")
        span = primary[0]
        assert_true(span.get("file_name") == "src/main.rs", f"{name} diagnostic escaped fixture source")
        note_matches = [note for note in expected_notes if diagnostic_note(diagnostic) == note]
        assert_true(len(note_matches) == 1, f"{name} diagnostic has an unexpected migration note")
        note = note_matches[0]
        line = span.get("line_start")
        assert_true(line in expected_spans[note], f"{name} diagnostic is bound to the wrong fixture item/span")
        observed_spans[note].append(line)
    for note, expected in expected_spans.items():
        assert_true(
            sorted(observed_spans[note]) == sorted(expected),
            f"{name} warning spans for {note} are {sorted(observed_spans[note])}, expected {sorted(expected)}",
        )


def check_fixture(name: str, expected_notes: tuple[str, ...] = ()) -> list[dict]:
    result, diagnostics = cargo_check_json(name)
    assert_true(result.returncode == 0, f"{name} cargo check failed:\n{result.stderr}")
    deprecated = [d for d in diagnostics if d.get("code", {}).get("code") == "deprecated"]
    assert_true(len(deprecated) == len(diagnostics), f"{name} emitted an unexpected non-deprecation warning")
    source = (FIXTURE_ROOT / name / "src" / "main.rs").read_text()
    validate_diagnostics(name, diagnostics, source, expected_notes)
    messages = "\n".join(rendered(d) for d in deprecated)
    for note in expected_notes:
        assert_true(note in messages, f"{name} lacks expected deprecation diagnostic: {note}")
    return_code = run(
        "cargo",
        "run",
        "--locked",
        "--manifest-path",
        str(FIXTURE_ROOT / name / "Cargo.toml"),
        "--quiet",
    ).returncode
    assert_true(return_code == 0, f"{name} cargo run failed")
    return deprecated


def check_partial_allow(source: str | None = None) -> None:
    source = source or (FIXTURE_ROOT / "partial" / "src" / "main.rs").read_text()
    assert_true(
        re.search(r"#\[allow\(\s*deprecated\s*,\s*reason\s*=", source, re.S) is not None,
        "partial fixture lacks a reason-bearing local compatibility allow",
    )
    assert_true(
        re.search(r"#!\[allow\(\s*(deprecated|warnings)", source, re.S) is None,
        "partial fixture contains a broad crate-level allow",
    )


def check_negative_controls() -> None:
    partial_source = (FIXTURE_ROOT / "partial" / "src" / "main.rs").read_text()
    broad = partial_source.replace(
        "#[allow(\n    deprecated,",
        "#![allow(deprecated)]\n\n#[allow(\n    deprecated,",
    )
    try:
        check_partial_allow(broad)
    except AssertionError:
        pass
    else:
        raise AssertionError("negative broad-allow control did not trigger the real predicate")

    source = (ROOT / "crates" / "sc-observability-types" / "src" / "errors.rs").read_text()
    identity_note = "Use sc_observability_types::typed::IdentityFailure; see migrate-error-api.md."

    def rejected(mutated: str, message: str) -> None:
        try:
            item_window(mutated, "IdentityError", identity_note)
        except AssertionError:
            return
        raise AssertionError(message)

    rejected(source.replace('since = "1.4.0"', 'since = "1.3.0"', 1), "wrong version negative did not trigger")
    rejected(source.replace(identity_note, "Use the wrong replacement; see migrate-error-api.md.", 1), "wrong note negative did not trigger")
    identity_attribute = re.search(
        r"#\[deprecated\(\n    since = \"1\.4\.0\",\n    note = \"Use sc_observability_types::typed::IdentityFailure; see migrate-error-api\.md\.\"\n\)\]\n",
        source,
    )
    assert_true(identity_attribute is not None, "identity attribute fixture is missing")
    misplaced = source.replace(identity_attribute.group(0), "", 1).replace(
        "impl sealed::Sealed for IdentityError",
        identity_attribute.group(0) + "impl sealed::Sealed for IdentityError",
        1,
    )
    rejected(misplaced, "misplaced attribute negative did not trigger")

    legacy_source = (FIXTURE_ROOT / "legacy" / "src" / "main.rs").read_text()
    notes = migration_notes()
    result, diagnostics = cargo_check_json("legacy")
    assert_true(result.returncode == 0, "negative-control legacy fixture could not compile")
    expected_diagnostics = [d for d in diagnostics if d.get("code", {}).get("code") == "deprecated"]
    validate_diagnostics("legacy", expected_diagnostics, legacy_source, notes)

    def rejects_diagnostics(mutated: list[dict], message: str) -> None:
        try:
            validate_diagnostics("negative", mutated, legacy_source, notes)
        except AssertionError:
            return
        raise AssertionError(message)

    wrong_code = deepcopy(expected_diagnostics)
    wrong_code[0]["code"]["code"] = "unused_imports"
    rejects_diagnostics(wrong_code, "wrong diagnostic code negative did not trigger")
    wrong_note = deepcopy(expected_diagnostics)
    note_index = next(
        index for index, diagnostic in enumerate(wrong_note) if diagnostic_note(diagnostic) == notes[0]
    )
    wrong_note[note_index]["message"] = wrong_note[note_index]["message"].replace(
        notes[0], "wrong migration note"
    )
    wrong_note[note_index]["rendered"] = wrong_note[note_index]["rendered"].replace(
        notes[0], "wrong migration note"
    )
    rejects_diagnostics(wrong_note, "wrong diagnostic note negative did not trigger")
    appended_note = deepcopy(expected_diagnostics)
    appended_index = next(
        index for index, diagnostic in enumerate(appended_note) if diagnostic_note(diagnostic) == notes[0]
    )
    appended_note[appended_index]["message"] += " EXTRA TEXT"
    appended_note[appended_index]["rendered"] += " EXTRA TEXT"
    rejects_diagnostics(appended_note, "appended diagnostic note negative did not trigger")
    duplicate_span = deepcopy(expected_diagnostics)
    default_note = next(note for note in notes if "ObservabilityConfig::default_for_typed" in note)
    default_indices = [
        index for index, diagnostic in enumerate(duplicate_span) if diagnostic_note(diagnostic) == default_note
    ]
    assert_true(len(default_indices) >= 2, "default_for diagnostic negative lacks repeated target spans")
    duplicate_span[default_indices[1]] = deepcopy(duplicate_span[default_indices[0]])
    rejects_diagnostics(duplicate_span, "duplicate diagnostic span negative did not trigger")
    missing = expected_diagnostics[:-1]
    rejects_diagnostics(missing, "missing diagnostic negative did not trigger")
    extra = expected_diagnostics + [deepcopy(expected_diagnostics[0])]
    rejects_diagnostics(extra, "extra diagnostic negative did not trigger")
    wrong_span = deepcopy(expected_diagnostics)
    wrong_span[0]["spans"][0]["line_start"] += 1
    rejects_diagnostics(wrong_span, "wrong diagnostic span negative did not trigger")


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
        validate_diagnostics(
            "migrated",
            migrated_diagnostics,
            (FIXTURE_ROOT / "migrated" / "src" / "main.rs").read_text(),
            (),
        )
        assert_true(
            not any(d.get("code", {}).get("code") == "deprecated" for d in migrated_diagnostics),
            "migrated fixture emitted a deprecated diagnostic",
        )
        assert_true(
            run(
                "cargo",
                "run",
                "--locked",
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
