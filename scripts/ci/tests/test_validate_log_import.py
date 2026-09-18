#!/usr/bin/env python3
"""Fixture coverage for the B.1 immutable import/acceptance contract validator.

Every fixture here is synthetic (temporary Git repositories with honest,
made-up review records) -- this task builds and proves the tool ahead of
B.1's real copy, and never fabricates a production import-provenance.json
or handoff-b-p3.md.

Two separate synthetic repositories model the two real-world owners: the
review document is BTIT's own acceptance record and lives in `source_repo`;
the target document and the handoff revision are sc-observability's own
documents and live in `doc_repo`.
"""

from __future__ import annotations

import difflib
import subprocess
import json
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from validate_log_import import validate_import, validate_qa_delta_review_citation  # noqa: E402


def run(*args: str, cwd: Path) -> str:
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def blob_id(content: str, repo: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), "hash-object", "--stdin"],
        input=content, check=True, capture_output=True, text=True,
    ).stdout.strip()


class ImportContractFixture:
    """Builds a matched (source-repo/BTIT, doc-repo/sc-observability, destination) fixture set."""

    FILES = {
        "crates/sc-observability-log/Cargo.toml": '[package]\nname = "sc-observability-log"\n',
        "crates/sc-observability-log/src/lib.rs": (
            'pub fn noop() {}\n\n#[path = "tests/original.rs"]\nmod tests;\n'
        ),
        "crates/sc-observability-log-macros/Cargo.toml": (
            '[package]\nname = "sc-observability-log-macros"\n\n'
            '[dependencies]\nsc-observability-log = { version = "0.1.0" }\n'
        ),
        "crates/sc-observability-log-consumer-check/Cargo.toml": '[package]\nname = "consumer-check"\n',
        "crates/sc-observability-log-consumer-check/src/main.rs": "fn main() {}\n",
        "crates/sc-observability-log/src/control.rs": "pub fn control() {}\n",
        "crates/sc-observability-log/src/handle.rs": "pub fn handle() {}\n",
        "crates/sc-observability-log/src/mapping.rs": "pub fn mapping() {}\n",
        "crates/sc-observability-log/tests/ui/rejected.stderr": (
            "error[E0308]: mismatched types\n"
            " --> tests/ui/rejected.rs:3:5\n"
            "  |\n"
            "3 |     5\n"
            "  |     ^ expected `()`, found integer\n"
        ),
    }

    review_path = "docs/reviews/btit-critical-review.md"
    handoff_path = "docs/plans/phase-b/handoff-b-p3.md"

    def __init__(self, root: Path) -> None:
        self.root = root
        self.source_repo = root / "source-repo"  # BTIT
        self.destination = root / "destination"
        self.doc_repo = root / "doc-repo"  # sc-observability
        self.source_repo.mkdir()
        self.destination.mkdir()
        self.doc_repo.mkdir()
        self._init_source_repo()
        self._write_destination(self.FILES)
        self.write_review(self.source_commit, "accepted")
        self._init_doc_repo()
        self.handoff_revision = self.commit_handoff(self.handoff_text())

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

    def write_review(self, reviewed_commit: str, verdict: str) -> str:
        """Commit a review document (BTIT's own record) citing `reviewed_commit`/`verdict`."""
        review = self.source_repo / self.review_path
        review.parent.mkdir(parents=True, exist_ok=True)
        review.write_text(f"BTIT source review\n\nReviewed commit: {reviewed_commit}\nVerdict: {verdict}\n")
        run("git", "add", "-A", cwd=self.source_repo)
        run("git", "commit", "--quiet", "--allow-empty", "-m", "review document", cwd=self.source_repo)
        commit = run("git", "rev-parse", "HEAD", cwd=self.source_repo)
        self.review_commit = commit
        return commit

    target_path = "docs/plans/phase-b/runtime-level-contract.md"

    def _init_doc_repo(self) -> None:
        self._init_git_repo(self.doc_repo)
        contract = self.doc_repo / self.target_path
        contract.parent.mkdir(parents=True, exist_ok=True)
        contract.write_text("Target contract accepted for staging.\n")
        run("git", "add", "-A", cwd=self.doc_repo)
        run("git", "commit", "--quiet", "-m", "target contract accepted", cwd=self.doc_repo)
        self.target_commit = run("git", "rev-parse", "HEAD", cwd=self.doc_repo)

    def commit_orphan_doc_file(self, path: str, content: str) -> str:
        """Commit a true orphan commit into doc_repo containing only `path`; returns the SHA.

        A real, existing commit with no ancestry to the branch history and no
        other content -- the exact bypass shape aobs reported (an orphan
        commit that exists but does not hold the target document). Direct
        commit lookup (`git cat-file -e <sha>^{commit}`) finds it regardless
        of branch reachability, so no ref needs to retain it afterward.
        """
        branch = run("git", "symbolic-ref", "--short", "HEAD", cwd=self.doc_repo)
        run("git", "checkout", "--quiet", "--orphan", "tmp-orphan-doc", cwd=self.doc_repo)
        run("git", "rm", "-r", "--cached", "--quiet", ".", cwd=self.doc_repo)
        for existing in self.doc_repo.iterdir():
            if existing.name != ".git":
                if existing.is_dir():
                    for f in existing.rglob("*"):
                        if f.is_file():
                            f.unlink()
                else:
                    existing.unlink()
        full = self.doc_repo / path
        full.parent.mkdir(parents=True, exist_ok=True)
        full.write_text(content)
        run("git", "add", "-A", cwd=self.doc_repo)
        run("git", "commit", "--quiet", "-m", "orphan unrelated file", cwd=self.doc_repo)
        orphan_commit = run("git", "rev-parse", "HEAD", cwd=self.doc_repo)
        run("git", "checkout", "--quiet", branch, cwd=self.doc_repo)
        run("git", "branch", "--quiet", "-D", "tmp-orphan-doc", cwd=self.doc_repo)
        return orphan_commit

    def commit_handoff(self, text: str) -> str:
        """Commit `text` as the handoff document (sc-observability's own record); returns 'path@sha'."""
        handoff_file = self.doc_repo / self.handoff_path
        handoff_file.parent.mkdir(parents=True, exist_ok=True)
        handoff_file.write_text(text)
        run("git", "add", "-A", cwd=self.doc_repo)
        run("git", "commit", "--quiet", "--allow-empty", "-m", "handoff revision", cwd=self.doc_repo)
        commit = run("git", "rev-parse", "HEAD", cwd=self.doc_repo)
        return f"{self.handoff_path}@{commit}"

    def _write_destination(self, files: dict[str, str]) -> None:
        for path, content in files.items():
            full = self.destination / path
            full.parent.mkdir(parents=True, exist_ok=True)
            full.write_text(content)

    def init_destination_repo(self) -> None:
        self._init_git_repo(self.destination)

    def commit_destination_paths(self, *paths: str) -> str:
        if paths:
            run("git", "add", "--", *paths, cwd=self.destination)
        else:
            run("git", "add", "-A", cwd=self.destination)
        run("git", "commit", "--quiet", "-m", "approved QA delta", cwd=self.destination)
        return run("git", "rev-parse", "HEAD", cwd=self.destination)

    def qa_delta_adaptation(self, path: str, after: str, commit: str) -> dict:
        before = self.FILES[path]
        patch = "".join(
            difflib.unified_diff(
                before.splitlines(keepends=True),
                after.splitlines(keepends=True),
                fromfile=f"accepted/{path}",
                tofile=f"approved/{commit}/{path}",
            )
        )
        return {
            "path": path,
            "kind": "approved_qa_delta",
            "reason": "approved QA delta in an exact immutable fixture",
            "before_blob": blob_id(before, self.source_repo),
            "after_blob": blob_id(after, self.destination),
            "blocks": [],
            "qa_delta": {
                "commit": commit,
                "reason": "approved QA delta in an exact immutable fixture",
                "review_citation": {
                    "url": "https://github.com/randlee/sc-observability/pull/148#issuecomment-5724132637",
                    "reviewed_commit": "5bcc27ee8f228d396bb603fc7dfc1a1c65861fe0",
                },
                "patch": patch,
            },
        }

    def recorded_inventory(self) -> dict[str, str]:
        return {path: blob_id(content, self.source_repo) for path, content in self.FILES.items()}

    def post_import_adaptations(self) -> dict:
        blocks = {
            "crates/sc-observability-log/src/control.rs": (
                "#[allow(\n"
                "    deprecated,\n"
                "    reason = \"copied bridge compatibility boundary\"\n"
                ")]"
            ),
            "crates/sc-observability-log/src/handle.rs": (
                "#[allow(\n"
                "    deprecated,\n"
                "    reason = \"copied bridge lifecycle boundary\"\n"
                ")]"
            ),
            "crates/sc-observability-log/src/mapping.rs": (
                "#[allow(\n"
                "    deprecated,\n"
                "    reason = \"copied bridge identity boundary\"\n"
                ")]"
            ),
        }
        adaptations = []
        for path, block in blocks.items():
            before = self.FILES[path]
            after = f"{block}\n{before}"
            (self.destination / path).write_text(after)
            adaptations.append({
                "path": path,
                "kind": "deprecated_warning_allowance",
                "reason": "retain the copied bridge's legacy compatibility boundary",
                "before_blob": blob_id(before, self.source_repo),
                "after_blob": blob_id(after, self.source_repo),
                "blocks": [block],
            })
        return {
            "historical_provenance": "docs/plans/phase-b/import-provenance.json",
            "source_commit": self.source_commit,
            "adaptations": adaptations,
        }

    def handoff_text(self, *, accepted_sha: str | None = None, verdict: str = "accepted",
                      acceptance: str = "accepted", include_review: bool = True,
                      review_path: str | None = None, review_commit: str | None = None,
                      include_target: bool = True, target_path: str | None = None,
                      target_commit: str | None = None) -> str:
        accepted_sha = accepted_sha if accepted_sha is not None else self.source_commit
        lines = [f"Accepted source SHA: `{accepted_sha}`"]
        if include_review:
            rp = review_path if review_path is not None else self.review_path
            rc = review_commit if review_commit is not None else self.review_commit
            lines.append(f"Review document: `{rp}` at commit `{rc}`")
        if include_target:
            tp = target_path if target_path is not None else self.target_path
            tc = target_commit if target_commit is not None else self.target_commit
            lines.append(f"Target document: `{tp}` at commit `{tc}`")
        lines.append(f"Verdict: {verdict}")
        lines.append(f"sc-observability acceptance: {acceptance}")
        return "\n".join(lines) + "\n"

    def provenance(self, *, source_commit: str | None = None, inventory: dict[str, str] | None = None,
                    adaptations: list[dict[str, str]] | None = None, review_path: str | None = None,
                    review_commit: str | None = None, target_path: str | None = None,
                    target_commit: str | None = None,
                    review_verdict: str = "accepted", handoff_revision: str | None = None) -> dict:
        return {
            "repository_url": "https://example.invalid/beads-task-issue-tracker.git",
            "source_commit": source_commit if source_commit is not None else self.source_commit,
            "target_document": {
                "path": target_path if target_path is not None else self.target_path,
                "commit": target_commit if target_commit is not None else self.target_commit,
            },
            "handoff_revision": handoff_revision if handoff_revision is not None else self.handoff_revision,
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

    def test_release_and_warning_adaptations_compose_without_weakening_inventory(self) -> None:
        from _log_staging import PACKAGES
        from _log_release_adaptations import blob
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            warnings = fixture.post_import_adaptations()
            license_bytes = b"synthetic MIT license\n"
            (fixture.destination / "LICENSE").write_bytes(license_bytes)
            record = {"schema_version": 1, "candidate_version": "1.4.0",
                      "root_license_sha256": hashlib.sha256(license_bytes).hexdigest(),
                      "license_copies": {}, "publish_flags": {}}
            for name in PACKAGES:
                path = f"crates/{name}/LICENSE"
                target = fixture.destination / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(license_bytes)
                record["license_copies"][path] = blob(license_bytes)
            adaptations = []
            for name in ("sc-observability-log", "sc-observability-log-macros"):
                path = f"crates/{name}/Cargo.toml"
                original = fixture.FILES[path]
                before = original.replace("[package]\n", "[package]\npublish = false\n")
                after = before.replace("publish = false", "publish = true")
                adaptations.append({"path": path, "reason": "private mechanical import",
                                    "kind": "package_metadata", "before": original, "after": before})
                (fixture.destination / path).write_text(after)
                record["publish_flags"][path] = {"before_blob": blob(before.encode()), "after_blob": blob(after.encode())}
            record_path = fixture.root / "release.json"
            record_path.write_text(json.dumps(record))
            kwargs = {"doc_repo": fixture.doc_repo, "post_import_adaptations": warnings,
                      "release_adaptations": record_path}
            args = (fixture.provenance(adaptations=adaptations), fixture.source_repo,
                    fixture.destination, fixture.handoff_text())
            validate_import(*args, **kwargs)
            (fixture.destination / "crates/sc-observability-log/src/unrecorded.rs").write_text("unrecorded")
            with self.assertRaisesRegex(SystemExit, "unexplained extra"):
                validate_import(*args, **kwargs)

    def test_accepts_separate_post_import_warning_adaptations(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            post_import = fixture.post_import_adaptations()
            validate_import(
                fixture.provenance(),
                fixture.source_repo,
                fixture.destination,
                fixture.handoff_text(),
                doc_repo=fixture.doc_repo,
                post_import_adaptations=post_import,
            )

    def test_accepts_qa_delta_bound_to_recorded_after_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [fixture.qa_delta_adaptation(path, after, commit)],
            }
            validate_import(
                fixture.provenance(), fixture.source_repo, fixture.destination,
                fixture.handoff_text(), doc_repo=fixture.doc_repo,
                post_import_adaptations=post_import,
            )

    def test_accepts_recorded_real_qa_delta_review_citations(self) -> None:
        manifest = json.loads(
            (Path(__file__).resolve().parents[3] / "docs/plans/phase-b/post-import-adaptations.json").read_text()
        )
        qa_deltas = [
            item["qa_delta"]
            for item in manifest["adaptations"]
            if "qa_delta" in item
        ]
        self.assertEqual(len(qa_deltas), 4)
        for item in qa_deltas:
            validate_qa_delta_review_citation(item, "real-manifest-fixture")

    def test_rejects_missing_qa_delta_review_citation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            adaptation = fixture.qa_delta_adaptation(path, after, commit)
            del adaptation["qa_delta"]["review_citation"]
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [adaptation],
            }
            with self.assertRaisesRegex(SystemExit, "lacks review citation"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_malformed_qa_delta_review_citation_url(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            adaptation = fixture.qa_delta_adaptation(path, after, commit)
            adaptation["qa_delta"]["review_citation"]["url"] = "https://example.invalid/review"
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [adaptation],
            }
            with self.assertRaisesRegex(SystemExit, "invalid review citation URL"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_malformed_qa_delta_reviewed_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            adaptation = fixture.qa_delta_adaptation(path, after, commit)
            adaptation["qa_delta"]["review_citation"]["reviewed_commit"] = "not-a-commit"
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [adaptation],
            }
            with self.assertRaisesRegex(SystemExit, "invalid reviewed commit citation"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_qa_delta_existing_unrelated_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            unrelated = fixture.destination / "unrelated.txt"
            unrelated.write_text("unrelated commit content\n")
            commit = fixture.commit_destination_paths("unrelated.txt")
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [fixture.qa_delta_adaptation(path, after, commit)],
            }
            with self.assertRaisesRegex(SystemExit, "does not contain the recorded after file"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_qa_delta_altered_patch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            (fixture.destination / path).write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            adaptation = fixture.qa_delta_adaptation(path, after, commit)
            adaptation["qa_delta"]["patch"] += "tampered\n"
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [adaptation],
            }
            with self.assertRaisesRegex(SystemExit, "does not match exact result evidence"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_qa_delta_changed_after_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/control.rs"
            after = fixture.FILES[path] + "pub fn approved_delta() {}\n"
            destination_path = fixture.destination / path
            destination_path.write_text(after)
            fixture.init_destination_repo()
            commit = fixture.commit_destination_paths(path)
            adaptation = fixture.qa_delta_adaptation(path, after, commit)
            destination_path.write_text(after + "pub fn changed_after_recording() {}\n")
            post_import = {
                "historical_provenance": "docs/plans/phase-b/import-provenance.json",
                "source_commit": fixture.source_commit,
                "adaptations": [adaptation],
            }
            with self.assertRaisesRegex(SystemExit, "after blob"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_post_import_body_change(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            post_import = fixture.post_import_adaptations()
            path = "crates/sc-observability-log/src/control.rs"
            (fixture.destination / path).write_text(
                (fixture.destination / path).read_text().replace("control()", "control_changed()")
            )
            with self.assertRaisesRegex(SystemExit, "after blob"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_post_import_signature_change(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            post_import = fixture.post_import_adaptations()
            path = "crates/sc-observability-log/src/handle.rs"
            (fixture.destination / path).write_text(
                (fixture.destination / path).read_text().replace("pub fn handle()", "pub fn handle(extra: usize)")
            )
            with self.assertRaisesRegex(SystemExit, "after blob"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_rejects_post_import_undeclared_file_change(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            post_import = fixture.post_import_adaptations()
            (fixture.destination / "crates/sc-observability-log/src/lib.rs").write_text(
                "pub fn noop() { /* undeclared adaptation */ }\n\n#[path = \"tests/original.rs\"]\nmod tests;\n"
            )
            with self.assertRaisesRegex(SystemExit, "destination tree.*src/lib.rs"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                    post_import_adaptations=post_import,
                )

    def test_accepts_declared_package_metadata_workspace_inheritance(self) -> None:
        """Converting hard-coded [package] fields to `.workspace = true` is a permitted metadata change."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/Cargo.toml"
            before = fixture.FILES[path]
            after = '[package]\nname = "sc-observability-log"\nversion.workspace = true\nrepository.workspace = true\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path,
                "reason": "inherit version/repository from this workspace",
                "kind": "package_metadata",
                "before": before,
                "after": after,
            }])
            validate_import(provenance, fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_accepts_declared_trybuild_diagnostic_text_change(self) -> None:
        """A toolchain-drift .stderr rewording is permitted when codes/locations are unchanged."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/tests/ui/rejected.stderr"
            before = fixture.FILES[path]
            after = (
                "error[E0308]: mismatched types\n"
                " --> tests/ui/rejected.rs:3:5\n"
                "  |\n"
                "3 |     5\n"
                "  |     ^ expected `()`, found integer value\n"
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path,
                "reason": "this workspace pins a different rustc than BTIT's; rustc wording drifted",
                "kind": "trybuild_diagnostic_text",
                "before": before,
                "after": after,
            }])
            validate_import(provenance, fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_trybuild_diagnostic_text_kind_on_non_stderr_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/lib.rs"
            before = fixture.FILES[path]
            after = "pub fn noop() { /* changed */ }\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "not actually a trybuild fixture", "kind": "trybuild_diagnostic_text",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "is not a tests/ui/\\*.stderr file"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_trybuild_diagnostic_text_empty_profile(self) -> None:
        """A stray non-diagnostic .stderr file must not launder this kind."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/tests/ui/rejected.stderr"
            before = fixture.FILES[path]
            after = "note: nothing to see here\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "empty it out", "kind": "trybuild_diagnostic_text",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "empty diagnostic error/location profile"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_trybuild_diagnostic_text_changes_error_code(self) -> None:
        """A kind label must not launder a change to which diagnostic actually fired."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/tests/ui/rejected.stderr"
            before = fixture.FILES[path]
            after = (
                "error[E0599]: no method named `foo` found\n"
                " --> tests/ui/rejected.rs:3:5\n"
                "  |\n"
                "3 |     5\n"
                "  |     ^ expected `()`, found integer\n"
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "swap the diagnostic", "kind": "trybuild_diagnostic_text",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "changes the diagnostic error codes"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_trybuild_diagnostic_text_changes_location(self) -> None:
        """A kind label must not launder a change to where the diagnostic points."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/tests/ui/rejected.stderr"
            before = fixture.FILES[path]
            after = (
                "error[E0308]: mismatched types\n"
                " --> tests/ui/rejected.rs:99:1\n"
                "  |\n"
                "3 |     5\n"
                "  |     ^ expected `()`, found integer\n"
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "move the location", "kind": "trybuild_diagnostic_text",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "changes the diagnostic source locations"):
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

    def test_rejects_symlinked_directory_in_destination_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            outside = fixture.root / "outside-dir"
            outside.mkdir()
            (outside / "sneaky.rs").write_text("pub fn sneaky() {}\n")
            link_dir = fixture.destination / "crates/sc-observability-log/src/extra_dir"
            link_dir.symlink_to(outside, target_is_directory=True)
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

    def test_rejects_fake_review_commit_not_in_source_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fake_commit = "1" * 40
            with self.assertRaisesRegex(SystemExit, "review document commit not found in source repository"):
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

    def test_rejects_review_verdict_mismatch_against_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            differently_worded_commit = fixture.write_review(fixture.source_commit, "conditionally-accepted")
            with self.assertRaisesRegex(SystemExit, "verdict does not match"):
                validate_import(
                    fixture.provenance(review_commit=differently_worded_commit, review_verdict="accepted"),
                    fixture.source_repo, fixture.destination,
                    fixture.handoff_text(review_commit=differently_worded_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_matching_rejected_review_and_provenance(self) -> None:
        """A self-consistent rejected verdict must still fail: acceptance is required, not internal agreement."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            rejected_commit = fixture.write_review(fixture.source_commit, "rejected")
            with self.assertRaisesRegex(SystemExit, "review_verdict must be accepted"):
                validate_import(
                    fixture.provenance(review_commit=rejected_commit, review_verdict="rejected"),
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

    def test_rejects_target_document_commit_not_found(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fake_commit = "7" * 40
            with self.assertRaisesRegex(SystemExit, "target document commit not found in doc repository"):
                validate_import(
                    fixture.provenance(target_commit=fake_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(target_commit=fake_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_target_document_missing_at_cited_commit(self) -> None:
        """A real commit that exists but does not hold the target document must not pass."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            orphan_commit = fixture.commit_orphan_doc_file(
                "unrelated.txt", "nothing to do with the target contract\n",
            )
            with self.assertRaisesRegex(SystemExit, "target document not found at cited commit"):
                validate_import(
                    fixture.provenance(target_commit=orphan_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(target_commit=orphan_commit), doc_repo=fixture.doc_repo,
                )

    def test_rejects_missing_target_document_citation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "missing target-document path/immutable commit"):
                validate_import(
                    fixture.provenance(), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(include_target=False), doc_repo=fixture.doc_repo,
                )

    def test_rejects_handoff_provenance_target_citation_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            orphan_commit = fixture.commit_orphan_doc_file("unrelated2.txt", "still unrelated\n")
            with self.assertRaisesRegex(SystemExit, "target-document citation mismatch"):
                validate_import(
                    fixture.provenance(target_commit=orphan_commit), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_fake_handoff_revision(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            with self.assertRaisesRegex(SystemExit, "handoff_revision must be"):
                validate_import(
                    fixture.provenance(handoff_revision="docs/plans/phase-b/handoff-b-p3.md@synthetic"),
                    fixture.source_repo, fixture.destination, fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_handoff_revision_commit_not_in_doc_repo(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            fake_revision = f"{fixture.handoff_path}@{'8' * 40}"
            with self.assertRaisesRegex(SystemExit, "handoff_revision commit not found in doc repository"):
                validate_import(
                    fixture.provenance(handoff_revision=fake_revision), fixture.source_repo, fixture.destination,
                    fixture.handoff_text(), doc_repo=fixture.doc_repo,
                )

    def test_rejects_handoff_revision_content_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            different_valid_text = fixture.handoff_text() + "\n"
            stale_revision = fixture.commit_handoff(different_valid_text)
            with self.assertRaisesRegex(SystemExit, "does not match the provided --handoff"):
                validate_import(
                    fixture.provenance(handoff_revision=stale_revision), fixture.source_repo, fixture.destination,
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

    def test_rejects_arbitrary_runtime_change_labeled_dependency_path(self) -> None:
        """A kind label alone must not launder an arbitrary content change."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-consumer-check/Cargo.toml"
            before = fixture.FILES[path]
            after = "totally different manifest content entirely\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "relocate dependency", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "not a permitted dependency_path mechanical change"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_accepts_declared_dependency_path_relocation(self) -> None:
        """A genuine relocation retargets an *existing* dependency's location key only."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-macros/Cargo.toml"
            before = fixture.FILES[path]
            after = before.replace(
                'sc-observability-log = { version = "0.1.0" }',
                'sc-observability-log = { path = "../sc-observability-log" }',
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "point at the staged sibling crate", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            validate_import(provenance, fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_dependency_path_adds_new_dependency_table(self) -> None:
        """A kind label must not launder appending a whole new dependency table."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-consumer-check/Cargo.toml"
            before = fixture.FILES[path]
            after = before.rstrip("\n") + (
                '\n\n[dependencies]\n'
                'new_runtime_dependency = { version = "99", features = ["change_behavior"] }\n'
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "add dependency", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "adds or removes a dependency table"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_dependency_path_adds_new_dependency_entry(self) -> None:
        """A kind label must not launder appending a new entry to an existing table."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-macros/Cargo.toml"
            before = fixture.FILES[path]
            after = before.rstrip("\n") + '\nnew_runtime_dependency = { version = "99", features = ["change_behavior"] }\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "add dependency", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "adds or removes a dependency entry"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_dependency_path_changes_non_location_keys(self) -> None:
        """Relocating a dependency must preserve its other keys (features, etc.)."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-macros/Cargo.toml"
            before = fixture.FILES[path]
            after = before.replace(
                'sc-observability-log = { version = "0.1.0" }',
                'sc-observability-log = { path = "../sc-observability-log", features = ["extra"] }',
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "point at the staged sibling crate", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "non-location dependency keys"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_dependency_path_kind_on_non_cargo_toml_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log-consumer-check/src/main.rs"
            before = fixture.FILES[path]
            after = "fn main() { println!(); }\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "not actually a manifest", "kind": "dependency_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "is not a Cargo.toml file"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_relocated_kind_arbitrary_code_containing_path_substring(self) -> None:
        """A changed line merely containing '.rs' inside a string literal must not launder arbitrary code."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/lib.rs"
            before = fixture.FILES[path]
            after = before + 'pub fn bypass() { panic!("behavior.rs"); }\n'
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "relocate test path reference", "kind": "relocated_doc_or_test_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(
                SystemExit, "not a permitted relocated_doc_or_test_path mechanical change",
            ):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_accepts_declared_relocated_doc_or_test_path(self) -> None:
        """A genuine relocation retargets an *existing* mod's #[path] attribute only."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/lib.rs"
            before = fixture.FILES[path]
            after = before.replace(
                '#[path = "tests/original.rs"]',
                '#[path = "tests/relocated.rs"]',
            )
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "relocate test module path", "kind": "relocated_doc_or_test_path",
                "before": before, "after": after,
            }])
            validate_import(provenance, fixture.source_repo, fixture.destination,
                             fixture.handoff_text(), doc_repo=fixture.doc_repo)

    def test_rejects_relocated_kind_adds_new_mod_declaration(self) -> None:
        """A kind label must not launder appending a whole new mod declaration."""
        with tempfile.TemporaryDirectory() as temp:
            fixture = ImportContractFixture(Path(temp))
            path = "crates/sc-observability-log/src/lib.rs"
            before = fixture.FILES[path]
            after = before.rstrip("\n") + "\nmod new_runtime_module;\n"
            (fixture.destination / path).write_text(after)
            provenance = fixture.provenance(adaptations=[{
                "path": path, "reason": "relocate test path reference", "kind": "relocated_doc_or_test_path",
                "before": before, "after": after,
            }])
            with self.assertRaisesRegex(SystemExit, "adds or removes a mod declaration"):
                validate_import(provenance, fixture.source_repo, fixture.destination,
                                 fixture.handoff_text(), doc_repo=fixture.doc_repo)


if __name__ == "__main__":
    unittest.main()
