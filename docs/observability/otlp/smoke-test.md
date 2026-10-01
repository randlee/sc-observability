# OTLP smoke test

This is the current, repository-local smoke path. It supersedes only the
historical plan's external operational instructions.

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
