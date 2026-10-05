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
    python3 scripts/api/run_unit_tests.py -- cargo test --workspace --no-fail-fast
    cargo test --manifest-path examples/otlp-sdk/Cargo.toml --features sdk-fixture --locked
    python3 -m unittest scripts.ci.tests.test_prepare_release_staged_packages scripts.ci.tests.test_publish_retry_idempotency scripts.integrate.tests.test_dispatch scripts.api.test_history scripts.api.test_python_surface scripts.api.test_unit_runner

# Public API checks; these need the nightly toolchain (see .github/workflows/ci.yml).
public-api:
    python3 -m unittest discover -s scripts/ci/tests -p test_validate_public_api.py -v
    # Exit 1 means a diff to review; semver and docs remain mandatory below.
    status=0; bash scripts/ci/validate_public_api_diff.sh || status=$?; if [ "$status" -gt 1 ]; then exit "$status"; fi
    python3 scripts/ci/validate_public_api_semver.py
    bash scripts/ci/validate_public_api_docs.sh

# cargo-deny with policy/deny-durable-store.toml over the durable-store, otlp-telemetry and sc-otel-cli graphs.
deny:
    cargo deny --manifest-path crates/sc-observability-otlp/Cargo.toml --features durable-store --locked check --config policy/deny-durable-store.toml licenses bans advisories
    cargo deny --manifest-path bindings/python/sc-observability-py/Cargo.toml --features otlp-telemetry --locked check --config policy/deny-durable-store.toml licenses bans advisories
    cargo deny --manifest-path crates/sc-otel-cli/Cargo.toml --all-features --locked check --config policy/deny-durable-store.toml licenses bans advisories

# Full gate: lint, tests and the scoped dependency audit.
validate: lint test deny

# On-demand integration; selectors and results are documented in docs/integration.md.
[positional-arguments]
integrate branch *args:
    python3 scripts/integrate/dispatch.py "$@"
