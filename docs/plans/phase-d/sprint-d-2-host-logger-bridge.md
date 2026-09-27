# d-2: Host-owned logger bridge and event policy (#204)

Generated projection of `obs-d-2`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 6
- Assignee / model: lobs / luna
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: sc-observability-log bridge module
- Branch: `sprint/d-2-host-logger-bridge`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-2-host-logger-bridge`
- PR target (merge order only): `sprint/d-1-log-settings`
- Blocked by: `obs-d-13-sanity`
- Requirements: PHB-013, LOG-011, LOG-012, NFR-004, NFR-008, NFR-010, NFR-012, PHD-001, PHD-002
- ADRs: ADR-002, ADR-003, ADR-005, ADR-009, ADR-010, ADR-011, ADR-013, ADR-014, ADR-017, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-log/src/bridge.rs`
  - `crates/sc-observability-log/tests/bridge_*.rs`
  - `docs/logging/d-2-host-logger-bridge.md`
  - `docs/plans/phase-d/sprint-d-2-host-logger-bridge.md`

## Goal and dependency

Let `sc-observability-log` route `log` macros and
`#[instrument]` records to an existing `Arc<sc_observability::Logger>` without
creating another writer, file sink, shutdown owner, or level owner. This is 2.0 contract implementation; it does not change direct logger admission semantics or
the existing public `BridgeOptions` shape.


## Deliverables

1. Implement `attach_logger` without constructing a logger or acquiring,
   cloning, or synthesizing `LevelOwner`. Coordinate slot closure and in-flight
   bridge calls so successful explicit detach releases every attachment-owned
   `Arc<Logger>` reference.

2. Apply policy on the reused `CoreLoggerBackend`/`bridge_backend` path before
   every `try_log`. Rejection records the existing `DropCause::InvalidEvent`
   accounting bucket and never calls the sink; this preserves the existing `DropCause` accounting contract. Do not add a second counter or redaction system.
   Policy panics are contained at the boundary.

3. Preserve current owned-init and `ForeignLoggerInstalled` behavior. Document
   facade ownership with a tracing bridge and distinguish the owned `LogGuard`
   lifecycle from the non-owning attachment lifecycle.

4. Add public integration fixtures for direct plus macro logging through one
   recording sink; allowlist/redaction, bounded-payload, rejection, panic,
   foreign-facade, concurrent detach, and ownership recovery cases.

5. Consume D.13 DetachError and test every slot transition, reattachment, stale control, foreign ownership, timeout retry and the shared drain; define no competing error type.

## This Sprint Does Not Close

No tracing redesign, global facade replacement, owner-capability duplication,
OTLP export, or #88 work.

## Design

## Implementation contract

Consume obs-d-13 Attachment contract (ADR-011/017); implement only bridge.rs and its bridge_* test targets. Route policy through one shared backend, preserve redaction once, contain policy panic, and maintain Empty/Owned/Attached/Closing slots. detach(&mut self, timeout) retains the handle on Timeout and releases all attachment references on success; never gains shutdown or LevelOwner authority. Reuse D.13 DetachError and D.12 registry codes. D.16 owns non-bridge log modules. Canonical errors are 2.0, with no blanket 1.x semver assertion.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-13 (wave 1)

Created by obs-d-13, owned here from wave 2. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-log/src/bridge.rs`

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-2, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-log/tests/bridge_jsonl.rs`

## ADR-019 applicability

ADR-019 governs this consumer because it binds obs-d-13’s open host-attachment policy and canonical 2.0 error surface in `bridge.rs`; PHD-001/PHD-002 therefore govern the numbered implementation deliverables.

## Handoff from obs-d-12 and obs-d-13 (wave 1)

Consume obs-d-13's frozen concrete attachment signature specification and obs-d-12's canonical error/registry artifact. In wave 2, bind the resulting errors and codes only in owned `bridge.rs`; do not alter either producer contract.
## Acceptance criteria

- [ ] `cargo test -p sc-observability-log --test bridge_jsonl --locked` and individual explicitly named bridge attachment/policy test targets added by D.2 pass; never pass bridge_* as a literal cargo target (D1–D5).
- [ ] boundary:sc-observability-log — one recording-sink fixture proves policy admission/rejection/panic, one redaction pass, host-owned logger, no extra LevelOwner and exact dropped-event accounting (D1–D4).
- [ ] boundary:sc-observability-log — foreign facade rejection, init/attach exclusion, detach timeout retry, stale NotInstalled, reattachment and Arc::try_unwrap after successful detach pass using D.13 errors (D5).
- [ ] This sprint does not close cross-crate logging/release qualification; obs-d-18 does.
