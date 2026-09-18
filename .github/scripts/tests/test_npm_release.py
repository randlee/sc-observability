"""Nonpublishing npm contract, artifact, and retry tests (all writes mocked)."""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import urllib.error
from unittest.mock import patch

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import npm_release as npm
from release_manifest import _public_registry_checks, load_channel_contracts
from test_install import INSTALL, InstallValuesTests


@pytest.fixture
def release(tmp_path):
    manifest = {"npm_packages": [{"name": "@example/client", "source": "bindings/typescript"}], "channels": {"npm": {"workflow": "npm-publish.yml", "dispatch_inputs": {}}}}
    path = tmp_path / "example-client-1.2.3.tgz"
    data = json.dumps({"name": "@example/client", "version": "1.2.3"}).encode()
    with tarfile.open(path, "w:gz") as archive:
        member = tarfile.TarInfo("package/package.json")
        member.size = len(data)
        archive.addfile(member, io.BytesIO(data))
    (tmp_path / "checksums.txt").write_text(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name + "\n")
    return manifest, tmp_path, path


def existing(path):
    return {"name": "@example/client", "version": "1.2.3", "dist": {"integrity": "sha512-" + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode()}}


def test_installer_npm_roundtrip(tmp_path):
    values = InstallValuesTests.valid_values()
    values["npm_packages"] = [{"name": "@example/client", "source": "bindings/typescript"}]
    values["channels"]["npm"] = {"workflow": "npm-publish.yml", "dispatch_inputs": {"dry_run": "false"}}
    input_path = tmp_path / "input.json"
    input_path.write_text(json.dumps(values))
    loaded = INSTALL.load_install_values(input_path)
    assert INSTALL.template_values(loaded)["has_channel_npm"] is True
    import tomllib
    output = tmp_path / "manifest.toml"
    INSTALL.render_template(Path("release/publish-artifacts.toml.j2"), loaded, output)
    rendered = tomllib.loads(output.read_text())
    assert rendered["npm_packages"] == values["npm_packages"]
    assert rendered["channels"]["npm"] == values["channels"]["npm"]


def test_channel_scope_and_scoped_urls():
    contracts = load_channel_contracts(INSTALL.PACKAGE_ROOT / "release/publish-channel-contracts.toml.j2")
    assert contracts["npm"]["environment_secrets"] == [{"environment": "npm", "name": "NPM_TOKEN"}]
    check, = _public_registry_checks(contracts, "npm", "@example/client", "1.2.3")
    assert check["version_lookup_url"] == "https://registry.npmjs.org/%40example%2Fclient/1.2.3"


def test_verify_metadata_and_checksum(release):
    manifest, directory, path = release
    assert npm.verify(manifest, "v1.2.3", directory) == [("@example/client", path)]
    path.write_bytes(path.read_bytes() + b"tampered")
    with pytest.raises(ValueError, match="checksum"):
        npm.verify(manifest, "v1.2.3", directory)


def test_mismatched_archive_identity(release):
    _, _, path = release
    with pytest.raises(ValueError, match="identity"):
        npm.check_metadata(path, "@other/client", "1.2.3")


def test_preflight_never_publishes(release):
    manifest, directory, _ = release
    with patch.object(npm, "registry_version", return_value=None), patch.object(npm.subprocess, "run") as run:
        npm.publish(manifest, "v1.2.3", directory)
    run.assert_not_called()


def test_identical_version_skips(release):
    manifest, directory, path = release
    with patch.object(npm, "registry_version", return_value=existing(path)), patch.object(npm.subprocess, "run") as run:
        npm.publish(manifest, "v1.2.3", directory, dry_run=False)
    run.assert_not_called()


def test_conflicting_version_fails_closed(release):
    manifest, directory, _ = release
    with patch.object(npm, "registry_version", return_value={"dist": {"integrity": "other"}}), patch.object(npm.subprocess, "run") as run:
        with pytest.raises(ValueError, match="differs"):
            npm.publish(manifest, "v1.2.3", directory, dry_run=False)
    run.assert_not_called()


