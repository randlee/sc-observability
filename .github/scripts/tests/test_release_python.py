"""Wheel target planning and artifact coverage without publication."""
import json
from pathlib import Path
import sys
from unittest.mock import patch
import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from release_python import wheel_targets, verify_platforms
from test_install import INSTALL, InstallValuesTests

TARGETS = [
 {"id":"linux-x86_64","os":"ubuntu-latest","target":"x86_64-unknown-linux-gnu","platform":"manylinux_2_28_x86_64","manylinux":"2_28"},
 {"id":"linux-aarch64","os":"ubuntu-24.04-arm","target":"aarch64-unknown-linux-gnu","platform":"manylinux_2_28_aarch64","manylinux":"2_28"},
 {"id":"macos-x86_64","os":"macos-15-intel","target":"x86_64-apple-darwin","platform":"macosx_10_13_x86_64","deployment_target":"10.13"},
 {"id":"macos-arm64","os":"macos-latest","target":"aarch64-apple-darwin","platform":"macosx_11_0_arm64","deployment_target":"11.0"},
 {"id":"windows-x86_64","os":"windows-latest","target":"x86_64-pc-windows-msvc","platform":"win_amd64"},
]


def test_five_platform_contract_renders_and_validates(tmp_path):
    values = InstallValuesTests.valid_values()
    values["python_distributions"][0]["wheels"] = TARGETS
    source = tmp_path / "input.json"
    source.write_text(json.dumps(values))
    loaded = INSTALL.load_install_values(source)
    output = tmp_path / "manifest.toml"
    INSTALL.render_template(Path("release/publish-artifacts.toml.j2"), loaded, output)
    import tomllib
    distribution = tomllib.loads(output.read_text())["python_distributions"][0]
    assert distribution["wheels"] == TARGETS
    assert len(wheel_targets(distribution)) == 5
    paths = [Path(f'example-1.2.3-cp310-abi3-{target["platform"]}.whl') for target in TARGETS]
    verify_platforms(distribution, paths)
    with pytest.raises(ValueError, match="missing"):
        verify_platforms(distribution, paths[:-1])
    with pytest.raises(ValueError, match="unique"):
        verify_platforms(distribution, paths[:-1] + paths[:1])


@pytest.mark.parametrize("wheel", ["cp310-abi3-win_amd64", {**TARGETS[0],"os":"windows-latest"}, {**TARGETS[2],"deployment_target":"11.0"}, {**TARGETS[0],"manylinux":"off"}])
def test_bad_wheel_contracts_rejected(wheel):
    with pytest.raises(ValueError):
        wheel_targets({"wheels":[wheel]})


def test_legacy_runner_strings_remain_supported():
    for runner in ["ubuntu-latest","ubuntu-24.04-arm","macos-latest","windows-latest"]:
        target, = wheel_targets({"wheels":[runner]})
        assert target["os"] == runner
        assert target["target"] == ""


def test_workflow_reuses_maturin_zig_and_declares_all_build_dependencies():
    import yaml
    root = INSTALL.PACKAGE_ROOT / ".github"
    release = yaml.safe_load((root / "workflows/release.yml").read_text())
    wheels = release["jobs"]["build-python-wheels"]
    assert wheels["runs-on"] == "${{ matrix.os }}"
    assert all("maturin-action" not in step.get("uses", "") for step in wheels["steps"])
    declared = next(step for step in wheels["steps"] if step.get("name") == "Build declared wheel target (maturin)")
    assert '--compatibility "manylinux_${WHEEL_MANYLINUX}" --zig' in declared["run"]
    assert '--target "$WHEEL_TARGET"' in declared["run"]
    assert declared["env"]["MACOSX_DEPLOYMENT_TARGET"] == "${{ matrix.deployment_target }}"
    assert 'maturin[zig]==1.9.4' in (root / "actions/setup-python-release-build/action.yml").read_text()
    assert "has_release_binaries" in release["jobs"]["build"]["if"]
    github_release = release["jobs"]["release"]
    assert {"build-python-wheels", "build-python-sdists", "build-npm"} <= set(github_release["needs"])
    assert "!failure()" in github_release["if"]
    assert "needs.build.result == 'skipped'" in github_release["if"]


@pytest.mark.parametrize('workflow_name', ['release.yml', 'crates-publish.yml'])
def test_actual_publish_shell_uses_standalone_manifest_with_mock_cargo(tmp_path, workflow_name):
    import os
    import re
    import subprocess
    import yaml
    workflow = yaml.safe_load((INSTALL.PACKAGE_ROOT / '.github/workflows' / workflow_name).read_text())
    step = next(step for job in workflow['jobs'].values() for step in job.get('steps', []) if step.get('name') == 'Publish crates in order (idempotent)')
    shell = re.sub(r'\$\{\{[^}]+\}\}', '1.2.3', step['run'])
    stub = tmp_path / 'python3'
    stub.write_text('''#!/bin/sh
case "$2" in
 list-publish-plan) printf 'standalone|0|bindings/standalone/Cargo.toml\\n' ;;
 public-registry-inquiry-plan) printf '{"checks":[{"version_lookup_url":"https://example.invalid"}]}\\n' ;;
 registry-status) printf 'absent\\n' ;;
 *) exit 2 ;;
esac
''')
    stub.chmod(0o755)
    cargo = tmp_path / 'cargo'
    cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > cargo-args\n')
    cargo.chmod(0o755)
    result = subprocess.run(['bash', '-c', shell], cwd=tmp_path, env={**os.environ, 'PATH':str(tmp_path)+os.pathsep+os.environ['PATH'], 'RELEASE_ARTIFACT_MANIFEST':'release/publish-artifacts.toml', 'RELEASE_TAG':'v1.2.3'}, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    assert (tmp_path / 'cargo-args').read_text().splitlines() == ['publish', '--manifest-path', 'bindings/standalone/Cargo.toml', '--locked']
