---
status: proposed_scope_disposition
---

# Phase B issue disposition

This inventory records the user's discussed scope and plan ownership. It does
not assert live GitHub open/closed status or completion of any implementation.
All implementation rows remain planned until their sprint acceptance is proven.

| Issue/topic | Disposition | Owning sprint and closure |
| --- | --- | --- |
| #92 improved error interface | Included, additive with warning-only migration; no scheduled removal | B.1a types/adapters, B.1b logger, B.1c observation, B.1d telemetry, B.1e deprecation/adoption guide, B.2 publication |
| #97 runtime level elevation | Included as pre-copy core capability | B.P1 core, B.P2 release, B.P3 BTIT integration; B.1 copies accepted reference |
| #96 settings loader | Independent/deferred from Phase B | No Phase B implementation promise or dependency; baseline comes from LoggerConfig |
| BTIT initial bridge design/review | Required before copy, not assumed finished | B.P3 accepted critical re-review/source; authoritative B.1 entry gate |
| Shared frontend/backend logging | Included, Tauri first | B.3 schema, B.3b shared native runtime, B.3a real Tauri IPC/client; B.7 registry proof |
| First-class Python | Included, owned and host-attached | B.3b shared native runtime, B.4 projection, B.4a wheels/platform matrix, B.5 Handler/context, B.6 optional waits, B.7 publication |
| sc-runtime topology, Node.js and Go | Deferred without selecting transport/runtime design | No Phase B owner or implied implementation; in-process Python support remains included |

The [review record](review-consistency.md) tracks resolved planning findings.
This inventory must not be used to close a GitHub issue merely because a plan
or documentation-only correction merged.
