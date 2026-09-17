//! One `init` per test binary, with `queue_capacity = 1`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::time::{Duration, Instant};

use sc_observability_log::{ActionName, BridgeOptions, DropCause, LoggerConfig, ServiceName};

const THREADS: usize = 8;
const RECORDS_PER_THREAD: usize = 5_000;

fn flood() {
    let handles: Vec<_> = (0..THREADS)
        .map(|thread| {
            std::thread::spawn(move || {
                for record in 0..RECORDS_PER_THREAD {
                    log::info!(target: "flood", "thread {thread} record {record}");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn full_queue_drops_without_blocking() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("queue-full").unwrap(),
        root.path().to_path_buf(),
    );
    config.queue_capacity = 1;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();

    let before = guard.dropped_events();
    let started = Instant::now();
    flood();
    let elapsed = started.elapsed();
    let after = guard.dropped_events();

    assert!(
        after.get(DropCause::QueueFull) > before.get(DropCause::QueueFull),
        "expected QueueFull drops, got {after:?}"
    );
    assert_eq!(
        after.total(),
        DropCause::ALL
            .iter()
            .map(|cause| after.get(*cause))
            .sum::<u64>()
    );
    // Non-blocking: 40k records into a one-slot queue finish far below one blocking write each.
    assert!(elapsed < Duration::from_secs(30), "flood took {elapsed:?}");

    guard.shutdown(Duration::from_secs(10)).unwrap();
}
