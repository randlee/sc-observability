from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
LOADER = importlib.machinery.SourceFileLoader("sanity_create_findings", str(SCRIPTS / "sanity-create-findings"))
SPEC = importlib.util.spec_from_loader("sanity_create_findings", LOADER)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class ParentLayerTests(unittest.TestCase):
    def test_blocking_child_ranks_one_above_its_parent(self):
        self.assertEqual([module.child_priority("blocking", p) for p in (0, 1, 2, 3, 4, 5)], [1, 1, 1, 2, 3, 4])
        self.assertEqual(module.child_priority("minor", 1), 4)

    def test_sprint_bead_follows_findings_to_the_sprint(self):
        beads = {
            "d-1": {"labels": ["stage:dev", "stage:sprint"], "metadata": {}},
            "d-1-qa-f7": {"labels": ["stage:finding"], "metadata": {"sprint_bead": "d-1"}},
            "d-1-qa-f7.1": {"labels": ["stage:finding"], "metadata": {"sprint_bead": "d-1-qa-f7"}},
        }
        for bead in beads:
            self.assertEqual(module.sprint_bead_of(bead, beads.__getitem__), "d-1")
        beads["orphan"] = {"labels": ["stage:finding"], "metadata": {}}
        with self.assertRaises(module.HandoffError):
            module.sprint_bead_of("orphan", beads.__getitem__)

    def test_dev_bead_layer(self):
        self.assertEqual(module.parent_layer({"layer": 3, "found_on_layer": 1}), 3)

    def test_finding_bead_falls_back_to_found_on_layer(self):
        self.assertEqual(module.parent_layer({"found_on_layer": "2"}), "2")

    def test_neither_is_a_handoff_error(self):
        with self.assertRaises(module.HandoffError):
            module.parent_layer({"phase": "d"})


def finding(number, severity, depends_on=()):
    return {"kind": "skipped", "deliverable": number, "deliverable_text": f"deliverable {number}",
            "file": f"f{number}.rs", "line": number, "issue": f"issue {number}", "finding_ref": f"F{number}",
            "severity": severity, "depends_on": [{"deliverable": d, "file": f"f{d}.rs", "line": d} for d in depends_on]}


class FakeBd:
    """Stands in for bd_commands.command: serves `bd show`/`bd list`, records creates, updates and edges."""
    def __init__(self, beads):
        self.beads, self.calls, self.created = beads, [], []

    def __call__(self, argv, actor="", *, capture=False):
        argv = list(argv)
        self.calls.append(argv)
        if argv[:2] == ["bd", "show"]:
            if argv[2] not in self.beads:
                raise module.BdCommandError(f"no bead {argv[2]}")
            return json.dumps([self.beads[argv[2]]])
        if argv[:2] == ["bd", "list"]:
            return "[]"
        if argv[:2] == ["bd", "create"]:
            child = f"new-{len(self.created) + 1}"
            value = lambda flag: argv[argv.index(flag) + 1]
            self.created.append({"id": child, "parent": value("--parent"), "metadata": json.loads(value("--metadata"))})
            return child
        return ""

    def edges(self):
        return [call[3:5] for call in self.calls if call[:3] == ["bd", "dep", "add"]]

    def listed(self):
        return [call[4] for call in self.calls if call[:2] == ["bd", "list"]]


CHAIN_STEP = {"id": "obs-d-30.chain.dev", "priority": 2, "labels": ["phase-d", "stage:dev", "stack:s1", "wave:3"],
              "metadata": {"sprint_bead": "obs-d-30", "sprint": "d-30", "stack": "s1", "layer": "2", "phase": "d",
                           "wave": 3, "difficulty": "normal", "pr_target": "integrate/phase-d"}}
CONTAINER = {"id": "obs-d-30", "priority": 2, "labels": ["phase-d", "stage:sprint", "wave:3"],
             "metadata": {"sprint": "d-30", "phase": "d", "wave": 3, "stack": "s1", "layer": 2}}


