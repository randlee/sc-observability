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


def run(*args: str, cwd: Path) -> str:
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def blob_id(content: str, repo: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), "hash-object", "--stdin"],
        input=content, check=True, capture_output=True, text=True,
    ).stdout.strip()


class ImportContractFixture:
    """Builds a matched (source-repo, doc-repo, destination, provenance, handoff) fixture set."""

    FILES = {
        "crates/sc-observability-log/Cargo.toml": '[package]\nname = "sc-observability-log"\n',
        "crates/sc-observability-log/src/lib.rs": "pub fn noop() {}\n",
        "crates/sc-observability-log-macros/Cargo.toml": '[package]\nname = "sc-observability-log-macros"\n',
        "crates/sc-observability-log-consumer-check/Cargo.toml": '[package]\nname = "consumer-check"\n',
        "crates/sc-observability-log-consumer-check/src/main.rs": "fn main() {}\n",
    }

    def __init__(self, root: Path) -> None:
        self.root = root
        self.source_repo = root / "source-repo"
        self.destination = root / "destination"
        self.doc_repo = root / "doc-repo"
        self.source_repo.mkdir()
        self.destination.mkdir()
        self.doc_repo.mkdir()
        self._init_source_repo()
        self._write_destination(self.FILES)
        self._init_doc_repo()

    def _init_git_repo(self, repo: Path) -> None:
        run("git", "init", "--quiet", cwd=repo)
        run("git", "config", "user.email", "fixture@example.invalid", cwd=repo)
        run("git", "config", "user.name", "fixture", cwd=repo)

    def _init_source_repo(self) -> None:
        self._init_git_repo(self.source_repo)
        for path, content in self.FILES.items():
            full = self.source_repo / path
            full.parent.mkdir(parents=True, exist_ok=True)
            full.write_text(content)
        run("git", "add", "-A", cwd=self.source_repo)
        run("git", "commit", "--quiet", "-m", "synthetic BTIT source", cwd=self.source_repo)
        self.source_commit = run("git", "rev-parse", "HEAD", cwd=self.source_repo)

    def advance_source_repo(self) -> None:
        """Add a later commit so HEAD moves past the accepted commit."""
        extra = self.source_repo / "crates/sc-observability-log/src/later.rs"
        extra.write_text("pub fn later() {}\n")
        run("git", "add", "-A", cwd=self.source_repo)
        run("git", "commit", "--quiet", "-m", "later unrelated change", cwd=self.source_repo)

    def _init_doc_repo(self) -> None:
        self._init_git_repo(self.doc_repo)
        contract = self.doc_repo / "docs/plans/phase-b/runtime-level-contract.md"
        contract.parent.mkdir(parents=True, exist_ok=True)
        contract.write_text("Target contract accepted for staging.\n")
        run("git", "add", "-A", cwd=self.doc_repo)
        run("git", "commit", "--quiet", "-m", "target contract accepted", cwd=self.doc_repo)
        self.target_commit = run("git", "rev-parse", "HEAD", cwd=self.doc_repo)
        self.review_path = "docs/reviews/btit-critical-review.md"
        self.write_review(self.source_commit, "accepted")

    def write_review(self, reviewed_commit: str, verdict: str) -> str:
        """Commit a review document citing `reviewed_commit`/`verdict`; returns its commit SHA."""
        review = self.doc_repo / self.review_path
        review.parent.mkdir(parents=True, exist_ok=True)
        review.write_text(f"BTIT source review\n\nReviewed commit: {reviewed_commit}\nVerdict: {verdict}\n")
        run("git", "add", "-A", cwd=self.doc_repo)
        run("git", "commit", "--quiet", "--allow-empty", "-m", "review document", cwd=self.doc_repo)
        commit = run("git", "rev-parse", "HEAD", cwd=self.doc_repo)
        self.review_commit = commit
        return commit

    def _write_destination(self, files: dict[str, str]) -> None:
        for path, content in files.items():
            full = self.destination / path
            full.parent.mkdir(parents=True, exist_ok=True)
            full.write_text(content)

    def recorded_inventory(self) -> dict[str, str]:
        return {path: blob_id(content, self.source_repo) for path, content in self.FILES.items()}

    def handoff_text(self, *, accepted_sha: str | None = None, verdict: str = "accepted",
                      acceptance: str = "accepted", include_review: bool = True,
                      review_path: str | None = None, review_commit: str | None = None) -> str:
        accepted_sha = accepted_sha if accepted_sha is not None else self.source_commit
        lines = [f"Accepted source SHA: `{accepted_sha}`"]
        if include_review:
            rp = review_path if review_path is not None else self.review_path
            rc = review_commit if review_commit is not None else self.review_commit
            lines.append(f"Review document: `{rp}` at commit `{rc}`")
        lines.append(f"Verdict: {verdict}")
        lines.append(f"sc-observability acceptance: {acceptance}")
        return "\n".join(lines) + "\n"

    def provenance(self, *, source_commit: str | None = None, inventory: dict[str, str] | None = None,
                    adaptations: list[dict[str, str]] | None = None, review_path: str | None = None,
                    review_commit: str | None = None, target_contract_commit: str | None = None,
                    review_verdict: str = "accepted") -> dict:
        return {
            "repository_url": "https://example.invalid/beads-task-issue-tracker.git",
            "source_commit": source_commit if source_commit is not None else self.source_commit,
            "target_contract_commit": target_contract_commit if target_contract_commit is not None else self.target_commit,
            "handoff_revision": "handoff-b-p3.md@synthetic",
            "review_document": {
                "path": review_path if review_path is not None else self.review_path,
                "commit": review_commit if review_commit is not None else self.review_commit,
            },
            "review_verdict": review_verdict,
            "file_inventory": inventory if inventory is not None else self.recorded_inventory(),
            "adaptations": adaptations or [],
        }


