"""validate-plan, sprint_index_common and bead_schema on a container phase (d-3 depends on d-1 qa and the d-2 sprint;
d-2 on d-1 sanity), plus the legacy phase they must keep validating."""
from __future__ import annotations

import copy
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
SCRIPT = SCRIPTS / "validate-plan"


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module  # pydantic resolves the models' postponed annotations through sys.modules
    spec.loader.exec_module(module)
    return module


common = load("sprint_index_common")
schema = load("bead_schema")

ROOT = "x-phase-d"
PLAN = [["d-1", 1, []], ["d-2", 1, ["d-1"]], ["d-3", 2, [["d-1", "qa"], ["d-2", "sprint"]]]]
LEGACY_PLAN = [["d-1", "x-d-1-sanity", []], ["d-2", "x-d-2-sanity", ["d-1"]]]
ROOT_EPIC = {"id": ROOT, "issue_type": "epic", "status": "open", "created_at": "2026-09-25T00:00:00Z"}


def dep(kind: str, target: str) -> dict:
    return {"type": kind, "depends_on_id": target}


def container(sprint: str, wave: int, layer: int) -> dict:
    return {"id": f"x-{sprint}", "issue_type": "epic", "status": "open", "parent": ROOT,
            "labels": ["phase-d", "stage:sprint", f"wave:{wave}", "stack:phase-d"],
            "description": "Goal.\n\n## Deliverables\n1. the thing\n", "acceptance_criteria": "- [ ] #1: done",
            "dependencies": [dep("parent-child", ROOT)],
            "metadata": {"phase": "d", "sprint": sprint, "wave": wave, "stack": "phase-d", "layer": layer,
                         "pr_target": "integrate/phase-d", "difficulty": "normal", "requirements": ["NONE"],
                         "adrs": ["ADR-1"], "owned_paths": [f"src/{sprint}"]}}


def chain(sprint: str, wave: int, layer: int, blocks: list[str]) -> list[dict]:
    sb = f"x-{sprint}"
    meta = {"sprint_bead": sb, "sprint": sprint, "stack": "phase-d", "layer": str(layer), "phase": "d",
            "wave": wave, "difficulty": "normal", "pr_target": "integrate/phase-d"}
    stages = {"dev": "stage:dev", "sanity": "stage:dev-sanity", "qa": "stage:qa"}
    steps = []
    for step, needs, extra in (("dev", None, {}), ("sanity", "dev", {"dev_bead": f"{sb}.chain.dev"}),
                               ("qa", "sanity", {"checked_bead": f"{sb}.chain.dev"})):
        deps = [dep("parent-child", f"{sb}.chain")] + ([dep("blocks", f"{sb}.chain.{needs}")] if needs else [])
        if step == "dev":
            deps += [dep("blocks", b) for b in blocks]
        steps.append({"id": f"{sb}.chain.{step}", "issue_type": "task", "status": "open", "parent": f"{sb}.chain",
                      "labels": ["phase-d", stages[step], "stack:phase-d", f"wave:{wave}"],
                      "dependencies": deps, "metadata": {**meta, **extra}})
    return [{"id": f"{sb}.chain", "issue_type": "epic", "status": "open", "parent": sb,
             "dependencies": [dep("parent-child", sb)]}] + steps


CONTAINERS = [ROOT_EPIC, container("d-1", 1, 1), container("d-2", 1, 2), container("d-3", 2, 1)]
POURED = (CONTAINERS + chain("d-1", 1, 1, []) + chain("d-2", 1, 2, ["x-d-1.chain.sanity"])
          + chain("d-3", 2, 1, ["x-d-1.chain.qa", "x-d-2"]))


def legacy_sprint(bid: str, blocks: list[str]) -> dict:
    return {"id": bid, "parent": ROOT, "assignee": "dev", "acceptance_criteria": "- [ ] #1: done",
            "description": "Goal.\n\n## Deliverables\n1. the thing\n",
            "dependencies": [{"type": "blocks", "depends_on_id": b} for b in blocks],
            "metadata": {"requirements": ["NONE"], "adrs": ["ADR-1"], "worktree": "wt", "branch": f"sprint/{bid}",
                         "pr_target": "integrate/phase-d", "difficulty": "normal"}}


