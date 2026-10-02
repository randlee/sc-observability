from __future__ import annotations

import shlex
import subprocess
import unittest
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[3]
EXCLUDED_PREFIXES = {
    ".claude/": "User-owned agent tooling is intentionally outside product CI.",
    "docs/plans/": "Planning documents and their examples are not executable test suites.",
    "bindings/python/sc-observability-py/tests_telemetry/": (
        "Requires an installed wheel with otlp-telemetry and private test-hooks; "
        "the existing three-platform build omits the native test-double entry point."
    ),
}


def _is_excluded(path: str) -> bool:
    return any(path == prefix.rstrip("/") or path.startswith(prefix) for prefix in EXCLUDED_PREFIXES)


def _tracked_python_test_files(root: Path = ROOT) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True
    )
    return [
        path
        for path in result.stdout.decode().split("\0")
        if path
        and PurePosixPath(path).name.startswith("test_")
        and PurePosixPath(path).suffix == ".py"
    ]


def _test_directories(test_files: list[str]) -> set[str]:
    return {
        PurePosixPath(path).parent.as_posix()
        for path in test_files
        if not _is_excluded(path)
    }


def _logical_lines(text: str) -> list[str]:
    lines: list[str] = []
    pending = ""
    for line in text.splitlines():
        stripped = line.rstrip()
        if stripped.endswith("\\"):
            pending += stripped[:-1] + " "
        else:
            lines.append(pending + line)
            pending = ""
    if pending:
        lines.append(pending)
    return lines


def _tokens(text: str) -> list[list[str]]:
    commands: list[list[str]] = []
    for line in _logical_lines(text):
        lexer = shlex.shlex(line, punctuation_chars=";&|")
        lexer.whitespace_split = True
        lexer.commenters = "#"
        try:
            commands.append(list(lexer))
        except ValueError:
            continue
    return commands


def _ci_sources(root: Path) -> list[tuple[str, str]]:
    sources: list[tuple[str, str]] = []
    for workflow in sorted((root / ".github/workflows").glob("*.yml")):
        sources.append((workflow.relative_to(root).as_posix(), workflow.read_text()))
    for workflow in sorted((root / ".github/workflows").glob("*.yaml")):
        sources.append((workflow.relative_to(root).as_posix(), workflow.read_text()))

    tracked = subprocess.run(
        ["git", "ls-files", "-z", "scripts/ci"],
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout.decode().split("\0")
    for relative in tracked:
        if not relative or PurePosixPath(relative).suffix not in {".py", ".sh"}:
            continue
        path = PurePosixPath(relative)
        if path.name.startswith("test_") or "tests" in path.parts:
            continue
        source = root / relative
        sources.append((relative, source.read_text()))
    return sources


def _invocation_paths(text: str, root: Path) -> list[str]:
    paths: list[str] = []
    for tokens in _tokens(text):
        for index, token in enumerate(tokens):
            if token == "-m" and index + 1 < len(tokens):
                runner = tokens[index + 1]
                if runner == "pytest":
                    for candidate in tokens[index + 2 :]:
                        if candidate in {";", "&&", "||", "|"}:
                            break
                        if candidate.startswith("-") or "$" in candidate:
                            continue
                        normalized = PurePosixPath(candidate).as_posix()
                        if (root / normalized).is_dir():
                            paths.append(normalized)
                elif runner == "unittest":
                    arguments = tokens[index + 2 :]
                    if arguments and arguments[0] == "discover":
                        for option_index, option in enumerate(arguments[:-1]):
                            if option == "-s":
                                candidate = arguments[option_index + 1]
                                if "$" not in candidate:
                                    normalized = PurePosixPath(candidate).as_posix()
                                    if (root / normalized).is_dir():
                                        paths.append(normalized)
                    else:
                        for module in arguments:
                            if module in {";", "&&", "||", "|"}:
                                break
                            if module.startswith("-") or "$" in module:
                                continue
                            if module.endswith(".py") and (root / module).is_file():
                                paths.append(PurePosixPath(module).parent.as_posix())
                                continue
                            module_path = PurePosixPath(module.replace(".", "/"))
                            module_file = root / f"{module_path}.py"
                            module_directory = root / module_path
                            if module_file.is_file():
                                paths.append(module_path.parent.as_posix())
                            elif module_directory.is_dir():
                                paths.append(module_path.as_posix())
    return paths


def _staged_test_sources(text: str) -> dict[str, str]:
    staged: dict[str, str] = {}
    for tokens in _tokens(text):
        for index, token in enumerate(tokens):
            if token != "cp":
                continue
            arguments = tokens[index + 1 :]
            if arguments and arguments[0] == "-R":
                arguments = arguments[1:]
            if len(arguments) >= 2:
                source, destination = arguments[:2]
                if source.startswith("bindings/") and "$" in destination:
                    staged[destination] = PurePosixPath(source).as_posix()
    return staged


def _covered_paths(root: Path) -> list[str]:
    sources = _ci_sources(root)
    covered: list[str] = []
    staged_sources: dict[str, str] = {}
    for _, text in sources:
        staged_sources.update(_staged_test_sources(text))
    for _, text in sources:
        covered.extend(_invocation_paths(text, root))
        for token_line in _tokens(text):
            for index, token in enumerate(token_line):
                is_pytest = (
                    token == "-m"
                    and index + 1 < len(token_line)
                    and token_line[index + 1] == "pytest"
                )
                if is_pytest:
                    for candidate in token_line[index + 2 :]:
                        if candidate in {";", "&&", "||", "|"}:
                            break
                        if candidate in staged_sources:
                            covered.append(staged_sources[candidate])
    return covered


def _uncovered_directories(test_files: list[str], invoked_paths: list[str]) -> list[str]:
    directories = _test_directories(test_files)
    normalized_invocations = {
        PurePosixPath(path).as_posix().rstrip("/") for path in invoked_paths
    }
    return sorted(
        directory
        for directory in directories
        if not any(
            directory == invocation or directory.startswith(invocation + "/")
            for invocation in normalized_invocations
        )
    )


class PythonSuiteCoverageTests(unittest.TestCase):
    def test_every_tracked_python_test_directory_has_a_ci_runner(self) -> None:
        uncovered = _uncovered_directories(
            _tracked_python_test_files(), _covered_paths(ROOT)
        )
        self.assertEqual(
            uncovered,
            [],
            "tracked Python test directories without a pytest/unittest invocation: "
            + ", ".join(uncovered),
        )

    def test_uncovered_directory_is_reported(self) -> None:
        uncovered = _uncovered_directories(
            ["uncovered/suite/test_example.py"], ["scripts/tests"]
        )
        self.assertEqual(uncovered, ["uncovered/suite"])


if __name__ == "__main__":
    unittest.main()
