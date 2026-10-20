//! A tiny scripted HTTP/1.1 server on 127.0.0.1 for adapter tests. No real network.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use craft_ai_client::{HttpConfig, RetryPolicy};

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Clone, Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Sent one by one with `gap` between them, as a chunked body when there is more than one.
    pub chunks: Vec<Vec<u8>>,
    pub gap: Duration,
    /// Wait this long before sending anything.
    pub stall: Duration,
}

impl Reply {
    pub fn json(status: u16, value: serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            chunks: vec![value.to_string().into_bytes()],
            gap: Duration::ZERO,
            stall: Duration::ZERO,
        }
    }

    pub fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self { status, headers: vec![("Content-Type".into(), content_type.into())], chunks: vec![body], gap: Duration::ZERO, stall: Duration::ZERO }
    }

    pub fn stream(content_type: &str, chunks: Vec<String>, gap: Duration) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), content_type.into())],
            chunks: chunks.into_iter().map(String::into_bytes).collect(),
            gap,
            stall: Duration::ZERO,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn stalled(mut self, stall: Duration) -> Self {
        self.stall = stall;
        self
    }
}

type Handler = dyn Fn(&Recorded, usize) -> Reply + Send + Sync;

pub struct MockServer {
    pub base: String,
    pub requests: Arc<Mutex<Vec<Recorded>>>,
}

impl MockServer {
    /// Starts a server; `handler` gets each request and how many came before it.
    pub fn start(handler: impl Fn(&Recorded, usize) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);
        let seen = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let handler = Arc::clone(&handler);
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || serve(stream, &*handler, &seen));
            }
        });
        Self { base, requests }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

fn serve(stream: TcpStream, handler: &Handler, seen: &Mutex<Vec<Recorded>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    let len = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(0);
    let mut body = vec![0; len];
    let _ = reader.read_exact(&mut body);
    let request = Recorded { method, path, headers, body };
    let index = {
        let mut all = seen.lock().unwrap();
        all.push(request.clone());
        all.len() - 1
    };
    let reply = handler(&request, index);
    std::thread::sleep(reply.stall);
    let mut out = stream;
    let mut head = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", reply.status);
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    let chunked = reply.chunks.len() > 1;
    if chunked {
        head.push_str("Transfer-Encoding: chunked\r\n\r\n");
    } else {
        head.push_str(&format!("Content-Length: {}\r\n\r\n", reply.chunks.first().map_or(0, Vec::len)));
    }
    if out.write_all(head.as_bytes()).is_err() {
        return;
    }
    for chunk in &reply.chunks {
        let ok = if chunked {
            out.write_all(format!("{:x}\r\n", chunk.len()).as_bytes()).and_then(|()| out.write_all(chunk)).and_then(|()| out.write_all(b"\r\n"))
        } else {
            out.write_all(chunk)
        };
        if ok.and_then(|()| out.flush()).is_err() {
            return;
        }
        std::thread::sleep(reply.gap);
    }
    if chunked {
        let _ = out.write_all(b"0\r\n\r\n");
    }
    let _ = out.flush();
}

/// Short timeouts and quick retries, no proxy.
pub fn config() -> HttpConfig {
    HttpConfig {
        connect_timeout: Duration::from_secs(2),
        request_timeout: Duration::from_secs(5),
        stream_timeout: Duration::from_secs(10),
        retry: RetryPolicy { max_attempts: 3, base_delay: Duration::from_millis(10), max_delay: Duration::from_millis(50) },
        use_env_proxy: false,
        ..HttpConfig::default()
    }
}

/// Polls until the job ends, collecting events.
pub fn finish(
    p: &dyn craft_ai_core::Provider,
    job: &craft_ai_core::JobId,
) -> (craft_ai_core::JobState, Vec<craft_ai_core::Event>, Option<craft_ai_core::Usage>) {
    let start = std::time::Instant::now();
    let mut events = Vec::new();
    let mut usage = None;
    loop {
        let snap = p.poll(job).unwrap();
        events.extend(snap.events);
        usage = snap.usage.or(usage);
        if snap.state.is_terminal() {
            return (snap.state, events, usage);
        }
        assert!(start.elapsed() < Duration::from_secs(20), "job did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn texts(events: &[craft_ai_core::Event]) -> (Vec<String>, Vec<String>) {
    let mut deltas = Vec::new();
    let mut outputs = Vec::new();
    for e in events {
        match e {
            craft_ai_core::Event::TextDelta { text } => deltas.push(text.clone()),
            craft_ai_core::Event::Output(o) => {
                if let craft_ai_core::Payload::Text { text } = &o.payload {
                    outputs.push(text.clone());
                }
            }
            _ => {}
        }
    }
    (deltas, outputs)
}
