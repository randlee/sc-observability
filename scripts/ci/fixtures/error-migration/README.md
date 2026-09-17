# B.1e external migration fixtures

These are standalone Cargo workspaces so their lint behavior is independent of
the repository workspace:

- `legacy` uses the retained wrappers and methods with default warnings and
  checks the serialized legacy `InitError` golden.
- `migrated` denies deprecated items and uses typed constructors, lifecycle
  methods, and `ClassifiedError::kind()` matching.
- `partial` denies deprecated items except for one function-scoped,
  reason-bearing allow at an explicit compatibility boundary.

The validator runs `cargo check` and `cargo run` for each fixture and parses
Cargo JSON diagnostics for expected deprecation codes, notes, and source spans.

