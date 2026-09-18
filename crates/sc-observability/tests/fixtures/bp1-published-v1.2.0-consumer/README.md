# B.P1 published-consumer compatibility source

This source is frozen from the published `v1.2.0` consumer shape (release
provenance `dcc52685fd845c8d1bddde29199e799ae921cf5c`) and deliberately calls
only legacy constructors. Compile it against the B.P1 candidate package with:

```sh
cargo run --manifest-path crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.toml
```

Do not rewrite it to use the new owner APIs; it is compatibility evidence.
