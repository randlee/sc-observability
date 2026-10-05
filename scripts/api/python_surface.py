"""Reflect the installed Python facade. No source parsing or builds.

Runtime signatures/annotations and public class members are covered. Static-only
.pyi declarations, dynamically manufactured attributes and behavior are outside
this projection; existing typing and native qualification remain independent.
"""
from __future__ import annotations

import dataclasses
import enum
import importlib
import inspect
import json
from pathlib import Path
import pkgutil
import typing
import types

try:
    from api_rows import row_differences
except ModuleNotFoundError:
    from scripts.api.api_rows import row_differences


def value_name(value):
    if value is inspect.Signature.empty:
        return '<absent>'
    if value is dataclasses.MISSING:
        return '<missing>'
    if value is dataclasses._HAS_DEFAULT_FACTORY:
        return '<factory>'
    if value is None or isinstance(value, (str, int, float, bool)):
        return repr(value)
    if isinstance(value, enum.Enum):
        return type(value).__module__ + '.' + type(value).__qualname__ + '.' + value.name
    origin = typing.get_origin(value)
    if origin is not None and origin is not value:
        return ('Union' if origin in (types.UnionType, typing.Union) else value_name(origin)) + '[' + ','.join(value_name(arg) for arg in typing.get_args(value)) + ']'
    if isinstance(value, typing.TypeVar):
        return 'TypeVar(' + value.__name__ + ',' + value_name(value.__bound__) + ',' + repr(value.__covariant__) + ',' + repr(value.__contravariant__) + ',' + ','.join(value_name(x) for x in value.__constraints__) + ')'
    if isinstance(value, (tuple, list)):
        return type(value).__name__ + '(' + ','.join(value_name(x) for x in value) + ')'
    if isinstance(value, dict):
        return '{' + ','.join(value_name(k) + ':' + value_name(v) for k, v in sorted(value.items())) + '}'
    if hasattr(value, '__qualname__'):
        return getattr(value, '__module__', getattr(getattr(value, '__objclass__', None), '__module__', 'builtins')) + '.' + value.__qualname__
    if type(value).__module__ == 'typing':
        return str(value)
    return '<' + type(value).__module__ + '.' + type(value).__qualname__ + '>'


def signature(value):
    sig = inspect.signature(value)
    return '(' + ','.join(p.name + ':' + p.kind.name + ':' + value_name(p.annotation) + '=' + value_name(p.default)
                          for p in sig.parameters.values()) + ')->' + value_name(sig.return_annotation)


def project(package):
    rows = set()
    active = set()

    def visit(path, value):
        if (hasattr(value, '__qualname__') and getattr(value, '__module__', '').split('.')[0] not in (package.__name__, '')
                and typing.get_origin(value) is None and not isinstance(value, typing.TypeVar)):
            rows.add('external ' + path + ' = ' + value_name(value))
            return
        if inspect.ismodule(value):
            rows.add('module ' + path)
        elif inspect.isclass(value):
            rows.add('class ' + path + ' = ' + value_name(value))
            if not value.__module__.startswith(package.__name__) or value in active:
                return
            active.add(value)
            rows.add('bases ' + path + ' ' + ','.join(value_name(x) for x in value.__bases__))
            if getattr(value.__init__, '__module__', '').startswith(package.__name__):
                rows.add('constructor ' + path + ' ' + signature(value))
            else:
                rows.add('constructor ' + path + ' inherited=' + value_name(value.__init__))
            for name, annotation in sorted(getattr(value, '__annotations__', {}).items()):
                if not name.startswith('_'):
                    rows.add('field ' + path + '.' + name + ': ' + value_name(annotation))
            if dataclasses.is_dataclass(value):
                rows.add('dataclass ' + path + ' ' + ','.join(name + '=' + str(getattr(value.__dataclass_params__, name)) for name in ('init', 'repr', 'eq', 'order', 'unsafe_hash', 'frozen')))
                for field in dataclasses.fields(value):
                    rows.add('default ' + path + '.' + field.name + ' ' + value_name(field.default) + ' type=' + value_name(field.type) + ' factory=' + value_name(field.default_factory) + ' init=' + str(field.init))
            for name in sorted(dir(value)):
                member = inspect.getattr_static(value, name)
                if name.startswith('_'):
                    continue
                if isinstance(member, (classmethod, staticmethod)):
                    if not (getattr(member.__func__, '__module__', '') or '').startswith(package.__name__):
                        continue
                    rows.add(type(member).__name__ + ' ' + path + '.' + name)
                    member = member.__func__
                if isinstance(member, property):
                    rows.add('property ' + path + '.' + name + ' ' + signature(member.fget) + ' setter=' + (signature(member.fset) if member.fset else '<absent>'))
                elif inspect.isfunction(member) and member.__module__.startswith(package.__name__):
                    visit(path + '.' + name, member)
                elif isinstance(member, enum.Enum):
                    rows.add('enum ' + path + '.' + name + '=' + value_name(member.value))
                elif name in vars(value):
                    rows.add('member ' + path + '.' + name + '=' + value_name(member))
            active.remove(value)
        elif inspect.isfunction(value):
            rows.add('function ' + path + ' ' + signature(value) + ' async=' + str(inspect.iscoroutinefunction(value)))
        else:
            rows.add('value ' + path + ' = ' + value_name(value))

    modules = [package]
    if hasattr(package, '__path__'):
        modules += [importlib.import_module(item.name) for item in pkgutil.walk_packages(package.__path__, package.__name__ + '.')
                    if not any(part.startswith('_') for part in item.name.split('.'))]
    for module in modules:
        names = getattr(module, '__all__', [name for name in vars(module) if not name.startswith('_')])
        for name in sorted(names):
            visit(module.__name__ + '.' + name, getattr(module, name))
    return sorted(rows)


def compare(rows, path, version):
    expected = json.loads(Path(path).read_text(encoding='utf-8'))
    assert expected['version'] == version, 'installed Python version differs from selected history'
    assert expected['format'] == 'python-runtime/v1', 'unsupported Python API snapshot format'
    if rows != expected['rows']:
        differences = row_differences(expected['rows'], rows)
        raise AssertionError('Python API differs for ' + version + '\n' + '\n'.join(differences[:15]) + '\nIncrement the package version and capture at release-cut; retain accepted history.')
