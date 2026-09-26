from __future__ import annotations
import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).parents[1] / "blocking-finding-gates.py"
spec = importlib.util.spec_from_file_location("gates", path); assert spec and spec.loader
gates = importlib.util.module_from_spec(spec); spec.loader.exec_module(gates)

def bead(key, labels, status="open", metadata=None, dependencies=(), assignee=""):
    return {"id": key, "labels": labels, "status": status, "assignee": assignee,
            "metadata": metadata or {}, "dependencies": [{"type": "blocks", "depends_on_id": x} for x in dependencies]}

class BlockingGateTests(unittest.TestCase):
    def test_only_open_downstream_work_is_selected(self):
        rows = [bead("s", ["stage:dev"]), bead("s-sanity", ["stage:dev-sanity"], metadata={"dev_bead":"s"}, dependencies=["s"]),
                bead("open", ["stage:dev"], dependencies=["s-sanity"]), bead("active", ["stage:dev"], "in_progress", dependencies=["s-sanity"]),
                bead("child", ["stage:fix"], metadata={"sprint_bead":"open"})]
        finding = bead("f", ["stage:finding"], metadata={"severity":"blocking", "sprint_bead":"s"})
        self.assertEqual(gates.targets(finding, rows), ("s-sanity", ["child", "open"]))
        finding["metadata"]["severity"] = "important"
        self.assertEqual(gates.targets(finding, rows), (None, []))

if __name__ == "__main__": unittest.main()
