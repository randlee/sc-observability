# Phase B native binding runtime public API

The initial `sc-observability-binding-runtime` 1.4.0 API implements the signatures
in `docs/plans/phase-b/native-binding-runtime.md`: provided core/bridge backends,
unique core owner and bounded operation observations. No existing native crate
API is changed by this layer. The policy records an unpublished initial baseline.

Appointed lead aobs must review the actual cargo-public-api export digest at
completeness. The implementation author does not grant approval. The machine
approval entry is added only after that review. B.7 retains publication authority.

## Digest-refresh narrative debt

The retained native-runtime export evidence and
`docs/api-approvals/phase-b-native-runtime.json` agree on digest
`5a60e6ebf6b87b9899a87375b5598ccefb1172196aadedae4380868724c60794` at
source `b868147a20471ff9d65ca010fb9053c6254e57f6`. The older narrative in
`docs/plans/phase-b/handoff-b-3b.md` still names digest
`4b3e55fcc66c721dabb43298f5a7948bdb326330190e45a75a443c93d0e9d99b`.
This layer records the discrepancy as documentation debt; it does not rewrite
the historical handoff or claim a fresh native-runtime approval. Coordinator
completeness must reconcile the narrative against the retained evidence.
