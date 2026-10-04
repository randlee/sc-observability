"""Canonical sprint membership/edges; bead data is queried, never serialized here."""
from graphlib import TopologicalSorter, CycleError
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET


def prerequisites(row):
    return row['depends_on']


def metrics(rows):
    graph = {r['sprint']: set(prerequisites(r)) for r in rows}
    order = tuple(TopologicalSorter(graph).static_order())
    depths, ancestors = {}, {}
    for node in order:
        depths[node] = 1 + max((depths[d] for d in graph[node]), default=0)
        ancestors[node] = set(graph[node])
        for dep in graph[node]:
            ancestors[node].update(ancestors[dep])
    # Dilworth: width = nodes - maximum matching in the transitive DAG.
    matching = {}
    def augment(node, visited):
        for dep in sorted(ancestors[node]):
            if dep in visited:
                continue
            visited.add(dep)
            if dep not in matching or augment(matching[dep], visited):
                matching[dep] = node
                return True
        return False
    count = sum(augment(node, set()) for node in order)
    return max(depths.values(), default=0), len(rows) - count


def load(path):
    rows = []
    seen = set()
    for number, line in enumerate(Path(path).read_text().splitlines(), 1):
        if not line.strip():
            continue
        row = json.loads(line)
        if not isinstance(row, dict) or set(row) != {'sprint', 'depends_on'}:
            raise ValueError(f'{path}:{number}: expected sprint and depends_on only')
        bid = row['sprint']
        if not isinstance(bid, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', bid) or bid in seen:
            raise ValueError(f'{path}:{number}: invalid or duplicate sprint id')
        seen.add(bid)
        deps = row['depends_on']
        if not isinstance(deps, list) or any(not isinstance(d, str) or not d.strip() for d in deps):
            raise ValueError(f'{bid}: depends_on must be an array of sprint ids')
        if len(set(deps)) != len(deps) or bid in deps:
            raise ValueError(f'{bid}: duplicate or self dependency')
        rows.append(row)
    if not rows:
        raise ValueError(f'{path}: empty plan')
    for row in rows:
        unknown = set(prerequisites(row)) - seen
        if unknown:
            raise ValueError(f"{row['sprint']}: unknown prerequisites {sorted(unknown)}")
    try:
        tuple(TopologicalSorter({r['sprint']: prerequisites(r) for r in rows}).static_order())
    except CycleError as exc:
        raise ValueError('plan contains a dependency cycle') from exc
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
         'depends_on_sanity_bead_ids': [gates[d] for d in prerequisites(r)]} for r in rows]}


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
    # Include QA/finding gates belonging to another sprint, so a dependency
    # cannot hide by pointing at QA instead of the predecessor's sanity.
    owners = {bid: bid for bid in planned | actual}
    changed = True
    while changed:
        changed = False
        for b in beads:
            parent = b.get('parent') or next(iter(deps(b, 'parent-child')), None)
            meta = b.get('metadata') or {}
            owner = owners.get(parent) or owners.get(meta.get('dev_bead')) or owners.get(meta.get('checked_bead'))
            if owner and b['id'] not in owners:
                owners[b['id']] = owner
                changed = True
    for row in runtime['sprints']:
        dev, gate = row['dev_bead_id'], row['sanity_bead_id']
        for bid, model in ((dev, SprintBead), (gate, SanityBead)):
            if bid in by_id:
                yield from problems(by_id[bid], model)
        expected = set(row['depends_on_sanity_bead_ids'])
        have = {d for d in deps(by_id.get(dev, {}), 'blocks') if d in owners and owners[d] != dev}
        for dep in sorted(expected - have):
            yield f'{dev}: missing sprint dependency on {dep}'
        for dep in sorted(have - expected):
            yield f'{dev}: extra sprint dependency on {dep}; replan required'
    allowed = {(r['dev_bead_id'], d) for r in runtime['sprints'] for d in r['depends_on_sanity_bead_ids']}
    for b in beads:
        if b['id'] not in owners:
            continue
        for target in deps(b, 'blocks'):
            if target in owners and owners[b['id']] != owners[target] and (b['id'], target) not in allowed:
                yield f"{b['id']}: cross-sprint edge to {target} must be a planned dev-to-sanity dependency"
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
    nodes, pairs, edges = {}, {}, set()
    ns = '{http://www.w3.org/2000/svg}'
    for group in svg.iter(ns + 'g'):
        if group.get('class') not in ('node', 'edge'):
            continue
        title = group.find(ns + 'title')
        if title is None or not title.text:
            raise ValueError('DAG node/edge missing title')
        title.text = title.text.split(' — ', 1)[0]
        if group.get('class') == 'edge':
            edge = tuple(title.text.split('->'))
            if len(edge) != 2 or edge in edges:
                raise ValueError('DAG has invalid or duplicate edge')
            edges.add(edge)
        else:
            kind, sprint = group.get('data-kind'), group.get('data-sprint')
            if kind not in ('sprint', 'sanity') or not sprint or title.text in nodes:
                raise ValueError('DAG has invalid or duplicate node')
            nodes[title.text] = (kind, sprint)
            if kind == 'sanity':
                if sprint in pairs:
                    raise ValueError('DAG has duplicate sanity pairing')
                pairs[sprint] = title.text
            elif title.text != sprint:
                raise ValueError('DAG sprint identity differs from node title')
    expected_sprints = {r['sprint'] for r in rows}
    if {bid for bid, (kind, _) in nodes.items() if kind == 'sprint'} != expected_sprints or set(pairs) != expected_sprints:
        raise ValueError('DAG sprint set differs from plan')
    expected_edges = {(dev, gate) for dev, gate in pairs.items()}
    expected_edges |= {(pairs[d], r['sprint']) for r in rows for d in prerequisites(r)}
    if edges != expected_edges:
        raise ValueError('DAG edges differ from plan')
