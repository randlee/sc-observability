//! One attached test logger per binary: a stuck flush leaves one detached helper, never more.
//!
//! QA-2 RSH-004. A test-only registered sink blocks its first `flush` on a
//! channel. This makes the stalled flush independent of OS pipe-buffer quota:
//! the first flush times out, retries return `FlushError::InProgress` without
//! starting another helper, release completes the helper, and the next flush
//! succeeds.
//!
#![cfg(feature = "v1")]
#![allow(
    deprecated,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
use std::time::Instant;

use sc_observability::v2::LogSink;
use sc_observability::{SinkHealth, SinkHealthState, SinkName, SinkRegistration};
use sc_observability_log::v2::FlushError;
use sc_observability_log::{
    ActionName, AttachmentOptions, BridgeEventDecision, BridgeEventPolicy, BridgeOptions,
    HelperHealth, LoggerConfig, ServiceName, attach_logger,
};
use sc_observability_log::{LevelFilter, error_codes};
use sc_observability_types::LogEvent;

const STUCK_FLUSH_TIMEOUT: Duration = Duration::from_millis(100);
/// Generous: a retried flush must be rejected long before this could elapse.
const RETRY_TIMEOUT: Duration = Duration::from_secs(30);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const HELPER_FINISH_DEADLINE: Duration = Duration::from_secs(20);

struct StuckSink {
    entered: mpsc::Receiver<()>,
    release: mpsc::SyncSender<()>,
    flush_calls: Arc<AtomicUsize>,
}

impl StuckSink {
    fn prepare() -> (Self, Arc<dyn LogSink>) {
        let (entered_tx, entered) = mpsc::sync_channel(1);
        let (release, release_rx) = mpsc::sync_channel(0);
        let flush_calls = Arc::new(AtomicUsize::new(0));
        let sink = Arc::new(BlockingFlushSink {
            entered: Mutex::new(Some(entered_tx)),
            release: Mutex::new(Some(release_rx)),
            flush_calls: Arc::clone(&flush_calls),
        });
        (
            Self {
                entered,
                release,
                flush_calls,
            },
            sink,
        )
    }

    fn wait_until_blocked(&self) {
        self.entered.recv_timeout(IO_TIMEOUT).unwrap();
    }

    fn release(&self) {
        self.release.send(()).unwrap();
    }

    fn flush_calls(&self) -> usize {
        self.flush_calls.load(Ordering::SeqCst)
    }
}

struct BlockingFlushSink {
    entered: Mutex<Option<mpsc::SyncSender<()>>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
    flush_calls: Arc<AtomicUsize>,
}

impl LogSink for BlockingFlushSink {
    fn write(
        &self,
        _: &sc_observability_types::LogEvent,
    ) -> Result<(), sc_observability_types::v2::LogSinkError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), sc_observability_types::v2::LogSinkError> {
        self.flush_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = self.entered.lock().unwrap().take() {
            entered.send(()).unwrap();
        }
        if let Some(release) = self.release.lock().unwrap().take() {
            release.recv().unwrap();
        }
        Ok(())
    }

    fn health(&self) -> SinkHealth {
        SinkHealth {
            name: SinkName::new("flush-single-flight-blocking").unwrap(),
            state: SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

struct Admit;

impl BridgeEventPolicy for Admit {
    fn decide(&self, _: &LogEvent) -> BridgeEventDecision {
        BridgeEventDecision::Admit
    }
}

fn attachment_options() -> AttachmentOptions {
    AttachmentOptions::new(
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
        Arc::new(Admit),
    )
}

fn blocking_logger(sink: Arc<dyn LogSink>) -> Arc<sc_observability::v2::Logger> {
    let mut config = LoggerConfig::default_for(
        ServiceName::new("flush-single-flight").unwrap(),
        std::env::temp_dir(),
    );
    config.level = LevelFilter::Info;
    config.enable_file_sink = false;
    config.enable_console_sink = false;
    let mut builder = sc_observability::v2::LoggerBuilder::new(config).unwrap();
    builder.register_sink(SinkRegistration::typed(sink));
    Arc::new(builder.build().unwrap())
}

fn assert_helpers(control: &sc_observability_log::v2::LogControl, expected: HelperHealth) {
    assert_eq!(control.health().unwrap().helpers, expected);
}

#[test]
fn stuck_flush_keeps_one_detached_helper_and_rejects_retries() {
    let (stuck_sink, sink) = StuckSink::prepare();
    let host = blocking_logger(sink);
    let mut attachment = attach_logger(Arc::clone(&host), attachment_options()).unwrap();
    let control = attachment.control();

    assert_helpers(
        &control,
        HelperHealth {
            flush_in_flight: false,
            detached: 0,
        },
    );

    // (a) The first flush times out and detaches exactly one helper.
    let first = control.flush_with_timeout(STUCK_FLUSH_TIMEOUT);
    let first_error = first.unwrap_err();
    assert!(
        matches!(first_error, FlushError::Drain { .. }),
        "{first_error:?}"
    );
    assert_eq!(
        first_error.diagnostic().code,
        error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT
    );
    stuck_sink.wait_until_blocked();
    assert_eq!(stuck_sink.flush_calls(), 1);
    assert_helpers(
        &control,
        HelperHealth {
            flush_in_flight: true,
            detached: 1,
        },
    );

    // A retry is rejected at once and spawns nothing.
    for _ in 0..3 {
        let started = Instant::now();
        let retry = control.flush_with_timeout(RETRY_TIMEOUT);
        assert!(matches!(retry, Err(FlushError::Drain { .. })), "{retry:?}");
        assert!(
            started.elapsed() < RETRY_TIMEOUT / 4,
            "InProgress must not wait for the flush timeout"
        );
        assert_eq!(
            retry.unwrap_err().diagnostic().code,
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
        assert_eq!(
            stuck_sink.flush_calls(),
            1,
            "a rejected retry must not call the sink"
        );
        assert_helpers(
            &control,
            HelperHealth {
                flush_in_flight: true,
                detached: 1,
            },
        );
    }
    assert_eq!(
        serde_json::to_value(control.health().unwrap()).unwrap()["helpers"],
        serde_json::json!({"flush_in_flight": true, "detached": 1})
    );

    // (b) Release the sink: the detached helper returns and a later flush works.
    stuck_sink.release();
    let deadline = Instant::now() + HELPER_FINISH_DEADLINE;
    loop {
        match control.flush_with_timeout(IO_TIMEOUT) {
            Ok(()) => break,
            Err(error)
                if error.diagnostic().code
                    == error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
                    && Instant::now() < deadline =>
            {
                std::thread::yield_now();
            }
            Err(error) => panic!("released flush must succeed: {error:?}"),
        }
    }
    assert_eq!(stuck_sink.flush_calls(), 2);
    assert_helpers(
        &control,
        HelperHealth {
            flush_in_flight: false,
            detached: 0,
        },
    );
    attachment.detach(IO_TIMEOUT).unwrap();
    assert_eq!(control.health().unwrap().helpers.detached, 0);
    Arc::try_unwrap(host)
        .unwrap_or_else(|_| panic!("detach releases the attachment logger"))
        .shutdown()
        .unwrap();
    assert_eq!(control.health().unwrap().helpers.detached, 0);
}
