from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest

MODULE = Path(__file__).parents[1] / "assignment-gates.py"
FIXTURES = Path(__file__).parent / "fixtures"
SPEC = importlib.util.spec_from_file_location("assignment_gates", MODULE)
assert SPEC and SPEC.loader
gates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gates)


class FakeRunner:
    def __init__(self, responses):
        self.responses = responses
        self.cwds = []
        self.calls = []
    def __call__(self, args, **kwargs):
        self.cwds.append(kwargs.get("cwd"))
        self.calls.append(tuple(args))
        response = self.responses.get(tuple(args), (0, ""))
        return subprocess.CompletedProcess(args, response[0], response[1], response[2] if len(response) > 2 else "")


def ns(kind, **overrides):
    values = dict(kind=kind, root="obs-phase-d", bead="bead", pr_target="target", identity="terra",
                  pr_number="7", commit="head", checked_bead="checked",
                  worktree="/wt", branch="fix/finding", stack_view="/installed/gh_stack_view.py", stack_report="")
    if kind != "sanity": values["worktree"] = ""
    values.update(overrides)
    return SimpleNamespace(**values)


def dumped(value): return json.dumps(value)


def dev_runner(overrides=None):
    data = {
        (gates.VALIDATE_PLAN, "--root", "obs-phase-d", "--scope", "bead"): (0, ""),
        ("bd", "ready", "-n", "0", "--json"): (0, dumped([{"id": "bead"}])),
        ("bd", "show", "bead", "--json"): (0, dumped([{"status": "open", "assignee": "", "metadata": {"difficulty": "normal", "pr_target": "target"}}])),
        ("atm", "members", "--json"): (0, dumped([{"identity": "terra", "model": "gpt-6-terra"}])),
        ("git", "merge-base", "--is-ancestor", "origin/target", "HEAD"): (0, ""),
    }; data.update(overrides or {}); return FakeRunner(data)


PR_COMMAND = ("gh", "pr", "view", "7", "--json", "baseRefName,headRefOid,headRefName,state")
VIEW_COMMAND = (sys.executable, "/installed/gh_stack_view.py", "--json")


def stack_report(**row_overrides):
    row = dict(pr=7, branch="fix/finding", pr_base="target", head="head", origin="head", pr_head="head",
               merged=False, origin_ok=True, base_ok=True, needs_rebase=False, mergeable="MERGEABLE",
               behind_trunk=False, ci="FAILURE")
    row.update(row_overrides)
    return {"stacks": [{"coherent": True, "problems": [], "rows": [row]}], "skipped_worktrees": []}


def sanity_runner(overrides=None, target="target"):
    report = stack_report(pr_base=target)
    data = {
        ("bd", "show", "checked", "--json"): (0, dumped([{"metadata": {"pr_target": target}}])),
        PR_COMMAND: (0, dumped({"baseRefName": target, "headRefOid": "head", "headRefName": "fix/finding", "state": "OPEN"})),
        VIEW_COMMAND: (0, dumped(report)),
        ("git", "fetch", "origin"): (0, ""),
        ("git", "rev-parse", "HEAD"): (0, "head"),
        ("git", "branch", "--show-current"): (0, "fix/finding"),
        ("git", "rev-parse", "origin/fix/finding"): (0, "head"),
        ("git", "log", "--format=%H", f"origin/{target}..head"): (0, "delta"),
        ("git", "status", "--porcelain", "--untracked-files=all"): (0, "?? .beads.gate.lock"),
        ("bd", "history", "bead", "--json"): (0, dumped({"events": []})),
    }; data.update(overrides or {}); return FakeRunner(data)


def qa_runner(overrides=None):
    data = {
        ("bd", "show", "bead", "--json"): (0, dumped([{"metadata": {"checked_bead": "checked", "pr_target": "target"}}])),
        ("bd", "show", "checked", "--json"): (0, dumped([{"metadata": {"sanity_pass_commit": "head"}}])),
        ("gh", "pr", "view", "7", "--json", "baseRefName,headRefOid"): (0, dumped({"baseRefName": "target", "headRefOid": "head"})),
        ("git", "rev-parse", "HEAD"): (0, "head"),
    }; data.update(overrides or {}); return FakeRunner(data)


