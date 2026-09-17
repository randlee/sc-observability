#!/usr/bin/env python3
"""Fixture coverage for the B.1 immutable import/acceptance contract validator.

Every fixture here is synthetic (temporary Git repositories with honest,
made-up review records) -- this task builds and proves the tool ahead of
B.1's real copy, and never fabricates a production import-provenance.json
or handoff-b-p3.md.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from validate_log_import import validate_import  # noqa: E402

ACCEPTED_SHA_PLACEHOLDER = "0" * 40
REVIEW_COMMIT_PLACEHOLDER = "1" * 40


def run(*args: str, cwd: Path) -> str:
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def blob_id(content: str, repo: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), "hash-object", "--stdin"],
        input=content, check=True, capture_output=True, text=True,
    ).stdout.strip()


class ImportContractFixture:
    """Builds a matched (source-repo, destination, provenance, handoff) fixture set."""

    FILES = {
        "crates/sc-observability-log/Cargo.toml": '[package]\nname = "sc-observability-log"\n',
        "crates/sc-observability-log/src/lib.rs": "pub fn noop() {}\n",
        "crates/sc-observability-log-macros/Cargo.toml": '[package]\nname = "sc-observability-log-macros"\n',
        "crates/sc-observability-log-consumer-check/Cargo.toml": '[package]\nname = "consumer-check"\n',
    }

    def __init__(self, root: Path) -> None:
        self.root = root
        self.source_repo = root / "source-repo"
        self.destination = root / "destination"
        self.source_repo.mkdir()
        self.destination.mkdir()
        self._init_source_repo()
        self._write_destination(self.FILES)

    def _init_source_repo(self) -> None:
        run("git", "init", "--quiet", cwd=self.source_repo)
        run("git", "config", "user.email", "fixture@example.invalid", cwd=self.source_repo)
        run("git", "config", "user.name", "fixture", cwd=self.source_repo)
        for path, content in self.FILES.items():
            full = self.source_repo / path
            full.parent.mkdir(parents=True, exist_ok=True)
            full.write_text(content)
        run("git", "add", "-A", cwd=self.source_repo)
        run("git", "commit", "--quiet", "-m", "synthetic BTIT source", cwd=self.source_repo)
        self.source_commit = run("git", "rev-parse", "HEAD", cwd=self.source_repo)

    def _write_destination(self, files: dict[str, str]) -> None:
        for path, content in files.items():
            full = self.destination / path
            full.parent.mkdir(parents=True, exist_ok=True)
            full.write_text(content)

    def recorded_inventory(self) -> dict[str, str]:
        return {path: blob_id(content, self.source_repo) for path, content in self.FILES.items()}

    def handoff_text(self, *, accepted_sha: str | None = None, verdict: str = "accepted",
                      acceptance: str = "accepted", include_review: bool = True) -> str:
        accepted_sha = accepted_sha if accepted_sha is not None else self.source_commit
        lines = [f"Accepted source SHA: `{accepted_sha}`"]
        if include_review:
            lines.append(f"Review document: `docs/reviews/btit-critical-review.md` at commit `{REVIEW_COMMIT_PLACEHOLDER}`")
        lines.append(f"Verdict: {verdict}")
        lines.append(f"sc-observability acceptance: {acceptance}")
        return "\n".join(lines) + "\n"

    def provenance(self, *, source_commit: str | None = None, inventory: dict[str, str] | None = None,
                    adaptations: list[dict[str, str]] | None = None) -> dict:
        return {
            "repository_url": "https://example.invalid/beads-task-issue-tracker.git",
            "source_commit": source_commit if source_commit is not None else self.source_commit,
            "target_contract_commit": REVIEW_COMMIT_PLACEHOLDER,
            "handoff_revision": "handoff-b-p3.md@synthetic",
            "review_document": {"path": "docs/reviews/btit-critical-review.md", "commit": REVIEW_COMMIT_PLACEHOLDER},
            "review_verdict": "accepted",
            "file_inventory": inventory if inventory is not None else self.recorded_inventory(),
            "adaptations": adaptations or [],
        }


class ValidateLogImportTests(unittest.TestCase):
    def test_accepts_valid_immutable_fixture(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            validate_import(fixture.provenance(), fixture.source_repo, fixture.destination, fixture.handoff_text())

    def test_accepts_declared_adaptation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/Cargo.toml"
            (fixture.destination / path).write_text('[package]\nname = "sc-observability-log"\npublish = false\n')
            provenance = fixture.provenance(adaptations=[{"path": path, "reason": "set publish = false for staging"}])
            validate_import(provenance, fixture.source_repo, fixture.destination, fixture.handoff_text())

    def test_rejects_missing_handoff_acceptance(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "missing or unaccepted B.P3 handoff"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(acceptance="pending"),
                )

    def test_rejects_missing_review_document_citation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "missing review-document path/immutable commit"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(include_review=False),
                )

    def test_rejects_source_repo_head_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            stale_sha = "2" * 40
            provenance = fixture.provenance(source_commit=stale_sha)
            with self.assertRaisesRegex(SystemExit, "different source commit than the source repository HEAD"):
                validate_import(
                    provenance, fixture.source_repo, fixture.destination,
                    fixture.handoff_text(accepted_sha=stale_sha),
                )

    def test_rejects_handoff_provenance_sha_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "handoff and provenance source SHA mismatch"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(accepted_sha="3" * 40),
                )

    def test_rejects_extra_file_in_source_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            del inventory["crates/sc-observability-log-consumer-check/Cargo.toml"]
            with self.assertRaisesRegex(SystemExit, "unexplained extra file.*source repository"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(),
                )

    def test_rejects_omitted_file_from_source_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["crates/sc-observability-log/src/extra_recorded_only.rs"] = blob_id("ghost\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "omitted file.*source repository"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(),
                )

    def test_rejects_undeclared_content_difference_in_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            (fixture.destination / "crates/sc-observability-log/src/lib.rs").write_text("pub fn changed() {}\n")
            with self.assertRaisesRegex(SystemExit, "unexplained content difference in the destination tree"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination, fixture.handoff_text())

    def test_rejects_omitted_file_from_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            (fixture.destination / "crates/sc-observability-log/src/lib.rs").unlink()
            with self.assertRaisesRegex(SystemExit, "omitted file.*destination tree"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination, fixture.handoff_text())

    def test_rejects_escaping_path_in_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["../../etc/passwd"] = blob_id("root:x:0:0\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "escaping path"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(),
                )

    def test_rejects_out_of_scope_path_in_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["crates/sc-observability/src/lib.rs"] = blob_id("pub fn x() {}\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "escaping path"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(),
                )


if __name__ == "__main__":
    unittest.main()
