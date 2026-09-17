#!/usr/bin/env python3
"""Run the packed client and packaged adapter through real desktop webview IPC.

Preparation can use the network. Every Cargo build, dependency inspection and
webview execution in the proof uses the B.4a checkout/cache/network sandbox.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

from _python_sandbox import Sandbox, registered_checkouts
from build_binding_source_bundle import build, digest, verify_bundle, registry_identities

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'scripts/ci/fixtures/tauri-qualification'


def run(arguments, cwd, log):
    command = [str(arg) for arg in arguments]
    if os.name == 'nt' and command[0] == 'npm':
        command[0] = 'npm.cmd'
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    log.append({'command': command, 'exit_code': result.returncode,
                'stdout': result.stdout, 'stderr': result.stderr})
    if result.returncode:
        raise RuntimeError(f'{command}:\n{result.stdout}\n{result.stderr}')
    return result.stdout


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError(f'production host observation anchor changed: {old!r}')
    return source.replace(old, new, 1)


def stage_host(destination, bundle, report):
    """Preserve command source exactly; add only independent observation hooks."""
    source = ROOT / 'examples/tauri-logging/src-tauri'
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns('target', 'gen'))
    original = (source / 'src/main.rs').read_text()
    instrumented = 'mod qualification;\n' + original
    # Inner crate attributes must remain at the beginning of the file.
    instrumented = original.replace('#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]',
                                    '#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]\nmod qualification;', 1)
    instrumented = replace_once(instrumented, '    let policy = AdapterPolicy {',
        '    if let Err(error) = qualification::seed(&backend) {\n        eprintln!("{error}");\n        std::process::exit(1);\n    }\n    let policy = AdapterPolicy {')
    instrumented = replace_once(instrumented,
        '.invoke_handler(tauri::generate_handler![app_observability_level_change])',
        '.invoke_handler(tauri::generate_handler![app_observability_level_change, qualification::qualification_report])\n        .setup(qualification::setup)')
    (destination / 'src/main.rs').write_text(instrumented)
    shutil.copyfile(FIXTURE / 'qualification.rs', destination / 'src/qualification.rs')
    report['host_source_sha256'] = digest(source / 'src/main.rs')
    report['instrumented_host_sha256'] = digest(destination / 'src/main.rs')
    report['observation_module_sha256'] = digest(destination / 'src/qualification.rs')
    config = json.loads((destination / 'tauri.conf.json').read_text())
    config['app']['withGlobalTauri'] = True
    config['bundle']['icon'] = ['icons/icon.png']
    (destination / 'tauri.conf.json').write_text(json.dumps(config, indent=2))
    manifest = (destination / 'Cargo.toml').read_text()
    entries = {entry['name']: entry for entry in bundle['packages']}
    for name, entry in entries.items():
        pattern = r'(?m)^' + re.escape(name) + r'\s*=\s*\{[^\n]*\}'
        match = re.search(pattern, manifest)
        if match:
            dependency = re.sub(r'path\s*=\s*"[^"]*",?\s*', '', match.group())
            dependency = re.sub(r'version\s*=\s*"[^"]*",?\s*', '', dependency)
            dependency = dependency.replace('{', '{ version = "=' + entry['version'] + '",', 1)
            manifest = manifest[:match.start()] + dependency + manifest[match.end():]
    manifest += '\n[patch.crates-io]\n' + '\n'.join(
        f'{name} = {{ path = "../bundle/{entry["root"]}" }}' for name, entry in entries.items()) + '\n'
    (destination / 'Cargo.toml').write_text(manifest)
    if re.search(r'path\s*=\s*"\.\./\.\./', manifest):
        raise RuntimeError('checkout dependency remained in external host manifest')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--bundle', type=Path)
    args = parser.parse_args()
    output = args.evidence.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {'schema_version': 1, 'status': 'failed', 'platform': platform.system(),
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'commands': [], 'publication': 'pending_B.7'}
    try:
        with tempfile.TemporaryDirectory(prefix='tauri-artifact-consumer-') as temporary:
            external = Path(temporary).resolve()
            artifact = external / 'bundle'
            if args.bundle:
                verify_bundle(args.bundle)
                shutil.copytree(args.bundle, artifact)
            else:
                build(ROOT / 'bindings/tauri/Cargo.toml', artifact)
            bundle = verify_bundle(artifact)
            if bundle['root_package'] != 'sc-observability-tauri':
                raise RuntimeError('qualification requires a packaged Tauri adapter bundle')
            shutil.copyfile(artifact / 'manifest.json', output / 'bundle-manifest.json')
            shutil.copytree(artifact / 'archives', output / 'rust-archives', dirs_exist_ok=True)
            report['bundle_manifest_sha256'] = digest(artifact / 'manifest.json')
            report['rust_archives'] = {entry['name']: entry['archive_sha256'] for entry in bundle['packages']}
            report['bundle_source_commit'] = bundle['source_commit']
            if bundle['source_commit'] != report['source_commit']:
                raise RuntimeError('bundle source revision differs from qualification revision')
            commands = report['commands']
            package = ROOT / 'bindings/typescript'
            run(['npm', 'ci', '--ignore-scripts'], package, commands)
            run(['npm', 'run', 'build'], package, commands)
            run(['npm', 'test'], package, commands)
            run(['npm', 'pack', '--pack-destination', output], package, commands)
            archives = list(output.glob('*.tgz'))
            if len(archives) != 1:
                raise RuntimeError('exactly one npm package archive is required')
            archive = archives[0]
            report['npm_archive'] = {'filename': archive.name, 'sha256': digest(archive)}
            consumer = external / 'frontend'
            shutil.copytree(FIXTURE, consumer)
            run(['npm', 'ci', '--ignore-scripts'], consumer, commands)
            run(['npm', 'install', '--ignore-scripts', '--no-save', archive], consumer, commands)
            shutil.copyfile(ROOT / 'bindings/conformance/v1/schema-cases.json', consumer / 'schema-cases.json')
            try:
                run(['node', 'faults.mjs'], consumer, commands)
            finally:
                if (consumer / 'fault-results.json').exists():
                    shutil.copyfile(consumer / 'fault-results.json', output / 'fault-results.json')
            run(['node', 'node_modules/typescript/bin/tsc', '--strict', '--noEmit', '--target', 'ES2022',
                 '--moduleResolution', 'node', 'narrowing.ts'], consumer, commands)
            dist = external / 'dist'
            dist.mkdir()
            run(['node', 'node_modules/esbuild/bin/esbuild', 'frontend.js', '--bundle', '--platform=browser',
                 '--outfile=' + str(dist / 'qualification.js')], consumer, commands)
            (dist / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Real Tauri qualification</title><script src="qualification.js"></script>')
            host = external / 'host'
            stage_host(host, bundle, report)
            # The example's reviewed lock owns its extra OS webview dependencies.
            reviewed = registry_identities(tomllib.loads((host / 'Cargo.lock').read_text()))
            run(['cargo', 'metadata', '--offline', '--format-version', '1'], host, commands)
            selected = registry_identities(tomllib.loads((host / 'Cargo.lock').read_text()))
            source_registry = {(entry['name'], entry['version']): entry for entry in reviewed}
            for entry in selected:
                if source_registry.get((entry['name'], entry['version'])) != entry:
                    raise RuntimeError(f'consumer registry selection drift: {entry}')
            report['registry_selection'] = selected
            config = run(['cargo', 'vendor', '--locked', 'vendor'], host, commands)
            (host / '.cargo').mkdir(exist_ok=True)
            (host / '.cargo/config.toml').write_text(config)
            report['host_lock_sha256'] = digest(host / 'Cargo.lock')
            shutil.copyfile(host / 'Cargo.lock', output / 'host-Cargo.lock')
            raw_report = external / 'ipc.json'
            sandbox = Sandbox(external, registered_checkouts(ROOT))
            try:
                with sandbox:
                    report['isolation'] = sandbox.prove_denials(sys.executable, ROOT)
                    sandbox.env['SC_TAURI_QUALIFICATION_REPORT'] = str(raw_report)
                    metadata = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline', '--format-version', '1'], host))
                    for item in metadata['packages']:
                        if not Path(item['manifest_path']).resolve().is_relative_to(external):
                            raise RuntimeError('dependency escaped external consumer: ' + item['manifest_path'])
                    report['resolved_dependencies'] = [{key: item[key] for key in ('name', 'version', 'source', 'manifest_path')} for item in metadata['packages']]
                    sandbox.run([sandbox.cargo, 'build', '--locked', '--offline'], host)
                    executable = Path(sandbox.env['CARGO_TARGET_DIR']) / 'debug' / ('tauri-logging-example.exe' if os.name == 'nt' else 'tauri-logging-example')
                    report['executable_sha256'] = digest(executable)
                    sandbox.run([str(executable)], host)
            finally:
                report['commands'].extend(sandbox.commands)
                if raw_report.exists():
                    shutil.copyfile(raw_report, output / 'ipc.json')
                if (host / 'logs').exists():
                    shutil.copytree(host / 'logs', output / 'logs', dirs_exist_ok=True)
            ipc = json.loads(raw_report.read_text())
            if set(ipc) != {'main', 'forbidden'} or not all(record['passed'] for record in ipc.values()):
                raise RuntimeError('incomplete or failed actual-webview qualification')
            report['ipc_sha256'] = digest(output / 'ipc.json')
            report['case_count'] = sum(len(item['records']) for item in ipc.values())
            jsonl = list((output / 'logs').rglob('*.jsonl'))
            if not jsonl:
                raise RuntimeError('real JSONL artifact missing')
            report['jsonl'] = {str(path.relative_to(output)): digest(path) for path in jsonl}
            report['status'] = 'passed'
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        (output / 'platform.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print('TAURI_REAL_IPC_ARTIFACT_QUALIFICATION_PASSED ' + str(output / 'platform.json'))


if __name__ == '__main__':
    main()
