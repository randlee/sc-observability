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
from _tauri_webview import execute as execute_webview
from _tauri_build_inputs import inventory, materialize, verify_inputs
from build_binding_source_bundle import build, digest, verify_bundle, registry_identities

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'scripts/ci/fixtures/tauri-qualification'


def run(arguments, cwd, log):
    command = [str(arg) for arg in arguments]
    if os.name == 'nt' and command[0] == 'npm':
        command[0] = 'npm.cmd'
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, encoding='utf-8')
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
    original = (source / 'src/main.rs').read_text(encoding='utf-8')
    instrumented = 'mod qualification;\n' + original
    # Inner crate attributes must remain at the beginning of the file.
    instrumented = original.replace('#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]',
                                    '#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]\nmod qualification;', 1)
    instrumented = replace_once(instrumented, '    let config = sc_observability::LoggerConfig::default_for(service, PathBuf::from("logs"));',
        '    let mut config = sc_observability::LoggerConfig::default_for(service, PathBuf::from("logs"));\n    qualification::configure(&mut config);')
    instrumented = replace_once(instrumented, '    let policy = AdapterPolicy {',
        '    if let Err(error) = qualification::seed(&backend) {\n        eprintln!("{error}");\n        std::process::exit(1);\n    }\n    let policy = AdapterPolicy {')
    instrumented = replace_once(instrumented, '    let policy = AdapterPolicy {',
        '    let backend: Arc<dyn sc_observability_binding_runtime::HostLoggingBackend> = Arc::new(backend);\n    let policy = AdapterPolicy {')
    instrumented = replace_once(instrumented, 'plugin::<tauri::Wry>(Arc::new(backend), policy)',
        'plugin::<tauri::Wry>(Arc::clone(&backend), policy)')
    instrumented = replace_once(instrumented, 'let result = tauri::Builder::default()',
        'let result = tauri::Builder::default().manage(qualification::Backend(Arc::clone(&backend)))')
    instrumented = replace_once(instrumented,
        '.invoke_handler(tauri::generate_handler![app_observability_level_change])',
        '.invoke_handler(tauri::generate_handler![app_observability_level_change, qualification::qualification_report, qualification::qualification_owner_gate, qualification::qualification_output_gate, qualification::qualification_host_flush, qualification::qualification_shutdown])\n        .setup(qualification::setup)')
    (destination / 'src/main.rs').write_text(instrumented)
    build_script = (destination / 'build.rs').read_text(encoding='utf-8')
    build_script = replace_once(build_script, '.commands(&["app_observability_level_change"])',
        '.commands(&["app_observability_level_change", "qualification_report", "qualification_owner_gate", "qualification_output_gate", "qualification_host_flush", "qualification_shutdown"])')
    (destination / 'build.rs').write_text(build_script)
    capabilities = destination / 'capabilities'
    capabilities.mkdir(exist_ok=True)
    (capabilities / 'qualification-observation.json').write_text(json.dumps({
        'identifier': 'qualification-observation', 'windows': ['main', 'forbidden'],
        'permissions': ['allow-qualification-report', 'allow-qualification-owner-gate', 'allow-qualification-output-gate', 'allow-qualification-host-flush', 'allow-qualification-shutdown'],
    }, indent=2))
    shutil.copyfile(FIXTURE / 'qualification.rs', destination / 'src/qualification.rs')
    report['host_source_sha256'] = digest(source / 'src/main.rs')
    report['instrumented_host_sha256'] = digest(destination / 'src/main.rs')
    report['observation_module_sha256'] = digest(destination / 'src/qualification.rs')
    config = json.loads((destination / 'tauri.conf.json').read_text(encoding='utf-8'))
    config['app']['withGlobalTauri'] = True
    (destination / 'tauri.conf.json').write_text(json.dumps(config, indent=2))
    manifest = (destination / 'Cargo.toml').read_text(encoding='utf-8')
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
    parser.add_argument('--npm-archive', type=Path)
    parser.add_argument('--npm-manifest', type=Path)
    args = parser.parse_args()
    output = args.evidence.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {'source_dirty': subprocess.run(['git', 'diff', '--quiet', 'HEAD'], cwd=ROOT).returncode != 0,
              'schema_version': 1, 'status': 'failed', 'platform': platform.system(),
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'commands': [], 'publication': 'pending_B.7'}
    try:
        with tempfile.TemporaryDirectory(prefix='tauri-artifact-consumer-') as temporary:
            external = Path(temporary).resolve()
            pristine = external / 'pristine'
            pristine.mkdir()
            artifact = pristine / 'bundle'
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
            if bool(args.npm_archive) != bool(args.npm_manifest):
                raise RuntimeError('shared npm artifact requires its exact producer manifest')
            if args.npm_archive:
                producer = json.loads(args.npm_manifest.read_text(encoding='utf-8'))
                if producer['source_commit'] != report['source_commit'] or producer['sha256'] != digest(args.npm_archive):
                    raise RuntimeError('shared npm source revision or archive hash mismatch')
                shutil.copyfile(args.npm_archive, output / args.npm_archive.name)
                shutil.copyfile(args.npm_manifest, output / 'npm-producer.json')
            else:
                run(['npm', 'pack', '--pack-destination', output], package, commands)
            archives = list(output.glob('*.tgz'))
            if len(archives) != 1:
                raise RuntimeError('exactly one npm package archive is required')
            archive = archives[0]
            report['conformance_fixture_sha256'] = digest(ROOT / 'bindings/conformance/v1/schema-cases.json')
            report['npm_archive'] = {'filename': archive.name, 'sha256': digest(archive)}
            consumer = external / 'frontend'
            shutil.copytree(FIXTURE, consumer, ignore=shutil.ignore_patterns('node_modules'))
            shutil.copyfile(ROOT / 'examples/tauri-logging/src/main.ts', consumer / 'host-client.ts')
            report['host_client_source_sha256'] = digest(consumer / 'host-client.ts')
            run(['npm', 'ci', '--ignore-scripts'], consumer, commands)
            run(['npm', 'install', '--ignore-scripts', '--no-save', archive], consumer, commands)
            shutil.copyfile(ROOT / 'bindings/conformance/v1/schema-cases.json', consumer / 'schema-cases.json')
            run(['node', 'node_modules/esbuild/bin/esbuild', 'host-client.ts', '--bundle', '--platform=node', '--format=esm', '--external:@sc-observability/client', '--external:@tauri-apps/api/core', '--outfile=host-client.mjs'], consumer, commands)
            try:
                run(['node', 'faults.mjs'], consumer, commands)
            except RuntimeError as error:
                report['fault_error'] = str(error)
            finally:
                if (consumer / 'fault-results.json').exists():
                    shutil.copyfile(consumer / 'fault-results.json', output / 'fault-results.json')
            run(['node', 'node_modules/typescript/bin/tsc', '--strict', '--noEmit', '--target', 'ES2022',
                 '--moduleResolution', 'node', 'narrowing.ts', 'host-client.ts'], consumer, commands)
            dist = pristine / 'dist'
            dist.mkdir()
            run(['node', 'node_modules/esbuild/bin/esbuild', 'frontend.js', '--bundle', '--platform=browser',
                 '--outfile=' + str(dist / 'qualification.js')], consumer, commands)
            (dist / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Real Tauri qualification</title><script src="qualification.js"></script>')
            host = pristine / 'host'
            stage_host(host, bundle, report)
            # The example's reviewed lock owns its extra OS webview dependencies.
            reviewed = registry_identities(tomllib.loads((host / 'Cargo.lock').read_text(encoding='utf-8')))
            run(['cargo', 'fetch', '--locked', '--manifest-path', ROOT / 'examples/tauri-logging/src-tauri/Cargo.toml'], ROOT, commands)
            run(['cargo', 'metadata', '--offline', '--format-version', '1'], host, commands)
            selected = registry_identities(tomllib.loads((host / 'Cargo.lock').read_text(encoding='utf-8')))
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
            # Tauri's build script can regenerate permission documentation in
            # its own registry source. Never reuse that modified tree for the
            # next Cargo profile; both builds start from these exact bytes.
            pristine_files = inventory(pristine)
            (output / 'build-inputs.json').write_text(json.dumps({'files': pristine_files}, sort_keys=True) + '\n')
            report['pristine_inputs_sha256'] = digest(output / 'build-inputs.json')
            report['build_inputs'] = {}
            profile_hosts = {}
            for profile in ('debug', 'release'):
                materialize(pristine, external / (profile + '-build'), pristine_files)
            raw_report = external / 'ipc.json'
            sandbox = Sandbox(external, registered_checkouts(ROOT))
            for variable in ('XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME'):
                location = external / variable.lower()
                location.mkdir()
                sandbox.env[variable] = str(location)
            try:
                with sandbox:
                    report['isolation'] = sandbox.prove_denials(sys.executable, ROOT)
                    for profile in ('debug', 'release'):
                        profile_root = external / (profile + '-build')
                        verify_inputs(profile_root, pristine_files)
                        profile_host = profile_root / 'host'
                        profile_hosts[profile] = profile_host
                        verify_bundle(profile_root / 'bundle')
                        sandbox.env['CARGO_HOME'] = str(external / (profile + '-cargo-home'))
                        sandbox.env['CARGO_TARGET_DIR'] = str(external / (profile + '-cargo-target'))
                        runtime_output = external if profile == 'debug' else external / 'capped'
                        runtime_output.mkdir(exist_ok=True)
                        if profile == 'release':
                            (profile_root / 'dist/index.html').write_text('<!doctype html><meta charset="utf-8"><title>Capped native host</title><script>window.qualificationCapped=true</script><script src="qualification.js"></script>', encoding='utf-8')
                        report['build_inputs'][profile] = {
                            'pristine_inputs_sha256': report['pristine_inputs_sha256'],
                            'bundle_manifest_sha256': digest(profile_root / 'bundle/manifest.json'),
                            'host_lock_sha256': digest(profile_host / 'Cargo.lock'),
                            'build_root': str(profile_root),
                            'cargo_home': sandbox.env['CARGO_HOME'],
                            'cargo_target_dir': sandbox.env['CARGO_TARGET_DIR'],
                            'frontend_entry_sha256': digest(profile_root / 'dist/index.html'),
                        }
                        sandbox.env['SC_TAURI_QUALIFICATION_REPORT'] = str(runtime_output / 'ipc.json')
                        sandbox.env['SC_TAURI_QUALIFICATION_POLICY'] = str(external / 'policy-results.json')
                        metadata = json.loads(sandbox.run([sandbox.cargo, 'metadata', '--locked', '--offline', '--format-version', '1'], profile_host))
                        for item in metadata['packages']:
                            if not Path(item['manifest_path']).resolve().is_relative_to(profile_root):
                                raise RuntimeError('dependency escaped profile consumer: ' + item['manifest_path'])
                        report.setdefault('resolved_dependencies_by_profile', {})[profile] = [{key: item[key] for key in ('name', 'version', 'source', 'manifest_path')} for item in metadata['packages']]
                        if profile == 'debug':
                            report['resolved_dependencies'] = report['resolved_dependencies_by_profile'][profile]
                        command = [sandbox.cargo, 'build', '--locked', '--offline']
                        if profile == 'release':
                            command += ['--release', '--features', 'sc-observability-log/static_level_cap_test']
                        sandbox.run(command, profile_host)
                        executable = Path(sandbox.env['CARGO_TARGET_DIR']) / profile / ('tauri-logging-example.exe' if os.name == 'nt' else 'tauri-logging-example')
                        report['executable_sha256' if profile == 'debug' else 'capped_executable_sha256'] = digest(executable)
                        transitions = execute_webview(sandbox, executable, profile_host, runtime_output, runtime_output)
                        if profile == 'debug':
                            report['output_gate_transitions'] = transitions
                        verify_inputs(pristine, pristine_files)
            finally:
                report['commands'].extend(sandbox.commands)
                if (external / 'policy-results.json').exists():
                    shutil.copyfile(external / 'policy-results.json', output / 'policy-results.json')
                for runtime_log in external.glob('webview-*.log'):
                    shutil.copyfile(runtime_log, output / runtime_log.name)
                if raw_report.exists():
                    shutil.copyfile(raw_report, output / 'ipc.json')
                if (external / 'capped').exists():
                    shutil.copytree(external / 'capped', output / 'capped', dirs_exist_ok=True)
                for profile, built_host in profile_hosts.items():
                    if (built_host / 'logs').exists():
                        log_output = output / ('logs' if profile == 'debug' else 'capped/logs')
                        shutil.copytree(built_host / 'logs', log_output, dirs_exist_ok=True)
            ipc = json.loads(raw_report.read_text(encoding='utf-8'))
            if set(ipc) != {'main', 'forbidden'} or not all(record['passed'] for record in ipc.values()):
                raise RuntimeError('incomplete or failed actual-webview qualification')
            capped_ipc = json.loads((output / 'capped/ipc.json').read_text(encoding='utf-8'))
            if set(capped_ipc) != {'main', 'forbidden'} or not all(record['passed'] for record in capped_ipc.values()):
                raise RuntimeError('incomplete or failed capped native host qualification')
            report['capped_ipc_sha256'] = digest(output / 'capped/ipc.json')
            report['policy_results_sha256'] = digest(output / 'policy-results.json')
            report['fault_results_sha256'] = digest(output / 'fault-results.json')
            report['ipc_sha256'] = digest(output / 'ipc.json')
            report['case_count'] = sum(len(item['records']) for item in ipc.values())
            jsonl = list((output / 'logs').rglob('*.jsonl'))
            if not jsonl:
                raise RuntimeError('real JSONL artifact missing')
            report['private_host_record_persisted'] = any(json.loads(line).get('target') == 'host-private' for path in jsonl for line in path.read_text(encoding='utf-8').splitlines() if line.strip())
            if not report['private_host_record_persisted']:
                raise RuntimeError('private native host record missing; unfiltered-query authorization proof is incomplete')
            report['runtime_logs'] = {path.relative_to(output).as_posix(): digest(path) for path in output.rglob('webview-*.log')}
            report['jsonl'] = {path.relative_to(output).as_posix(): digest(path) for path in jsonl}
            if report.get('fault_error'):
                raise RuntimeError('packed client fault/conformance cases failed; see fault-results.json')
            final_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
            report['source_dirty'] |= subprocess.run(['git', 'diff', '--quiet', 'HEAD'], cwd=ROOT).returncode != 0
            if final_commit != report['source_commit']:
                raise RuntimeError('qualification source revision changed during execution')
            if report['source_dirty']:
                raise RuntimeError('qualification source has uncommitted edits; commit and rerun before claiming evidence')
            report['status'] = 'passed'
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        (output / 'platform.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print('TAURI_REAL_IPC_ARTIFACT_QUALIFICATION_PASSED ' + str(output / 'platform.json'))


if __name__ == '__main__':
    main()
