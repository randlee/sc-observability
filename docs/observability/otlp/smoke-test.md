# OTLP smoke test

<!--
source: docs/observability/smoke-test-plan-phase-aw.md
git_blob: 9b284670444143b3b57ecac5fd526ce4c9f5e426
sha256: 16c12b0a375cb78fc1db9c69dcb8ab2407c8e8e5af09436f9bf2921d98f35b3c
disposition: translate-current-schema
allowed_delta: neutral-current-schema
-->

This is the current, repository-local smoke path. It supersedes only the
historical plan's external operational instructions; the pinned source above
remains the provenance record for that disposition.

Run the canonical ingress corpus through both supported production backends:

```sh
cargo test --locked -p sc-observability-otlp --test canonical_ingress \
  --features otlp-sdk,sync-http
```

The corpus sends logs, completed spans, and metrics through the public
configuration. It checks `service.name`, signal values, and the neutral OTLP
wire representations at a loopback collector. This check does not configure
or query any external observability service.

For the focused synchronous HTTP public-factory smoke, run:

```sh
python3 scripts/ci/otlp_dev_install_smoke.py
```
