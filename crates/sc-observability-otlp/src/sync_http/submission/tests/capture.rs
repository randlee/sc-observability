use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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

pub(super) fn capture_server(
    listener: TcpListener,
    statuses: &[u16],
) -> (mpsc::Receiver<(String, Value)>, thread::JoinHandle<()>) {
    let (captured_tx, captured_rx) = mpsc::channel();
    let statuses = statuses.to_vec();
    let server = thread::spawn(move || {
        for status in statuses {
            let (mut stream, _) = listener.accept().expect("accept submission request");
            let request = read_request(&mut stream);
            captured_tx.send(request).expect("deliver captured request");
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
    });
    (captured_rx, server)
}