class ValidateLogImportTests(unittest.TestCase):
    def test_accepts_valid_immutable_fixture(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_accepts_declared_adaptation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/Cargo.toml"
            before = fixture.FILES[path]
            after = '[package]\nname = "sc-observability-log"\npublish = false\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path,
                "reason": "set publish = false for staging",
                "kind": "package_metadata",
                "before": before,
                "after": after,
            }])
            validate_import(provenance, fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_accepts_when_repo_advances_past_accepted_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fixture.advance_source_repo()
            validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_missing_handoff_acceptance(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "missing or unaccepted B.P3 handoff"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(acceptance="pending"), doc_repo=fixture.doc_repo,
                )

    def test_rejects_missing_review_document_citation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "missing review-document path/immutable commit"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(include_review=False), doc_repo=fixture.doc_repo,
                )

    def test_rejects_nonexistent_source_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fake_sha = "4" * 40
            with self.assertRaisesRegex(SystemExit, "accepted source commit not found"):
                validate_import(
                    fixture.provenance(source_commit=fake_sha), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(accepted_sha=fake_sha), doc_repo=fixture.doc_repo,
                )

    def test_rejects_handoff_provenance_sha_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "handoff and provenance source SHA mismatch"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(accepted_sha="3" * 40), doc_repo=fixture.doc_repo,
                )

    def test_rejects_extra_file_in_source_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            del inventory["crates/sc-observability-log-consumer-check/Cargo.toml"]
            with self.assertRaisesRegex(SystemExit, "unexplained extra file.*source repository"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_omitted_file_from_source_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["crates/sc-observability-log/src/extra_recorded_only.rs"] = blob_id("ghost\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "omitted file.*source repository"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_empty_crate_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            del inventory["crates/sc-observability-log-macros/Cargo.toml"]
            with self.assertRaisesRegex(SystemExit, "no files for required crate"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_undeclared_content_difference_in_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            (fixture.destination / "crates/sc-observability-log/src/lib.rs").write_text("pub fn changed() {}\n")
            with self.assertRaisesRegex(SystemExit, "unexplained content difference in the destination tree"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_omitted_file_from_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            (fixture.destination / "crates/sc-observability-log/src/lib.rs").unlink()
            with self.assertRaisesRegex(SystemExit, "omitted file.*destination tree"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_unrecorded_extra_file_actually_present_in_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            extra = fixture.destination / "crates/sc-observability-log/src/undeclared.rs"
            extra.write_text("pub fn undeclared() {}\n")
            with self.assertRaisesRegex(SystemExit, "unexplained extra file.*destination tree"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_symlink_in_destination_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            link = fixture.destination / "crates/sc-observability-log/src/link.rs"
            link.symlink_to(fixture.destination / "crates/sc-observability-log/src/lib.rs")
            with self.assertRaisesRegex(SystemExit, "symlink not permitted"):
                validate_import(fixture.provenance(), fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_escaping_path_in_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["../../etc/passwd"] = blob_id("root:x:0:0\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "escaping path"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_out_of_scope_path_in_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            inventory = fixture.recorded_inventory()
            inventory["crates/sc-observability/src/lib.rs"] = blob_id("pub fn x() {}\n", fixture.source_repo)
            with self.assertRaisesRegex(SystemExit, "escaping path"):
                validate_import(
                    fixture.provenance(inventory=inventory), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_fake_review_commit_not_in_doc_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fake_commit = "1" * 40
            with self.assertRaisesRegex(SystemExit, "review document commit not found"):
                validate_import(
                    fixture.provenance(review_commit=fake_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(review_commit=fake_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_review_citation_source_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            wrong_commit = fixture.write_review("5" * 40, "accepted")
            with self.assertRaisesRegex(SystemExit, "does not cover the accepted source commit"):
                validate_import(
                    fixture.provenance(review_commit=wrong_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(review_commit=wrong_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_review_citation_verdict_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            rejected_commit = fixture.write_review(fixture.source_commit, "rejected")
            with self.assertRaisesRegex(SystemExit, "verdict does not match"):
                validate_import(
                    fixture.provenance(review_commit=rejected_commit, review_verdict="accepted"),
                    fixture.source_repo, fixture.destination,
                    fixture.handoff_text(review_commit=rejected_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_handoff_provenance_review_citation_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            original_commit = fixture.review_commit
            new_commit = fixture.write_review(fixture.source_commit, "accepted")
            with self.assertRaisesRegex(SystemExit, "review-document citation mismatch"):
                validate_import(
                    fixture.provenance(review_commit=new_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(review_commit=original_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_target_contract_commit_not_found(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "target contract commit not found"):
                validate_import(
                    fixture.provenance(target_contract_commit="7" * 40), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_adaptation_missing_reason(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/Cargo.toml"
            after = '[package]\nname = "sc-observability-log"\npublish = false\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "  ", "kind": "package_metadata",
                "before": fixture.FILES[path], "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "missing a reason"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_adaptation_disallowed_kind(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/lib.rs"
            after = "pub fn arbitrary_runtime_change() {}\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "swap runtime behavior", "kind": "runtime_behavior_change",
                "before": fixture.FILES[path], "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "not a permitted mechanical change"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_adaptation_before_content_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/Cargo.toml"
            after = '[package]\nname = "sc-observability-log"\npublish = false\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "set publish = false for staging", "kind": "package_metadata",
                "before": "this is not the recorded source content\n", "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "'before' content .* does not match the recorded source blob"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)


if __name__ == "__main__":
    unittest.main()
