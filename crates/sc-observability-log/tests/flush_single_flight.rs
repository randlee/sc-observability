//! One `init` per test binary: a stuck flush leaves one detached helper, never more.
//!
//! QA-2 RSH-004. The active JSONL path is replaced by a platform-native pipe
//! with no reader, so the writer thread blocks and sc-observability's flush
//! never returns: a stuck sink. The first flush times out and detaches its
//! helper; a retried flush returns `FlushError::InProgress` at once without
//! starting a thread (`helpers.detached` stays 1). Reading the pipe releases
//! the writer: the helper finishes, the counter returns to 0 and the next
//! flush succeeds.
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

#[cfg(unix)]
use std::fs::{File, OpenOptions};
use std::path::Path;
#[cfg(windows)]
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
#[cfg(windows)]
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

use sc_observability_log::{
    ActionName, BridgeOptions, HelperHealth, LogControl, LoggerConfig, ServiceName,
};
use sc_observability_log::{FlushError, LevelFilter, error_codes};

const STUCK_FLUSH_TIMEOUT: Duration = Duration::from_millis(100);
/// Generous: a retried flush must be rejected long before this could elapse.
const RETRY_TIMEOUT: Duration = Duration::from_secs(30);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const HELPER_FINISH_DEADLINE: Duration = Duration::from_secs(20);

#[cfg(unix)]
struct StuckSink {
    path: std::path::PathBuf,
    reader: Option<File>,
}

#[cfg(unix)]
impl StuckSink {
    fn prepare(path: &Path) -> Self {
        if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success(), "mkfifo failed: {status}");
        Self {
            path: path.to_path_buf(),
            reader: None,
        }
    }

    fn wait_until_connected(&self) {
        assert!(self.path.exists(), "FIFO fixture must exist before logging");
    }

    fn release(&mut self) {
        self.reader = Some(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.path)
                .unwrap(),
        );
    }
}

#[cfg(windows)]
struct StuckSink {
    blocked_record_bytes: usize,
    connected: Receiver<()>,
    release: SyncSender<()>,
    reader: Option<JoinHandle<()>>,
}

#[cfg(windows)]
impl StuckSink {
    fn prepare(path: &Path) -> Self {
        use std::ptr;
        use windows_sys::Win32::Foundation::{
            CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE,
        };
        use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
        use windows_sys::Win32::System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeInfo,
            PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
        };

        let name: Vec<u16> = path
            .to_str()
            .expect("named-pipe path must be Unicode")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let (connected_tx, connected) = sync_channel(1);
        let (buffer_size_tx, buffer_size_rx) = sync_channel(1);
        let (release, release_rx) = sync_channel(0);
        let reader = std::thread::spawn(move || {
            let pipe = unsafe {
                CreateNamedPipeW(
                    name.as_ptr(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    1,
                    0,
                    1,
                    0,
                    ptr::null(),
                )
            };
            assert_ne!(pipe, INVALID_HANDLE_VALUE, "create named pipe failed");
            let mut inbound_buffer_size = 0;
            assert_ne!(
                unsafe {
                    GetNamedPipeInfo(
                        pipe,
                        ptr::null_mut(),
                        ptr::null_mut(),
                        &raw mut inbound_buffer_size,
                        ptr::null_mut(),
                    )
                },
                0,
                "get named pipe info failed: {}",
                unsafe { GetLastError() }
            );
            buffer_size_tx
                .send(usize::try_from(inbound_buffer_size).unwrap())
                .unwrap();
            let connected_now = unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) };
            assert!(
                connected_now != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED,
                "connect named pipe failed: {}",
                unsafe { GetLastError() }
            );
            connected_tx.send(()).unwrap();
            release_rx.recv().unwrap();

            // The test emits a message one byte larger than this quota. Reading it
            // only after the assertions releases the blocked writer without
            // timing-based ordering.
            let read_buffer_size = usize::try_from(inbound_buffer_size)
                .unwrap()
                .checked_add(1)
                .and_then(|size| size.checked_mul(2))
                .unwrap();
            let mut bytes = vec![0_u8; read_buffer_size];
            let mut read = 0;
            let byte_count = u32::try_from(bytes.len()).expect("read buffer must fit Win32 length");
            assert_ne!(
                unsafe {
                    ReadFile(
                        pipe,
                        bytes.as_mut_ptr().cast(),
                        byte_count,
                        &raw mut read,
                        ptr::null_mut(),
                    )
                },
                0,
                "read named pipe failed: {}",
                unsafe { GetLastError() }
            );
            unsafe {
                DisconnectNamedPipe(pipe);
                CloseHandle(pipe);
            }
        });
        Self {
            blocked_record_bytes: buffer_size_rx.recv().unwrap().checked_add(1).unwrap(),
            connected,
            release,
            reader: Some(reader),
        }
    }

    fn wait_until_connected(&self) {
        self.connected.recv_timeout(IO_TIMEOUT).unwrap();
    }

    fn release(&mut self) {
        self.release.send(()).unwrap();
        self.reader.take().unwrap().join().unwrap();
    }

    fn blocked_record(&self) -> String {
        "x".repeat(self.blocked_record_bytes)
    }
}

