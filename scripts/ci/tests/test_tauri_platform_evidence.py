"""Evidence must reject incomplete, stale, tampered and nonisolated runs."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import validate_tauri_platform_evidence as gate


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        fixture = Path(gate.__file__).resolve().parents[2] / 'bindings/conformance/v1/schema-cases.json'
        canonical = json.loads(fixture.read_text(encoding='utf-8'))
        for name in ('Darwin', 'Linux', 'Windows'):
            directory = self.root / name
            directory.mkdir()
            def write(relative, value):
                path = directory / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(value), encoding='utf-8')
                return gate.digest(path)
            def cases(names):
                return [{'name': name, 'passed': True} for name in sorted(names)]
            report = {
                'platform': name, 'status': 'passed', 'schema_version': 1,
                'source_dirty': False, 'private_host_record_persisted': True, 'source_commit': 'reviewed-source', 'case_count': 103,
                'isolation': {'checkout': True, 'cargo_cache': True, 'network': True},
                'npm_archive': {'filename': 'client.tgz', 'sha256': write('client.tgz', 'packed-client')},
                'host_lock_sha256': write('host-Cargo.lock', 'reviewed-lock'),
                'ipc_sha256': write('ipc.json', {
                    window: {'passed': True, 'uncaught': [], 'records': cases(required)}
                    for window, required in [('main', gate.REQUIRED_MAIN), ('forbidden', gate.REQUIRED_FORBIDDEN)]}),
                'capped_ipc_sha256': write('capped/ipc.json', {
                    window: {'passed': True, 'uncaught': [], 'records': cases(required)}
                    for window, required in [('main', gate.REQUIRED_CAPPED), ('forbidden', gate.REQUIRED_FORBIDDEN)]}),
                'policy_results_sha256': write('policy-results.json', {'passed': True, 'records': cases(gate.REQUIRED_POLICIES)}),
                'fault_results_sha256': write('fault-results.json', {'passed': True, 'results': cases({'schema-' + case['id'] for case in canonical} | gate.REQUIRED_FAULTS)}),
                'conformance_fixture_sha256': gate.digest(fixture),
                'runtime_logs': {name: write(name, 'raw runtime log') for name in ('webview-stdout.log', 'webview-stderr.log', 'capped/webview-stdout.log', 'capped/webview-stderr.log')},
                'jsonl': {'logs/events.jsonl': write('logs/events.jsonl', {'target': 'host-private'})},
                'rust_archives': {'adapter': write('rust-archives/adapter.crate', 'packed-adapter')},
            }
            report['bundle_manifest_sha256'] = write('bundle-manifest.json', {
                'source_commit': 'reviewed-source', 'packages': [{
                    'name': 'adapter', 'archive': 'archives/adapter.crate', 'archive_sha256': report['rust_archives']['adapter']}],
            })
            report['pristine_inputs_sha256'] = write('build-inputs.json', {'files': {
                'bundle/manifest.json': report['bundle_manifest_sha256'],
                'host/Cargo.lock': report['host_lock_sha256'],
            }})
            report['build_inputs'] = {
                profile: {**{key: report[key] for key in ('pristine_inputs_sha256', 'bundle_manifest_sha256', 'host_lock_sha256')},
                          **{key: profile + '/' + key for key in ('build_root', 'cargo_home', 'cargo_target_dir')}}
                for profile in ('debug', 'release')
            }
            native = {'source_commit': 'reviewed-source', 'runtime_source_sha256': gate.native_source_digest(), 'profiles': {}}
            for profile in ('debug', 'release'):
                log = profile + '.log'
                native['profiles'][profile] = {
                    'status': 'passed', 'cases': gate.native_cases(), 'helper_counts': ['4'],
                    'log': log, 'sha256': write('native-runtime/' + log, 'native-cases'),
                }
            write('native-runtime/' + name.lower() + '.json', native)
            write('platform.json', report)

    def report(self, mutate):
        path = self.root / 'Linux/platform.json'
        value = json.loads(path.read_text())
        mutate(value)
        path.write_text(json.dumps(value))

    def test_complete_three_platform_evidence(self):
        self.assertEqual(gate.validate(self.root, 'reviewed-source')['status'], 'passed')

    def test_missing_pristine_profile(self):
        self.report(lambda report: report['build_inputs'].pop('release'))
        with self.assertRaisesRegex(ValueError, 'missing pristine profile inputs'):
            gate.validate(self.root)

    def test_profile_artifact_drift(self):
        self.report(lambda report: report['build_inputs']['release'].update(bundle_manifest_sha256='different'))
        with self.assertRaisesRegex(ValueError, 'profile input artifact mismatch'):
            gate.validate(self.root)

    def test_shared_profile_directory(self):
        for key in ('build_root', 'cargo_home', 'cargo_target_dir'):
            with self.subTest(key=key):
                self.report(lambda report: report['build_inputs']['release'].update({key: report['build_inputs']['debug'][key]}))
                with self.assertRaisesRegex(ValueError, 'reused profile build directory'):
                    gate.validate(self.root)
                self.report(lambda report: report['build_inputs']['release'].update({key: 'release/' + key}))

    def test_tampered_pristine_inventory(self):
        (self.root / 'Linux/build-inputs.json').write_text('tampered')
        with self.assertRaisesRegex(ValueError, 'artifact hash/path mismatch'):
            gate.validate(self.root)

    def test_rehashed_pristine_artifact_drift(self):
        path = self.root / 'Linux/build-inputs.json'
        value = json.loads(path.read_text())
        value['files']['host/Cargo.lock'] = 'different'
        path.write_text(json.dumps(value))
        self.report(lambda report: report.update(pristine_inputs_sha256=gate.digest(path)))
        with self.assertRaisesRegex(ValueError, 'pristine inventory differs'):
            gate.validate(self.root)

    def test_missing_platform(self):
        (self.root / 'Windows/platform.json').unlink()
        with self.assertRaisesRegex(ValueError, 'required platforms missing'):
            gate.validate(self.root)

    def test_tampered_archive(self):
        (self.root / 'Linux/client.tgz').write_text('tampered')
        with self.assertRaisesRegex(ValueError, 'artifact hash/path mismatch'):
            gate.validate(self.root)

    def test_stale_source(self):
        with self.assertRaisesRegex(ValueError, 'source revision mismatch'):
            gate.validate(self.root, 'different-source')

    def test_missing_isolation(self):
        self.report(lambda report: report['isolation'].update(network=False))
        with self.assertRaisesRegex(ValueError, 'missing isolation probes'):
            gate.validate(self.root)

    def test_rehashed_skipped_real_ipc_case(self):
        path = self.root / 'Linux/ipc.json'
        record = json.loads(path.read_text())
        record['forbidden']['records'].pop()
        path.write_text(json.dumps(record))
        self.report(lambda report: report.update(ipc_sha256=gate.digest(path)))
        with self.assertRaisesRegex(ValueError, 'skipped IPC cases'):
            gate.validate(self.root)

    def test_missing_release_runtime_case(self):
        path = self.root / 'Linux/native-runtime/linux.json'
        record = json.loads(path.read_text())
        record['profiles']['release']['cases'].pop()
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, 'missing native lifecycle/resource fixture'):
            gate.validate(self.root)


if __name__ == '__main__':
    unittest.main()
