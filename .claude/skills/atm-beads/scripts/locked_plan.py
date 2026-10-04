"""Canonical sprint membership; bead data is queried, never serialized here."""
from plan_contract import (DEV_LABEL, SPRINT_LABEL, SANITY_LABEL, FIX_LABEL, FINDING_LABEL,
                           BLOCKS_RELATION, PARENT_CHILD_RELATION, PROBLEM_LINE)
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET


def load(path):
    rows = []
    seen = set()
    for number, line in enumerate(Path(path).read_text().splitlines(), 1):
        if not line.strip():
            continue
        row = json.loads(line)
        if not isinstance(row, dict) or not {'sprint'} <= set(row) or set(row) - {'sprint', 'depends_on'}:
            raise ValueError(f'{path}:{number}: expected sprint and optional depends_on')
        bid = row['sprint']
        if not isinstance(bid, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', bid) or bid in seen:
            raise ValueError(f'{path}:{number}: invalid or duplicate sprint id')
        dependencies = row.get('depends_on', [])
        if not isinstance(dependencies, list) or any(not isinstance(d, str) for d in dependencies) or len(set(dependencies)) != len(dependencies):
            raise ValueError(f'{path}:{number}: invalid depends_on')
        seen.add(bid)
        rows.append(row)
    if not rows:
        raise ValueError(f'{path}: empty plan')
    for row in rows:
        if any(d not in seen or d == row['sprint'] for d in row.get('depends_on', [])):
            raise ValueError(f"{row['sprint']}: unknown or self dependency")
    visiting, visited = set(), set()
    by_id = {r['sprint']: r for r in rows}
    def visit(bid):
        if bid in visiting:
            raise ValueError(f'{bid}: cyclic depends_on')
        if bid in visited:
            return
        visiting.add(bid)
        for dep in by_id[bid].get('depends_on', []):
            visit(dep)
        visiting.remove(bid)
        visited.add(bid)
    for bid in by_id:
        visit(bid)
    return rows


def deps(bead, kind):
    return {e.get('depends_on_id') or e.get('id') for e in bead.get('dependencies', [])
            if (e.get('type') or e.get('dependency_type')) == kind}


def index(rows, beads, root):
    gates = {}
    for row in rows:
        dev = row['sprint']
        matches = [b['id'] for b in beads if (SANITY_LABEL in (b.get('labels') or []) or b['id'] in {dev + '-sanity', dev + '.sanity'})
                   and dev in deps(b, BLOCKS_RELATION)]
        if len(matches) != 1:
            raise ValueError(f'{dev}: expected one sanity bead, found {matches}')
        gates[dev] = matches[0]
    return {'root_bead_id': root, 'sprints': [
        {'dev_bead_id': r['sprint'], 'sanity_bead_id': gates[r['sprint']],
         'depends_on_sanity_bead_ids': sorted(deps(next((b for b in beads if b['id'] == r['sprint']), {}), BLOCKS_RELATION) & set(gates.values()))} for r in rows]}


def alignment(rows, beads, root, runtime=None):
    by_id = {b['id']: b for b in beads}
    planned = {r['sprint'] for r in rows}
    def is_sprint(b):
        labels = set(b.get('labels') or [])
        parent = b.get('parent') or next(iter(deps(b, PARENT_CHILD_RELATION)), None)
        return parent == root and bool(labels & {DEV_LABEL, SPRINT_LABEL}) and not (
            labels & {FIX_LABEL, FINDING_LABEL} or b.get('issue_type') == 'bug')
    actual = {b['id'] for b in beads if is_sprint(b)}
    for bid in sorted(actual - planned):
        yield PROBLEM_LINE.format(bead=bid, message='extra live sprint; replan required')
    for bid in sorted(planned - set(by_id)):
        yield PROBLEM_LINE.format(bead=bid, message=f'missing live sprint under {root}')
    for row in rows:
        for predecessor in row.get('depends_on', []):
            gates = {b['id'] for b in beads if predecessor in deps(b, BLOCKS_RELATION)
                     and (SANITY_LABEL in (b.get('labels') or []) or b['id'] in {predecessor + '-sanity', predecessor + '.sanity'})}
            if not deps(by_id.get(row['sprint'], {}), BLOCKS_RELATION) & (gates | {predecessor}):
                yield PROBLEM_LINE.format(bead=row['sprint'], message=f'depends_on {predecessor} lacks edge to predecessor sanity or sprint')


def annotate(svg, runtime):
    tree = ET.fromstring(svg)
    pairs = {r['dev_bead_id']: r['sanity_bead_id'] for r in runtime['sprints']}
    owner = {gate: dev for dev, gate in pairs.items()}
    for node in tree.iter('{http://www.w3.org/2000/svg}g'):
        if node.get('class') == 'node':
            title = node.find('{http://www.w3.org/2000/svg}title').text.split(' — ', 1)[0]
            node.set('data-sprint', owner.get(title, title))
            node.set('data-kind', 'sanity' if title in owner else 'sprint')
    tree.set('data-phase-root', runtime['root_bead_id'])
    return ET.tostring(tree, encoding='unicode')


def check_html(rows, html, root):
    match = re.search(r'<svg\b.*?</svg>', html, re.S)
    if not match:
        raise ValueError('DAG HTML has no SVG')
    svg = ET.fromstring(match.group())
    if svg.get('data-phase-root') != root:
        raise ValueError('DAG root differs from configured root')
    seen = set()
    ns = '{http://www.w3.org/2000/svg}'
    for group in svg.iter(ns + 'g'):
        if group.get('class') != 'node' or group.get('data-kind') != 'sprint':
            continue
        title = group.find(ns + 'title')
        if title is None or not title.text:
            raise ValueError('DAG sprint node missing title')
        bid = title.text.split(' — ', 1)[0]
        if bid != group.get('data-sprint') or bid in seen:
            raise ValueError('DAG has invalid or duplicate sprint node')
        seen.add(bid)
    if seen != {r['sprint'] for r in rows}:
        raise ValueError('DAG sprint set differs from plan')
