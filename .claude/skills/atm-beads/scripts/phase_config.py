"""Resolve a configured canonical plan and its co-located diagram."""
from pathlib import Path
import tomllib


def load(repo, config):
    repo = Path(repo).resolve()
    config = Path(config)
    config = config if config.is_absolute() else repo / config
    if config.name == 'current-phase.toml':
        return load(repo, select(repo))
    data = tomllib.loads(config.read_text())
    for key in ('root', 'sprints', 'integration_branch'):
        if not isinstance(data.get(key), str) or not data[key].strip():
            raise ValueError(f'{config}: missing {key}')
    path = Path(data['sprints'])
    if path.is_absolute() or '..' in path.parts:
        raise ValueError(f'{config}: sprints must be a repository-relative path')
    plan = (repo / path).resolve()
    if not plan.is_relative_to(repo) or plan.suffix != '.jsonl':
        raise ValueError(f'{config}: sprints must name a repository JSONL plan')
    return data['root'], plan, plan.with_name(plan.stem + '-dag.html')


def select(repo, root=None):
    repo = Path(repo)
    if not root:
        current = repo / '.atm-bd/current-phase.toml'
        if current.exists():
            root = tomllib.loads(current.read_text()).get('root')
    if root:
        phase = root.split('-phase-', 1)[-1]
        for name in (f'phase-{phase}.toml', f'{phase}.toml'):
            candidate = repo / '.atm-bd' / name
            if candidate.exists():
                return candidate
        raise ValueError(f'no phase TOML for {root}; pass --config')
    candidates = [p for p in (repo / '.atm-bd').glob('*.toml') if p.name != 'current-phase.toml']
    if len(candidates) != 1:
        raise ValueError('pass --config .atm-bd/<phase>.toml; no unique phase configuration')
    return candidates[0]
