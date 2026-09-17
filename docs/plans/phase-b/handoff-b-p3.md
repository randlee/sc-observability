# B.P3 BTIT contract-correction inspection record

Status: correction evidence only; **not** BTIT source acceptance, a public-API
approval, an independent QA PASS, a completed B.P3 implementation, or permission
to copy/publish anything. The runtime-level contract remains owner-deferred and
`proposed_for_public_api_review` (ATM `01M2PKX8R4J4VJP5V6RRV9JJPB`); historical
QA verdicts remain historical.

## Immutable inspection provenance

| Item | Value |
| --- | --- |
| Source repository | `/Users/randlee/github/beads-task-issue-tracker` (read-only) |
| Inspected commit | `8d8e82ae9f8501dbbf78c76df24d943918d7a6e4` |
| Contract revised | `target-bridge-api.md` in this B.P3 layer |
| Staged core prerequisite | B.P2 `1.3.0`, source `5347c56efb1d1f62ab0253350ea60078b498ccc8`; archive checksums are in [handoff-b-p2.md](handoff-b-p2.md) |
| Review correction | [PR #103 correction comment](https://github.com/randlee/sc-observability/pull/103#issuecomment-5707823516) |

The inspection used `git rev-parse`, `git ls-tree -r --name-only`, `git grep`
for root/public declarations, and `git show COMMIT:path` on the bridge, macro,
error, health, control, manifest, and API-freeze files. The root and hidden
support families, observed lifecycle/error shape, and their dispositions are
recorded in [the target matrix](target-bridge-api.md#snapshot-reconciliation-and-disposition).
This is a reconciled snapshot only. BTIT may change its source; B.P3 cannot mark
the final inventory, critical review/re-review, source commands/results, or
handoff SHA accepted until BTIT supplies immutable evidence for the actual
implementation commit.

## Boundary and evidence gate before B.1

This workspace cannot claim that its CI validates BTIT workspace exports,
compile-fail fixtures, boundary rules, feature graph, or platform checks. Before
B.1 copy, the final B.P3 handoff must instead retain all of the following for one
immutable BTIT source SHA:

1. complete root and `__private` export/derive inventory with a target
   disposition for every observed item;
2. BTIT's exact commands and results for bridge/macro/API/UI consumers,
   core-sharing integration, deterministic races, fault injection, release-mode
   logging, capped-build behavior, and B.P3 LogControl non-mutation compile-fail
   proof;
3. the BTIT critical review and re-review repository-relative path, immutable
   document commit, verdict, and the exact source SHA they cover; and
4. B.P2 staged package version/checksum resolution plus B.1 provenance validation
   against the same accepted source SHA.

Until then, the inspection is not a substitute for source-repository validation;
the B.P3/B.1 inventory gate remains open. Later B.3a/B.4 binding work must add
host-routing fixtures showing TypeScript and attached Python request changes via
an application-owned handler, not LogControl. That is an assigned future-binding
fixture, not a claim that pre-copy bindings exist.

## Finding dispositions

| ID | Disposition and verification evidence |
| --- | --- |
| QA-B013 | Fixed: governing docs now say B.P2 qualifies staged artifacts, B.P3 integrates them, and B.7 alone publishes/does registry proof. Verified by focused cross-document search. |
| QA-B014 / RBP-F001 | Fixed: the runtime contract and target matrix distinguish narrow OperationDiagnostic from full Diagnostic, record identical exact derives, fields/projection, and unchanged code/remediation; no public type rewrite is claimed. |
| QA-B015 | Fixed: runtime contract now distinguishes crate-private core owner lifecycle from bridge completion; bridge Failed/unconfirmed does not claim core stopped. Pinned BTIT snapshot confirms its older `BridgeLifecycle` has only running/shutdown-timed-out/stopped, so no source behavior is attributed to the proposed Failed state. |
| QA-B016 | Fixed for the cited stale-baseline defect: `baseline_inspected` now names the actual inspected commit, and the target matrix records dispositions for every observed export family. Final source reconciliation remains explicitly open. |
| QA-B017 / RBQA-F002 | Gate retained and made explicit: source-repository commands, immutable review evidence, and B.1 provenance are required; this workspace makes no BTIT-CI coverage claim. |
| RBQA-F004 | Process note, not a product finding: PR #103 correction records that reviewer dispatch incorrectly described the implemented core types as absent. No code/design change is attributed to it. |
| RBQA-F005 | Fixed per the corrected report detail: B.P3 requires LogControl non-mutation compile-fail proof; B.3a/B.4 own the binding host-routing proof when bindings exist. |
| RBP-F002 | Fixed in the proposed contract: FieldKeyError is nested under EmitError::InvalidField, derives Debug/Clone/Serialize/Deserialize, and implements Display/Error. It has no independent operation code/remediation or top-level result role. The inspected snapshot has no FieldKeyError, so this is a target revision rather than a claim about source implementation. |

No disposition converts the review into a QA PASS or an acceptance verdict.
