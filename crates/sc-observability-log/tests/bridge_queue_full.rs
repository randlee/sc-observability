//! One `init` per test binary, with `queue_capacity = 1`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use sc_observability_log::{ActionName, BridgeOptions, DropCause, LoggerConfig, ServiceName};

const THREADS: usize = 8;
const RECORDS_PER_THREAD: usize = 5_000;

fn flood() {
    let (done_tx, done_rx) = mpsc::sync_channel(THREADS);
    let handles: Vec<_> = (0..THREADS)
        .map(|thread| {
            let done_tx = done_tx.clone();
            std::thread::spawn(move || {
                for record in 0..RECORDS_PER_THREAD {
                    log::info!(target: "flood", "thread {thread} record {record}");
                }
                done_tx.send(thread).expect("flood completion receiver");
            })
        })
        .collect();
    drop(done_tx);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut completed = [false; THREADS];
    for _ in 0..THREADS {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let thread = done_rx
            .recv_timeout(remaining)
            .expect("flood worker did not complete before deadline");
        completed[thread] = true;
    }
    assert!(
        completed.into_iter().all(|done| done),
        "all flood workers must report completion"
    );
    for handle in handles {
        handle.join().unwrap();
    }
}

fn hold_stdout() -> (mpsc::Sender<()>, std::thread::JoinHandle<()>) {
    let (release, receive) = mpsc::channel();
    let (ready, entered) = mpsc::sync_channel(0);
    let holder = std::thread::spawn(move || {
        let stdout = std::io::stdout();
        let lock = stdout.lock();
        ready.send(()).expect("stdout holder ready");
        receive
            .recv_timeout(Duration::from_secs(30))
            .expect("stdout holder release");
        drop(lock);
    });
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("stdout holder entered");
    (release, holder)
}

#[test]
fn full_queue_drops_without_blocking() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("queue-full").unwrap(),
        root.path().to_path_buf(),
    );
    config.queue_capacity = 1;
    config.enable_console_sink = true;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let (release_stdout, stdout_holder) = hold_stdout();

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
    // Non-blocking: every worker reported completion before the deadline while
    // the one-slot queue was dropping admissions; retain a broad failure guard.
    assert!(elapsed < Duration::from_secs(30), "flood took {elapsed:?}");

    release_stdout.send(()).unwrap();
    stdout_holder.join().unwrap();
    guard.shutdown(Duration::from_secs(10)).unwrap();
}
