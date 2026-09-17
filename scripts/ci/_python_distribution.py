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


def runtime_options(contract: dict) -> tuple[list[str], dict[str, str]]:
    """Only strengthening interpreter settings are configurable by later suites."""
    flags, environment = ['-I'], {}
    for key in ('asyncio_debug', 'warnings_as_errors', 'embedding_in_each_cell'):
        if key in contract and type(contract[key]) is not bool:
            raise DistributionError(f'{key} must be a boolean')
    if contract.get('asyncio_debug'):
        flags += ['-X', 'dev']
        environment['PYTHONASYNCIODEBUG'] = '1'
    if contract.get('warnings_as_errors'):
        flags += ['-W', 'error']
        environment['PYTHONWARNINGS'] = 'error'
    return flags, environment


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


def verify_native_architecture(data: bytes, tag: str) -> None:
    """Inspect executable headers independently of the wheel's claimed tag."""
    if tag.startswith('manylinux_'):
        expected = 183 if tag.endswith('aarch64') else 62
        valid = (len(data) >= 20 and data[:6] == b'\x7fELF\x02\x01'
                 and int.from_bytes(data[18:20], 'little') == expected)
    elif tag.startswith('macosx_'):
        expected = 0x100000c if tag.endswith('arm64') else 0x1000007
        valid = (len(data) >= 8 and data[:4] == b'\xcf\xfa\xed\xfe'
                 and int.from_bytes(data[4:8], 'little') == expected)
    elif tag == 'win_amd64':
        offset = int.from_bytes(data[60:64], 'little') if len(data) >= 64 else len(data)
        valid = (data[:2] == b'MZ' and data[offset:offset + 4] == b'PE\0\0'
                 and int.from_bytes(data[offset + 4:offset + 6], 'little') == 0x8664)
    else:
        valid = False
    if not valid:
        raise DistributionError('native executable architecture differs from wheel platform')


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
        verify_native_architecture(archive.read(native[0]), policy['wheel_platform'])
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
