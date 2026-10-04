//! Shared loopback HTTP support for durable integration-style tests.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread;
use std::time::Duration;

/// Serves scripted HTTP statuses and retains every received request path.
pub(super) struct ScriptedStatusCollector {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    paths: Arc<Mutex<Vec<String>>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl ScriptedStatusCollector {
    /// Starts a loopback collector using each scripted status before its fallback.
    pub(super) fn start(statuses: Vec<&'static str>, fallback_status: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback collector");
        listener
            .set_nonblocking(true)
            .expect("make loopback collector nonblocking");
        let address = listener
            .local_addr()
            .expect("read loopback collector address");
        let stop = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(Mutex::new(Vec::new()));
        let (thread_stop, thread_calls, thread_paths) =
            (Arc::clone(&stop), Arc::clone(&calls), Arc::clone(&paths));
        let worker = thread::spawn(move || {
            loop {
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("accept collector request: {error}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("make accepted collector stream blocking");
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .expect("set collector read deadline");
                thread_paths
                    .lock()
                    .expect("record collector request path")
                    .push(read_http_request_path(&mut stream));
                let call = thread_calls.fetch_add(1, Ordering::AcqRel);
                let status = statuses.get(call).copied().unwrap_or(fallback_status);
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .expect("reply to collector request");
            }
        });
        Self {
            address,
            stop,
            paths,
            worker: Some(worker),
        }
    }

    /// Returns the loopback endpoint for a telemetry client.
    pub(super) fn endpoint(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Returns each request path observed by the collector.
    pub(super) fn paths(&self) -> Vec<String> {
        self.paths
            .lock()
            .expect("read collector request paths")
            .clone()
    }

    /// Replaces the observable address to prove drop never opens a wake connection.
    pub(super) fn set_address_for_drop_regression(&mut self, address: SocketAddr) {
        self.address = address;
    }
}

impl Drop for ScriptedStatusCollector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if !thread::panicking()
            && let Some(worker) = self.worker.take()
        {
            worker.join().expect("collector worker exits");
        }
    }
}

/// Reads one framed HTTP request and returns its request-target path.
pub(super) fn read_http_request_path(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let end = loop {
        let count = stream.read(&mut buffer).expect("read request headers");
        assert_ne!(count, 0, "client closed before complete request headers");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..end]).expect("request headers UTF-8");
    let path = headers
        .lines()
        .next()
        .expect("request line")
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let size: usize = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().expect("numeric content length"))
        })
        .expect("content length");
    while bytes.len() < end + size {
        let count = stream.read(&mut buffer).expect("read request body");
        assert_ne!(count, 0, "client closed before complete request body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    path
}
