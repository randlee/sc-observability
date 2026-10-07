use serde_json::Value;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

pub(super) const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) fn read_request(stream: &mut TcpStream) -> (String, Value) {
    stream
        .set_read_timeout(Some(CAPTURE_TIMEOUT))
        .expect("set capture timeout");
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).expect("read request");
        assert_ne!(count, 0, "client closed before complete request");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end]).expect("request headers UTF-8");
    let path = headers
        .lines()
        .next()
        .expect("request line")
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .expect("content length")
        .parse::<usize>()
        .expect("numeric content length");
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).expect("read request body");
        assert_ne!(count, 0, "client closed before complete request body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let body = serde_json::from_slice(&bytes[header_end..header_end + content_length])
        .expect("OTLP/JSON body parses");
    (path, body)
}

pub(super) struct CaptureServer {
    completed: mpsc::Receiver<Result<(), String>>,
    received: Arc<AtomicUsize>,
    expected: usize,
    server: thread::JoinHandle<()>,
}

impl CaptureServer {
    pub(super) fn join(self) -> Result<usize, String> {
        let completed = self.completed.recv_timeout(CAPTURE_TIMEOUT).map_err(|error| {
            format!(
                "capture server did not complete after receiving {} of {} expected requests: {error}",
                self.received.load(Ordering::Relaxed),
                self.expected,
            )
        })?;
        self.server
            .join()
            .map_err(|_| "capture server panicked".to_owned())?;
        completed?;
        Ok(self.received.load(Ordering::Relaxed))
    }
}

pub(super) fn capture_server(
    listener: TcpListener,
    statuses: &[u16],
) -> (mpsc::Receiver<(String, Value)>, CaptureServer) {
    capture_server_with_accept_timeout(listener, statuses, CAPTURE_TIMEOUT)
}

fn capture_server_with_accept_timeout(
    listener: TcpListener,
    statuses: &[u16],
    accept_timeout: Duration,
) -> (mpsc::Receiver<(String, Value)>, CaptureServer) {
    let (captured_tx, captured_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let statuses = statuses.to_vec();
    let expected = statuses.len();
    let received = Arc::new(AtomicUsize::new(0));
    let received_by_server = Arc::clone(&received);
    let server = thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set capture listener nonblocking");
        let accept_deadline = Instant::now() + accept_timeout;
        for status in statuses {
            let (mut stream, _) = match accept_before(&listener, accept_deadline) {
                Ok(connection) => connection,
                Err(error) => {
                    let _ = completed_tx.send(Err(format!(
                        "accept submission request before deadline: {error}"
                    )));
                    return;
                }
            };
            stream
                .set_nonblocking(false)
                .expect("set capture stream blocking");
            let request = read_request(&mut stream);
            captured_tx.send(request).expect("deliver captured request");
            received_by_server.fetch_add(1, Ordering::Relaxed);
            let reason = if status < 300 {
                "OK"
            } else if status < 500 {
                "Bad Request"
            } else {
                "Service Unavailable"
            };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .expect("write capture response");
        }
        completed_tx
            .send(Ok(()))
            .expect("report capture completion");
    });
    (
        captured_rx,
        CaptureServer {
            completed: completed_rx,
            received,
            expected,
            server,
        },
    )
}

fn accept_before(
    listener: &TcpListener,
    deadline: Instant,
) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
    loop {
        match listener.accept() {
            Ok(connection) => return Ok(connection),
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::yield_now();
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(std::io::Error::new(
                    ErrorKind::TimedOut,
                    "capture server accept deadline elapsed",
                ));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_server_stops_waiting_when_no_request_arrives() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture listener");
        let (_, server) =
            capture_server_with_accept_timeout(listener, &[200], Duration::from_millis(50));

        let started = Instant::now();
        let error = server.join().expect_err("missing request should time out");

        assert!(
            started.elapsed() < CAPTURE_TIMEOUT / 2,
            "capture server exceeded its accept deadline: {error}"
        );
        assert!(
            error.contains(
                "accept submission request before deadline: capture server accept deadline elapsed"
            ),
            "capture server did not report its bounded accept timeout: {error}"
        );
    }
}
