---
status: proposed
owner: B.3b
---

# Shared native binding runtime contract

`crates/sc-observability-binding-runtime/` (proposed crates.io name
`sc-observability-binding-runtime`) owns runtime-to-DTO conversion and bounded
native operations for Tauri and Python. It depends on the published core, DTOs
and `sc-observability-log`; it never depends on Tauri or PyO3. Python depends on
this crate and thus transitively on the bridge crate, without installing its
process-global facade. This keeps bridge constants and conversions single-owned.

The bridge retains its separate, already accepted pre-copy lifecycle coordinator.
This binding coordinator handles independent core loggers; BridgeControlBackend
uses the bridge control/lifecycle semantics and does not recreate ownership.
This is deliberate separation of an accepted bridge lifecycle from a new
core-host adapter, not an added core API or post-copy bridge redesign.

## Public Rust contract

All DTO names refer to the shared schema. `Failure` is the DTO failure union.
Opaque types have private fields. Backend handles are Clone+Send+Sync; ownership
handles are Send+Sync and not Clone. No public constructor panics on failure.

```rust
pub enum ProducerOrigin { TauriFrontend, Python, RustHost }
pub trait HostLoggingBackend: Send + Sync {
    fn try_log(&self, event: LogEventDto, origin: ProducerOrigin)
        -> Result<AdmissionDto, Failure>;
    fn start_query(&self, query: LogQueryDto)
        -> Result<Operation<LogSnapshotDto>, Failure>;
    fn health(&self) -> Result<LogHealthDto, Failure>;
    fn start_flush(&self) -> Result<Operation<CompletionDto>, Failure>;
}
pub enum OperationState<T> { Pending, Completed { result: Result<T, Failure> } }
pub struct Operation<T> { /* shared bounded completion */ }
pub struct CompletionSubscription { /* cancel registration on drop */ }
impl<T: Clone + Send + Sync + 'static> Operation<T> {
    pub fn state(&self) -> OperationState<T>;
    pub fn wait(&self, timeout: std::time::Duration) -> Result<T, Failure>;
    pub fn completion(&self, timeout: std::time::Duration)
        -> impl std::future::Future<Output = Result<T, Failure>> + Send + 'static;
    pub fn subscribe(&self, callback: Box<dyn FnOnce(Result<T, Failure>) + Send>)
        -> Result<CompletionSubscription, Failure>;
}
pub struct CoreLoggerBackend { /* nonowning lifecycle capability */ }
pub struct CoreLoggerOwner { /* sole shutdown + LevelOwner capability */ }
pub struct BridgeControlBackend { /* LogControl, never LogGuard */ }
pub fn create_core_backend(config: sc_observability::LoggerConfig)
    -> Result<(CoreLoggerOwner, CoreLoggerBackend), Failure>;
pub fn bridge_backend(control: sc_observability_log::LogControl)
    -> Result<BridgeControlBackend, Failure>;
impl CoreLoggerOwner {
    pub fn start_shutdown(&self) -> Result<Operation<LogHealthDto>, Failure>;
    pub fn shutdown(&self, timeout: std::time::Duration)
        -> Result<LogHealthDto, Failure>;
    pub fn wait_stopped(&self, timeout: std::time::Duration)
        -> Result<LogHealthDto, Failure>;
    pub fn elevate_level(&mut self, level: LevelFilter, source: LevelChangeSource)
        -> Result<LevelChangeDto, Failure>;
    pub fn reset_level(&mut self, source: LevelChangeSource)
        -> Result<LevelChangeDto, Failure>;
}
impl HostLoggingBackend for CoreLoggerBackend { /* complete implementation */ }
impl HostLoggingBackend for BridgeControlBackend { /* complete implementation */ }
```

`Operation` and subscriptions are adapter infrastructure, not Python public
flush receipts. Their retained completions are bounded by caller ownership;
Python exposes no accessor for an old flush after timeout. `completion` checks
an absolute monotonic deadline without blocking its poll thread. Use one process-shared monotonic timer helper with a deadline heap and
one entry per registered timed waiter, not a thread or executor per wait.
Initialize that helper fallibly before per-backend workers; spawn failure uses
COORDINATOR_START_FAILED. Removing a waiter also removes its heap entry. Subscription callbacks run once outside state locks;
foreign callback panics are contained and counted without replacing the saved
result. Subscription cancellation removes its callback; callbacks already claimed
may finish. At most 64 waiters/subscriptions total per operation, including sync
waits; overflow returns queue_full/BINDING_WAITERS_FULL before retaining a callback.
Completed state inspection does not register a waiter. Dropping a Future releases
its registration and timer, never cancels the native operation.

## Native operation coordinator

Creation reserves exactly three helper threads per backend before exposing it:
one operation worker, one lifecycle worker and one completion worker. Core mode additionally
has core's existing writer thread; bridge mode already has the bridge's existing
writer/coordinator and retains no LogGuard. No helper is created per query,
flush, timeout, waiter or Python call. The global timer service is one shared
process helper, not one per logger. Helpers start parked; failure at any spawn
returns unavailable/SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED, stops and
joins already-started parked helpers before constructing a core logger. Core
construction failure similarly rolls back idle helpers. No native thread-start
failure is hidden as success or intentional panic.