class CreateFindingsTests(unittest.TestCase):
    def run_main(self, beads, bead, findings, *extra):
        fake = FakeBd(beads)
        with tempfile.TemporaryDirectory() as tmp:
            vars_path = Path(tmp) / "vars.json"
            vars_path.write_text(json.dumps({"task_id": "obs-d-30.chain.sanity", "checked_bead": bead,
                                             "verdict": "FAIL", "commit": "abc123", "findings": findings}))
            out, err = io.StringIO(), io.StringIO()
            with mock.patch.object(module, "command", fake), redirect_stdout(out), redirect_stderr(err):
                code = module.main(["--task", "obs-d-30.chain.sanity", "--bead", bead, "--vars", str(vars_path),
                                    "--reviewer", "sc-sanity-llm", *extra])
            report = json.loads(vars_path.read_text())
        return code, fake, (json.loads(out.getvalue()) if code == 0 else err.getvalue()), report

    def test_chain_step_blocking_stays_under_checked_bead_others_go_to_the_phase_root(self):
        beads = {"obs-d-30.chain.dev": CHAIN_STEP, "obs-d-30": CONTAINER}
        findings = [finding(1, "blocking"), finding(2, "important", depends_on=[1]),
                    finding(3, "minor", depends_on=[2]), finding(4, "blocking", depends_on=[1, 2])]
        code, fake, out, report = self.run_main(beads, "obs-d-30.chain.dev", findings, "--pr-number", "412",
                                                 "--root", "obs-phase-d")
        self.assertEqual(code, 0, out)
        self.assertEqual([c["parent"] for c in fake.created],
                         ["obs-d-30.chain.dev", "obs-phase-d", "obs-phase-d", "obs-d-30.chain.dev"])
        important = fake.created[1]["metadata"]
        for key, value in {"sprint_bead": "obs-d-30", "sanity_bead": "obs-d-30.chain.sanity", "found_at_commit": "abc123",
                           "pr_number": 412, "wave": 3, "found_on_layer": "2", "severity": "important"}.items():
            self.assertEqual(important[key], value, key)
        self.assertEqual(sorted(fake.listed()), ["obs-d-30.chain.dev", "obs-phase-d"])
        self.assertEqual(fake.edges(), [["new-3", "new-2"], ["new-4", "new-1"]])  # same-parent edges only
        self.assertEqual(out["skipped_edges"], [{"from": "new-2", "to": "new-1"}, {"from": "new-4", "to": "new-2"}])
        self.assertEqual(report["finding_bead_ids"], ["new-1", "new-2", "new-3", "new-4"])

    def test_root_arg_and_wave_from_the_container(self):
        step = json.loads(json.dumps(CHAIN_STEP))
        del step["metadata"]["wave"]
        beads = {"obs-d-30.chain.dev": step, "obs-d-30": CONTAINER}
        code, fake, out, _ = self.run_main(beads, "obs-d-30.chain.dev", [finding(1, "minor")], "--root", "x-phase-d")
        self.assertEqual(code, 0, out)
        self.assertEqual(fake.created[0]["parent"], "x-phase-d")
        self.assertEqual(fake.created[0]["metadata"]["wave"], 3)
        self.assertNotIn("pr_number", fake.created[0]["metadata"])

    def test_legacy_dev_bead_keeps_every_blocking_child_and_edge(self):
        legacy = {"id": "obs-d-12", "priority": 2, "labels": ["phase-d", "stage:dev", "stage:sprint"],
                  "metadata": {"phase": "d", "sprint": "d-12", "stack": "s1", "layer": 1, "difficulty": "normal"}}
        code, fake, out, _ = self.run_main({"obs-d-12": legacy}, "obs-d-12",
                                           [finding(1, "blocking"), finding(2, "blocking", depends_on=[1])])
        self.assertEqual(code, 0, out)
        self.assertEqual([c["parent"] for c in fake.created], ["obs-d-12", "obs-d-12"])
        self.assertEqual(fake.listed(), ["obs-d-12"])
        self.assertEqual(fake.edges(), [["new-2", "new-1"]])
        self.assertEqual(fake.created[0]["metadata"]["sprint_bead"], "obs-d-12")
        self.assertNotIn("wave", fake.created[0]["metadata"])

    def test_genuinely_missing_field_is_still_an_error(self):
        for field in ("phase", "sprint", "stack", "layer"):
            step = json.loads(json.dumps(CHAIN_STEP))
            del step["metadata"][field]
            with self.subTest(field=field):
                code, fake, err, _ = self.run_main({"obs-d-30.chain.dev": step, "obs-d-30": CONTAINER},
                                                   "obs-d-30.chain.dev", [finding(1, "blocking")])
                self.assertEqual(code, 2)
                self.assertIn(f"lacks {field}", err)
                self.assertEqual(fake.created, [])


if __name__ == "__main__":
    unittest.main()
