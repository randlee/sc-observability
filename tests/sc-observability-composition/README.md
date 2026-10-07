# Combined external consumer

This unpublished workspace crate exercises public APIs without adding dependency edges to any published library. Its test-local sink forwards a bridge-admitted `LogEvent` into `sc-observe`; the telemetry projector then uses the actual production exporter. This adapter is test glue, not a production bridge feature.

Run `cargo test -p sc-observability-composition` and `cargo clippy -p sc-observability-composition --all-targets --all-features -- -D warnings`.

The released and canonical consumers each use a real loopback SDK gRPC collector and a real synchronous HTTP JSON collector. Collector plumbing is adapted from the separately owned `compatible_stack.rs` package-consumer tests; this harness adds the public log attachment and core writer before observation/projector/export delivery. SDK construction and async flush/shutdown occur inside a caller-owned Tokio runtime. The test does not decide SDK construction outside a runtime.

Each bridge case runs in a fresh subprocess, with a 45-second kill-and-reap bound. Collectors have bounded startup, socket I/O and watchdogs; their threads are joined and temporary log roots are owned by guards. No external collector installation or Grafana is required.

Assertions distinguish admitted delivery from policy rejection, check exact log receipt and local output, released spans/gauges and canonical span flags/kind/parent/link/event plus histogram buckets/temporality/timestamps, preserve a caller-owned projector diagnostic/context/source through public telemetry wrappers, and exercise detach, exporter shutdown and closed-observe forwarding. Core health deliberately exposes diagnostic summaries rather than source pointers; the test does not claim source identity survives that summary boundary.

This crate is not the D9 decoded-corpus/fault/feature qualification, the nine-package semver sweep, or the complete D18 binding/wheel qualification.
