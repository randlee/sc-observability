"""Equivalence of the two installed Wave 5 submission front ends."""
from __future__ import annotations

import json
from pathlib import Path

from conftest import GOLDENS, canonical_json, normalise_system_generated_fields, run_cli, run_installed_python


def _python_build_script() -> str:
    return """\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import build_envelope

result = build_envelope(json.load(sys.stdin))
if isinstance(result, Ok):
    print(json.dumps({"kind": "ok", "envelope": json.loads(result.value)}, separators=(",", ":")))
else:
    print(json.dumps({"kind": "error", "code": result.error.code}, separators=(",", ":")))
"""


def test_every_d29_golden_matches_installed_python_and_cli(installed_artifacts: dict[str, Path], tmp_path: Path) -> None:
    """Both public front ends accept the single D29 SubmissionInput contract."""
    fixtures = sorted(path for path in GOLDENS.iterdir() if (path / "input.json").is_file())
    assert fixtures, "the D29 golden corpus must not silently disappear"
    for fixture in fixtures:
        source = (fixture / "input.json").read_text(encoding="utf-8")
        python = run_installed_python(installed_artifacts, _python_build_script(), cwd=tmp_path, input=source)
        cli = run_cli(installed_artifacts, "validate", "--stdin", cwd=tmp_path, input=source)
        assert python.returncode == 0, python.stderr
        py_result = json.loads(python.stdout)
        cli_result = json.loads(cli.stdout)
        expected_error = fixture / "expected.error.json"
        if expected_error.exists():
            expected = json.loads(expected_error.read_text(encoding="utf-8"))["code"]
            assert py_result == {"kind": "error", "code": expected}, fixture.name
            assert cli.returncode == 3, fixture.name
            assert cli_result["error"]["code"] == expected, fixture.name
            continue
        assert py_result["kind"] == "ok", fixture.name
        assert cli.returncode == 0, fixture.name
        # The values originate in distinct installed front ends.  Comparing the
        # serialized canonical documents catches ordering and conversion drift.
        # Only D29's three invocation-generated fields may differ.
        submitted = json.loads(source)
        assert canonical_json(normalise_system_generated_fields(py_result["envelope"], submitted)) == canonical_json(
            normalise_system_generated_fields(cli_result["envelope"], submitted)
        ), fixture.name