def legacy_sanity(bid: str, dev: str) -> dict:
    return {"id": bid, "parent": ROOT, "assignee": "sanity", "metadata": {"dev_bead": dev},
            "dependencies": [{"dependency_type": "blocks", "id": dev}]}


LEGACY = [ROOT_EPIC, legacy_sprint("x-d-1", []), legacy_sanity("x-d-1-sanity", "x-d-1"),
          legacy_sprint("x-d-2", ["x-d-1-sanity"]), legacy_sanity("x-d-2-sanity", "x-d-2")]


def write_plan(d: str, plan: list) -> Path:
    index = Path(d) / "docs" / "plans" / "phase-d" / "sprints.jsonl"
    index.parent.mkdir(parents=True)
    index.write_text("".join(json.dumps(r) + "\n" for r in plan))
    return index


def run(beads: list[dict], mode: str | None = None, plan: list = PLAN) -> tuple[int, list[str]]:
    with tempfile.TemporaryDirectory() as d:
        index, data = write_plan(d, plan), Path(d) / "beads.json"
        data.write_text(json.dumps(beads))
        args = [str(SCRIPT), "--root", ROOT, "--index", str(index), "--beads", str(data), "--no-doctor"]
        out = subprocess.run(args + (["--mode", mode] if mode else []), capture_output=True, text=True)
    return out.returncode, out.stdout.splitlines()


def changed(beads: list[dict], bid: str, change) -> list[dict]:
    beads = copy.deepcopy(beads)
    change(next(b for b in beads if b["id"] == bid))
    return beads


class ParsePlan(unittest.TestCase):
    def test_rows_derive_chain_ids_and_edges(self):
        with tempfile.TemporaryDirectory() as d:
            index = write_plan(d, PLAN)
            self.assertEqual(common.plan_format(index), "container")
            rows = common.parse_container_plan(index, "x-")
            normalized = common.load_phase_plan(index, ROOT)
        self.assertEqual(rows[0]["chain"], {"chain": "x-d-1.chain", "dev": "x-d-1.chain.dev",
                                            "sanity": "x-d-1.chain.sanity", "qa": "x-d-1.chain.qa"})
        self.assertEqual(rows[1]["edges"], [("x-d-2.chain.dev", "x-d-1.chain.sanity", "sanity")])
        self.assertEqual(rows[2]["edges"], [("x-d-3.chain.dev", "x-d-1.chain.qa", "qa"),
                                            ("x-d-3.chain.dev", "x-d-2", "sprint")])
        self.assertEqual(normalized["format"], "container")
        self.assertEqual(normalized["sprints"][2]["depends_on_bead_ids"], ["x-d-1.chain.qa", "x-d-2"])
        self.assertEqual(common.index_bead_pairs(normalized)["x-d-2.chain.dev"], "x-d-2.chain.sanity")

    def test_legacy_rows_unchanged(self):
        with tempfile.TemporaryDirectory() as d:
            index = write_plan(d, LEGACY_PLAN)
            self.assertEqual(common.plan_format(index), "legacy")
            self.assertEqual(common.load_phase_plan(index), {"root_bead_id": "obs-phase-d", "sprints": [
                {"dev_bead_id": "obs-d-1", "sanity_bead_id": "x-d-1-sanity", "depends_on_sanity_bead_ids": []},
                {"dev_bead_id": "obs-d-2", "sanity_bead_id": "x-d-2-sanity",
                 "depends_on_sanity_bead_ids": ["x-d-1-sanity"]}]})

    def test_bad_rows(self):
        cases = {"mixes container rows": [PLAN[0], LEGACY_PLAN[1]],
                 "a depends_on entry is": [["d-1", 1, []], ["d-2", 1, [["d-1", "sanity"]]]],
                 "duplicate or self": [["d-1", 1, []], ["d-2", 1, ["d-1", ["d-1", "qa"]]]],
                 "unknown sprint": [["d-1", 1, ["d-9"]]]}
        for want, plan in cases.items():
            with self.subTest(problem=want), tempfile.TemporaryDirectory() as d:
                with self.assertRaisesRegex(RuntimeError, want):
                    common.load_phase_plan(write_plan(d, plan))


