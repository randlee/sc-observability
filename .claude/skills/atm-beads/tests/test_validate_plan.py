"""Locked plan membership, live edge alignment, and offline diagram checks."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT.parent / 'sprint-report/scripts'))
from locked_plan import load, index, alignment, annotate, check_html
from phase_config import load as config_load
from sprint_dag import plan_graph, dot_source, render, html_view

ROWS = [{'sprint': 'x-t-1'}, {'sprint': 'x-t-2'}]


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


BEADS = [{'id': 'x-phase-t', 'issue_type': 'epic', 'created_at': '2026-01-01', 'metadata': {'integration_branch': 'integrate/phase-t'}},
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
        return list(alignment(rows, beads, 'x-phase-t'))

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

    def test_live_edges_are_not_locked_by_plan(self):
        beads = copy.deepcopy(BEADS)
        beads[3]['dependencies'] = []
        self.assertEqual(self.check(beads), [])
        runtime = index(ROWS, beads, 'x-phase-t')
        self.assertEqual(runtime['sprints'][1]['depends_on_sanity_bead_ids'], [])

    def test_finding_fix_and_operational_gate_do_not_expand_sprints(self):
        beads = copy.deepcopy(BEADS)
        beads += [{'id': 'finding', 'parent': 'x-phase-t', 'issue_type': 'bug', 'labels': ['stage:dev']},
                  {'id': 'fix', 'parent': 'x-t-1', 'labels': ['stage:dev']}]
        beads[1]['dependencies'].append({'type': 'blocks', 'depends_on_id': 'plan-review'})
        self.assertEqual(self.check(beads), [])

    def test_config_colocation_and_path_escape(self):
        cfg = self.directory / 'config.toml'
        cfg.write_text('root="x-phase-t"\nsprints="arbitrary/plans/t.jsonl"\nintegration_branch="integrate/phase-t"\n')
        root, plan, html = config_load(self.directory, cfg)
        self.assertEqual(html, plan.with_name('t-dag.html'))
        cfg.write_text('root="x-phase-t"\nsprints="../escape.jsonl"\nintegration_branch="integrate/phase-t"\n')
        with self.assertRaises(ValueError):
            config_load(self.directory, cfg)

    def test_offline_html_checks_rendered_nodes_not_status(self):
        runtime = index(ROWS, BEADS, 'x-phase-t')
        dot, svg = self.directory / 'plan.dot', self.directory / 'plan.svg'
        dot.write_text(dot_source(plan_graph(runtime), 't'))
        render('layout', dot, svg)
        html = html_view(annotate(svg.read_text(), runtime), 't', 'x-phase-t')
        check_html(ROWS, html, 'x-phase-t')
        check_html(ROWS, html + '<!-- status timestamp changed -->', 'x-phase-t')
        with self.assertRaisesRegex(ValueError, 'sprint set differs'):
            check_html(ROWS[:1], html, 'x-phase-t')
        with self.assertRaisesRegex(ValueError, 'root differs'):
            check_html(ROWS, html, 'wrong-root')

    def test_schemas_unchanged(self):
        subprocess.run([sys.executable, str(ROOT/'scripts/bead_schema.py'), 'export', str(self.directory)], check=True)
        for path in self.directory.glob('*.schema.json'):
            self.assertEqual(path.read_text(), (ROOT/'schemas'/path.name).read_text())


class PlanCli(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.repo = self.directory / 'repo'
        self.repo.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.repo)], check=True)
        (self.repo / '.atm-bd').mkdir()
        (self.repo / 'plans').mkdir()
        self.config = self.repo / '.atm-bd/phase-t.toml'
        self.config.write_text('root="x-phase-t"\nsprints="plans/t.jsonl"\nintegration_branch="integrate/phase-t"\n')
        self.plan = self.repo / 'plans/t.jsonl'
        self.write_plan(ROWS)
        self.beads = self.directory / 'beads.json'
        self.beads.write_text(json.dumps(BEADS))
        self.bin = self.directory / 'bin'
        self.bin.mkdir()
        self.bd = self.bin / 'bd'
        self.bd.write_text('#!' + sys.executable + '\n' +
            "import json, os, pathlib, sys\n"
            "if sys.argv[1] == 'list':\n"
            "    print(pathlib.Path(os.environ['TEST_BEADS']).read_text())\n"
            "elif sys.argv[1] == 'doctor':\n"
            "    print('doctor retained warning', file=sys.stderr)\n"
            "    print(json.dumps({'checks': []}))\n"
            "elif sys.argv[1] == 'blocked': print('[]')\n"
            "else: sys.exit(99)\n")
        self.bd.chmod(0o755)
        atm = self.bin / 'atm'
        atm.write_text('#!/bin/sh\nexit 1\n')
        atm.chmod(0o755)
        self.env = {**os.environ, 'TEST_BEADS': str(self.beads),
                    'PATH': str(self.bin) + os.pathsep + os.environ['PATH']}
        self.html = self.repo / 'plans/t-dag.html'

    def write_plan(self, rows):
        self.plan.write_text(''.join(json.dumps(r) + '\n' for r in rows))

    def run_cli(self, *args, explicit=True):
        command = [sys.executable, str(ROOT / 'scripts/validate-plan')]
        if explicit:
            command += ['--config', '.atm-bd/phase-t.toml']
        return subprocess.run(command + list(args), cwd=self.repo, env=self.env, capture_output=True, text=True)

    def assert_problem(self, result, bead, fragment):
        self.assertEqual(result.returncode, 5, result.stderr + result.stdout)
        self.assertTrue(any(line.startswith(bead + ': ') and fragment in line
                            for line in result.stdout.splitlines()), result.stdout)
        self.assertNotIn('Traceback', result.stderr)

    def test_five_blocking_categories_are_stdout_problem_lines(self):
        cases = []
        cases.append(([{'sprint': 'x-t-1', 'unexpected': True}], BEADS, 'x-phase-t', 'invalid plan schema'))
        cases.append((ROWS, [b for b in BEADS if b['id'] != 'x-t-2'], 'x-t-2', 'missing live sprint'))
        cases.append((ROWS, BEADS + [sprint('x-t-3', [])], 'x-t-3', 'extra live sprint'))
        mismatched = copy.deepcopy(BEADS)
        mismatched[0]['metadata']['integration_branch'] = 'wrong'
        cases.append((ROWS, mismatched, 'x-phase-t', 'integration_branch'))
        missing_edge = copy.deepcopy(BEADS)
        missing_edge[3]['dependencies'] = []
        dependent = [ROWS[0], {'sprint': 'x-t-2', 'depends_on': ['x-t-1']}]
        cases.append((dependent, missing_edge, 'x-t-2', 'depends_on'))
        for rows, beads, bead, fragment in cases:
            with self.subTest(fragment=fragment):
                self.write_plan(rows)
                self.beads.write_text(json.dumps(beads))
                self.assert_problem(self.run_cli(), bead, fragment)

    def test_missing_sprints_are_reported_together(self):
        self.beads.write_text(json.dumps([BEADS[0]]))
        result = self.run_cli()
        for bid in ('x-t-1', 'x-t-2'):
            self.assert_problem(result, bid, 'missing live sprint')

    def test_dependency_accepts_sanity_or_sprint_edge(self):
        self.write_plan([ROWS[0], {'sprint': 'x-t-2', 'depends_on': ['x-t-1']}])
        for predecessor in ('x-t-1-sanity', 'x-t-1'):
            beads = copy.deepcopy(BEADS)
            beads[3]['dependencies'] = [{'type': 'blocks', 'depends_on_id': predecessor}]
            self.beads.write_text(json.dumps(beads))
            result = self.run_cli()
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)

    def test_config_errors_exit_two(self):
        original = self.config.read_text()
        for content in ('[broken', 'root="x-phase-t"\n', None):
            with self.subTest(content=content):
                if content is None:
                    self.config.unlink()
                else:
                    self.config.write_text(content)
                result = self.run_cli()
                self.assertEqual(result.returncode, 2, result.stderr + result.stdout)
                self.assertNotIn('Traceback', result.stderr)
                self.config.write_text(original)

    def test_bd_failure_and_bad_output_exit_two(self):
        for output in ({'unexpected': []}, [None], ['not a bead'], [{'id': 3}]):
            with self.subTest(output=output):
                self.beads.write_text(json.dumps(output))
                result = self.run_cli()
                self.assertEqual(result.returncode, 2, result.stderr + result.stdout)
                self.assertNotIn('Traceback', result.stderr)
        self.beads.write_text('not json')
        self.assertEqual(self.run_cli().returncode, 2)
        self.bd.write_text('#!/bin/sh\nexit 99\n')
        self.assertEqual(self.run_cli().returncode, 2)

    def test_missing_bd_executable_exits_two(self):
        self.bd.unlink()
        self.env['PATH'] = str(self.bin)
        # git is required to locate the repository before bd is queried.
        import shutil
        (self.bin / 'git').symlink_to(shutil.which('git'))
        result = self.run_cli()
        self.assertEqual(result.returncode, 2, result.stderr + result.stdout)
        self.assertIn('bd', result.stderr)
        self.assertNotIn('Traceback', result.stderr)

    def test_doctor_and_render_failures_are_nonfatal(self):
        self.bd.write_text(self.bd.read_text().replace(
            "print(json.dumps({'checks': []}))", "sys.exit(3)"))
        # No sanity beads: validation still succeeds; rendering can warn.
        self.beads.write_text(json.dumps([b for b in BEADS if not b['id'].endswith('-sanity')]))
        result = self.run_cli('--refresh')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn('doctor retained warning', result.stderr)
        self.assertIn('doctor exited 3', result.stderr)
        self.assertIn('DAG rendering', result.stderr)

    def test_warnings_preserve_doctor_stderr_and_do_not_block(self):
        beads = copy.deepcopy(BEADS)
        beads[1]['metadata']['requirements'] = []
        for bead in (beads[2], beads[4]):
            bead.pop('labels')
            bead.pop('metadata')
        self.beads.write_text(json.dumps(beads))
        result = self.run_cli()
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn('doctor retained warning', result.stderr)
        self.assertIn('requirements', result.stderr)

    def test_assignment_does_not_write_or_need_renderer(self):
        self.html.write_text('unchanged artifact')
        result = self.run_cli('--scope', 'x-t-1')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertEqual(self.html.read_text(), 'unchanged artifact')
        self.assertNotIn('DAG rendering', result.stderr)

    def test_preimport_never_writes_html_even_with_refresh(self):
        jsonl = self.directory / 'beads.jsonl'
        jsonl.write_text(''.join(json.dumps(b) + '\n' for b in BEADS))
        for option, path in (('--beads', self.beads), ('--file', jsonl)):
            result = self.run_cli(option, str(path), '--refresh')
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            self.assertFalse(self.html.exists())

    def test_refresh_and_offline_ci(self):
        result = self.run_cli('--refresh')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertTrue(self.html.exists(), result.stderr)
        self.bd.write_text('#!/bin/sh\nexit 99\n')
        result = self.run_cli('--ci')
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.write_plan(ROWS[:1])
        result = self.run_cli('--ci')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('sprint set differs', result.stdout + result.stderr)

    def test_selected_phase_ignores_other_config_and_current_path(self):
        (self.repo / '.atm-bd/phase-other.toml').write_text('[malformed')
        (self.repo / '.atm-bd/current-phase.toml').write_text('root="x-phase-t"\nsprints="missing.jsonl"\n')
        for args in (('--root', 'x-phase-t'), ()):
            result = self.run_cli(*args, explicit=False)
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)


if __name__ == '__main__':
    unittest.main()
