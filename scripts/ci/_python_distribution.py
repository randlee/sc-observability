"""Strict identities and archive checks for immutable Python distributions."""
from __future__ import annotations

import json
import platform
import sys
import sysconfig
import tarfile
import zipfile
from pathlib import Path, PurePosixPath
from _hashing import digest

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


class DistributionError(ValueError):
    """A distribution failed an explicit qualification boundary."""


def validate_requires_python(value: str, minimum: str = '3.10') -> str:
    """Parse an open-ended lower-bound requirement, rejecting caps/exclusions."""
    from packaging.specifiers import InvalidSpecifier, SpecifierSet
    from packaging.version import Version

    if not isinstance(value, str) or not value.strip():
        raise DistributionError('Requires-Python must be a non-empty specifier')
    try:
        specifiers = list(SpecifierSet(value))
        minimum_version = Version(minimum)
    except InvalidSpecifier as error:
        raise DistributionError('invalid Requires-Python specifier') from error
    except Exception as error:
        raise DistributionError('invalid Python minimum version') from error
    if len(specifiers) != 1 or specifiers[0].operator != '>=':
        raise DistributionError('Requires-Python must be an open-ended lower bound')
    if Version(specifiers[0].version) != minimum_version:
        raise DistributionError(f'Requires-Python must be >= {minimum}')
    return str(specifiers[0])


def source_python_contract(root: Path) -> str:
    """Parse source metadata and retain the single expected wheel requirement."""
    try:
        pyproject = tomllib.loads((root / 'pyproject.toml').read_text())
        cargo = tomllib.loads((root / 'Cargo.toml').read_text())
    except (OSError, ValueError, TypeError) as error:
        raise DistributionError('source Python/Cargo metadata is not valid TOML') from error
    try:
        requires_python = pyproject['project']['requires-python']
        maturin_features = pyproject['tool']['maturin']['features']
        pyo3 = cargo['dependencies']['pyo3']
        cargo_features = pyo3.get('features', []) if isinstance(pyo3, dict) else []
    except (KeyError, TypeError) as error:
        raise DistributionError('source Python/Cargo metadata is incomplete') from error
    expected = validate_requires_python(requires_python)
    if (not isinstance(maturin_features, list) or not isinstance(cargo_features, list)
            or 'pyo3/abi3-py310' not in maturin_features or 'abi3-py310' not in cargo_features):
        raise DistributionError('source metadata is missing the abi3-py310 feature')
    return expected


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


def fault_paths(contract: dict) -> list[str]:
    paths = contract.get('fault_pytest_paths', [])
    if (not isinstance(paths, list) or any(not isinstance(path, str) for path in paths)
            or len(paths) != len(set(paths))):
        raise DistributionError('fault_pytest_paths must contain unique test file paths')
    for path in paths:
        confined(Path('/suite'), path)
        if not path.startswith('tests/') or not path.endswith('.py'):
            raise DistributionError('fault suite exclusions must be explicit files under tests/')
    return paths


def release_wheel(record: dict) -> dict:
    if (record.get('role') != 'production' or record.get('publication') != 'pending_B.7'
            or 'test-hooks' in record.get('maturin_features', [])):
        raise DistributionError('instrumented artifact cannot satisfy the production release wheel gate')
    return record


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
    return {**record, 'expected_requires_python': source_python_contract(root)}


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
    elif tag == 'win_arm64':
        from python_arm64 import is_pe_arm64
        valid = is_pe_arm64(data)
    else:
        valid = False
    if not valid:
        raise DistributionError('native executable architecture differs from wheel platform')


def inspect_wheel(wheel: Path, policy: dict, version: str,
                  expected_requires_python: str | None = None) -> dict:
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
        metadata = [name for name in names if name.endswith('.dist-info/METADATA')]
        if len(metadata) != 1:
            raise DistributionError('ambiguous wheel metadata')
        from email.parser import Parser
        headers = Parser().parsestr(archive.read(metadata[0]).decode())
        values = headers.get_all('Requires-Python', [])
        if len(values) > 1:
            raise DistributionError('duplicate Requires-Python metadata')
        requires_python = values[0].strip() if values else None
        expected = expected_requires_python or policy.get('expected_requires_python', '>=3.10')
        if requires_python is None or validate_requires_python(requires_python) != validate_requires_python(expected):
            raise DistributionError('source and wheel Requires-Python metadata disagree')
    return {'wheel': wheel.name, 'sha256': digest(wheel), 'tags': sorted(map(str, tags)),
            'native_member': native[0], 'expected_requires_python': requires_python}


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