class ContainerPlan(unittest.TestCase):
    def test_plan_mode_is_the_default_and_needs_no_chain(self):
        self.assertEqual(run(CONTAINERS), (0, ["plan valid: 3 sprints"]))
        self.assertEqual(run(CONTAINERS, "plan"), (0, ["plan valid: 3 sprints"]))

    def test_execution_mode_passes_when_poured(self):
        self.assertEqual(run(POURED, "execution"), (0, ["plan valid: 3 sprints"]))
        self.assertEqual(run(POURED, "plan"), (0, ["plan valid: 3 sprints"]))

    def test_execution_mode_live_with_fake_bd(self):
        with tempfile.TemporaryDirectory() as d:
            bin_dir, listing = Path(d) / "bin", Path(d) / "list.json"
            bin_dir.mkdir()
            listing.write_text(json.dumps(POURED))
            fake = bin_dir / "bd"
            fake.write_text(f'#!/bin/sh\ncase "$1" in\n  list) cat "{listing}" ;;\n'
                            '  doctor) echo \'{"checks": [{"name": "db", "status": "ok"}]}\' ;;\n'
                            '  *) echo "unexpected bd $*" >&2; exit 1 ;;\nesac\n')
            fake.chmod(0o755)
            index = write_plan(d, PLAN)
            env = {**os.environ, "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}"}
            out = subprocess.run([str(SCRIPT), "--mode", "execution", "--root", ROOT, "--index", str(index),
                                  "--scope", "x-d-1.chain.dev"], capture_output=True, text=True, env=env)
        self.assertEqual((out.returncode, out.stdout.splitlines()), (0, ["plan valid: 3 sprints"]), out.stderr)

    def test_missing_chain_fails_execution_only(self):
        beads = [b for b in POURED if not b["id"].startswith("x-d-2.chain")]
        self.assertEqual(run(beads, "plan")[0], 0)
        rc, lines = run(beads, "execution")
        self.assertEqual(rc, 5)
        self.assertEqual(len(lines), 1, lines)
        self.assertTrue(lines[0].startswith("x-d-2: incomplete chain, missing x-d-2.chain"), lines)

    def test_execution_problems(self):
        def reblock(targets):
            return lambda b: b.update(dependencies=[dep("parent-child", "x-d-3.chain")]
                                      + [dep("blocks", t) for t in targets])
        cases = {
            "x-d-3.chain.dev: blocks on x-d-1.chain.sanity instead of x-d-1.chain.qa":
                ("x-d-3.chain.dev", reblock(["x-d-1.chain.sanity", "x-d-2"])),
            "x-d-3.chain.dev: missing blocks edge to x-d-2 ":
                ("x-d-3.chain.dev", reblock(["x-d-1.chain.qa"])),
            "x-d-2.chain.dev: blocks edge to x-d-1.chain.qa is not declared":
                ("x-d-2.chain.dev", lambda b: b["dependencies"].append(dep("blocks", "x-d-1.chain.qa"))),
            "x-d-1.chain.qa: missing blocks edge to x-d-1.chain.sanity":
                ("x-d-1.chain.qa", lambda b: b.update(dependencies=[dep("parent-child", "x-d-1.chain")])),
            "x-d-1.chain: parent is \"x-d-2\", not x-d-1":
                ("x-d-1.chain", lambda b: b.update(parent="x-d-2")),
            "x-d-1.chain.sanity: parent is \"x-d-1\", not x-d-1.chain":
                ("x-d-1.chain.sanity", lambda b: b.update(parent="x-d-1")),
            "x-d-1.chain.qa: metadata.wave is 2, the sprint's is 1":
                ("x-d-1.chain.qa", lambda b: (b["metadata"].update(wave=2), b.update(labels=["stage:qa", "wave:2"]))),
            "x-d-1.chain.sanity: metadata.dev_bead is \"x-d-2.chain.dev\", not x-d-1.chain.dev":
                ("x-d-1.chain.sanity", lambda b: b["metadata"].update(dev_bead="x-d-2.chain.dev")),
        }
        for want, (bid, change) in cases.items():
            with self.subTest(problem=want):
                rc, lines = run(changed(POURED, bid, change), "execution")
                self.assertEqual(rc, 5)
                self.assertEqual(len(lines), 1, lines)
                self.assertTrue(lines[0].startswith(want), lines)

    def test_plan_problems(self):
        cases = {
            "x-d-2: metadata.wave is 2, sprints.jsonl wave is 1":
                ("x-d-2", lambda b: (b["metadata"].update(wave=2), b.update(labels=["stage:sprint", "wave:2"]))),
            "x-d-2: labels ['wave:2'] must be exactly [\"wave:1\"]":
                ("x-d-2", lambda b: b.update(labels=["stage:sprint", "wave:2"])),
            "x-d-2: missing label stage:sprint":
                ("x-d-2", lambda b: b.update(labels=["wave:1"])),
            "x-d-2: metadata.wave: ": ("x-d-2", lambda b: b["metadata"].update(wave="1")),
            "x-d-2: metadata.pr_target: ": ("x-d-2", lambda b: b["metadata"].pop("pr_target")),
            "x-d-2: metadata.difficulty: ": ("x-d-2", lambda b: b["metadata"].update(difficulty="medium")),
            "x-d-2: description: ": ("x-d-2", lambda b: b.update(description="none")),
        }
        for want, (bid, change) in cases.items():
            with self.subTest(problem=want):
                rc, lines = run(changed(CONTAINERS, bid, change), "plan")
                self.assertEqual(rc, 5)
                self.assertEqual(len(lines), 1, lines)
                self.assertTrue(lines[0].startswith(want), lines)

    def test_missing_container_and_unlisted_sprint(self):
        self.assertEqual(run(CONTAINERS[:3]), (5, ["x-d-3: sprint d-3 is in sprints.jsonl but not in beads"]))
        extra = {**container("d-9", 1, 3), "id": "x-d-9"}
        self.assertEqual(run(CONTAINERS + [extra]), (5, ["x-d-9: open sprint under x-phase-d is not in sprints.jsonl"]))

    def test_prerequisite_in_a_later_wave(self):
        plan = [["d-1", 2, []], ["d-2", 1, ["d-1"]], PLAN[2]]
        beads = changed(CONTAINERS, "x-d-1", lambda b: (b["metadata"].update(wave=2),
                                                        b.update(labels=["stage:sprint", "wave:2"])))
        self.assertEqual(run(beads, plan=plan),
                         (5, ["x-d-2: prerequisite d-1 is in wave 2, after this sprint's wave 1"]))