class AssignmentGateTests(unittest.TestCase):
    def expected(self, name): return json.loads((FIXTURES / name).read_text())["expected"]

    def assert_fixture(self, name, gate_args, runner):
        code = gates.evaluate(gate_args, runner)
        self.assertEqual(code, self.expected(name))
        output = io.StringIO()
        argv = [gate_args.kind, "--root", gate_args.root, "--bead", gate_args.bead, "--pr-target", gate_args.pr_target,
                "--identity", gate_args.identity, "--pr-number", gate_args.pr_number, "--commit", gate_args.commit,
                "--checked-bead", gate_args.checked_bead, "--worktree", gate_args.worktree,
                "--branch", gate_args.branch, "--stack-view", gate_args.stack_view]
        with redirect_stdout(output): exit_code = gates.main(argv, runner)
        self.assertEqual(output.getvalue().strip(), code)
        self.assertEqual(exit_code, 0 if code == "READY" else 5)

    def test_dev_refusals_and_ready(self):
        cases = [
            ("plan-invalid.json", dev_runner({(gates.VALIDATE_PLAN, "--root", "obs-phase-d", "--scope", "bead"): (5, "bad")})),
            ("not-ready.json", dev_runner({("bd", "ready", "-n", "0", "--json"): (0, "[]")})),
            ("unclaimable.json", dev_runner({("bd", "show", "bead", "--json"): (0, dumped([{"status": "open", "assignee": "other", "metadata": {"difficulty": "normal"}}]))})),
            ("wrong-base.json", dev_runner({("git", "merge-base", "--is-ancestor", "origin/target", "HEAD"): (1, "")})),
            ("difficulty-mismatch.json", dev_runner({("bd", "show", "bead", "--json"): (0, dumped([{"status": "open", "assignee": "", "metadata": {"difficulty": "hard"}}]))})),
            ("dev-ready.json", dev_runner()),
        ]
        for fixture, runner in cases:
            with self.subTest(fixture=fixture): self.assert_fixture(fixture, ns("dev"), runner)

    def test_dev_gate_runs_validate_plan_in_primary_and_git_in_worktree(self):
        runner = dev_runner({("git", "-C", "/wt", "merge-base", "--is-ancestor", "origin/target", "HEAD"): (0, "")})
        self.assertEqual(gates.evaluate(ns("dev", worktree="/wt"), runner), "READY")
        self.assertEqual(runner.cwds[0], str(gates.PRIMARY))
        behind = dev_runner({("git", "-C", "/other", "merge-base", "--is-ancestor", "origin/target", "HEAD"): (1, "")})
        self.assertEqual(gates.evaluate(ns("dev", worktree="/other"), behind), "WRONG_BASE")

    def test_sanity_refusals_and_ready(self):
        cases = [
            ("pr-required.json", ns("sanity", pr_number=""), sanity_runner()),
            ("stale-base.json", ns("sanity"), sanity_runner({PR_COMMAND: (0, dumped({"baseRefName": "wrong", "headRefOid": "head"}))})),
            ("zero-delta.json", ns("sanity"), sanity_runner({("git", "log", "--format=%H", "origin/target..head"): (0, "")})),
            ("dirty-tree.json", ns("sanity"), sanity_runner({("git", "status", "--porcelain", "--untracked-files=all"): (0, " M tracked.py")})),
            ("sanity-frozen.json", ns("sanity"), sanity_runner({("bd", "history", "bead", "--json"): (0, dumped({"verdict": "PASS"}))})),
            ("sanity-ready.json", ns("sanity"), sanity_runner()),
        ]
        for fixture, gate_args, runner in cases:
            with self.subTest(fixture=fixture): self.assert_fixture(fixture, gate_args, runner)

    def test_dependent_pr_requires_one_canonical_view_and_known_stack(self):
        runner = sanity_runner()
        self.assertEqual(gates.evaluate(ns("sanity"), runner), "READY")
        self.assertEqual(runner.calls.count(VIEW_COMMAND), 1)
        for call, cwd in zip(runner.calls, runner.cwds):
            if call[0] in ("git", "gh", sys.executable):
                self.assertEqual(cwd, "/wt")
        empty = (2, "", "gh-stack-view: no open gh stack found. Stacks are discovered through worktrees.\n")
        cases = [
            (empty, "STACK_REQUIRED"),
            ((2, "", "gh-stack-view: authentication failed"), "STACK_UNVERIFIED"),
            ((0, "not-json"), "GATE_CANNOT_RUN"),
            ((0, dumped({"stacks": [None]})), "STACK_UNVERIFIED"),
            ((0, dumped({"stacks": [{"rows": [None]}]})), "STACK_UNVERIFIED"),
            ((0, dumped({"stacks": []})), "STACK_REQUIRED"),
            ((0, dumped(stack_report(base_ok=None))), "STACK_UNVERIFIED"),
            ((0, dumped(stack_report(pr_head="old"))), "STACK_INCOHERENT"),
            ((0, dumped(stack_report(pr=9))), "STACK_REQUIRED"),
            ((0, dumped(stack_report(needs_rebase=None))), "STACK_UNVERIFIED"),
            ((0, dumped(stack_report(mergeable="UNKNOWN"))), "STACK_UNVERIFIED"),
        ]
        for response, expected in cases:
            with self.subTest(response=response):
                self.assertEqual(gates.evaluate(ns("sanity"), sanity_runner({VIEW_COMMAND: response})), expected)

    def test_stack_problems_and_unknown_lower_rows_refuse(self):
        for report in [
            {"stacks": [{"coherent": False, "problems": ["stale"], "rows": stack_report()["stacks"][0]["rows"]}]},
            {"stacks": [dict(stack_report()["stacks"][0], rows=[dict(stack_report()["stacks"][0]["rows"][0], pr=6, origin=None)] + stack_report()["stacks"][0]["rows"]) ]},
            dict(stack_report(), skipped_worktrees=["unreadable worktree"]),
        ]:
            with self.subTest(report=report):
                self.assertNotEqual(gates.evaluate(ns("sanity"), sanity_runner({VIEW_COMMAND: (1, dumped(report))})), "READY")

    def test_behind_moving_trunk_and_red_ci_do_not_rewrite_frozen_layer(self):
        report = stack_report(base_ok=False, behind_trunk=True, needs_rebase=True)
        self.assertEqual(gates.evaluate(ns("sanity"), sanity_runner({VIEW_COMMAND: (0, dumped(report))})), "READY")

    def test_direct_pr_exception_is_membership_only(self):
        empty = (2, "", "gh-stack-view: no open gh stack found. Stacks are discovered through worktrees.\n")
        for target in ("develop", "integrate/phase-d"):
            args = ns("sanity", pr_target=target)
            self.assertEqual(gates.evaluate(args, sanity_runner({VIEW_COMMAND: empty}, target)), "READY")
            unrelated = stack_report(pr=99)
            unrelated["stacks"][0].update(coherent=False, problems=["unrelated stack stale"])
            self.assertEqual(gates.evaluate(args, sanity_runner({VIEW_COMMAND: (1, dumped(unrelated))}, target)), "READY")
            for command, response, expected in [
                (VIEW_COMMAND, (2, "", "gh-stack-view: auth failed"), "STACK_UNVERIFIED"),
                (("git", "rev-parse", "HEAD"), (0, "old"), "HEAD_MISMATCH"),
                (("git", "rev-parse", "origin/fix/finding"), (0, "old"), "HEAD_MISMATCH"),
                (("git", "branch", "--show-current"), (0, "other"), "HEAD_MISMATCH"),
                (("git", "status", "--porcelain", "--untracked-files=all"), (0, "?? unknown.py"), "DIRTY_TREE"),
                (("bd", "history", "bead", "--json"), (0, dumped({"verdict": "PASS"})), "SANITY_FROZEN"),
                (("bd", "show", "checked", "--json"), (0, dumped([{"metadata": {"pr_target": "other"}}])), "PR_TARGET_MISMATCH"),
                (PR_COMMAND, (0, dumped({"baseRefName": target, "headRefName": "fix/finding", "headRefOid": "old", "state": "OPEN"})), "STALE_BASE"),
            ]:
                with self.subTest(target=target, command=command):
                    self.assertEqual(gates.evaluate(args, sanity_runner({VIEW_COMMAND: empty, command: response}, target)), expected)
        for target in ("main", "integrate/phase-", "feature/develop"):
            self.assertEqual(gates.evaluate(ns("sanity", pr_target=target), sanity_runner({VIEW_COMMAND: empty}, target)), "STACK_REQUIRED")

    def test_cleanliness_exemptions_match_only_untracked_root_scratch(self):
        for status in ("", "?? .beads.gate.lock", "?? .sc-compose/", "?? .sc-compose/log.json"):
            self.assertTrue(gates.is_clean(status), status)
        for status in ("?? .beads.gate.lock.bak", "?? nested/.beads.gate.lock", "?? nested/.sc-compose/log",
                       "?? .sc-compose-old/log", " M .beads.gate.lock", " M .sc-compose/tracked",
                       "R  file -> .sc-compose/file", "?? .sc-compose/log\n?? unexpected.py"):
            self.assertFalse(gates.is_clean(status), status)

    def test_qa_refusals_and_ready(self):
        cases = [
            ("stale-sanity.json", qa_runner({("gh", "pr", "view", "7", "--json", "baseRefName,headRefOid"): (0, dumped({"baseRefName": "target", "headRefOid": "old"}))})),
            ("qa-head-mismatch.json", qa_runner({("git", "rev-parse", "HEAD"): (0, "other")})),
            ("qa-ready.json", qa_runner()),
        ]
        for fixture, runner in cases:
            with self.subTest(fixture=fixture): self.assert_fixture(fixture, ns("qa"), runner)


if __name__ == "__main__": unittest.main()
