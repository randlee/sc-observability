"""Canonical sprint membership; bead data is queried, never serialized here."""
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
        if not isinstance(row, dict) or set(row) != {'sprint'}:
            raise ValueError(f'{path}:{number}: expected sprint only')
        bid = row['sprint']
        if not isinstance(bid, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', bid) or bid in seen:
            raise ValueError(f'{path}:{number}: invalid or duplicate sprint id')
        seen.add(bid)
        rows.append(row)
    if not rows:
        raise ValueError(f'{path}: empty plan')
    return rows


def deps(bead, kind):
    return {e.get('depends_on_id') or e.get('id') for e in bead.get('dependencies', [])
            if (e.get('type') or e.get('dependency_type')) == kind}


def index(rows, beads, root):
    gates = {}
    for row in rows:
        dev = row['sprint']
        matches = [b['id'] for b in beads if 'stage:dev-sanity' in (b.get('labels') or [])
                   and (b.get('metadata') or {}).get('dev_bead') == dev
                   and dev in deps(b, 'blocks')]
        if len(matches) != 1:
            raise ValueError(f'{dev}: expected one sanity bead, found {matches}')
        gates[dev] = matches[0]
    return {'root_bead_id': root, 'sprints': [
        {'dev_bead_id': r['sprint'], 'sanity_bead_id': gates[r['sprint']],
         'depends_on_sanity_bead_ids': sorted(deps(next((b for b in beads if b['id'] == r['sprint']), {}), 'blocks') & set(gates.values()))} for r in rows]}


def alignment(rows, beads, root, runtime):
    from bead_schema import problems, SprintBead, SanityBead
    by_id = {b['id']: b for b in beads}
    planned = {r['sprint'] for r in rows}
    def is_sprint(b):
        labels = set(b.get('labels') or [])
        parent = b.get('parent') or next(iter(deps(b, 'parent-child')), None)
        return parent == root and bool(labels & {'stage:dev', 'stage:sprint'}) and not (
            labels & {'stage:fix', 'stage:finding'} or b.get('issue_type') == 'bug')
    actual = {b['id'] for b in beads if is_sprint(b)}
    for bid in sorted(actual - planned):
        yield f'{bid}: extra live sprint; replan required'
    for bid in sorted(planned - actual):
        yield f'{bid}: missing live sprint under {root}'
    for row in runtime['sprints']:
        for bid, model in ((row['dev_bead_id'], SprintBead), (row['sanity_bead_id'], SanityBead)):
            if bid in by_id:
                yield from problems(by_id[bid], model)
    since = by_id.get(root, {}).get('created_at', '')
    for b in beads:
        if (b.get('status') != 'closed' and not b.get('parent') and not deps(b, 'parent-child')
                and b.get('issue_type') != 'epic' and b.get('created_at', '') >= since):
            yield f"{b['id']}: task at the top level; parent it under its epic"


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
