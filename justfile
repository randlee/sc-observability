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
    python3 -m unittest discover -s scripts/integrate/suites/collector -p test_run.py
    python3 -m unittest discover -s scripts/integrate/suites/rust-consumers -p test_run.py
    python3 -m unittest discover -s scripts/integrate/suites/rust-viewer -p test_run.py
    python3 -m unittest discover -s scripts/integrate/suites/tauri/tests -p test_run.py
    python3 -m unittest discover -s scripts/integrate/suites/wheel-cli-viewer/tests -p test_run.py
    python3 -m unittest discover -s scripts/integrate/suites/wheels/tests -p test_run.py
    python3 -m unittest discover -s scripts/ci/fixtures/otlp/desktop-viewer -p 'test_*.py'
    cargo test --workspace --no-fail-fast
    cargo test --manifest-path examples/otlp-sdk/Cargo.toml --locked
    python3 -m unittest scripts.ci.tests.test_prepare_release_staged_packages scripts.ci.tests.test_publish_retry_idempotency scripts.ci.tests.test_stock_public_api scripts.integrate.tests.test_dispatch scripts.integrate.tests.test_runner_workflow scripts.api.test_history scripts.api.test_python_surface

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
