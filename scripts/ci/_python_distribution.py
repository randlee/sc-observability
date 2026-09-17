"""Strict identities and archive checks for immutable Python distributions."""
from __future__ import annotations

import hashlib
import json
import platform
import sys
import sysconfig
import tarfile
import zipfile
from pathlib import Path, PurePosixPath

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


class DistributionError(ValueError):
    """A distribution failed an explicit qualification boundary."""


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def confined(root: Path, relative: str) -> Path:
    value = PurePosixPath(relative)
    if (not relative or value.is_absolute() or '..' in value.parts
            or '\\' in relative or ':' in relative):
        raise DistributionError(f'unsafe artifact path: {relative}')
    path = root / value
    if not path.resolve().is_relative_to(root.resolve()):
        raise DistributionError(f'escaping artifact path: {relative}')
    return path


def extract_sdist(archive: Path, output: Path) -> Path:
    """Extract only unique ordinary files, with a single identity root."""
    output.mkdir(parents=True, exist_ok=False)
    with tarfile.open(archive, 'r:gz') as stream:
        names, roots = set(), set()
        for member in stream.getmembers():
            target = confined(output, member.name)
            if member.name in names or not (member.isfile() or member.isdir()):
                raise DistributionError(f'nonregular or duplicate sdist member: {member.name}')
            names.add(member.name)
            roots.add(PurePosixPath(member.name).parts[0])
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                content = stream.extractfile(member)
                if content is None:
                    raise DistributionError(f'unreadable member: {member.name}')
                target.write_bytes(content.read())
        if len(roots) != 1:
            raise DistributionError('sdist must contain one root directory')
    return output / roots.pop()


def verify_source(root: Path) -> dict:
    """Check the frozen distribution inventory before Cargo or imports execute."""
    record = json.loads((root / 'distribution-manifest.json').read_text())
    if record.get('schema_version') != 1 or record.get('publication') != 'pending_B.7':
        raise DistributionError('invalid distribution manifest')
    if len(record.get('source_commit', '')) != 40:
        raise DistributionError('missing immutable source revision')
    for relative, expected in record['files'].items():
        path = confined(root, relative)
        if path.is_symlink() or not path.is_file() or digest(path) != expected:
            raise DistributionError(f'missing or tampered distribution member: {relative}')
    required = ('Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'pyproject.toml',
                'python/sc_observability/__init__.py', 'python/sc_observability/generated/__init__.pyi',
                'python/sc_observability/py.typed', 'rust-bundle/manifest.json')
    if not set(required) <= record['files'].keys():
        raise DistributionError('sdist inventory omits required package data')
    actual = {path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file()}
    unexpected = actual - set(record['files']) - {'distribution-manifest.json', 'PKG-INFO'}
    if unexpected:
        raise DistributionError(f'unrecorded distribution members: {sorted(unexpected)}')
    for path in root.rglob('*'):
        if path.is_symlink():
            raise DistributionError(f'symlink in unpacked distribution: {path}')
    return record


def inspect_wheel(wheel: Path, policy: dict, version: str) -> dict:
    from packaging.utils import parse_wheel_filename
    name, actual_version, _, tags = parse_wheel_filename(wheel.name)
    if name != 'sc-observability' or str(actual_version) != version:
        raise DistributionError('wrong wheel distribution identity')
    if not tags or any(tag.interpreter != 'cp310' or tag.abi != 'abi3'
                       or tag.platform != policy['wheel_platform'] for tag in tags):
        raise DistributionError(f'wrong wheel ABI/platform tags: {sorted(map(str, tags))}')
    required = {'sc_observability/__init__.py',
                'sc_observability/py.typed', 'sc_observability/generated/__init__.py',
                'sc_observability/generated/__init__.pyi'}
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise DistributionError('duplicate wheel member')
        for item in archive.infolist():
            confined(Path('/wheel'), item.filename)
            if (item.external_attr >> 16) & 0o170000 == 0o120000:
                raise DistributionError('wheel contains a symlink')
        if not required <= set(names):
            raise DistributionError(f'wheel missing package data: {sorted(required - set(names))}')
        native = [name for name in names if name.startswith('sc_observability/_native.')
                  and name.endswith(('.so', '.pyd'))]
        if len(native) != 1:
            raise DistributionError('wheel must contain exactly one native extension')
        manifests = [name for name in names if name.endswith('.dist-info/WHEEL')]
        if len(manifests) != 1:
            raise DistributionError('ambiguous wheel metadata')
        declared = {line.removeprefix('Tag: ') for line in archive.read(manifests[0]).decode().splitlines()
                    if line.startswith('Tag: ')}
        if declared != {str(tag) for tag in tags}:
            raise DistributionError('wheel filename and embedded tags disagree')
    return {'wheel': wheel.name, 'sha256': digest(wheel), 'tags': sorted(map(str, tags)),
            'native_member': native[0]}


def actual_cell(policy: dict) -> dict:
    python = f'{sys.version_info.major}.{sys.version_info.minor}'
    if python not in policy['interpreters'] or platform.python_implementation() != 'CPython':
        raise DistributionError('unsupported interpreter')
    if sysconfig.get_config_var('Py_GIL_DISABLED'):
        raise DistributionError('free-threaded Python is outside qualification')
    system, machine = platform.system(), platform.machine()
    platform_id = ({'Darwin': 'macos', 'Linux': 'linux', 'Windows': 'windows'}[system]
                   + '-' + {'AMD64': 'x86_64', 'ARM64': 'arm64'}.get(machine, machine))
    matches = [p for p in policy['platforms'] if p['id'] == platform_id]
    if len(matches) != 1:
        raise DistributionError(f'unsupported execution platform: {system}/{machine}')
    return {'python': python, 'python_full': sys.version, 'platform': platform_id,
            'platform_full': platform.platform(), 'machine': machine, 'gil_enabled': True}