def test_publish_only_tarball_and_disable_scripts(release):
    manifest, directory, path = release
    with patch.object(npm, "registry_version", return_value=None), patch.object(npm.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
        npm.publish(manifest, "v1.2.3", directory, dry_run=False)
    command = run.call_args.args[0]
    assert command[:3] == ["npm", "publish", str(path.resolve())]
    assert "--ignore-scripts" in command
    assert command[-2:] == ["--tag", "latest"]


@pytest.mark.parametrize("accepted", [True, False])
def test_failed_publish_rechecks_registry_for_retry(release, accepted):
    manifest, directory, path = release
    with patch.object(npm, "registry_version", side_effect=[None, existing(path) if accepted else None]), patch.object(npm.subprocess, "run", return_value=subprocess.CompletedProcess([], 1)):
        if accepted:
            npm.publish(manifest, "v1.2.3", directory, dry_run=False)
        else:
            with pytest.raises(RuntimeError, match="retry"):
                npm.publish(manifest, "v1.2.3", directory, dry_run=False)


@pytest.mark.parametrize("code", [401, 403, 429, 500])
def test_registry_errors_not_absence(code):
    error = urllib.error.HTTPError("https://registry.npmjs.org", code, "error", {}, None)
    with patch.object(npm.urllib.request, "urlopen", side_effect=error):
        with pytest.raises(RuntimeError, match="indeterminate"):
            npm.registry_version("@example/client", "1.2.3")


def test_registry_404_is_absent():
    error = urllib.error.HTTPError("https://registry.npmjs.org", 404, "missing", {}, None)
    with patch.object(npm.urllib.request, "urlopen", side_effect=error):
        assert npm.registry_version("@example/client", "1.2.3") is None


def test_unsafe_source_rejected():
    with pytest.raises(ValueError, match="unsafe"):
        npm.packages({"npm_packages": [{"name": "example", "source": "../escape"}], "channels": {"npm": {}}})


def test_build_uses_lockfile_and_never_publishes(release, monkeypatch):
    manifest, directory, path = release
    monkeypatch.chdir(directory)
    source = directory / "bindings/typescript"
    source.mkdir(parents=True)
    (source / "package.json").write_text(json.dumps({"name": "@example/client", "version": "1.2.3"}))
    with patch.object(npm.subprocess, "run") as run:
        npm.build(manifest, "v1.2.3", directory)
    assert [call.args[0][:2] for call in run.call_args_list] == [["npm", "ci"], ["npm", "run"], ["npm", "pack"]]
    assert "--ignore-scripts" in run.call_args_list[-1].args[0]


def test_version_mismatch_fails_before_build(release, monkeypatch):
    manifest, directory, _ = release
    monkeypatch.chdir(directory)
    source = directory / "bindings/typescript"
    source.mkdir(parents=True)
    (source / "package.json").write_text(json.dumps({"name": "@example/client", "version": "0.0.1"}))
    with patch.object(npm.subprocess, "run") as run:
        with pytest.raises(ValueError, match="source npm"):
            npm.build(manifest, "v1.2.3", directory)
    run.assert_not_called()


def test_all_archives_checked_before_first_publication(release):
    manifest, directory, _ = release
    manifest["npm_packages"].append({"name": "other", "source": "other"})
    with patch.object(npm.subprocess, "run") as run, patch.object(npm, "registry_version") as lookup:
        with pytest.raises(ValueError, match="missing regular"):
            npm.publish(manifest, "v1.2.3", directory, dry_run=False)
    run.assert_not_called()
    lookup.assert_not_called()


def test_tarball_slug_collision_rejected():
    manifest = {"npm_packages": [{"name": "@example/client", "source": "a"}, {"name": "example-client", "source": "b"}], "channels": {"npm": {}}}
    with pytest.raises(ValueError, match="duplicate"):
        npm.packages(manifest)


def test_workflow_credential_and_artifact_contract():
    import yaml
    root = INSTALL.PACKAGE_ROOT / ".github"
    workflow = yaml.safe_load((root / "workflows/npm-publish.yml").read_text())
    job = workflow["jobs"]["publish"]
    assert job["environment"] == "npm"
    assert workflow["permissions"] == {"contents": "read"}
    credential_steps = [step for step in job["steps"] if "NPM_TOKEN" in str(step)]
    assert len(credential_steps) == 1
    assert credential_steps[0]["if"] == "${{ !inputs.dry_run }}"
    assert "npm_release.py publish" in credential_steps[0]["run"]
    download = next(step for step in job["steps"] if step.get("name") == "Download immutable release assets")
    assert '.immutable == true' in download["run"]
    assert "checksums.txt" in download["run"]
    assert all("npm ci" not in step.get("run", "") for step in job["steps"])


def test_node24_runtime_floors():
    import re
    floors = {"checkout": 5, "setup-python": 6, "setup-node": 6, "cache": 5, "upload-artifact": 6, "download-artifact": 7}
    for path in (INSTALL.PACKAGE_ROOT / ".github").rglob("*.yml"):
        for name, major in re.findall(r"uses:\s*actions/([\w-]+)@v(\d+)", path.read_text()):
            assert int(major) >= floors[name], str(path)
    assert "softprops/action-gh-release@v3" in (INSTALL.PACKAGE_ROOT / ".github/workflows/release.yml").read_text()


@pytest.mark.parametrize('immutable,expected', [(True, 0), (False, 1)])
def test_download_step_requires_immutable_release(tmp_path, immutable, expected):
    import os
    import yaml
    workflow = yaml.safe_load((INSTALL.PACKAGE_ROOT / '.github/workflows/npm-publish.yml').read_text())
    step = next(step for step in workflow['jobs']['publish']['steps'] if step.get('name') == 'Download immutable release assets')
    gh = tmp_path / 'gh'
    gh.write_text('#!/bin/sh\nif [ "$1" = api ]; then\n  printf \'%s\\n\' \'{"immutable":' + str(immutable).lower() + ',"draft":false}\'\nelse\n  touch downloaded\nfi\n')
    gh.chmod(0o755)
    env = {**os.environ, 'PATH': str(tmp_path) + os.pathsep + os.environ['PATH'], 'RELEASE_TAG': 'v1.2.3', 'RELEASE_REPOSITORY': 'example/project'}
    result = subprocess.run(['bash', '-c', step['run']], cwd=tmp_path, env=env, capture_output=True)
    assert result.returncode == expected
    assert (tmp_path / 'downloaded').exists() is immutable
