# BTIT API handoff recommendations

Status: proposed recommendations, not an accepted BTIT change request.
Inspected BTIT `integrate/phase-a` at
`5fd63ca697fb36e91d754610cb631b1ddb9a3a31`. This record complements its separate
`docs/plans/phase-a/review-a-5.md`; it does not replace its critical review,
claim pending fixes are complete, or approve an import SHA.

## Existing API impact

BTIT `crates/sc-observability-log/src/lib.rs` already returns Rust Result from
init, flush and shutdown. `src/error.rs` defines typed InitError, FlushError and
ShutdownError variants, stable codes, and remediation accessors. Rust Result
and these enums already are discriminated unions. `thiserror::Error` derives
formatting/error-trait implementations; they do not throw exceptions.

The missing binding path is per-record submission to the installed logger:
`src/handle.rs::submit_guarded` returns unit after counting its internal Result;
`src/lib.rs::__private::emit` is hidden support, also returning unit. Exposing
mutable Logger ownership or making the bindings call __private would violate
the intended public boundary.

## Recommend before source API freeze

1. Preserve existing Result-returning lifecycle APIs and typed variants. Add
   no exception emulation or JSON/exporter/PyO3 dependencies to the Rust facade.
2. On the lifecycle-control handle being designed for the existing BTIT fixes,
   expose nonblocking structured emission to the same installed writer with an
   inspectable Result. Proposed shape, with final names/input type settled in
   BTIT's API review:

   ```rust
   pub fn try_log(&self, event: LogEvent) -> Result<EmitOutcome, BridgeEmitError>;
   pub enum EmitOutcome { Accepted, Filtered }
   ```

   The accepted public input must preserve validated correlation/trace fields
   and define identity/target policy. Queue-full, invalid event, stopped logger,
   reentrancy and contained formatter/backend failure need typed variants or
   typed wrapped causes, stable codes and remediation. Do not invent a Filtered
   result unless the selected path actually distinguishes filtering from queue
   admission; otherwise explicitly document accepted-or-filtered semantics.
3. Make the one guarded internal submission operation return its Result. The
   direct API preserves it; the compatibility facade/macros may intentionally
   discard it after exactly-once drop accounting. Standard log::Log methods
   have unit returns and cannot be changed to Result without abandoning that
   trait. Preserve compatibility and document the result-discarding adapter.
4. Complete the existing health/lifecycle review requirements: read-only health,
   one shutdown owner, and observation of late completion after timeout. Expose
   results without handing bindings ownership of the mutable logger. State
   admission versus flush versus durable-storage guarantees explicitly.

These are targeted recommendations to coordinate with BTIT before its next API
freeze. B.1 still only copies accepted corrected source. Record BTIT's disposition
and final signatures in B.1's provenance/handoff. If the accepted copy lacks the
required direct submission/control capability, B.3's bridge-backed integration
entry remains blocked until a separately approved upstream or destination API
change lands; do not implement a hidden second logger or silently redesign B.1.

## Owned here in Phase B

B.3 owns JSON result/error projections, serializer/exporter compatibility,
exhaustive union-version policy and Tauri Result preservation. B.4–B.6 own Python
result factories, native/async adapters and event-loop behavior. BTIT should not
implement those mechanisms merely to satisfy the copy handoff.

BTIT's current Tauri app commands also flatten errors into Result<T, String> or
return unit for frontend logs. When BTIT adopts the later shared bindings, its
command wrappers should preserve a tagged success/error envelope end-to-end
rather than flattening errors into text or exposing invoke Promise rejections.
That is an application integration change, separate from the generic crate copy.

## Acceptance evidence to request with the recommendation

A direct-submission consumer checks both successful admission and each typed
failure, proving the same writer is used by facade, frontend and backend calls.
Facade/direct paths each increment a drop counter exactly once; ignoring the
returned Result does not raise or retry. Health and timeout-completion results
remain inspectable after the lifecycle owner initiates shutdown. Code/remediation
mapping fixtures preserve native variant meaning through the later DTO layer.

References: [Rust Result](https://doc.rust-lang.org/std/result/enum.Result.html)
and the fixed-return [log::Log trait](https://docs.rs/log/latest/log/trait.Log.html).