class SanityException(unittest.TestCase):
    def finding(self, severity: str) -> dict:
        return {"id": "x-d-1-f1", "parent": ROOT, "labels": ["stage:finding", f"severity:{severity}"],
                "metadata": {"severity": severity}}

    def sanity(self, blocks: bool) -> dict:
        return {"id": "x-d-1-f1-sanity", "parent": "x-d-1-f1", "assignee": "sanity",
                "metadata": {"dev_bead": "x-d-1-f1"},
                "dependencies": [dep("parent-child", "x-d-1-f1")] + ([dep("blocks", "x-d-1-f1")] if blocks else [])}

    def check(self, severity: str, blocks: bool) -> list[str]:
        finding = self.finding(severity)
        return schema.problems(self.sanity(blocks), schema.SanityBead, {finding["id"]: finding})

    def test_important_fix_sanity_has_no_edge(self):
        self.assertEqual(self.check("important", False), [])
        self.assertEqual(self.check("important", True),
                         ["x-d-1-f1-sanity: blocks edge to its important finding x-d-1-f1 must be absent (dev_bead_open)"])

    def test_other_sanity_beads_keep_the_edge(self):
        self.assertEqual(self.check("blocking", True), [])
        self.assertEqual(self.check("blocking", False), ["x-d-1-f1-sanity: missing blocks edge to its sprint x-d-1-f1"])
        # without the finding in view the edge stays required
        self.assertEqual(schema.problems(self.sanity(False), schema.SanityBead),
                         ["x-d-1-f1-sanity: missing blocks edge to its sprint x-d-1-f1"])


class LegacyPhase(unittest.TestCase):
    def test_legacy_passes_in_both_modes(self):
        for mode in (None, "plan", "execution"):
            with self.subTest(mode=mode):
                self.assertEqual(run(LEGACY, mode, LEGACY_PLAN), (0, ["plan valid: 2 sprints"]))

    def test_legacy_keeps_its_checks(self):
        beads = changed(LEGACY, "x-d-2", lambda b: b.update(dependencies=[]))
        rc, lines = run(beads, "execution", LEGACY_PLAN)
        self.assertEqual(rc, 5)
        self.assertTrue(lines[0].startswith("x-d-2: missing blocks edge to x-d-1-sanity"), lines)


if __name__ == "__main__":
    unittest.main()
