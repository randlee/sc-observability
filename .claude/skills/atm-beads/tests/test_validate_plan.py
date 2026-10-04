"""Locked plan membership, live edge alignment, and offline diagram checks."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT.parent / 'sprint-report/scripts'))
from locked_plan import load, index, alignment, annotate, check_html, metrics
from phase_config import load as config_load
from sprint_dag import plan_graph, dot_source, render, html_view

ROWS = [{'sprint': 'x-t-1', 'depends_on': []}, {'sprint': 'x-t-2', 'depends_on': ['x-t-1']}]


def sprint(bid, dependencies):
    return {'id': bid, 'parent': 'x-phase-t', 'status': 'open', 'labels': ['stage:dev'], 'assignee': 'dev',
            'description': '## Deliverables\n1. Implement the change.', 'acceptance_criteria': 'It works.',
            'dependencies': [{'type': 'blocks', 'depends_on_id': x} for x in dependencies],
            'metadata': {'requirements': ['NONE'], 'adrs': ['NONE'], 'worktree': '/tmp/example',
                         'branch': 'sprint/' + bid, 'pr_target': 'integrate/phase-t', 'difficulty': 'normal'}}


def sanity(dev):
    return {'id': dev + '-sanity', 'parent': 'x-phase-t', 'status': 'open', 'labels': ['stage:dev-sanity'],
            'assignee': 'sanity', 'metadata': {'dev_bead': dev},
            'dependencies': [{'type': 'blocks', 'depends_on_id': dev}]}


BEADS = [{'id': 'x-phase-t', 'issue_type': 'epic', 'created_at': '2026-01-01'},
         sprint('x-t-1', []), sanity('x-t-1'), sprint('x-t-2', ['x-t-1-sanity']), sanity('x-t-2')]


class PlanValidation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.plan = self.directory / 'phase-t.jsonl'

    def tearDown(self):
        self.temp.cleanup()

    def parse(self, rows):
        self.plan.write_text(''.join(json.dumps(r) + '\n' for r in rows))
        return load(self.plan)

    def check(self, beads, rows=ROWS):
        return list(alignment(rows, beads, 'x-phase-t', index(rows, beads, 'x-phase-t')))

    def test_valid_and_no_sanity_id_in_plan(self):
        self.assertEqual(self.parse(ROWS), ROWS)
        self.assertEqual(self.check(BEADS), [])

    def test_invalid_plan(self):
        cases = [[], ROWS + [ROWS[0]], [{'sprint': 'a', 'depends_on': ['missing']}],
                 [{'sprint': 'a', 'depends_on': ['b']}, {'sprint': 'b', 'depends_on': ['a']}],
                 [{'sprint': 'a', 'depends_on': [], 'title': 'duplicated'}],
                 [{'sprint': 'a', 'depends_on': ['b'], 'tight': ['b']}, {'sprint': 'b', 'depends_on': []}]]
        for rows in cases:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                self.parse(rows)

    def test_added_and_removed_sprint_rejected(self):
        extra = sprint('x-t-3', [])
        self.assertTrue(any('extra live sprint' in e for e in self.check(BEADS + [extra])))
        # Existing sanity cannot mask a missing dev.
        self.assertTrue(any('missing live sprint' in e for e in self.check([b for b in BEADS if b['id'] != 'x-t-2'])))

    def test_missing_or_extra_edge_rejected(self):
        for target, deps, phrase in [('x-t-2', [], 'missing sprint dependency'),
                                     ('x-t-1', ['x-t-2-sanity'], 'extra sprint dependency')]:
            beads = copy.deepcopy(BEADS)
            next(b for b in beads if b['id'] == target)['dependencies'] = [
                {'type': 'blocks', 'depends_on_id': d} for d in deps]
            self.assertTrue(any(phrase in e for e in self.check(beads)))

    def test_finding_fix_and_operational_gate_do_not_expand_sprints(self):
        beads = copy.deepcopy(BEADS)
        beads += [{'id': 'finding', 'parent': 'x-phase-t', 'issue_type': 'bug', 'labels': ['stage:dev']},
                  {'id': 'fix', 'parent': 'x-t-1', 'labels': ['stage:dev']}]
        beads[1]['dependencies'].append({'type': 'blocks', 'depends_on_id': 'plan-review'})
        self.assertEqual(self.check(beads), [])

    def test_cross_sprint_qa_and_container_edges_rejected(self):
        beads = copy.deepcopy(BEADS)
        beads.append({'id': 'x-t-1-qa', 'parent': 'x-t-1', 'labels': ['stage:qa']})
        for target in ('x-t-1-qa', 'x-t-1'):
            beads[3]['dependencies'][0]['depends_on_id'] = target
            self.assertTrue(any('cross-sprint' in e for e in self.check(beads)))

    def test_parallel_width_and_critical_path(self):
        rows = self.parse([{'sprint': 'a', 'depends_on': []}, {'sprint': 'b', 'depends_on': []},
                           {'sprint': 'c', 'depends_on': ['a']}, {'sprint': 'd', 'depends_on': ['a']}])
        self.assertEqual(metrics(rows), (2, 3))

    def test_config_colocation_and_path_escape(self):
        cfg = self.directory / 'config.toml'
        cfg.write_text('root="x-phase-t"\nsprints="arbitrary/plans/t.jsonl"\n')
        root, plan, html = config_load(self.directory, cfg)
        self.assertEqual(html, plan.with_name('t-dag.html'))
        cfg.write_text('root="x-phase-t"\nsprints="../escape.jsonl"\n')
        with self.assertRaises(ValueError):
            config_load(self.directory, cfg)

    def test_existing_bead_model_validation_is_preserved(self):
        beads = copy.deepcopy(BEADS)
        beads[1]['metadata']['requirements'] = []
        self.assertTrue(any('requirements' in e for e in self.check(beads)))

    def test_offline_html_checks_rendered_nodes_and_edges_not_status(self):
        runtime = index(ROWS, BEADS, 'x-phase-t')
        dot, svg = self.directory / 'plan.dot', self.directory / 'plan.svg'
        dot.write_text(dot_source(plan_graph(runtime), 't'))
        render('layout', dot, svg)
        html = html_view(annotate(svg.read_text(), runtime), 't', 'x-phase-t')
        check_html(ROWS, html, 'x-phase-t')
        check_html(ROWS, html + '<!-- status timestamp changed -->', 'x-phase-t')
        changed = [ROWS[0], {'sprint': 'x-t-2', 'depends_on': []}]
        with self.assertRaisesRegex(ValueError, 'edges differ'):
            check_html(changed, html, 'x-phase-t')
        with self.assertRaisesRegex(ValueError, 'sprint set differs'):
            check_html(ROWS[:1], html, 'x-phase-t')
        with self.assertRaisesRegex(ValueError, 'root differs'):
            check_html(ROWS, html, 'wrong-root')

    def test_schemas_unchanged(self):
        subprocess.run([sys.executable, str(ROOT/'scripts/bead_schema.py'), 'export', str(self.directory)], check=True)
        for path in self.directory.glob('*.schema.json'):
            self.assertEqual(path.read_text(), (ROOT/'schemas'/path.name).read_text())


if __name__ == '__main__':
    unittest.main()
