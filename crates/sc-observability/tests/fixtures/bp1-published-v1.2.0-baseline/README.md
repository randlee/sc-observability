# B.P1 independent released-baseline consumer

This leg resolves only the published, exact `1.2.0` crate versions. Its source
is intentionally identical to the candidate consumer's legacy API exercise;
the dependency origin is the difference. Run it before the candidate leg:

```sh
cargo run --locked --manifest-path crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-baseline/Cargo.toml
```

The release source provenance is commit
`dcc52685fd845c8d1bddde29199e799ae921cf5c`. Do not add B.P1 owner APIs to this
fixture.
