# Repository gates. `just validate` is the full gate that phase-ending QA
# requires from rust-qa-agent (see .claude/skills/codex-orchestration/SKILL.md).

# Format, clippy, and every repository validation script that CI runs.
lint:
    cargo fmt --check --all
    cargo clippy --all-targets --all-features -- -D warnings
    python3 .github/scripts/release_artifacts.py validate-publish-order \
        --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml
    python3 scripts/ci/validate_phase_c_install_contract.py
    bash scripts/ci/validate_docs_consistency.sh
    bash scripts/ci/validate_dependency_bans.sh
    python3 scripts/ci/validate_version_literals.py
    bash scripts/ci/validate_repo_boundaries.sh

# Workspace tests.
test:
    cargo test --workspace
    cargo test --manifest-path examples/otlp-sdk/Cargo.toml --features sdk-fixture --locked
    python3 -m unittest scripts.ci.tests.test_prepare_release_staged_packages scripts.ci.tests.test_publish_retry_idempotency

# Public API checks; these need the nightly toolchain (see .github/workflows/ci.yml).
public-api:
    python3 -m unittest discover -s scripts/ci/tests -p test_validate_public_api.py -v
    bash scripts/ci/validate_public_api_diff.sh
    python3 scripts/ci/validate_public_api_semver.py
    bash scripts/ci/validate_public_api_docs.sh

# Full gate: lint plus tests.
validate: lint test