The operation worker has a two-entry mailbox: one query slot and one flush slot.
Each slot covers queued plus executing work; another request of the same class
immediately returns queue_full with BINDING_QUERY_IN_PROGRESS or
BINDING_FLUSH_IN_PROGRESS. Queries and flushes execute in arrival order on this
single worker. Deadline expiry or dropping observers does not release a slot;
actual completion does. Bridge-native errors pass through, including native
LOG_FLUSH_IN_PROGRESS if an external bridge caller already owns its flush slot.
The adapter does not fabricate that native code for its own slot rejection.

Submission/health borrow the core logger only through a short admission gate.
A busy gate returns queue_full/BINDING_DISPATCH_FULL; no producer waits on sink
I/O or capacity. Each admitted call increments an active-operation counter and
holds a temporary logger reference; it drops the reference before decrementing
and signaling. Backend handles do not count as active operations. Shutdown
atomically closes admission, then the lifecycle worker waits for admitted calls
and the two slots to finish, takes the unique Logger once, and invokes core
shutdown once. No Arc polling or dependence on client-handle drop is permitted.
Final health/result survives in shared state. start_shutdown returns the same
operation on repeat calls; shutdown is its synchronous wait convenience. Python
starts shutdown while briefly borrowing its owner then releases the wrapper
borrow/lock before waiting on the returned Operation. Unconfirmed helper failure yields
internal and terminal failed, never stopped. Repeated owner shutdown observes
the same completion. Deadline expiry bounds only caller observation. Owner drop
signals shutdown once with no blocking join on the Python/interpreter thread.

Before shutdown, health uses the same core snapshot conversion; after admission
closes, it uses the retained native snapshot until final health is available;
no invented coordinator-phase field is added to LogHealthDto. Shutdown operations
report their own typed lifecycle outcome independently of this snapshot. The operation worker publishes saved results and releases slots before
notifying waiters; the separate completion worker invokes callbacks. The lifecycle
worker only coordinates final shutdown. No worker holds admission/state locks
while invoking callbacks, waiting on core, or joining. Subscriptions reserve one
of 128 backend-wide callback slots before registration; overflow returns
BINDING_WAITERS_FULL. The completion mailbox has exactly those 128 reserved
entries, so stalled callbacks cannot grow a queue without bound or block native
operations/shutdown. A callback must be nonblocking and cannot synchronously
wait on another callback; misuse can stall callback delivery but not logging or
shutdown. Cancelling a queued callback releases its reservation; a claimed one
releases it after returning.
Operation completion and callback storage are separate so callback delays cannot
hold the query/flush slot. No per-call callback thread is introduced.

Last-handle teardown is explicit. Core owner drop starts final shutdown once;
bridge-backend last-handle drop closes only adapter admission, never host shutdown.
After admitted work and registered callbacks finish, close all helper mailboxes
and let the three workers exit. Completed Operation values hold result state and
weak coordinator references, not helper ownership. Dropping all handles/observers
while work is blocked keeps only that bounded work alive; it cannot spawn
replacement workers. No destructor joins on a Python thread. Test attachment/drop
churn returning helper counts to baseline after completion. Callbacks already
claimed may finish; a callback violating the nonblocking contract cannot force
the process to kill its thread and is unsupported host behavior.

Python never registers a native callback that calls Python or acquires the GIL.
B.6 polls saved Operation state on the owning asyncio loop with call_later; module
teardown cancels those loop-local timers. Operation::state reads an immutable
published completion through nonblocking shared state (pending until published),
without a mutex or GIL transition. Native completion proceeds after interpreter
teardown with no Python references/callbacks. Attached teardown cannot shut down
the host. This removes gate/GIL lock-order and finalization races entirely.

## Conversion and provenance ownership

This crate alone maps bridge runtime health/errors and wraps DTO core conversions.
Provided backends perform validation, native submission/query/health conversion
and trusted provenance stamping. Tauri/Python wrappers project transport/language
values; host examples install a provided backend and never repeat variant maps.
Custom trait implementations remain possible but must satisfy the same fixtures.
The producer origin is selected by the trusted adapter, never from user DTO data.

## Required error and race fixtures

Both backends: full query/flush slots, cross-class ordering, zero timeout, timeout
then completion/slot release, no thread growth after repeated waits, waiter limit,
callback cancellation/completion races, retained result after observer drop,
foreign callback panic and failed accounting. Core: shared-timer/first/second/third helper spawn
failure and core-start rollback, shutdown racing admissions/query/flush/level
changes, handles surviving shutdown, held sink with responsive producers, and
failed helper without false stopped. Bridge: external flush overlap preserves
the original bridge code, controls never gain shutdown authority. Python teardown
and loop-closure fixtures run through B.4/B.6 without duplicating this runtime.

Query observers in Tauri and Python use the fixed 2000 ms deadline; flush uses
the validated caller timeout. A query timeout ends observation only and retains
the query slot until completion, just as flush does.

Additional fixtures: blocked callback while native shutdown and another query
complete; 128 reserved callbacks and overflow; last-handle drop in idle/running/
completed states; retained completed Operation without retained helpers; repeated
bridge attach/drop without host shutdown or thread growth. Level changes execute
directly after a nonblocking owner-gate acquisition and never queue behind I/O;
busy ownership returns queue_full/BINDING_DISPATCH_FULL without mutation.