fn helpers(control: &LogControl) -> HelperHealth {
    control.health().unwrap().helpers
}

#[test]
fn stuck_flush_keeps_one_detached_helper_and_rejects_retries() {
    #[cfg(unix)]
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("flush-single-flight").unwrap(),
        #[cfg(unix)]
        root.path().to_path_buf(),
        #[cfg(windows)]
        std::path::PathBuf::from(r"\\.\pipe"),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    let options = BridgeOptions {
        default_action: ActionName::new("log.record").unwrap(),
        parse_bracket_action: false,
    };
    let guard = sc_observability_log::init(config, options).unwrap();
    let control = guard.control();
    let path = control.active_log_path().unwrap().unwrap();

    // Nothing queued or in flight; then replace the active file with a pipe
    // whose server does not read until the assertions below complete.
    control.flush(IO_TIMEOUT).unwrap();
    let idle = helpers(&control);
    assert!(!idle.flush_in_flight);
    assert_eq!(idle.detached, 0);
    let mut stuck_sink = StuckSink::prepare(&path);

    // The Windows payload is larger than the actual pipe quota. Its message blocks
    // writer I/O until `release` starts the server-side read below; the Unix FIFO
    // blocks while opening because it has no reader.
    #[cfg(unix)]
    let blocked_record = "record behind a stuck sink";
    #[cfg(windows)]
    let blocked_record = stuck_sink.blocked_record();
    sc_observability_log::info!(
        target: "flush_single_flight",
        "{}",
        blocked_record
    );
    stuck_sink.wait_until_connected();

    // (a) The first flush times out and detaches exactly one helper.
    let first = control.flush(STUCK_FLUSH_TIMEOUT);
    let first_error = first.unwrap_err();
    assert!(
        matches!(first_error, FlushError::TimedOut { .. }),
        "{first_error:?}"
    );
    assert_eq!(
        first_error.code(),
        error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT
    );
    let stuck = helpers(&control);
    assert!(stuck.flush_in_flight);
    assert_eq!(stuck.detached, 1);

    // A retry is rejected at once and spawns nothing.
    for _ in 0..3 {
        let started = Instant::now();
        let retry = guard.flush(RETRY_TIMEOUT);
        assert!(matches!(retry, Err(FlushError::InProgress)), "{retry:?}");
        assert!(
            started.elapsed() < RETRY_TIMEOUT / 4,
            "InProgress must not wait for the flush timeout"
        );
        assert_eq!(
            retry.unwrap_err().code(),
            error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
        let still = helpers(&control);
        assert!(still.flush_in_flight);
        assert_eq!(still.detached, 1, "a rejected retry must not add a helper");
    }
    assert_eq!(
        serde_json::to_value(control.health().unwrap()).unwrap()["helpers"],
        serde_json::json!({"flush_in_flight": true, "detached": 1})
    );

    // (b) Read the pipe: the writer finishes and the detached helper returns.
    stuck_sink.release();
    let deadline = Instant::now() + HELPER_FINISH_DEADLINE;
    loop {
        let state = helpers(&control);
        if !state.flush_in_flight && state.detached == 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the detached helper never finished: {state:?}"
        );
        std::thread::yield_now();
    }

    control.flush(IO_TIMEOUT).unwrap();
    let done = helpers(&control);
    assert!(!done.flush_in_flight);
    assert_eq!(done.detached, 0);
    guard.shutdown(IO_TIMEOUT).unwrap();
    assert_eq!(helpers(&control).detached, 0);
}
