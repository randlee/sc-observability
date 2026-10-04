"""Resolve a configured canonical plan and its co-located diagram."""
from pathlib import Path
import tomllib


def load(repo, config):
    repo = Path(repo).resolve()
    config = Path(config)
    config = config if config.is_absolute() else repo / config
    data = tomllib.loads(config.read_text())
    for key in ('root', 'sprints'):
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
    current = repo / '.atm-bd/current-phase.toml'
    if current.exists() and (not root or load(repo, current)[0] == root):
        return current
    candidates = [p for p in (repo / '.atm-bd').glob('*.toml')
                  if p.name != 'current-phase.toml' and (not root or load(repo, p)[0] == root)]
    if len(candidates) != 1:
        raise ValueError('pass --config .atm-bd/<phase>.toml; no unique phase configuration')
    return candidates[0]
