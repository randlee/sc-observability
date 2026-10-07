# 1.4.1 release preparation

The coordinated 1.4.1 train is a maintenance continuation of the qualified
1.4.0 surface. All publishable Rust crates, the Python distribution, and the
TypeScript client are versioned 1.4.1 with matching internal dependency pins.

The 1.4.0 qualification candidate, API approvals, staged archive evidence,
tags, and published artifacts remain historical records and are not rewritten.
Publication of 1.4.1 remains separately authorized by the release owner.

## Validation scope

- Workspace and binding manifests agree on 1.4.1.
- Cargo.lock is regenerated through Cargo's normal resolver.
- Rust, Python, npm, manifest, API, documentation, and packaging checks run on
  the exact preparation commit.
